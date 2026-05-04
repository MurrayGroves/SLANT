//! A node behaviour which does nothing.

use crate::behaviours::NodeBehaviour;
use crate::managers::GlobalStateManager;
use crate::node::NodeData;
use crate::packets::Packet;
use crate::propagation_models::{PropagationModel, PropagationParams};
use crate::{Coord, SimConfig};
use std::marker::PhantomData;

#[derive(Clone)]
pub struct EmptyBehaviour<P: Packet> {
    phantom_p: PhantomData<P>,
}

impl<A: Coord<K>, const K: usize, P: Packet, PP: PropagationParams<A, K>> NodeBehaviour<A, K, PP>
    for EmptyBehaviour<P>
{
    type P = P;

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
        _node_data: &NodeData<A, K, <C::PM as PropagationModel<A, K>>::P>,
        _global_state_manager: &GlobalStateManager<A, K, C>,
        _incoming_packets: &Vec<Self::P>,
    ) -> Self {
        self
    }
}

impl<P: Packet> EmptyBehaviour<P> {
    pub fn new() -> Self {
        Self {
            phantom_p: PhantomData,
        }
    }
}
