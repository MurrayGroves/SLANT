mod common;

use log::trace;
use manetsim::builtin::move_behaviours::random_walk::RandomWalk;
use manetsim::traffic_generators::mixed_multicast_and_random_target_unicast;
use manetsim::types::{NodeInit, SimConfig, SimManager};
use rand::SeedableRng;
use rand_xoshiro::Xoshiro256Plus;

use common::behaviours::{LoggerNode, packet_counter};
use manetsim::builtin::propagation_models::simple_distance::{
    SimpleDistance, SimpleDistanceParams,
};

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
            move_behaviour: RandomWalk::new(1.0),
            propagation_params: propagation_params.clone(),
        },
        NodeInit {
            starting_position: [1.0, 0.0],
            node_behaviour: LoggerNode {},
            move_behaviour: RandomWalk::new(1.0),
            propagation_params: propagation_params.clone(),
        },
        NodeInit {
            starting_position: [1.0, 1.0],
            node_behaviour: LoggerNode {},
            move_behaviour: RandomWalk::new(1.0),
            propagation_params: propagation_params.clone(),
        },
        NodeInit {
            starting_position: [0.0, 1.0],
            node_behaviour: LoggerNode {},
            move_behaviour: RandomWalk::new(1.0),
            propagation_params: propagation_params.clone(),
        },
        NodeInit {
            starting_position: [1.0, 0.0],
            node_behaviour: LoggerNode {},
            move_behaviour: RandomWalk::new(1.0),
            propagation_params: propagation_params.clone(),
        },
        NodeInit {
            starting_position: [1.0, 5.0],
            node_behaviour: LoggerNode {},
            move_behaviour: RandomWalk::new(1.0),
            propagation_params: propagation_params.clone(),
        },
        NodeInit {
            starting_position: [1.0, 0.0],
            node_behaviour: LoggerNode {},
            move_behaviour: RandomWalk::new(1.0),
            propagation_params,
        },
    ];

    struct TestSimConfig;
    impl SimConfig<f64, 2> for TestSimConfig {
        type MB = RandomWalk<2, f64, 2>;
        type NB = LoggerNode;
        type PM = SimpleDistance;
        type S = ();
        type E = ();
    }

    let mut sim_manager: SimManager<_, _, TestSimConfig> =
        SimManager::new(nodes.clone(), 123, prop_model);
    let mut rng = Xoshiro256Plus::seed_from_u64(123456);
    for _ in 0..10 {
        mixed_multicast_and_random_target_unicast(
            &mut rng,
            &sim_manager.global_state_manager,
            5,
            5,
        );
        sim_manager.n_ticks(1);
    }

    let ctr = packet_counter.lock().unwrap();
    assert_eq!(*ctr, 48);

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
