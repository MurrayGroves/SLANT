use crate::common::behaviours::{Monotonic, StaticMovement, TestablePacket};
use log::debug;
use manetsim::example_behaviours::flood::Flood;
use manetsim::example_behaviours::flood::FloodPacket;
use manetsim::packets::{GloballySequencedPacket, Packet};
use manetsim::propagation_models::{
    FreeSpace, FreeSpaceParams, PropagationModel, PropagationParams,
};
use manetsim::types::{
    Coord, GlobalStateManager, NodeBehaviour, NodeData, NodeID, NodeInit, SimConfig, SimManager,
};

mod common;

#[derive(Clone, Debug)]
struct TestPacket {
    hops: u8,
    seq: u16,
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
        self.seq
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
}

impl TestablePacket<f32, 2> for TestPacket {
    fn new(content: Box<[u8]>) -> Self {
        TestPacket {
            hops: 5,
            seq: 0,
            content,
        }
    }
}

#[derive(Clone)]
struct MonotonicFlood {
    monotonic: Monotonic<TestPacket>,
    flood: Flood<TestPacket, f32, 2>,
}

impl NodeBehaviour<f32, 2> for MonotonicFlood {
    type P = TestPacket;
    fn tick<C: SimConfig<f32, 2, NB = impl NodeBehaviour<f32, 2, P = Self::P>>>(
        mut self,
        node_data: &NodeData<f32, 2, <<C as SimConfig<f32, 2>>::PM as PropagationModel<f32, 2>>::P>,
        global_state_manager: &GlobalStateManager<f32, 2, C>,
        packets: &Vec<Box<<Self as NodeBehaviour<f32, 2>>::P>>,
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
        Self {
            monotonic: Monotonic::new(20),
            flood: Flood::new(),
        }
    }
}

fn generate_nodes(
    num_nodes: usize,
) -> Vec<NodeInit<MonotonicFlood, StaticMovement, FreeSpaceParams<f32, 2>, f32, 2>> {
    let mut nodes = Vec::with_capacity(num_nodes);
    for i in 0..num_nodes {
        nodes.push(NodeInit {
            starting_position: [i as f32 * 100.0, i as f32 * 100.0],
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

    let nodes = generate_nodes(100);

    struct TestConfig;
    impl SimConfig<f32, 2> for TestConfig {
        type MB = StaticMovement;
        type NB = MonotonicFlood;
        type PM = FreeSpace;
        type S = ();
    }

    let mut sim: SimManager<_, _, TestConfig> = SimManager::new(nodes, 123456, FreeSpace);

    sim = sim.n_ticks(1000);

    for node in sim.global_state_manager.nodes() {
        debug!(
            "{}: {}",
            node.data().id,
            node.node_behaviour().monotonic.received_packets
        );
    }
}
