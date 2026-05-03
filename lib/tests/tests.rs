mod common;

use common::behaviours::{LoggerNode, packet_counter};
use common::traffic_generators::mixed_multicast_and_random_target_unicast;
use log::trace;
use manetsim::behaviours::MoveBehaviour;
use manetsim::builtin::move_behaviours::random_walk::RandomWalk;
use manetsim::builtin::node_behaviours::flood::{Flood, FloodPacket};
use manetsim::builtin::node_behaviours::monotonic::Monotonic;
use manetsim::builtin::propagation_models::simple_distance::{
    SimpleDistance, SimpleDistanceParams,
};
use manetsim::managers::SimManager;
use manetsim::node::{NodeData, NodeID, NodeInit};
use manetsim::packets::{GloballySequencedPacket, Packet};
use manetsim::propagation_models::PropagationParams;
use manetsim::{Coord, SimConfig};
use rand::SeedableRng;
use rand_xoshiro::Xoshiro256Plus;
use std::env;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::Arc;

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
#[test]
fn random_walk_scale_test() {
    //env_logger::init();

    let num_nodes = env::var("NUM_NODES")
        .unwrap_or("1024".into())
        .parse::<usize>()
        .unwrap();
    #[derive(Clone, Debug)]
    struct TestPacket {
        hops: u8,
        seq: u16,
        src: NodeID,
        content: Arc<Box<[u8]>>,
    }

    impl Packet for TestPacket {
        fn content(self) -> Arc<Box<[u8]>> {
            self.content
        }

        fn content_ref(&self) -> &Arc<Box<[u8]>> {
            &self.content
        }

        fn eager_targets(&self) -> Option<Vec<NodeID>> {
            None
        }

        fn targets<A: Coord<K>, const K: usize, P: PropagationParams<A, K>>(
            &self,
            target: &NodeData<A, K, P>,
        ) -> bool
        where
            Self: Sized,
        {
            true
        }
    }

    impl GloballySequencedPacket for TestPacket {
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

    impl FloodPacket for TestPacket {
        type H = u8;

        fn get_hop_count(&self) -> <Self as FloodPacket>::H {
            self.hops
        }

        fn set_hop_count(&mut self, count: <Self as FloodPacket>::H) {
            self.hops = count;
        }

        fn new<A: Coord<K>, const K: usize>(
            data: &NodeData<A, K, impl PropagationParams<A, K>>,
            hops: Self::H,
            seq: Self::S,
            content: Arc<Box<[u8]>>,
        ) -> Self {
            Self {
                hops,
                seq,
                content,
                src: data.id,
            }
        }
    }

    fn generate_nodes<MB: MoveBehaviour<f32, 2>, P: PropagationParams<f32, 2>>(
        num_nodes: usize,
        gap: f32,
        params: P,
        move_behaviour: MB,
    ) -> Vec<NodeInit<Monotonic<f32, 2, Flood<TestPacket, f32, 2>, TestPacket, P>, MB, P, f32, 2>>
    {
        let dim = num_nodes.isqrt();
        let mut nodes = Vec::with_capacity(num_nodes);
        for i in 0..num_nodes {
            let xy = [(i / dim) as f32 * gap, (i % dim) as f32 * gap];
            trace!("Spawning at {:?}", xy);
            nodes.push(NodeInit {
                starting_position: xy,
                node_behaviour: Monotonic::new(
                    Flood::new(5),
                    5,
                    Arc::new(|flood, data, contents| flood.gen_packet(data, contents)),
                ),
                move_behaviour: move_behaviour.clone(),
                propagation_params: params.clone(),
            })
        }
        nodes
    }

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
        type NB =
            Monotonic<f32, 2, Flood<TestPacket, f32, 2>, TestPacket, SimpleDistanceParams<f32, 2>>;
        type PM = SimpleDistance;
        type S = ();
        type E = ();
    }

    let mut sim: SimManager<_, _, TestConfig> = SimManager::new(nodes, 123456, SimpleDistance);

    sim.n_ticks(100);
}
