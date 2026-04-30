use crate::managers::GlobalStateManager;
use crate::node::NodeData;
use crate::packets::Packet;
use crate::propagation_models::{PropagationModel, PropagationParams};
use kiddo::float_leaf_slice::leaf_slice::{LeafSliceFloat, LeafSliceFloatChunk};
use linearize::Linearize;
use num_traits::Float;
use rand::distr::uniform::SampleUniform;
use std::iter::Sum;

/// Defines a configuration for a particular simulation.
pub trait SimConfig<A: Coord<K>, const K: usize>
where
    <Self::S as Linearize>::Storage<isize>: Send,
{
    /// The movement behaviour used by nodes in this simulation.
    /// If you wish to have multiple different behaviours you may define an enum over them that implements [MoveBehaviour].
    type MB: MoveBehaviour<A, K>;

    /// The node behaviour used by nodes in this simulation. This defines the logic each node follows for modifying state and sending/processing packets.
    /// If you wish to have multiple different behaviours you may define an enum over them that implements [NodeBehaviour].
    type NB: NodeBehaviour<
            A,
            K,
            <<Self as SimConfig<A, K>>::PM as PropagationModel<A, K>>::P,
            E = <Self as SimConfig<A, K>>::E,
        >;

    /// The propagation model that decides whether a given transmission between a transmitter and a receiver is received.
    type PM: PropagationModel<A, K>;

    /// The type used as a key for user-defined metrics. Typically this should be an enum.
    /// You can use [GlobalStateManager::inc] or [GlobalStateManager::dec] to modify metrics from your behaviours.
    type S: Linearize = ();

    /// Type for user-defined events, typically would be an enum.
    /// You can use [GlobalStateManager::add_event] in your behaviours to record an event.
    type E: Send + Clone = ();
}

/// Describes a node behaviour which performs some processing each tick to produce a new version of itself.
pub trait NodeBehaviour<A: Coord<K>, const K: usize, PP: PropagationParams<A, K>>:
    Sized + Send + Sync + Clone
{
    /// Packet type that this node can receive and process.
    type P: Packet + ?Sized;

    /// Type for events this node behaviour can produce
    type E = ();

    /// Note that this returns a *new* instance of `Self`, that is you should not modify state, but instead return a new state.
    /// It does however consume an owned version of itself, so you may (and should) move instead of copying/cloning where possible.
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

/// Describes a movement behaviour which is ticked each tick and returns a new position for a node.
pub trait MoveBehaviour<A: Coord<K>, const K: usize>: Sized + Send + Sync + Clone {
    /// Note that this returns a *new* instance of `Self`, that is you should not modify state, but instead return a new state.
    /// It does however consume an owned version of itself, so you may (and should) move instead of copying/cloning where possible.
    fn tick<C: SimConfig<A, K, MB = Self>>(
        self,
        data: &NodeData<A, K, <C::PM as PropagationModel<A, K>>::P>,
        global_state_manager: &GlobalStateManager<A, K, C>,
    ) -> (Self, [A; K]);
}

/// Value in one dimension in a coordinate-space, should be either f32 or f64
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

impl<const K: usize> Coord<K> for f32 {}

impl<const K: usize> Coord<K> for f64 {}
