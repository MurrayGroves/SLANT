use crate::packets::Packet;
use kiddo::SquaredEuclidean;
use kiddo::float_leaf_slice::leaf_slice::{LeafSliceFloat, LeafSliceFloatChunk};
use kiddo::immutable::float::kdtree::ImmutableKdTree;
use kiddo::traits::Content;
use num_traits::Float;
use num_traits::float::FloatCore;
use rand_xoshiro::Xoshiro256Plus;
use rand_xoshiro::rand_core::RngCore;
use std::cell::UnsafeCell;
use std::collections::HashMap;
use std::iter::Sum;
use std::sync::{Arc, Mutex};

/// Describes a node behaviour which performs some processing each tick to produce a new node behaviour
pub trait NodeBehaviour<A: Coord<K>, const K: usize>
where
    Self: Sized + Send + Clone,
{
    fn tick(
        self,
        node_data: &NodeData<A, K>,
        global_state_manager: &GlobalStateManager<Self, impl MoveBehaviour<A, K>, A, K>,
        incoming_packets: &Vec<Arc<dyn Packet<A, K>>>,
    ) -> Self;
}

pub trait MoveBehaviour<A: Coord<K>, const K: usize>
where
    Self: Sized + Send + Clone,
{
    fn tick(
        self,
        data: &NodeData<A, K>,
        global_state_manager: &GlobalStateManager<impl NodeBehaviour<A, K>, Self, A, K>,
    ) -> (Self, [A; K]);
}

pub type NodeID = usize;

#[derive(Clone)]
struct Node<
    NodeBehaviourType: NodeBehaviour<A, K>,
    MoveBehaviourType: MoveBehaviour<A, K>,
    A: Coord<K>,
    const K: usize,
> {
    behaviour: NodeBehaviourType,
    move_behaviour: MoveBehaviourType,
    pub(crate) data: NodeData<A, K>,
}

/// One dimension in a coordinate-space, should be either f32 or f64
pub trait Coord<const K: usize>:
    Float + Sum + kiddo::float::kdtree::Axis + LeafSliceFloatChunk<u32, K> + LeafSliceFloat<u32>
{
}

#[derive(Clone)]
pub struct NodeData<A: Coord<K>, const K: usize> {
    pub position: [A; K],
    pub id: NodeID,
    broadcast_distance: A,
}

impl<
    NodeBehaviourType: NodeBehaviour<A, K>,
    MoveBehaviourType: MoveBehaviour<A, K>,
    A: Coord<K>,
    const K: usize,
> Node<NodeBehaviourType, MoveBehaviourType, A, K>
{
    fn tick_behaviour(
        self,
        global_state_manager: &GlobalStateManager<NodeBehaviourType, MoveBehaviourType, A, K>,
        incoming_packets: &Vec<Arc<dyn Packet<A, K>>>,
    ) -> Self {
        Self {
            behaviour: self
                .behaviour
                .tick(&self.data, global_state_manager, incoming_packets),
            data: self.data,
            move_behaviour: self.move_behaviour,
        }
    }

    fn tick_movement(
        self,
        global_state_manager: &GlobalStateManager<NodeBehaviourType, MoveBehaviourType, A, K>,
    ) -> Self {
        let mut new = self.clone();
        let (move_behaviour, position) = self.move_behaviour.tick(&self.data, global_state_manager);
        new.move_behaviour = move_behaviour;
        let mut data = self.data;
        data.position = position;
        new.data = data;
        new
    }
}

pub struct GlobalStateManager<
    'a,
    NodeBehaviourType: NodeBehaviour<A, K>,
    MoveBehaviourType: MoveBehaviour<A, K>,
    A: Coord<K>,
    const K: usize,
> {
    pub sim_manager: &'a SimManager<'a, NodeBehaviourType, MoveBehaviourType, A, K>,
    pub(crate) nodes: Vec<Node<NodeBehaviourType, MoveBehaviourType, A, K>>,
    /// 32 is the bucket size, might be worth profiling different values (see https://github.com/sdd/kiddo/blob/20560517c7e06d71a6887a7662b89b70091ef8db/examples/cities.rs#L96)
    tree: ImmutableKdTree<A, u32, K, 32>,
    /// Packets that have been sent to each node in the previous tick.
    incoming_packets: HashMap<NodeID, Vec<Arc<dyn Packet<A, K>>>>,
    /// Packets that have been sent to each node during this tick.
    new_packets: HashMap<NodeID, Mutex<Vec<Arc<dyn Packet<A, K>>>>>,
}

impl<
    NodeBehaviourType: NodeBehaviour<A, K>,
    MoveBehaviourType: MoveBehaviour<A, K>,
    A: Coord<K>,
    const K: usize,
> GlobalStateManager<'_, NodeBehaviourType, MoveBehaviourType, A, K>
{
    fn tick(self) -> Self {
        let nodes: Vec<Node<NodeBehaviourType, MoveBehaviourType, A, K>> = self
            .nodes
            .clone()
            .into_iter()
            .map(|x| x.tick_movement(&self))
            .collect();

        let nodes = nodes
            .into_iter()
            .map(|x| {
                let packets = self.incoming_packets.get(&x.data.id).unwrap();
                x.tick_behaviour(&self, packets)
            })
            .collect();

        Self {
            nodes,
            sim_manager: self.sim_manager,
            tree: self.tree, // TODO - rebuild because of movement
            incoming_packets: self
                .new_packets
                .into_iter()
                .map(|(id, packets)| (id, packets.into_inner().unwrap()))
                .collect(),
            new_packets: HashMap::from_iter(
                (0..self.nodes.len()).map(|x| (x, Mutex::new(Vec::new()))),
            ), // TODO - Don't instantiate a new one each tick!
        }
    }

    pub fn transmit_packet(
        &self,
        transmitter: &NodeData<A, K>,
        packet: impl Packet<A, K> + 'static,
    ) {
        let eager_targets = packet.eager_targets();
        let recipients: Vec<&NodeID> = match &eager_targets {
            Some(targets) => {
                targets
                    .iter()
                    .filter(|target| {
                        // Leave distance unrooted so that we follow inverse-square law
                        let dist_sq = self.nodes[**target]
                            .data
                            .position
                            .iter()
                            .zip(transmitter.position)
                            .map(|(a, b)| *a - b)
                            .sum::<A>();

                        dist_sq < transmitter.broadcast_distance
                    })
                    .collect()
            }
            None => self
                .tree
                .within::<SquaredEuclidean>(&transmitter.position, transmitter.broadcast_distance)
                .iter()
                .map(|x| unsafe { &self.nodes.get_unchecked(x.item as usize).data.id })
                .collect(),
        };

        let packet = Arc::new(packet);
        for recipient in recipients {
            let mutex = unsafe { self.new_packets.get(recipient).unwrap_unchecked() };
            let mut packets = mutex.lock().unwrap();
            packets.push(packet.clone());
        }
    }
}

pub struct SimManager<
    'a,
    NodeBehaviourType: NodeBehaviour<A, K>,
    MoveBehaviourType: MoveBehaviour<A, K>,
    A: Coord<K>,
    const K: usize,
> {
    global_state_manager: GlobalStateManager<'a, NodeBehaviourType, MoveBehaviourType, A, K>,
    rngs: Vec<UnsafeCell<Xoshiro256Plus>>,
}

impl<
    NodeBehaviourType: NodeBehaviour<A, K>,
    MoveBehaviourType: MoveBehaviour<A, K>,
    A: Coord<K>,
    const K: usize,
> SimManager<'_, NodeBehaviourType, MoveBehaviourType, A, K>
{
    /// `id` must be a unique ID for the behaviour accessing the method. Ensures reproducibility.
    pub fn get_random_range(&self, id: usize, min: A, max: A) -> A {
        // Assumes RNGs initialised for all initialised IDs!
        let int = unsafe {
            let cell: *const UnsafeCell<Xoshiro256Plus> = &self.rngs[id];
            let rng = UnsafeCell::raw_get(cell);
            rng.as_mut().unwrap().next_u64()
        };

        // TODO - Verify distribution
        (A::from(int).unwrap() % (max - min)) + min
    }
}
