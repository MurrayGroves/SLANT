use log::trace;
use manetsim::behaviours::NodeBehaviour;
use manetsim::builtin::move_behaviours::random_walk::RandomWalk;
use manetsim::builtin::propagation_models::simple_distance::{
    SimpleDistance, SimpleDistanceParams,
};
use manetsim::managers::GlobalStateManager;
use manetsim::node::{NodeData, NodeID, NodeInit};
use manetsim::packets::{GloballySequencedPacket, Packet};
use manetsim::propagation_models::{PropagationModel, PropagationParams};
use manetsim::{Coord, SimConfig};
use num_traits::{Num, NumCast, One, Zero};
use std::collections::HashSet;
use std::fmt::Debug;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::marker::PhantomData;
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug)]
pub struct TestPacket {
    hops: u8,
    seq: u16,
    src: NodeID,
    content: Box<[u8]>,
}

impl Packet for TestPacket {
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
pub struct Monotonic<A: Coord<K>, const K: usize, P: Packet + Clone, PP: PropagationParams<A, K>> {
    packet_type: PhantomData<P>,
    gen_packet: Arc<dyn Fn(&NodeData<A, K, PP>, Box<[u8]>) -> P + Send + Sync>,
}

impl<A: Coord<K>, const K: usize, PP: PropagationParams<A, K>, P: Packet + Clone>
    Monotonic<A, K, P, PP>
{
    const CHANCE: f32 = 0.0002;

    pub fn new(gen_packet: Arc<dyn Fn(&NodeData<A, K, PP>, Box<[u8]>) -> P + Send + Sync>) -> Self {
        Monotonic {
            packet_type: Default::default(),
            gen_packet,
        }
    }
}

impl<A: Coord<K>, const K: usize, PP: PropagationParams<A, K>, P: GloballySequencedPacket + Clone>
    NodeBehaviour<A, K, PP> for Monotonic<A, K, P, PP>
{
    type P = P;
    type E = SeqPacketTransmit<<P as GloballySequencedPacket>::S>;

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
        trace!(
            "{} received {:?}",
            node_data.id,
            incoming_packets.iter().map(|x| x.content_ref()[0])
        );

        let rand: f32 = global_state_manager.get_random(node_data.id);

        if rand < Self::CHANCE {
            global_state_manager.transmit_packet(
                node_data,
                (self.gen_packet)(node_data, Box::new(node_data.id.to_be_bytes())),
            )
        }

        self
    }
}
#[derive(Clone)]
pub struct Flood<PT: FloodPacket + Clone + ?Sized, A: Coord<K>, const K: usize> {
    seen_packets: Arc<Mutex<HashSet<PT::S>>>,
    coord_type: PhantomData<A>,
    /// Seq of most recent packet generated
    seq: Arc<Mutex<PT::S>>,
    /// Hop count for new packets
    hop_count: usize,
}

pub trait FloodPacket: Packet + GloballySequencedPacket + Clone {
    /// Type of hop count
    type H: Num + NumCast + PartialOrd + Zero;
    fn get_hop_count(&self) -> <Self as FloodPacket>::H;

    fn set_hop_count(&mut self, count: <Self as FloodPacket>::H);

    fn new<A: Coord<K>, const K: usize>(
        data: &NodeData<A, K, impl PropagationParams<A, K>>,
        hops: Self::H,
        seq: Self::S,
        content: Box<[u8]>,
    ) -> Self;
}

impl<
    A,
    const K: usize,
    PT,
    H: Num + NumCast + PartialOrd + Zero + Send + Sync + Clone + Debug,
    PP: PropagationParams<A, K>,
> NodeBehaviour<A, K, PP> for Flood<PT, A, K>
where
    A: Coord<K>,
    PT: FloodPacket<H = H>,
{
    type P = PT;
    type E = SeqPacketTransmit<PT::S>;

    fn tick<
        C: SimConfig<
                A,
                K,
                PM = impl PropagationModel<A, K, P = PP>,
                NB = impl NodeBehaviour<A, K, PP, P = Self::P, E = Self::E>,
                E = Self::E,
            >,
    >(
        mut self,
        node_data: &NodeData<A, K, <C::PM as PropagationModel<A, K>>::P>,
        global_state_manager: &GlobalStateManager<A, K, C>,
        incoming_packets: &Vec<Self::P>,
    ) -> Self {
        let mut seen_packets = self.seen_packets.lock().unwrap();
        for packet in incoming_packets {
            // If packet hasn't been relayed before, and remaining hop count is greater than zero, retransmit
            if !seen_packets.contains(&packet.seq())
                && packet.get_hop_count() > <PT as FloodPacket>::H::zero()
            {
                let mut packet = packet.clone();
                packet.set_hop_count(packet.get_hop_count() - <PT as FloodPacket>::H::one());
                seen_packets.insert(packet.seq());
                trace!(
                    "{:?}: transmitting packet {:?} with {:?} hops left",
                    node_data.id,
                    packet.seq(),
                    packet.get_hop_count()
                );
                global_state_manager.add_event(SeqPacketTransmit {
                    node: node_data.id,
                    seq: packet.seq(),
                });
                global_state_manager.transmit_packet(node_data, packet);
            } else {
                trace!("{:?} has already seen {:?}", node_data.id, packet.seq());
            }
        }
        drop(seen_packets);
        self
    }
}

impl<PT: FloodPacket + Clone, A: Coord<K>, const K: usize> Flood<PT, A, K> {
    pub fn new(hops: usize) -> Self {
        Self {
            seen_packets: Arc::new(Mutex::new(HashSet::new())),
            coord_type: PhantomData,
            seq: Arc::new(Mutex::new(PT::S::zero())),
            hop_count: hops,
        }
    }

    pub fn gen_packet(
        &self,
        data: &NodeData<A, K, impl PropagationParams<A, K>>,
        content: Box<[u8]>,
    ) -> PT {
        let mut seq = self.seq.lock().unwrap();
        let packet = PT::new(
            data,
            <<PT as FloodPacket>::H as NumCast>::from(self.hop_count).unwrap(),
            *seq,
            content,
        );
        *seq += PT::S::one();
        packet
    }
}

impl<PT: FloodPacket + Clone, A: Coord<K>, const K: usize> Flood<PT, A, K> {
    pub fn seq(&self) -> PT::S {
        *self.seq.lock().unwrap()
    }
}

#[derive(Clone)]
pub struct MonotonicFlood {
    monotonic: Monotonic<f32, 2, TestPacket, SimpleDistanceParams<f32, 2>>,
    flood: Flood<TestPacket, f32, 2>,
}

impl NodeBehaviour<f32, 2, SimpleDistanceParams<f32, 2>> for MonotonicFlood {
    type P = TestPacket;
    type E = SeqPacketTransmit<<Self::P as GloballySequencedPacket>::S>;
    fn tick<
        C: SimConfig<
                f32,
                2,
                PM = impl PropagationModel<f32, 2, P = SimpleDistanceParams<f32, 2>>,
                NB = impl NodeBehaviour<f32, 2, SimpleDistanceParams<f32, 2>, P = Self::P, E = Self::E>,
                E = Self::E,
            >,
    >(
        mut self,
        node_data: &NodeData<f32, 2, <<C as SimConfig<f32, 2>>::PM as PropagationModel<f32, 2>>::P>,
        global_state_manager: &GlobalStateManager<f32, 2, C>,
        packets: &Vec<<Self as NodeBehaviour<f32, 2, SimpleDistanceParams<f32, 2>>>::P>,
    ) -> Self {
        self.monotonic = self
            .monotonic
            .tick(node_data, global_state_manager, packets);

        self.flood = self.flood.tick(node_data, global_state_manager, packets);
        self
    }
}

impl MonotonicFlood {
    fn new(hops: usize) -> Self {
        let flood = Flood::new(hops);
        let clone = flood.clone();
        Self {
            monotonic: Monotonic::new(Arc::new(move |data, content| {
                flood.gen_packet(data, content)
            })),
            flood: clone,
        }
    }
}

pub fn generate_nodes(
    num_nodes: usize,
    gap: f32,
) -> Vec<NodeInit<MonotonicFlood, RandomWalk<30, f32, 2>, SimpleDistanceParams<f32, 2>, f32, 2>> {
    let dim = num_nodes.isqrt();
    let mut nodes = Vec::with_capacity(num_nodes);
    for i in 0..num_nodes {
        let xy = [(i / dim) as f32 * gap, (i % dim) as f32 * gap];
        trace!("Spawning at {:?}", xy);
        nodes.push(NodeInit {
            starting_position: xy,
            node_behaviour: MonotonicFlood::new(15),
            move_behaviour: RandomWalk::new(100.0),
            // propagation_params: FreeSpaceParams::new(
            //     8.0,
            //     0.34538301613, // 868mhz in metres
            //     |_, _| 11.0,   // Omnidirectional
            //     11.0,
            //     |_, _| 0.0, // Omnidirectional
            //     -90.0,
            // ),
            propagation_params: SimpleDistanceParams {
                transmit_distance: 5000.0,
            },
        })
    }
    nodes
}

#[derive(Clone, Debug)]
pub struct SeqPacketTransmit<S: Clone> {
    pub node: NodeID,
    pub seq: S,
}

pub struct SimConf;
impl SimConfig<f32, 2> for SimConf {
    type MB = RandomWalk<30, f32, 2>;
    type NB = MonotonicFlood;
    type PM = SimpleDistance;
    type S = ();
    type E = SeqPacketTransmit<<TestPacket as GloballySequencedPacket>::S>;
}
