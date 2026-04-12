use crate::common::behaviours::{Monotonic, StaticMovement};
use log::{debug, info, trace};
use manetsim::example_behaviours::RandomWalk;
use manetsim::example_behaviours::flood::Flood;
use manetsim::example_behaviours::flood::FloodPacket;
use manetsim::packets::{GloballySequencedPacket, Packet};
use manetsim::propagation_models::{
    FreeSpace, FreeSpaceParams, PropagationModel, PropagationParams, SimpleDistance,
    SimpleDistanceParams,
};
use manetsim::types::{
    Coord, GlobalStateManager, MoveBehaviour, NodeBehaviour, NodeData, NodeID, NodeInit, SimConfig,
    SimManager,
};
use std::env;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::ops::BitAnd;
use std::sync::Arc;
use std::thread::sleep;

mod common;

#[derive(Clone, Debug)]
struct TestPacket {
    hops: u8,
    seq: u16,
    src: NodeID,
    content: Box<[u8]>,
}

impl<A: Coord<K>, const K: usize> Packet<A, K> for TestPacket {
    fn content(self) -> Box<[u8]> {
        self.content
    }

    fn content_ref(&self) -> &Box<[u8]> {
        &self.content
    }

    fn eager_targets(&self) -> Option<Vec<NodeID>> {
        None
    }

    fn targets<P: PropagationParams<A, K>>(&self, target: &NodeData<A, K, P>) -> bool
    where
        Self: Sized,
    {
        true
    }
}

impl<A: Coord<K>, const K: usize> GloballySequencedPacket<A, K> for TestPacket {
    type S = u16;

    fn seq(&self) -> Self::S {
        let mut s = DefaultHasher::new();
        self.seq.hash(&mut s);
        self.src.hash(&mut s);
        let hash = az::overflowing_cast(s.finish()).0;
        trace!("{:?}, {:?} hashed to {:?}", self.seq, self.src, hash);
        hash
    }
}

impl<A: Coord<K>, const K: usize> FloodPacket<A, K> for TestPacket {
    type H = u8;

    fn get_hop_count(&self) -> <Self as FloodPacket<A, K>>::H {
        self.hops
    }

    fn set_hop_count(&mut self, count: <Self as FloodPacket<A, K>>::H) {
        self.hops = count;
    }

    fn new(
        data: &NodeData<A, K, impl PropagationParams<A, K>>,
        hops: Self::H,
        seq: Self::S,
        content: Box<[u8]>,
    ) -> Self {
        Self {
            hops,
            seq,
            content,
            src: data.id,
        }
    }
}

#[derive(Clone)]
struct MonotonicFlood<A: Coord<K>, const K: usize, P: PropagationParams<A, K>> {
    monotonic: Monotonic<A, K, TestPacket, P>,
    flood: Flood<TestPacket, A, K>,
}

impl<A: Coord<K>, const K: usize, P: PropagationParams<A, K>> NodeBehaviour<A, K, P>
    for MonotonicFlood<A, K, P>
{
    type P = TestPacket;
    fn tick<
        C: SimConfig<
                A,
                K,
                PM = impl PropagationModel<A, K, P = P>,
                NB = impl NodeBehaviour<A, K, P, P = Self::P, E = Self::E>,
                E = Self::E,
            >,
    >(
        mut self,
        node_data: &NodeData<A, K, P>,
        global_state_manager: &GlobalStateManager<A, K, C>,
        packets: &Vec<<Self as NodeBehaviour<A, K, P>>::P>,
    ) -> Self {
        self.monotonic = self
            .monotonic
            .tick(node_data, global_state_manager, packets);

        self.flood = self.flood.tick(node_data, global_state_manager, packets);
        self
    }
}

impl<A: Coord<K> + 'static, const K: usize, P: PropagationParams<A, K>> MonotonicFlood<A, K, P> {
    fn new() -> Self {
        let flood = Flood::new(5);
        let clone = flood.clone();
        Self {
            monotonic: Monotonic::new(
                5,
                Arc::new(move |data, content| flood.gen_packet(data, content)),
            ),
            flood: clone,
        }
    }
}

fn generate_nodes<MB: MoveBehaviour<f32, 2>, P: PropagationParams<f32, 2>>(
    num_nodes: usize,
    gap: f32,
    params: P,
    move_behaviour: MB,
) -> Vec<NodeInit<MonotonicFlood<f32, 2, P>, MB, P, f32, 2>> {
    let dim = num_nodes.isqrt();
    let mut nodes = Vec::with_capacity(num_nodes);
    for i in 0..num_nodes {
        let xy = [(i / dim) as f32 * gap, (i % dim) as f32 * gap];
        trace!("Spawning at {:?}", xy);
        nodes.push(NodeInit {
            starting_position: xy,
            node_behaviour: MonotonicFlood::new(),
            move_behaviour: move_behaviour.clone(),
            propagation_params: params.clone(),
        })
    }
    nodes
}

#[test]
fn test_flood() {
    env_logger::init();

    let num_nodes = env::var("NUM_NODES")
        .unwrap_or("1024".into())
        .parse::<usize>()
        .unwrap();

    let nodes = generate_nodes(
        num_nodes,
        3_000.0,
        FreeSpaceParams::new(
            8.0,
            0.34538301613, // 868mhz in metres
            |_, _| 11.0,   // Omnidirectional
            11.0,
            |_, _| 0.0, // Omnidirectional
            -90.0,
        ),
        StaticMovement {},
    );

    struct TestConfig;
    impl SimConfig<f32, 2> for TestConfig {
        type MB = StaticMovement;
        type NB = MonotonicFlood<f32, 2, FreeSpaceParams<f32, 2>>;
        type PM = FreeSpace;
        type S = ();
        type E = ();
    }

    let mut sim: SimManager<_, _, TestConfig> = SimManager::new(nodes, 123456, FreeSpace);

    sim.n_ticks(10);

    let mut received_packets = 0;
    let mut originated_packets = 0;
    for node in sim.global_state_manager.nodes() {
        debug!(
            "{} ({}, {}): {}",
            node.data().id,
            node.data().position[0],
            node.data().position[1],
            node.node_behaviour().monotonic.received_packets
        );
        received_packets += node.node_behaviour().monotonic.received_packets;
        originated_packets += node.node_behaviour().flood.seq();
    }

    info!("Received {} packets total", received_packets);
    info!("Originated {} packets total", originated_packets);
}

#[test]
fn random_walk_scale_test() {
    env_logger::init();

    let num_nodes = env::var("NUM_NODES")
        .unwrap_or("1024".into())
        .parse::<usize>()
        .unwrap();

    let nodes = generate_nodes(
        num_nodes,
        3_000.0,
        SimpleDistanceParams {
            transmit_distance: 4000.0,
        },
        RandomWalk::new(1000.0),
    );

    struct TestConfig;
    impl SimConfig<f32, 2> for TestConfig {
        type MB = RandomWalk<20, f32, 2>;
        type NB = MonotonicFlood<f32, 2, SimpleDistanceParams<f32, 2>>;
        type PM = SimpleDistance;
        type S = ();
        type E = ();
    }

    let mut sim: SimManager<_, _, TestConfig> = SimManager::new(nodes, 123456, SimpleDistance);

    sim.n_ticks(100);
}
