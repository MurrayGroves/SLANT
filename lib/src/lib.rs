//! TEST
#![feature(sync_unsafe_cell)]
#![feature(adt_const_params)]
#![feature(associated_type_defaults)]
extern crate core;

use crate::behaviours::{MoveBehaviour, NodeBehaviour};
use crate::propagation_models::PropagationModel;
use kiddo::float_leaf_slice::leaf_slice::{LeafSliceFloat, LeafSliceFloatChunk};
use linearize::Linearize;
use num_traits::Float;
use rand::distr::uniform::SampleUniform;
use std::iter::Sum;

pub mod behaviours;
pub mod builtin;
pub mod managers;
pub mod node;
pub mod packets;
pub mod propagation_models;
pub mod stats;

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

/// Value in one dimension in a coordinate-space, should be either [f32] or [f64].
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
