//! Structs that manage the simulation and its state.
use crate::behaviours::NodeBehaviour;
use crate::node::{Node, NodeData, NodeID, NodeInit};
use crate::packets::Packet;
use crate::propagation_models::{PropagationModel, PropagationParams};
use crate::stats::InternalEvent::{PacketLink, PacketTransmit};
use crate::stats::InternalStatKey::PacketTransmits;
use crate::stats::{InternalStatKey, TimestepStats};
use crate::{Coord, SimConfig};
use kiddo::SquaredEuclidean;
use kiddo::immutable::float::kdtree::ImmutableKdTree;
use log::{debug, info, trace};
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

    /// The delta that [current_time] is increased by each tick.
    timestep_delta: f64,

    /// The current simulation time, may be fetched by behaviours using the getter.
    current_time: f64,

    /// The current simulation tick.
    current_tick: usize,

    /// The next simulation tick
    next_tick: usize,

    /// The next simulation time.
    next_time: f64,
}

impl<A: Coord<K>, const K: usize, C: SimConfig<A, K>> GlobalStateManager<A, K, C> {
    /// Used to create state at tick zero.
    fn new(
        nodes: Vec<Node<C::NB, C::MB, <C::PM as PropagationModel<A, K>>::P, A, K>>,
        seed: u64,
        propagation_model: C::PM,
        timestep_delta: f64,
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
            timestep_delta,
            current_time: 0.0,
            current_tick: 0,
            next_tick: 0,
            next_time: 0.0,
        }
    }

    /// Borrow the internal node storage, useful for retrieving state from your behaviours.
    pub fn nodes(&self) -> &Vec<Node<C::NB, C::MB, <C::PM as PropagationModel<A, K>>::P, A, K>> {
        &self.nodes
    }

    /// Get the configured timestep delta.
    pub fn timestep_delta(&self) -> f64 {
        self.timestep_delta
    }

    /// Get the current simulation time in this tick.
    /// Use this for scheduling events that should happen at the same simulation time regardless of the timestep delta.
    pub fn current_time(&self) -> f64 {
        self.current_time
    }

    /// Check if the current time value is the closest to a specific time.
    /// Use this to run events at specific times.
    pub fn is_time(&self, time: f64) -> bool {
        (self.current_time - time).abs() < (self.timestep_delta / 2.0)
    }

    /// Get the current simulation tick number.
    /// Use this if you want to run an event every X ticks, beware that this means changing the timestep delta will change your behaviour!
    pub fn current_tick(&self) -> usize {
        self.current_tick
    }

    /// Calling this will clear the internal stats buffer, so you can only call it once per tick!
    pub(crate) fn consume_stats(&mut self) -> TimestepStats<C::S, C::E> {
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

    /// Returns stats generated during tick.
    pub(crate) fn tick(&mut self) -> TimestepStats<C::S, C::E> {
        debug!("Ticking with {:?} threads", rayon::current_num_threads());
        self.stats = Arc::new(ThreadLocal::new());
        self.current_tick = self.next_tick;
        self.current_time = self.next_time;
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
                #[cfg(not(feature = "disable_internal_stats"))]
                self.stats
                    .get_or_default()
                    .borrow_mut()
                    .inc_internal(InternalStatKey::PacketReceives, packets.len() as isize);
                x.tick_node_behaviour(&self, packets)
            })
            .collect();

        // The only time we have other Arcs is during ticking
        let new_packets = Arc::into_inner(std::mem::take(&mut self.new_packets)).unwrap();

        self.tree = Arc::new(tree);
        self.incoming_packets = Arc::new(
            new_packets
                .into_iter()
                .map(|(id, packets)| (id, packets.into_inner().unwrap()))
                .collect(),
        );
        self.new_packets = Arc::new(HashMap::from_iter(
            (0..nodes.len()).map(|x| (x, Mutex::new(Vec::new()))),
        ));
        self.nodes = Arc::new(nodes);

        self.next_time += self.timestep_delta;
        self.next_tick += 1;

        self.consume_stats()
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
    /// Stores the state of the current tick.
    pub global_state_manager: GlobalStateManager<A, K, C>,
}

impl<A: Coord<K>, const K: usize, C: SimConfig<A, K>> SimManager<A, K, C> {
    /// # Arguments
    ///
    /// * `nodes`: Nodes that will be simulated.
    /// * `seed`: Seed used for randomness, ensure it's the same between runs you want to be reproducible!
    /// * `propagation_model`: Propagation model that the sim uses to check whether a packet transmission should be received.
    /// * `timestep_delta`: How far the simulation time advances each tick. Smaller numbers can be more accurate in exchange for longer runtime.
    ///
    /// # Examples
    ///
    /// ```ignore
    ///
    /// struct TestConfig;
    /// impl SimConfig<f32, 2> for TestConfig {
    ///     // In this simulation nodes won't move
    ///     type MB = StaticMovement;
    ///     // Nodes use the builtin Monotonic behaviour which broadcasts new packets every N ticks.
    ///     // We use Monotonic to wrap our existing behaviour.
    ///     type NB = Monotonic<f32, 2, Flood<f32, 2>, FloodPacket, FreeSpaceParams<f32, 2>>;
    ///     // We're going to use the Friis transmission equation here as our propagation model.
    ///     type PM = FreeSpace;
    ///     type S = OurStatKey;
    ///     type E = OurEventType;
    /// }
    ///
    /// let mut sim: SimManager<_, _, TestConfig> = SimManager::new(nodes, 123456, FreeSpace);
    /// ```
    pub fn new(
        nodes: Vec<NodeInit<C::NB, C::MB, <C::PM as PropagationModel<A, K>>::P, A, K>>,
        seed: u64,
        propagation_model: C::PM,
        timestep_delta: f64,
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
            global_state_manager: GlobalStateManager::new(
                nodes,
                seed,
                propagation_model,
                timestep_delta,
            ),
        }
    }

    /// Perform `n` ticks of the simulation, returning the stats generated during those ticks.
    pub fn n_ticks(&mut self, num_ticks: usize) -> Vec<TimestepStats<C::S, C::E>> {
        let mut stats = Vec::with_capacity(num_ticks);
        for i in 0..num_ticks {
            info!("Doing tick {}", i);
            stats.push(self.global_state_manager.tick());
        }
        stats
    }

    /// Perform one tick of the simulation, returning the stats generated during the tick.
    pub fn tick(&mut self) -> TimestepStats<C::S, C::E> {
        self.global_state_manager.tick()
    }

    /// Tick the simulation for a certain amount of simulation time.
    /// The actual number of ticks performed depends on the set timestep delta.
    pub fn tick_time(&mut self, time: f64) -> Vec<TimestepStats<C::S, C::E>> {
        let end_time = self.global_state_manager.current_time + time;
        let mut stats =
            Vec::with_capacity((time / self.global_state_manager.timestep_delta).ceil() as usize);
        while !self.global_state_manager.is_time(end_time) {
            stats.push(self.global_state_manager.tick());
        }
        stats
    }
}

#[cfg(test)]
mod tests {
    use crate::builtin::move_behaviours::static_movement::StaticMovement;
    use crate::builtin::node_behaviours::empty_behaviour::EmptyBehaviour;
    use crate::builtin::propagation_models::simple_distance::SimpleDistance;
    use crate::managers::SimManager;
    use crate::node::{NodeData, NodeID};
    use crate::packets::Packet;
    use crate::propagation_models::PropagationParams;
    use crate::{Coord, SimConfig};
    use std::sync::Arc;

    #[derive(Clone, Debug)]
    struct EmptyPacket {
        content: Arc<Box<[u8]>>,
    }
    impl Packet for EmptyPacket {
        fn content(self) -> Arc<Box<[u8]>> {
            self.content
        }

        fn content_ref(&self) -> &Arc<Box<[u8]>> {
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

    struct EmptyConfig;
    impl SimConfig<f32, 2> for EmptyConfig {
        type MB = StaticMovement;
        type NB = EmptyBehaviour<EmptyPacket>;
        type PM = SimpleDistance;
    }

    #[test]
    fn tick_time() {
        let delta = 0.5;

        let mut sim: SimManager<f32, 2, EmptyConfig> =
            SimManager::new(vec![], 100, SimpleDistance, delta);

        sim.tick_time(100.0);

        assert_eq!(sim.global_state_manager.current_time, 100.0);
        assert_eq!(
            sim.global_state_manager.current_tick,
            (100.0 / delta) as usize
        );
    }
}
