use crate::behaviours::MoveBehaviour;
use crate::managers::GlobalStateManager;
use crate::node::NodeData;
use crate::propagation_models::PropagationModel;
use crate::{Coord, SimConfig};
/// Movement behaviour that doesn't move the node.
#[derive(Clone)]
pub struct StaticMovement {}

impl<A: Coord<K>, const K: usize> MoveBehaviour<A, K> for StaticMovement {
    fn tick<C: SimConfig<A, K, MB = Self>>(
        self,
        data: &NodeData<A, K, <C::PM as PropagationModel<A, K>>::P>,
        _global_state_manager: &GlobalStateManager<A, K, C>,
    ) -> (Self, [A; K]) {
        (self, data.position)
    }
}
