//! Traits implemented by simulation behaviours.
use crate::managers::GlobalStateManager;
use crate::node::NodeData;
use crate::packets::Packet;
use crate::propagation_models::{PropagationModel, PropagationParams};
use crate::{Coord, SimConfig};
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
/// When calculating the new position, ensure you multiply any velocities by the timestep delta!
pub trait MoveBehaviour<A: Coord<K>, const K: usize>: Sized + Send + Sync + Clone {
    /// Note that this returns a *new* instance of `Self`, that is you should not modify state, but instead return a new state.
    /// It does however consume an owned version of itself, so you may (and should) move instead of copying/cloning where possible.
    fn tick<C: SimConfig<A, K, MB = Self>>(
        self,
        data: &NodeData<A, K, <C::PM as PropagationModel<A, K>>::P>,
        global_state_manager: &GlobalStateManager<A, K, C>,
    ) -> (Self, [A; K]);
}
