use log::trace;
use manetsim::example_behaviours::RandomWalk;
use manetsim::example_behaviours::flood::{Flood, FloodPacket};
use manetsim::packets::{GloballySequencedPacket, Packet};
use manetsim::propagation_models::{
    FreeSpace, FreeSpaceParams, PropagationModel, PropagationParams,
};
use manetsim::types::{
    Coord, GlobalStateManager, NodeBehaviour, NodeData, NodeID, NodeInit, SimConfig,
};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::marker::PhantomData;
use std::sync::Arc;

#[derive(Clone, Debug)]
pub struct TestPacket {
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
        let mut s = DefaultHasher::new();
        self.seq.hash(&mut s);
        self.src.hash(&mut s);
        let hash = az::overflowing_cast(s.finish()).0;
        trace!("{:?}, {:?} hashed to {:?}", self.seq, self.src, hash);
        hash
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
pub struct Monotonic<
    A: Coord<K>,
    const K: usize,
    P: Packet<A, K> + Clone,
    PP: PropagationParams<A, K>,
> {
    packet_type: PhantomData<P>,
    pub ticks_per_packet: usize,
    counter: usize,
    pub received_packets: usize,
    gen_packet: Arc<dyn Fn(&NodeData<A, K, PP>, Box<[u8]>) -> P + Send + Sync>,
}

impl<A: Coord<K>, const K: usize, PP: PropagationParams<A, K>, P: Packet<A, K> + Clone>
    Monotonic<A, K, P, PP>
{
    pub fn new(
        ticks_per_packet: usize,
        gen_packet: Arc<dyn Fn(&NodeData<A, K, PP>, Box<[u8]>) -> P + Send + Sync>,
    ) -> Self {
        Monotonic {
            packet_type: Default::default(),
            ticks_per_packet,
            counter: 0,
            received_packets: 0,
            gen_packet,
        }
    }
}

impl<A: Coord<K>, const K: usize, PP: PropagationParams<A, K>, P: Packet<A, K> + Clone>
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
pub struct MonotonicFlood {
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

pub fn generate_nodes(
    num_nodes: usize,
    gap: f32,
) -> Vec<NodeInit<MonotonicFlood, RandomWalk<30, f32, 2>, FreeSpaceParams<f32, 2>, f32, 2>> {
    let dim = num_nodes.isqrt();
    let mut nodes = Vec::with_capacity(num_nodes);
    for i in 0..num_nodes {
        let xy = [(i / dim) as f32 * gap, (i % dim) as f32 * gap];
        trace!("Spawning at {:?}", xy);
        nodes.push(NodeInit {
            starting_position: xy,
            node_behaviour: MonotonicFlood::new(),
            move_behaviour: RandomWalk::new(1000.0),
            propagation_params: FreeSpaceParams::new(
                8.0,
                0.34538301613, // 868mhz in metres
                |_, _| 11.0,   // Omnidirectional
                11.0,
                |_, _| 0.0, // Omnidirectional
                -90.0,
            ),
        })
    }
    nodes
}
pub struct SimConf;
impl SimConfig<f32, 2> for SimConf {
    type MB = RandomWalk<30, f32, 2>;
    type NB = MonotonicFlood;
    type PM = FreeSpace;
    type S = ();
}
