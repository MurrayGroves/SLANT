use crate::common::behaviours::{Monotonic, StaticMovement};
use log::{debug, trace};
use manetsim::example_behaviours::flood::Flood;
use manetsim::example_behaviours::flood::FloodPacket;
use manetsim::packets::{GloballySequencedPacket, Packet};
use manetsim::propagation_models::{
    FreeSpace, FreeSpaceParams, PropagationModel, PropagationParams,
};
use manetsim::types::{
    Coord, GlobalStateManager, NodeBehaviour, NodeData, NodeID, NodeInit, SimConfig, SimManager,
};
use std::ops::BitAnd;
use std::sync::Arc;

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
        self.seq.bitand(self.src as Self::S)
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
struct MonotonicFlood {
    monotonic: Monotonic<f32, 2, TestPacket, FreeSpaceParams<f32, 2>>,
    flood: Flood<TestPacket, f32, 2>,
}

impl NodeBehaviour<f32, 2, FreeSpaceParams<f32, 2>> for MonotonicFlood {
    type P = TestPacket;
    fn tick<
        C: SimConfig<
                f32,
                2,
                PM = impl PropagationModel<f32, 2, P = FreeSpaceParams<f32, 2>>,
                NB = impl NodeBehaviour<f32, 2, FreeSpaceParams<f32, 2>, P = Self::P>,
            >,
    >(
        mut self,
        node_data: &NodeData<f32, 2, <<C as SimConfig<f32, 2>>::PM as PropagationModel<f32, 2>>::P>,
        global_state_manager: &GlobalStateManager<f32, 2, C>,
        packets: &Vec<<Self as NodeBehaviour<f32, 2, FreeSpaceParams<f32, 2>>>::P>,
    ) -> Self {
        self.monotonic = self
            .monotonic
            .tick(node_data, global_state_manager, packets);

        self.flood = self.flood.tick(node_data, global_state_manager, packets);
        self
    }
}

impl MonotonicFlood {
    fn new() -> Self {
        let flood = Flood::new();
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

fn generate_nodes(
    num_nodes: usize,
    gap: f32,
) -> Vec<NodeInit<MonotonicFlood, StaticMovement, FreeSpaceParams<f32, 2>, f32, 2>> {
    let dim = num_nodes.isqrt() + 1;
    let mut nodes = Vec::with_capacity(num_nodes);
    for i in 0..num_nodes {
        let xy = [(i as f32 / dim as f32) * gap, (i % dim) as f32 * gap];
        trace!("Spawning at {:?}", xy);
        nodes.push(NodeInit {
            starting_position: xy,
            node_behaviour: MonotonicFlood::new(),
            move_behaviour: StaticMovement {},
            propagation_params: FreeSpaceParams::new(
                8.0,
                0.34538301613, // 868mhz in metres
                |_, _| 11.0,   // Omnidirectional
                0.0,
                |_, _| 0.0, // Omnidirectional
                -90.0,
            ),
        })
    }
    nodes
}

#[test]
fn test_flood() {
    env_logger::init();

    let nodes = generate_nodes(10_000, 3_000.0);

    struct TestConfig;
    impl SimConfig<f32, 2> for TestConfig {
        type MB = StaticMovement;
        type NB = MonotonicFlood;
        type PM = FreeSpace;
        type S = ();
    }

    let mut sim: SimManager<_, _, TestConfig> = SimManager::new(nodes, 123456, FreeSpace);

    sim = sim.n_ticks(10);

    for node in sim.global_state_manager.nodes() {
        debug!(
            "{}: {}",
            node.data().id,
            node.node_behaviour().monotonic.received_packets
        );
    }
}
