use kiddo::immutable::float::kdtree::ImmutableKdTree;
use rand_xoshiro::Xoshiro256Plus;
use rand_xoshiro::rand_core::RngCore;
use std::cell::UnsafeCell;
use std::collections::HashMap;
use std::sync::Mutex;

/// Describes a node behaviour which performs some processing each tick to produce a new node behaviour
pub trait NodeBehaviour<A: kiddo::float::kdtree::Axis, const K: usize>
where
    Self: Sized + Send + Clone,
{
    fn tick(
        self,
        node_data: &NodeData<A, K>,
        global_state_manager: &GlobalStateManager<Self, impl MoveBehaviour<A, K>, A, K>,
        incoming_packets: &Vec<Box<dyn Packet<A, K>>>,
    ) -> Self;
}

pub trait MoveBehaviour<A: kiddo::float::kdtree::Axis, const K: usize>
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

struct UnicastPacket {
    target: NodeID,
    content: Box<[u8]>,
}

impl<A: kiddo::float::kdtree::Axis, const K: usize> Packet<A, K> for UnicastPacket {
    fn content(self) -> Box<[u8]> {
        self.content
    }

    fn eager_targets(&self) -> Option<Vec<NodeID>> {
        Some(vec![self.target])
    }

    fn targets(&self, target: &NodeData<A, K>) -> bool {
        target.id == self.target
    }
}

struct MulticastPacket {
    content: Box<[u8]>,
}

impl<A: kiddo::float::kdtree::Axis, const K: usize> Packet<A, K> for MulticastPacket {
    fn content(self) -> Box<[u8]> {
        self.content
    }

    fn eager_targets(&self) -> Option<Vec<NodeID>> {
        None
    }

    fn targets(&self, target: &NodeData<A, K>) -> bool {
        true
    }
}

pub trait Packet<A: kiddo::float::kdtree::Axis, const K: usize> {
    fn content(self) -> Box<[u8]>;

    /// Returns a vec of all NodeIDs which could receive the packet if they are in range.
    /// Means the transmitter can avoid having to lookup all nodes in range.
    /// Return None if the targeting is dynamic.
    fn eager_targets(&self) -> Option<Vec<NodeID>>;

    /// Whether a packet should be received by a given node
    fn targets(&self, target: &NodeData<A, K>) -> bool;
}

#[derive(Clone)]
struct Node<
    NodeBehaviourType: NodeBehaviour<A, K>,
    MoveBehaviourType: MoveBehaviour<A, K>,
    A: kiddo::float::kdtree::Axis,
    const K: usize,
> {
    behaviour: NodeBehaviourType,
    move_behaviour: MoveBehaviourType,
    data: NodeData<A, K>,
}

#[derive(Clone)]
pub struct NodeData<A: kiddo::float::kdtree::Axis, const K: usize> {
    pub position: [A; K],
    pub id: NodeID,
}

impl<
    NodeBehaviourType: NodeBehaviour<A, K>,
    MoveBehaviourType: MoveBehaviour<A, K>,
    A: kiddo::float::kdtree::Axis,
    const K: usize,
> Node<NodeBehaviourType, MoveBehaviourType, A, K>
{
    fn tick_behaviour(
        self,
        global_state_manager: &GlobalStateManager<NodeBehaviourType, MoveBehaviourType, A, K>,
        incoming_packets: &Vec<Box<dyn Packet<A, K>>>,
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
    A: kiddo::float::kdtree::Axis,
    const K: usize,
> {
    pub sim_manager: &'a SimManager<'a, NodeBehaviourType, MoveBehaviourType, A, K>,
    nodes: Vec<Node<NodeBehaviourType, MoveBehaviourType, A, K>>,
    /// 32 is the bucket size, might be worth profiling different values (see https://github.com/sdd/kiddo/blob/20560517c7e06d71a6887a7662b89b70091ef8db/examples/cities.rs#L96)
    tree: ImmutableKdTree<A, u32, K, 32>,
    /// Packets that have been sent to each node in the previous tick.
    incoming_packets: HashMap<NodeID, Vec<Box<dyn Packet<A, K>>>>,
    /// Packets that have been sent to each node during this tick.
    new_packets: HashMap<NodeID, Mutex<Vec<Box<dyn Packet<A, K>>>>>,
}

impl<
    NodeBehaviourType: NodeBehaviour<A, K>,
    MoveBehaviourType: MoveBehaviour<A, K>,
    A: kiddo::float::kdtree::Axis,
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
            new_packets: HashMap::new(), // TODO - instantiate for each ID
        }
    }

    pub fn transmit_packet(&self, transmitter: &NodeData<A, K>, packet: impl Packet<A, K>) {
        // TODO
    }
}

pub struct SimManager<
    'a,
    NodeBehaviourType: NodeBehaviour<A, K>,
    MoveBehaviourType: MoveBehaviour<A, K>,
    A: kiddo::float::kdtree::Axis,
    const K: usize,
> {
    global_state_manager: GlobalStateManager<'a, NodeBehaviourType, MoveBehaviourType, A, K>,
    rngs: Vec<UnsafeCell<Xoshiro256Plus>>,
}

impl<
    NodeBehaviourType: NodeBehaviour<A, K>,
    MoveBehaviourType: MoveBehaviour<A, K>,
    A: kiddo::float::kdtree::Axis,
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
