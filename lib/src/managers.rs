use crate::node::{Node, NodeData, NodeID, NodeInit};
use crate::packets::Packet;
use crate::propagation_models::{PropagationModel, PropagationParams};
use crate::stats::InternalEvent::{PacketLink, PacketTransmit};
use crate::stats::InternalStatKey::PacketTransmits;
use crate::stats::TimestepStats;
use crate::traits::{Coord, NodeBehaviour, SimConfig};
use kiddo::SquaredEuclidean;
use kiddo::immutable::float::kdtree::ImmutableKdTree;
use log::{debug, info, trace};
use num_traits::{Float, NumCast};
use rand::distr::{Distribution, StandardUniform};
use rand::{RngExt, SeedableRng};
use rand_xoshiro::Xoshiro256Plus;
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use std::cell::{RefCell, SyncUnsafeCell};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use thread_local::ThreadLocal;

/// Holds the state of the simulation at a specific tick
pub struct GlobalStateManager<A: Coord<K>, const K: usize, C: SimConfig<A, K>> {
    /// Read only vector of nodes in ID order.
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

    /// Each node gets its own RNG for reproducibility.
    rngs: Arc<Vec<SyncUnsafeCell<Xoshiro256Plus>>>,

    /// Instance of the propagation model for
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

    /// Calling this will clear the internal stats buffer, so you can only call it once per tick!
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
                    x.tick_movement_behaviour(&self)
                },
            )
            .collect();

        let tree = ImmutableKdTree::new_from_slice(
            nodes
                .iter()
                .map(|x| x.data.position)
                .collect::<Vec<_>>()
                .as_slice(),
        );

        let nodes: Vec<Node<C::NB, C::MB, <C::PM as PropagationModel<A, K>>::P, A, K>> = nodes
            .into_par_iter()
            .map(|x| {
                let packets = self.incoming_packets.get(&x.data.id).unwrap();
                x.tick_node_behaviour(&self, packets)
            })
            .collect();

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

    /// Transmit a packet, the propagation model will be used to decide which nodes receive it.
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
                    // Perform unchecked get since we instantiate the node list at start and never change the number of elements
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

    /// Get a random value of type `T`, caller must provide its node ID so the correct RNG can be used in order to preserve reproducibility.
    pub fn get_random<T>(&self, node_id: NodeID) -> T
    where
        StandardUniform: Distribution<T>,
    {
        // Assumes RNGs initialised for all initialised IDs!
        unsafe {
            let cell: *const SyncUnsafeCell<Xoshiro256Plus> = &self.rngs[node_id];
            let rng = SyncUnsafeCell::raw_get(cell);
            rng.as_mut().unwrap().random()
        }
    }

    /// Add an event to this tick's stats buffer
    pub fn add_event(&self, event: C::E) {
        let mut stats = self.stats.get_or_default().borrow_mut();
        stats.add_user_event(event);
    }

    /// Increment a counter in this tick's stats buffer
    pub fn inc(&self, key: C::S, x: isize) {
        let mut stats = self.stats.get_or_default().borrow_mut();
        stats.inc(key, x);
    }

    /// Decrement a counter in this tick's stats buffer
    pub fn dec(&self, key: C::S, x: isize) {
        let mut stats = self.stats.get_or_default().borrow_mut();
        stats.dec(key, x);
    }
}

/// Manages the whole simulation throughout its lifetime.
pub struct SimManager<A: Coord<K>, const K: usize, C: SimConfig<A, K>> {
    /// Stores the state of the current tick
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
