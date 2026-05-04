mod common;

use crate::common::behaviours::StaticMovement;
use crate::common::helpers::generate_cloned_nodes;
use common::behaviours::{LoggerNode, packet_counter};
use common::traffic_generators::mixed_multicast_and_random_target_unicast;
use log::{info, trace};
use rand::SeedableRng;
use rand_xoshiro::Xoshiro256Plus;
use slant::behaviours::MoveBehaviour;
use slant::builtin::move_behaviours::random_walk::RandomWalk;
use slant::builtin::node_behaviours::empty_behaviour::EmptyBehaviour;
use slant::builtin::node_behaviours::flood::{Flood, FloodPacket};
use slant::builtin::node_behaviours::monotonic::Monotonic;
use slant::builtin::packets::multicast::MulticastPacket;
use slant::builtin::packets::multicast_or_unicast::MulticastOrUnicast;
use slant::builtin::propagation_models::simple_distance::{SimpleDistance, SimpleDistanceParams};
use slant::managers::SimManager;
use slant::node::{NodeData, NodeID, NodeInit};
use slant::packets::{GloballySequencedPacket, Packet};
use slant::propagation_models::PropagationParams;
use slant::stats::InternalStatKey;
use slant::{Coord, SimConfig};
use std::env;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::Arc;

#[test]
fn one_hundred_secs_random_walk() {
    let propagation_params = SimpleDistanceParams {
        transmit_distance: 1.0,
    };

    let prop_model = SimpleDistance;

    let nodes = generate_cloned_nodes(
        8,
        1.0,
        propagation_params.clone(),
        RandomWalk::new(0.1, 2.0),
        Monotonic::new(
            EmptyBehaviour::new(),
            1.0,
            Arc::new(|_, _, contents| {
                MulticastOrUnicast::MulticastPacket(MulticastPacket { content: contents })
            }),
        ),
    );

    struct TestSimConfig;
    impl SimConfig<f32, 2> for TestSimConfig {
        type MB = RandomWalk<f32, 2>;
        type NB =
            Monotonic<f32, 2, EmptyBehaviour<MulticastOrUnicast>, SimpleDistanceParams<f32, 2>>;
        type PM = SimpleDistance;
    }

    let mut sim_manager: SimManager<_, _, TestSimConfig> =
        SimManager::new(nodes.clone(), 123, prop_model, 0.1);

    let stats = sim_manager.tick_time(100.0);

    let mut packets_received = 0;
    let mut packets_transmitted = 0;
    for mut stat in stats {
        let internal_stats = stat.internal_stats();
        packets_transmitted += internal_stats[InternalStatKey::PacketTransmits];
        packets_received += internal_stats[InternalStatKey::PacketReceives];
    }

    assert_eq!(packets_transmitted, 800);
    assert_eq!(packets_received, 926);

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
    env_logger::init();

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
    ) -> Vec<NodeInit<Monotonic<f32, 2, Flood<TestPacket, f32, 2>, P>, MB, P, f32, 2>> {
        let dim = num_nodes.isqrt();
        let mut nodes = Vec::with_capacity(num_nodes);
        for i in 0..num_nodes {
            let xy = [(i / dim) as f32 * gap, (i % dim) as f32 * gap];
            trace!("Spawning at {:?}", xy);
            nodes.push(NodeInit {
                starting_position: xy,
                node_behaviour: Monotonic::new(
                    Flood::new(5),
                    5.0,
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
        RandomWalk::new(1000.0, 20.0),
    );

    struct TestConfig;
    impl SimConfig<f32, 2> for TestConfig {
        type MB = RandomWalk<f32, 2>;
        type NB = Monotonic<f32, 2, Flood<TestPacket, f32, 2>, SimpleDistanceParams<f32, 2>>;
        type PM = SimpleDistance;
        type S = ();
        type E = ();
    }

    let mut sim: SimManager<_, _, TestConfig> = SimManager::new(nodes, 123456, SimpleDistance, 1.0);

    let stats = sim.n_ticks(100);
    let packets = stats
        .into_iter()
        .map(|mut x| x.internal_stats()[InternalStatKey::PacketReceives])
        .reduce(std::ops::Add::add)
        .unwrap_or_default();
    info!("Received {} packets total", packets);
}
