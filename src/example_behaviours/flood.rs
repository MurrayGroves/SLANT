use crate::packets::{GloballySequencedPacket, Packet};
use crate::propagation_models::PropagationModel;
use crate::types::{Coord, GlobalStateManager, NodeBehaviour, NodeData, SimConfig};
use num_traits::Num;
use std::marker::PhantomData;
use std::sync::Arc;

#[derive(Clone)]
pub struct Flood<PT: GloballySequencedPacket<A, K> + Clone, A: Coord<K>, const K: usize> {
    seen_packets: Vec<PT>,
    coord_type: PhantomData<A>,
}

impl<A: Coord<K>, const K: usize, PT: GloballySequencedPacket<A, K> + Clone> NodeBehaviour<A, K>
    for Flood<PT, A, K>
{
    type PT = PT;

    fn tick<C: SimConfig<A, K, NB = Self>>(
        self,
        node_data: &NodeData<A, K, <C::PM as PropagationModel<A, K>>::P>,
        global_state_manager: &GlobalStateManager<A, K, C>,
        incoming_packets: &Vec<Arc<dyn Packet<A, K>>>,
    ) -> Self {
        todo!()
    }
}
