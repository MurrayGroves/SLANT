use crate::managers::GlobalStateManager;
use crate::propagation_models::{PropagationModel, PropagationParams};
use crate::traits::{Coord, MoveBehaviour, NodeBehaviour, SimConfig};

pub type NodeID = usize;

/// Stores behaviour-agnostic state for a node.
#[derive(Clone, Debug)]
pub struct NodeData<A: Coord<K>, const K: usize, P: PropagationParams<A, K> + Sized>
where
    Self: Sized,
{
    /// Current position of the node in your coordinate-space.
    pub position: [A; K],
    /// Unique ID of the node, assigned incrementally at the start of the simulation.
    pub id: NodeID,
    /// Holds whatever parameters your propagation model needs - e.g. transmit power, directionality
    pub propagation_params: P,
}

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
    pub(crate) behaviour: NB,
    pub(crate) move_behaviour: MB,
    pub(crate) data: NodeData<A, K, P>,
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

    pub fn data(&self) -> &NodeData<A, K, P> {
        &self.data
    }

    /// Ticks node behaviour and updates the behaviour to its new state.
    pub(crate) fn tick_node_behaviour<PM, C: SimConfig<A, K, PM = PM, NB = NB, E = NB::E>>(
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

    /// Ticks movement behaviour and updates the behaviour to its new state.
    pub(crate) fn tick_movement_behaviour<
        PM: PropagationModel<A, K, P = P>,
        C: SimConfig<A, K, PM = PM, MB = MB, NB = NB>,
    >(
        mut self,
        global_state_manager: &GlobalStateManager<A, K, C>,
    ) -> Self {
        let (move_behaviour, position) = self.move_behaviour.tick(&self.data, global_state_manager);
        self.move_behaviour = move_behaviour;
        self.data.position = position;
        self
    }
}

/// Constructed by end-user and passed into construction of sim to construct a [Node] instance.
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
