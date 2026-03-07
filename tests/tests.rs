mod common;

use lazy_static::lazy_static;
use log::trace;
use manetsim::example_behaviours::RandomWalk;
use manetsim::propagation_models::{
    PropagationModel, PropagationParams, SimpleDistance, SimpleDistanceParams,
};
use manetsim::traffic_generators::mixed_multicast_and_random_target_unicast;
use manetsim::types::{
    GlobalStateManager, MoveBehaviour, Node, NodeBehaviour, NodeData, NodeInit, SimManager,
};
use rand::SeedableRng;
use rand_xoshiro::Xoshiro256Plus;
use std::sync::{Arc, Mutex};

use common::behaviours::{LoggerNode, packet_counter};

#[test]
fn ten_ticks_random_walk() {
    env_logger::init();

    let propagation_params = SimpleDistanceParams {
        transmit_distance: 1.0,
    };

    let prop_model = SimpleDistance;

    let nodes: Vec<
        NodeInit<LoggerNode, RandomWalk<2, f64, 2>, SimpleDistanceParams<f64, 2>, f64, 2>,
    > = vec![
        NodeInit {
            starting_position: [0.0, 0.0],
            node_behaviour: LoggerNode {},
            move_behaviour: RandomWalk::new([0.0, 1.0]),
            propagation_params: propagation_params.clone(),
        },
        NodeInit {
            starting_position: [1.0, 0.0],
            node_behaviour: LoggerNode {},
            move_behaviour: RandomWalk::new([1.0, 1.0]),
            propagation_params: propagation_params.clone(),
        },
        NodeInit {
            starting_position: [1.0, 1.0],
            node_behaviour: LoggerNode {},
            move_behaviour: RandomWalk::new([1.0, 0.0]),
            propagation_params: propagation_params.clone(),
        },
        NodeInit {
            starting_position: [0.0, 1.0],
            node_behaviour: LoggerNode {},
            move_behaviour: RandomWalk::new([-1.0, 1.0]),
            propagation_params: propagation_params.clone(),
        },
        NodeInit {
            starting_position: [1.0, 0.0],
            node_behaviour: LoggerNode {},
            move_behaviour: RandomWalk::new([1.0, -3.0]),
            propagation_params: propagation_params.clone(),
        },
        NodeInit {
            starting_position: [1.0, 5.0],
            node_behaviour: LoggerNode {},
            move_behaviour: RandomWalk::new([6.0, 1.0]),
            propagation_params: propagation_params.clone(),
        },
        NodeInit {
            starting_position: [1.0, 0.0],
            node_behaviour: LoggerNode {},
            move_behaviour: RandomWalk::new([3.0, 1.0]),
            propagation_params,
        },
    ];

    let mut sim_manager = SimManager::new(nodes.clone(), 123, prop_model);
    let mut rng = Xoshiro256Plus::seed_from_u64(123456);
    for _ in 0..10 {
        mixed_multicast_and_random_target_unicast(
            &mut rng,
            &sim_manager.global_state_manager,
            5,
            5,
        );
        sim_manager = sim_manager.n_ticks(1);
    }

    let ctr = packet_counter.lock().unwrap();
    assert_eq!(*ctr, 42);

    assert!(
        sim_manager
            .global_state_manager
            .nodes()
            .iter()
            .zip(nodes.iter())
            .all(|(new, original)| {
                trace!(
                    "{:?} -> {:?}",
                    original.starting_position,
                    new.data().position
                );
                new.data().position != original.starting_position
            })
    );
}
