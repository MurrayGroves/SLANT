use log::info;
use slant::behaviours::{MoveBehaviour, NodeBehaviour};
use slant::builtin::move_behaviours::static_movement::StaticMovement;
use slant::builtin::node_behaviours::monotonic::Monotonic;
use slant::builtin::propagation_models::free_space::{FreeSpace, FreeSpaceParams};
use slant::managers::{GlobalStateManager, SimManager};
use slant::node::{NodeData, NodeID, NodeInit};
use slant::packets::{GloballySequencedPacket, Packet};
use slant::propagation_models::{PropagationModel, PropagationParams};
use slant::stats::InternalStatKey;
use slant::{Coord, SimConfig};
use std::collections::HashSet;
use std::env;
use std::fmt::Debug;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::marker::PhantomData;
use std::sync::Arc;

/// This is the struct for our node behaviour
#[derive(Clone)]
pub struct Flood<A: Coord<K>, const K: usize> {
    /// Sequence numbers of all packets this node has seen
    seen_packets: HashSet<u16>,
    /// Seq of most recent packet generated
    seq: u16,
    /// Hop count for new packets
    hop_count: u8,
    coord_type: PhantomData<A>,
}

impl<A, const K: usize, PP: PropagationParams<A, K>> NodeBehaviour<A, K, PP> for Flood<A, K>
where
    A: Coord<K>,
{
    // We define our node behaviour as only working for packets of our FloodPacket type.
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
                // Add packet to our seen packets so we don't rebroadcast it again in the future
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

    // We define this function to generate a new packet with an increasing sequence number so that we can use our behaviour with the builtin Monotonic wrapper.
    pub fn gen_packet(
        &mut self,
        data: &NodeData<A, K, impl PropagationParams<A, K>>,
        content: Arc<Box<[u8]>>,
    ) -> FloodPacket {
        let packet = FloodPacket::new(data, self.hop_count, self.seq, content);
        self.seq += 1;
        packet
    }

    pub fn seq(&self) -> u16 {
        self.seq
    }
}

// Here's our packet type that our node behaviour works with
#[derive(Clone, Debug)]
pub struct FloodPacket {
    /// Number of hops remaining in this packet, will cause it to be rebroadcast if above zero
    hops: u8,
    /// Sequence number is incremented for each new packet from a node
    seq: u16,
    /// We encode the source address so that we can generate a globally unique seq from it and the packet's local seq number
    src: NodeID,
    /// All packets must store some data
    content: Arc<Box<[u8]>>,
}

impl Packet for FloodPacket {
    fn content(self) -> Arc<Box<[u8]>> {
        self.content
    }

    fn content_ref(&self) -> &Arc<Box<[u8]>> {
        &self.content
    }

    // Our flood packets have no specific target, they can be received by anyone
    fn eager_targets(&self) -> Option<Vec<NodeID>> {
        None
    }

    // Any node can receive our packets
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

    // GloballySequencedPacket requires each packet to have a unique sequence number that is not repeated between nodes, so we hash our source and sequence number.
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

// Here's a little helper function which generates as many nodes as we want in a grid.
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
                // This closure is used by Monotonic to generate new packets.
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

    let num_nodes = env::var("NUM_NODES")
        .unwrap_or("1024".into())
        .parse::<usize>()
        .unwrap();

    struct TestConfig;
    impl SimConfig<f32, 2> for TestConfig {
        // In this simulation nodes won't move
        type MB = StaticMovement;
        // Nodes use the builtin Monotonic behaviour which broadcasts new packets every N ticks.
        // We use Monotonic to wrap our existing behaviour.
        type NB = Monotonic<f32, 2, Flood<f32, 2>, FloodPacket, FreeSpaceParams<f32, 2>>;
        // We're going to use the Friis transmission equation here as our propagation model.
        type PM = FreeSpace;
    }

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

    // We define a new simulation using our configuration.
    let mut sim: SimManager<_, _, TestConfig> = SimManager::new(nodes, 123456, FreeSpace);

    // n_ticks returns a vector of stats for each timestep.
    let stats = sim.n_ticks(10);

    let mut originated_packets = 0;
    for node in sim.global_state_manager.nodes() {
        originated_packets += node.node_behaviour().contained.seq();
    }

    // The simulator internally keeps track of packet receives, so we can just sum the count across all ticks.
    let received_packets = stats
        .into_iter()
        .map(|mut x| x.internal_stats()[InternalStatKey::PacketReceives])
        .reduce(std::ops::Add::add)
        .unwrap_or_default();

    info!("Received {} packets total", received_packets);
    info!("Originated {} packets total", originated_packets);
}
