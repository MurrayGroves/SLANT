use log::{debug, info, trace};
use manetsim::packets::{GloballySequencedPacket, Packet};
use manetsim::propagation_models::{
    FreeSpace, FreeSpaceParams, PropagationModel, PropagationParams,
};
use manetsim::types::{
    Coord, GlobalStateManager, MoveBehaviour, NodeBehaviour, NodeData, NodeID, NodeInit, SimConfig,
    SimManager,
};
use num_traits::{Num, NumCast, Zero};
use std::collections::HashSet;
use std::env;
use std::fmt::Debug;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::marker::PhantomData;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct Monotonic<A: Coord<K>, const K: usize, P: Packet + Clone, PP: PropagationParams<A, K>> {
    pub ticks_per_packet: usize,
    counter: usize,
    pub received_packets: usize,
    gen_packet: Arc<dyn Fn(&NodeData<A, K, PP>, Box<[u8]>) -> P + Send + Sync>,
}

impl<A: Coord<K>, const K: usize, PP: PropagationParams<A, K>, P: Packet + Clone>
    Monotonic<A, K, P, PP>
{
    pub fn new(
        ticks_per_packet: usize,
        gen_packet: Arc<dyn Fn(&NodeData<A, K, PP>, Box<[u8]>) -> P + Send + Sync>,
    ) -> Self {
        Monotonic {
            ticks_per_packet,
            counter: 0,
            received_packets: 0,
            gen_packet,
        }
    }
}

impl<A: Coord<K>, const K: usize, PP: PropagationParams<A, K>, P: Packet + Clone>
    NodeBehaviour<A, K, PP> for Monotonic<A, K, P, PP>
{
    type P = P;

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
        trace!("Ticking {}", node_data.id);
        self.received_packets += incoming_packets.len();
        trace!(
            "{} received {:?}",
            node_data.id,
            incoming_packets.iter().map(|x| x.content_ref()[0])
        );

        if self.counter % self.ticks_per_packet == 0 {
            global_state_manager.transmit_packet(
                node_data,
                (self.gen_packet)(node_data, Box::new(node_data.id.to_be_bytes())),
            )
        }

        self.counter += 1;

        self
    }
}

#[derive(Clone)]
pub struct StaticMovement {}

impl MoveBehaviour<f32, 2> for StaticMovement {
    fn tick<C: SimConfig<f32, 2, MB = Self>>(
        self,
        data: &NodeData<f32, 2, <C::PM as PropagationModel<f32, 2>>::P>,
        global_state_manager: &GlobalStateManager<f32, 2, C>,
    ) -> (Self, [f32; 2]) {
        (self, data.position)
    }
}
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
                trace!(
                    "{:?}: transmitting packet {:?} with {:?} hops left",
                    node_data.id,
                    packet.seq(),
                    packet.get_hop_count()
                );
                global_state_manager.transmit_packet(node_data, packet);
            } else {
                trace!("{:?} has already seen {:?}", node_data.id, packet.seq());
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
        &self,
        data: &NodeData<A, K, impl PropagationParams<A, K>>,
        content: Box<[u8]>,
    ) -> FloodPacket {
        let mut seq = self.seq;
        let packet = FloodPacket::new(data, self.hop_count, seq, content);
        seq += 1;
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
        let hash = az::overflowing_cast(s.finish()).0;
        trace!("{:?}, {:?} hashed to {:?}", self.seq, self.src, hash);
        hash
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

#[derive(Clone)]
struct MonotonicFlood<A: Coord<K>, const K: usize, P: PropagationParams<A, K>> {
    monotonic: Monotonic<A, K, FloodPacket, P>,
    flood: Flood<A, K>,
}

impl<A: Coord<K>, const K: usize, P: PropagationParams<A, K>> NodeBehaviour<A, K, P>
    for MonotonicFlood<A, K, P>
{
    type P = FloodPacket;
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

fn main() {
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
