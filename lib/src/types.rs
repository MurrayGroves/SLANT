use crate::packets::Packet;
use crate::propagation_models::{PropagationModel, PropagationParams, SimpleDistanceParams};
use crate::stats::InternalEvent::{PacketLink, PacketTransmit};
use crate::stats::InternalStatKey::PacketTransmits;
use crate::stats::TimestepStats;
use kiddo::SquaredEuclidean;
use kiddo::float_leaf_slice::leaf_slice::{LeafSliceFloat, LeafSliceFloatChunk};
use kiddo::immutable::float::kdtree::ImmutableKdTree;
use kiddo::traits::Content;
use linearize::Linearize;
use log::{debug, info, trace};
use num_traits::Float;
use num_traits::float::FloatCore;
use rand::RngExt;
use rand::distr::uniform::{SampleRange, SampleUniform};
use rand::distr::{Distribution, StandardUniform};
use rand_xoshiro::Xoshiro256Plus;
use rand_xoshiro::rand_core::{RngCore, SeedableRng};
use rayon::prelude::*;
use std::cell::{Cell, RefCell, SyncUnsafeCell};
use std::collections::HashMap;
use std::fmt::Debug;
use std::iter::Sum;
use std::sync::{Arc, Mutex};
use thread_local::ThreadLocal;

pub trait SimConfig<A: Coord<K>, const K: usize>
where
    <Self::S as Linearize>::Storage<isize>: Send,
{
    type MB: MoveBehaviour<A, K>;
    type NB: NodeBehaviour<
            A,
            K,
            <<Self as SimConfig<A, K>>::PM as PropagationModel<A, K>>::P,
            E = <Self as SimConfig<A, K>>::E,
        >;
    type PM: PropagationModel<A, K>;
    /// Key type for stored stats
    type S: Linearize;
    /// User-defined event type
    type E: Send + Clone = ();
}

/// Describes a node behaviour which performs some processing each tick to produce a new node behaviour
pub trait NodeBehaviour<A: Coord<K>, const K: usize, PP: PropagationParams<A, K>>:
    Sized + Send + Sync + Clone
{
    /// Packet type that this node can receive and process.
    type P: Packet<A, K> + ?Sized;

    /// Type for events this node behaviour can produce
    type E = ();

    fn tick<
        C: SimConfig<
                A,
                K,
                PM = impl PropagationModel<A, K, P = PP>,
                NB = impl NodeBehaviour<A, K, PP, P = Self::P, E = Self::E>,
                E = Self::E,
            >,
    >(
        self,
        node_data: &NodeData<A, K, <C::PM as PropagationModel<A, K>>::P>,
        global_state_manager: &GlobalStateManager<A, K, C>,
        incoming_packets: &Vec<Self::P>,
    ) -> Self;
}

pub trait MoveBehaviour<A: Coord<K>, const K: usize>: Sized + Send + Sync + Clone {
    fn tick<C: SimConfig<A, K, MB = Self>>(
        self,
        data: &NodeData<A, K, <C::PM as PropagationModel<A, K>>::P>,
        global_state_manager: &GlobalStateManager<A, K, C>,
    ) -> (Self, [A; K]);
}

pub type NodeID = usize;

#[derive(Clone)]
pub struct Node<
    NB: NodeBehaviour<A, K, P> + Sized,
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
    + PartialEq
{
}

impl Coord<2> for f32 {}

impl Coord<2> for f64 {}

#[derive(Clone, Debug)]
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
    NB: NodeBehaviour<A, K, P>,
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

    fn tick_behaviour<PM, C: SimConfig<A, K, PM = PM, NB = NB, E = NB::E>>(
        self,
        global_state_manager: &GlobalStateManager<A, K, C>,
        incoming_packets: &Vec<NB::P>,
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

    fn tick_movement<
        PM: PropagationModel<A, K, P = P>,
        C: SimConfig<A, K, PM = PM, MB = MB, NB = NB>,
    >(
        self,
        global_state_manager: &GlobalStateManager<A, K, C>,
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
pub struct GlobalStateManager<A: Coord<K>, const K: usize, C: SimConfig<A, K>> {
    pub(crate) nodes: Arc<Vec<Node<C::NB, C::MB, <C::PM as PropagationModel<A, K>>::P, A, K>>>,
    // 32 is the bucket size, might be worth profiling different values (see https://github.com/sdd/kiddo/blob/20560517c7e06d71a6887a7662b89b70091ef8db/examples/cities.rs#L96)
    /// KD Tree storing all nodes by position, allows for efficient spatial lookup
    tree: Arc<ImmutableKdTree<A, u32, K, 32>>,
    /// Packets that have been sent to each node in the previous tick.
    incoming_packets: Arc<
        HashMap<
            NodeID,
            Vec<<C::NB as NodeBehaviour<A, K, <C::PM as PropagationModel<A, K>>::P>>::P>,
        >,
    >,
    /// Packets that have been sent to each node during this tick.
    new_packets: Arc<
        HashMap<
            NodeID,
            Mutex<Vec<<C::NB as NodeBehaviour<A, K, <C::PM as PropagationModel<A, K>>::P>>::P>>,
        >,
    >,
    rngs: Arc<Vec<SyncUnsafeCell<Xoshiro256Plus>>>,
    propagation_model: C::PM,
    /// Thread-local stats
    stats: Arc<ThreadLocal<RefCell<TimestepStats<C::S, C::E>>>>,
}

impl<A: Coord<K>, const K: usize, C: SimConfig<A, K>> GlobalStateManager<A, K, C> {
    fn new(
        nodes: Vec<Node<C::NB, C::MB, <C::PM as PropagationModel<A, K>>::P, A, K>>,
        seed: u64,
        propagation_model: C::PM,
    ) -> GlobalStateManager<A, K, C> {
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
            stats: Arc::new(ThreadLocal::new()),
        }
    }

    pub fn nodes(&self) -> &Vec<Node<C::NB, C::MB, <C::PM as PropagationModel<A, K>>::P, A, K>> {
        &self.nodes
    }

    /// Calling this will clear the internal stats buffer!
    pub fn consume_stats(&mut self) -> TimestepStats<C::S, C::E> {
        let mut stats = TimestepStats::new();
        for thread in Arc::into_inner(std::mem::take(&mut self.stats))
            .unwrap()
            .into_iter()
        {
            stats.consume(thread.into_inner())
        }
        debug!(
            "Consumed stats buffer has {} events",
            stats.internal_events.len()
        );
        stats
    }
}

impl<A: Coord<K>, const K: usize, C: SimConfig<A, K>> GlobalStateManager<A, K, C> {
    /// Returns new state
    fn tick(mut self) -> Self {
        debug!("Ticking with {:?} threads", rayon::current_num_threads());
        self.stats = Arc::new(ThreadLocal::new());
        let nodes: Vec<Node<C::NB, C::MB, <C::PM as PropagationModel<A, K>>::P, A, K>> = (*self
            .nodes)
            .clone()
            .into_par_iter()
            .map(
                |x: Node<C::NB, C::MB, <C::PM as PropagationModel<A, K>>::P, A, K>| {
                    x.tick_movement(&self)
                },
            )
            .collect();

        let nodes: Vec<Node<C::NB, C::MB, <C::PM as PropagationModel<A, K>>::P, A, K>> = nodes
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
            stats: self.stats,
        }
    }

    pub fn transmit_packet(
        &self,
        transmitter: &NodeData<A, K, <C::PM as PropagationModel<A, K>>::P>,
        packet: <C::NB as NodeBehaviour<A, K, <C::PM as PropagationModel<A, K>>::P>>::P,
    ) {
        #[cfg(not(all(
            feature = "disable_internal_stats",
            feature = "disable_internal_events"
        )))]
        let mut stats = self.stats.get_or_default().borrow_mut();
        #[cfg(not(feature = "disable_internal_events"))]
        stats.add_internal_event(PacketTransmit(transmitter.id));
        #[cfg(not(feature = "disable_internal_stats"))]
        stats.inc_internal(PacketTransmits, 1);

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
        for recipient in recipients {
            #[cfg(not(feature = "disable_internal_events"))]
            stats.add_internal_event(PacketLink((transmitter.id, *recipient)));
            let mutex = unsafe { self.new_packets.get(recipient).unwrap_unchecked() };
            let mut packets = mutex.lock().unwrap();
            packets.push(packet.clone());
        }
    }

    /// `id` must be a unique ID for the behaviour accessing the method. Ensures reproducibility.
    pub fn get_random_range(&self, id: usize, min: A, max: A) -> A {
        // Assumes RNGs initialised for all initialised IDs!
        unsafe {
            let cell: *const SyncUnsafeCell<Xoshiro256Plus> = &self.rngs[id];
            let rng = SyncUnsafeCell::raw_get(cell);
            rng.as_mut().unwrap().random_range(min..max)
        }
    }

    pub fn get_random<T>(&self, id: usize) -> T
    where
        StandardUniform: Distribution<T>,
    {
        // Assumes RNGs initialised for all initialised IDs!
        unsafe {
            let cell: *const SyncUnsafeCell<Xoshiro256Plus> = &self.rngs[id];
            let rng = SyncUnsafeCell::raw_get(cell);
            rng.as_mut().unwrap().random()
        }
    }

    pub fn add_event(&self, event: C::E) {
        let mut stats = self.stats.get_or_default().borrow_mut();
        stats.add_user_event(event);
    }

    pub fn inc(&self, key: C::S, x: isize) {
        let mut stats = self.stats.get_or_default().borrow_mut();
        stats.inc(key, x);
    }

    pub fn dec(&self, key: C::S, x: isize) {
        let mut stats = self.stats.get_or_default().borrow_mut();
        stats.dec(key, x);
    }
}

pub struct SimManager<A: Coord<K>, const K: usize, C: SimConfig<A, K>> {
    pub global_state_manager: GlobalStateManager<A, K, C>,
}

impl<A: Coord<K>, const K: usize, C: SimConfig<A, K>> SimManager<A, K, C> {
    pub fn new(
        nodes: Vec<NodeInit<C::NB, C::MB, <C::PM as PropagationModel<A, K>>::P, A, K>>,
        seed: u64,
        propagation_model: C::PM,
    ) -> Self {
        let nodes = nodes
            .into_iter()
            .enumerate()
            .map(|(index, node)| {
                let data = NodeData {
                    id: index,
                    position: node.starting_position,
                    propagation_params: node.propagation_params,
                };
                Node {
                    behaviour: node.node_behaviour,
                    move_behaviour: node.move_behaviour,
                    data,
                }
            })
            .collect();

        Self {
            global_state_manager: GlobalStateManager::new(nodes, seed, propagation_model),
        }
    }

    /// Perform `n` ticks of the simulation, returning the new global state at the end
    pub fn n_ticks(&mut self, num_ticks: usize) {
        for i in 0..num_ticks {
            info!("Doing tick {}", i);
            take_mut::take(&mut self.global_state_manager, |state| state.tick());
        }
    }
}

/// Constructed by end-user and passed into construction of sim
#[derive(Clone)]
pub struct NodeInit<
    NB: NodeBehaviour<A, K, P>,
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
