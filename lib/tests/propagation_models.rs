mod common;

use common::behaviours::{Monotonic, StaticMovement};
use manetsim::packets::{MulticastPacket, Packet};
use manetsim::propagation_models::{FreeSpace, FreeSpaceParams, PropagationParams};
use manetsim::types::{NodeData, NodeID, NodeInit, SimConfig, SimManager};
use std::sync::Arc;

#[test]
fn free_space() {
    env_logger::init();

    let closure =
        Arc::new(
            |_: &NodeData<f32, 2, FreeSpaceParams<f32, 2>>, content| MulticastTestPacket {
                content,
            },
        );
    let nodes = vec![
        NodeInit {
            starting_position: [0.0, 0.0],
            node_behaviour: Monotonic::new(5, closure.clone()),
            move_behaviour: StaticMovement {},
            propagation_params: FreeSpaceParams::new(
                8.0,
                0.34538301613, // 868mhz in metres
                |_, _| 11.0,   // Omnidirectional
                0.0,
                |_, _| 0.0, // Omnidirectional
                -90.0,
            ),
        },
        NodeInit {
            starting_position: [6_000.0, 0.0], // 6km
            node_behaviour: Monotonic::new(5, closure.clone()),
            move_behaviour: StaticMovement {},
            propagation_params: FreeSpaceParams::new(
                8.0,
                0.34538301613, // 868mhz in metres
                |_, _| 11.0,   // Omnidirectional
                0.0,
                |_, _| 0.0, // Omnidirectional
                -90.0,
            ),
        },
        NodeInit {
            starting_position: [12_000.0, 0.0], // 12km
            node_behaviour: Monotonic::new(5, closure.clone()),
            move_behaviour: StaticMovement {},
            propagation_params: FreeSpaceParams::new(
                8.0,
                0.34538301613, // 868mhz in metres
                |_, _| 11.0,   // Omnidirectional
                0.0,
                |_, _| 0.0, // Omnidirectional
                -90.0,
            ),
        },
    ];

    #[derive(Clone, Debug)]
    struct MulticastTestPacket {
        content: Box<[u8]>,
    };

    impl Packet<f32, 2> for MulticastTestPacket {
        fn content(self) -> Box<[u8]> {
            self.content
        }

        fn content_ref(&self) -> &Box<[u8]> {
            &self.content
        }

        fn eager_targets(&self) -> Option<Vec<NodeID>> {
            None
        }

        fn targets<P: PropagationParams<f32, 2>>(&self, target: &NodeData<f32, 2, P>) -> bool
        where
            Self: Sized,
        {
            true
        }
    }

    struct TestSimConfig;
    impl SimConfig<f32, 2> for TestSimConfig {
        type MB = StaticMovement;
        type NB = Monotonic<f32, 2, MulticastTestPacket, FreeSpaceParams<f32, 2>>;
        type PM = FreeSpace;
        type S = ();
        type E = ();
    }

    let mut sim_manager: SimManager<f32, 2, TestSimConfig> =
        SimManager::new(nodes, 2138717, FreeSpace);

    sim_manager.n_ticks(11);

    assert_eq!(
        sim_manager.global_state_manager.nodes()[0]
            .node_behaviour()
            .received_packets,
        2
    );
    assert_eq!(
        sim_manager.global_state_manager.nodes()[2]
            .node_behaviour()
            .received_packets,
        2
    );
    assert_eq!(
        sim_manager.global_state_manager.nodes()[1]
            .node_behaviour()
            .received_packets,
        4
    );
}
