use crate::packets::Packet;
use crate::propagation_models::{PropagationModel, PropagationParams, SimpleDistanceParams};
use kiddo::SquaredEuclidean;
use kiddo::float_leaf_slice::leaf_slice::{LeafSliceFloat, LeafSliceFloatChunk};
use kiddo::immutable::float::kdtree::ImmutableKdTree;
use kiddo::traits::Content;
use log::trace;
use num_traits::Float;
use num_traits::float::FloatCore;
use rand::Rng;
use rand::distr::uniform::SampleUniform;
use rand_xoshiro::Xoshiro256Plus;
use rand_xoshiro::rand_core::{RngCore, SeedableRng};
use rayon::prelude::*;
use std::cell::SyncUnsafeCell;
use std::collections::HashMap;
use std::iter::Sum;
use std::sync::{Arc, Mutex};

/// Describes a node behaviour which performs some processing each tick to produce a new node behaviour
pub trait NodeBehaviour<A: Coord<K>, const K: usize>: Sized + Send + Sync + Clone {
    fn tick<P: PropagationParams<A, K>>(
        self,
        node_data: &NodeData<A, K, P>,
        global_state_manager: &GlobalStateManager<
            Self,
            impl MoveBehaviour<A, K>,
            impl PropagationModel<A, K, P = P>,
            A,
            K,
        >,
        incoming_packets: &Vec<Arc<dyn Packet<A, K>>>,
    ) -> Self;
}

pub trait MoveBehaviour<A: Coord<K>, const K: usize>: Sized + Send + Sync + Clone {
    fn tick<P: PropagationParams<A, K>>(
        self,
        data: &NodeData<A, K, P>,
        global_state_manager: &GlobalStateManager<
            impl NodeBehaviour<A, K>,
            Self,
            impl PropagationModel<A, K, P = P>,
            A,
            K,
        >,
    ) -> (Self, [A; K]);
}

pub type NodeID = usize;

#[derive(Clone)]
pub struct Node<
    NB: NodeBehaviour<A, K> + Sized,
    MB: MoveBehaviour<A, K> + Sized,
    P: PropagationParams<A, K> + Sized,
    A: Coord<K> + Sized,
    const K: usize,
> where
    Self: Sized,
{
    behaviour: NB,
    move_behaviour: MB,
    pub(crate) data: NodeData<A, K, P>,
}

/// One dimension in a coordinate-space, should be either f32 or f64
pub trait Coord<const K: usize>:
    Float
    + Sum
    + kiddo::float::kdtree::Axis
    + LeafSliceFloatChunk<u32, K>
    + LeafSliceFloat<u32>
    + SampleUniform
    + Sized
    + Clone
{
}

impl Coord<2> for f32 {}

impl Coord<2> for f64 {}

#[derive(Clone)]
pub struct NodeData<A: Coord<K>, const K: usize, P: PropagationParams<A, K> + Sized>
where
    Self: Sized,
{
    pub position: [A; K],
    pub id: NodeID,
    /// Holds whatever parameters your propagation model needs - e.g. transmit power, directionality
    pub propagation_params: P,
}

impl<
    NB: NodeBehaviour<A, K>,
    MB: MoveBehaviour<A, K>,
    A: Coord<K>,
    const K: usize,
    P: PropagationParams<A, K>,
> Node<NB, MB, P, A, K>
{
    pub fn node_behaviour(&self) -> &NB {
        &self.behaviour
    }

    pub fn move_behaviour(&self) -> &MB {
        &self.move_behaviour
    }

    fn tick_behaviour<PM>(
        self,
        global_state_manager: &GlobalStateManager<NB, MB, PM, A, K>,
        incoming_packets: &Vec<Arc<dyn Packet<A, K>>>,
    ) -> Self
    where
        PM: PropagationModel<A, K, P = P>,
    {
        Self {
            behaviour: self
                .behaviour
                .tick(&self.data, global_state_manager, incoming_packets),
            data: self.data,
            move_behaviour: self.move_behaviour,
        }
    }

    fn tick_movement<PM: PropagationModel<A, K, P = P>>(
        self,
        global_state_manager: &GlobalStateManager<NB, MB, PM, A, K>,
    ) -> Self {
        let mut new = self.clone();
        let (move_behaviour, position) = self.move_behaviour.tick(&self.data, global_state_manager);
        new.move_behaviour = move_behaviour;
        let mut data = self.data;
        data.position = position;
        new.data = data;
        new
    }

    pub fn data(&self) -> &NodeData<A, K, P> {
        &self.data
    }
}

/// Holds the state of the simulation at a specific tick
pub struct GlobalStateManager<
    NB: NodeBehaviour<A, K>,
    MB: MoveBehaviour<A, K>,
    PM: PropagationModel<A, K>,
    A: Coord<K>,
    const K: usize,
> {
    pub(crate) nodes: Arc<Vec<Node<NB, MB, PM::P, A, K>>>,
    // 32 is the bucket size, might be worth profiling different values (see https://github.com/sdd/kiddo/blob/20560517c7e06d71a6887a7662b89b70091ef8db/examples/cities.rs#L96)
    /// KD Tree storing all nodes by position, allows for efficient spatial lookup
    tree: Arc<ImmutableKdTree<A, u32, K, 32>>,
    /// Packets that have been sent to each node in the previous tick.
    incoming_packets: Arc<HashMap<NodeID, Vec<Arc<dyn Packet<A, K>>>>>,
    /// Packets that have been sent to each node during this tick.
    new_packets: Arc<HashMap<NodeID, Mutex<Vec<Arc<dyn Packet<A, K>>>>>>,
    rngs: Arc<Vec<SyncUnsafeCell<Xoshiro256Plus>>>,
    propagation_model: PM,
}

impl<
    NB: NodeBehaviour<A, K>,
    MB: MoveBehaviour<A, K>,
    PM: PropagationModel<A, K>,
    A: Coord<K>,
    const K: usize,
> GlobalStateManager<NB, MB, PM, A, K>
{
    fn new(
        nodes: Vec<Node<NB, MB, PM::P, A, K>>,
        seed: u64,
        propagation_model: PM,
    ) -> GlobalStateManager<NB, MB, PM, A, K> {
        Self {
            tree: Arc::new(ImmutableKdTree::new_from_slice(
                nodes
                    .iter()
                    .map(|x| x.data.position)
                    .collect::<Vec<_>>()
                    .as_slice(),
            )),
            incoming_packets: Arc::new(HashMap::from_iter(
                (0..nodes.len()).map(|x| (x, Vec::new())),
            )),
            new_packets: Arc::new(HashMap::from_iter(
                (0..nodes.len()).map(|x| (x, Mutex::new(Vec::new()))),
            )),
            rngs: Arc::new(
                (0..nodes.len())
                    .map(|x| SyncUnsafeCell::new(Xoshiro256Plus::seed_from_u64(seed + x as u64)))
                    .collect(),
            ),
            nodes: Arc::new(nodes),
            propagation_model,
        }
    }

    pub fn nodes(&self) -> &Vec<Node<NB, MB, PM::P, A, K>> {
        &self.nodes
    }
}

impl<
    NB: NodeBehaviour<A, K>,
    MB: MoveBehaviour<A, K>,
    PM: PropagationModel<A, K>,
    A: Coord<K>,
    const K: usize,
> GlobalStateManager<NB, MB, PM, A, K>
{
    fn tick(self) -> Self {
        let nodes: Vec<Node<NB, MB, PM::P, A, K>> = (*self.nodes)
            .clone()
            .into_par_iter()
            .map(|x: Node<NB, MB, PM::P, A, K>| x.tick_movement(&self))
            .collect();

        let nodes: Vec<Node<NB, MB, PM::P, A, K>> = nodes
            .into_par_iter()
            .map(|x| {
                let packets = self.incoming_packets.get(&x.data.id).unwrap();
                x.tick_behaviour(&self, packets)
            })
            .collect();

        let tree = ImmutableKdTree::new_from_slice(
            nodes
                .iter()
                .map(|x| x.data.position)
                .collect::<Vec<_>>()
                .as_slice(),
        );

        // The only time we have other Arcs is during ticking
        let new_packets = Arc::into_inner(self.new_packets).unwrap();

        Self {
            tree: Arc::new(tree),
            incoming_packets: Arc::new(
                new_packets
                    .into_iter()
                    .map(|(id, packets)| (id, packets.into_inner().unwrap()))
                    .collect(),
            ),
            new_packets: Arc::new(HashMap::from_iter(
                (0..nodes.len()).map(|x| (x, Mutex::new(Vec::new()))),
            )), // TODO - Don't instantiate a new one each tick!
            rngs: self.rngs,
            nodes: Arc::new(nodes),
            propagation_model: self.propagation_model,
        }
    }

    pub fn transmit_packet(
        &self,
        transmitter: &NodeData<A, K, PM::P>,
        packet: impl Packet<A, K> + 'static,
    ) {
        let eager_targets = packet.eager_targets();
        let recipients: Vec<&NodeID> = match &eager_targets {
            Some(targets) => targets
                .iter()
                .filter(|target| {
                    self.propagation_model
                        .signal_received(transmitter, &self.nodes[**target].data)
                })
                .collect(),
            None => self
                .tree
                .within_unsorted::<SquaredEuclidean>(
                    &transmitter.position,
                    transmitter
                        .propagation_params
                        .prune_distance()
                        .powf(A::from(2.0).unwrap()),
                )
                .iter()
                .filter_map(|x| unsafe {
                    let data = &self.nodes.get_unchecked(x.item as usize).data;
                    if data.id == transmitter.id {
                        return None;
                    };
                    if self.propagation_model.signal_received(transmitter, &data) {
                        Some(&data.id)
                    } else {
                        None
                    }
                })
                .collect(),
        };

        trace!(
            "{} potential recipients within {:?}",
            recipients.len(),
            transmitter.propagation_params.prune_distance()
        );
        let packet = Arc::new(packet);
        for recipient in recipients {
            let mutex = unsafe { self.new_packets.get(recipient).unwrap_unchecked() };
            let mut packets = mutex.lock().unwrap();
            packets.push(packet.clone());
        }
    }

    /// `id` must be a unique ID for the behaviour accessing the method. Ensures reproducibility.
    pub fn get_random_range(&self, id: usize, min: A, max: A) -> A {
        // Assumes RNGs initialised for all initialised IDs!
        let out = unsafe {
            let cell: *const SyncUnsafeCell<Xoshiro256Plus> = &self.rngs[id];
            let rng = SyncUnsafeCell::raw_get(cell);
            rng.as_mut().unwrap().random_range(min..max)
        };

        trace!("{} generated {:?}", id, out);
        out
    }
}

pub struct SimManager<
    NB: NodeBehaviour<A, K>,
    MB: MoveBehaviour<A, K>,
    PM: PropagationModel<A, K>,
    A: Coord<K>,
    const K: usize,
> {
    pub global_state_manager: GlobalStateManager<NB, MB, PM, A, K>,
}

impl<
    NB: NodeBehaviour<A, K>,
    MB: MoveBehaviour<A, K>,
    PM: PropagationModel<A, K>,
    A: Coord<K>,
    const K: usize,
> SimManager<NB, MB, PM, A, K>
{
    pub fn new(
        nodes: Vec<NodeInit<NB, MB, PM::P, A, K>>,
        seed: u64,
        propagation_model: PM,
    ) -> Self {
        let nodes = nodes
            .into_iter()
            .enumerate()
            .map(|(index, node)| Node {
                behaviour: node.node_behaviour,
                move_behaviour: node.move_behaviour,
                data: NodeData {
                    id: index,
                    position: node.starting_position,
                    propagation_params: node.propagation_params,
                },
            })
            .collect();

        Self {
            global_state_manager: GlobalStateManager::new(nodes, seed, propagation_model),
        }
    }

    /// Perform `n` ticks of the simulation, returning the new global state at the end
    pub fn n_ticks(mut self, num_ticks: usize) -> Self {
        for _ in 0..num_ticks {
            self.global_state_manager = self.global_state_manager.tick();
        }
        self
    }
}

/// Constructed by end-user and passed into construction of sim
#[derive(Clone)]
pub struct NodeInit<
    NB: NodeBehaviour<A, K>,
    MB: MoveBehaviour<A, K>,
    P: PropagationParams<A, K>,
    A: Coord<K>,
    const K: usize,
> {
    pub starting_position: [A; K],
    pub node_behaviour: NB,
    pub move_behaviour: MB,
    pub propagation_params: P,
}
