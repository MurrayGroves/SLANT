use log::info;
use manetsim::behaviours::{MoveBehaviour, NodeBehaviour};
use manetsim::builtin::move_behaviours::static_movement::StaticMovement;
use manetsim::builtin::node_behaviours::monotonic::Monotonic;
use manetsim::builtin::propagation_models::free_space::{FreeSpace, FreeSpaceParams};
use manetsim::managers::{GlobalStateManager, SimManager};
use manetsim::node::{NodeData, NodeID, NodeInit};
use manetsim::packets::{GloballySequencedPacket, Packet};
use manetsim::propagation_models::{PropagationModel, PropagationParams};
use manetsim::stats::InternalStatKey;
use manetsim::{Coord, SimConfig};
use std::collections::HashSet;
use std::env;
use std::fmt::Debug;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::marker::PhantomData;
use std::sync::Arc;
use std::time::Instant;

#[derive(Clone)]
pub struct Flood<A: Coord<K>, const K: usize> {
    seen_packets: HashSet<u16>,
    coord_type: PhantomData<A>,
    /// Seq of most recent packet generated
    seq: u16,
    /// Hop count for new packets
    hop_count: u8,
}

impl<A, const K: usize, PP: PropagationParams<A, K>> NodeBehaviour<A, K, PP> for Flood<A, K>
where
    A: Coord<K>,
{
    type P = FloodPacket;

    fn tick<
        C: SimConfig<
                A,
                K,
                PM = impl PropagationModel<A, K, P = PP>,
                NB = impl NodeBehaviour<A, K, PP, P = Self::P>,
            >,
    >(
        mut self,
        node_data: &NodeData<A, K, <C::PM as PropagationModel<A, K>>::P>,
        global_state_manager: &GlobalStateManager<A, K, C>,
        incoming_packets: &Vec<Self::P>,
    ) -> Self {
        for packet in incoming_packets {
            // If packet hasn't been relayed before, and remaining hop count is greater than zero, retransmit
            if !self.seen_packets.contains(&packet.seq()) && packet.get_hop_count() > 0 {
                let mut packet = packet.clone();
                packet.set_hop_count(packet.get_hop_count() - 1);
                self.seen_packets.insert(packet.seq());
                global_state_manager.transmit_packet(node_data, packet);
            }
        }
        self
    }
}

impl<A: Coord<K>, const K: usize> Flood<A, K> {
    pub fn new(hops: u8) -> Self {
        Self {
            seen_packets: HashSet::new(),
            coord_type: PhantomData,
            seq: 0,
            hop_count: hops,
        }
    }

    pub fn gen_packet(
        &mut self,
        data: &NodeData<A, K, impl PropagationParams<A, K>>,
        content: Box<[u8]>,
    ) -> FloodPacket {
        let packet = FloodPacket::new(data, self.hop_count, self.seq, content);
        self.seq += 1;
        packet
    }

    pub fn seq(&self) -> u16 {
        self.seq
    }
}

#[derive(Clone, Debug)]
pub struct FloodPacket {
    hops: u8,
    seq: u16,
    src: NodeID,
    content: Box<[u8]>,
}

impl Packet for FloodPacket {
    fn content(self) -> Box<[u8]> {
        self.content
    }

    fn content_ref(&self) -> &Box<[u8]> {
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

impl GloballySequencedPacket for FloodPacket {
    type S = u16;

    fn seq(&self) -> Self::S {
        let mut s = DefaultHasher::new();
        self.seq.hash(&mut s);
        self.src.hash(&mut s);
        az::overflowing_cast(s.finish()).0
    }
}

impl FloodPacket {
    fn get_hop_count(&self) -> u8 {
        self.hops
    }

    fn set_hop_count(&mut self, count: u8) {
        self.hops = count;
    }

    fn new<A: Coord<K>, const K: usize>(
        data: &NodeData<A, K, impl PropagationParams<A, K>>,
        hops: u8,
        seq: u16,
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

fn generate_nodes<MB: MoveBehaviour<f32, 2>, P: PropagationParams<f32, 2>>(
    num_nodes: usize,
    gap: f32,
    params: P,
    move_behaviour: MB,
) -> Vec<NodeInit<Monotonic<f32, 2, Flood<f32, 2>, FloodPacket, P>, MB, P, f32, 2>> {
    let dim = num_nodes.isqrt();
    let mut nodes = Vec::with_capacity(num_nodes);
    for i in 0..num_nodes {
        let xy = [(i / dim) as f32 * gap, (i % dim) as f32 * gap];
        nodes.push(NodeInit {
            starting_position: xy,
            node_behaviour: Monotonic::new(
                Flood::new(5),
                5,
                Arc::new(|flood, data, contents| {
                    let seq = flood.seq;
                    flood.seq += 1;
                    FloodPacket::new(data, 5, seq, contents)
                }),
            ),
            move_behaviour: move_behaviour.clone(),
            propagation_params: params.clone(),
        })
    }
    nodes
}

fn main() {
    env_logger::init();

    let start = Instant::now();

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
        type NB = Monotonic<f32, 2, Flood<f32, 2>, FloodPacket, FreeSpaceParams<f32, 2>>;
        type PM = FreeSpace;
    }

    let mut sim: SimManager<_, _, TestConfig> = SimManager::new(nodes, 123456, FreeSpace);

    let stats = sim.n_ticks(10);

    let mut originated_packets = 0;
    for node in sim.global_state_manager.nodes() {
        originated_packets += node.node_behaviour().contained.seq();
    }

    let received_packets = stats
        .into_iter()
        .map(|mut x| x.internal_stats()[InternalStatKey::PacketReceives])
        .reduce(std::ops::Add::add)
        .unwrap_or_default();

    info!("Received {} packets total", received_packets);
    info!("Originated {} packets total", originated_packets);
    info!("Simulation took {}", start.elapsed().as_secs_f64());
}
