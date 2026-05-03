//! SLANT (Simulator for Large Ad hoc Network Topologies) is a high performance network simulation library targeted at MANETs.
//! You provide the behaviours and the library handles all the complicated orchestration and state management.
//!
//! # Behaviours and packets
//! Each node has two behaviours - a [movement behaviour](MoveBehaviour) and a [node behaviour](NodeBehaviour) which are called each tick by the simulation.
//!
//! Movement behaviours can access the current position of the node (along with whatever state you choose to store in the behaviour itself), and each tick return a new position the node should be at.
//! Your simulation will be defined over a coordinate system of your choosing, so you can choose your desired floating point type and dimensionality.
//! Some basic movement behaviours are provided in [builtin::move_behaviours] to help you get started.
//!
//! Node behaviours receive a list of all packets the node received in a previous frame and may do whatever processing on them is desired (including modifying the node's state), and can transmit new packets out into the network.
//!
//! You'll likely define your node behaviours to work with a specific packet type so that your packets can store whatever information is needed for your behaviours.
//! Packets can carry whatever data you like, they just need to define a couple methods from the [packets::Packet] trait to allow the simulation to optimise their delivery.
//!
//! # Propagation Models
//! [Propagation models](PropagationModel) are responsible for deciding which nodes receive transmitted packets.
//! A propagation model is defined over your coordinate system and given a transmitter and a receiver's position and parameters must return a boolean indicating whether a packet is received or not.
//! If your requirements for accuracy are basic, there are some provided propagation models in [builtin::propagation_models].
//!
//! # Examples
//! An example simulation is available in the `examples` directory, in which we define a flood routing node behaviour and simulate it with a configurable number of nodes.
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
