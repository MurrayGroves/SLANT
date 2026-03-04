use crate::propagation_models::{PropagationModel, PropagationParams};
use crate::types::{
    Coord, GlobalStateManager, MoveBehaviour, NodeBehaviour, NodeData, NodeID, SimManager,
};
use num_traits::Float;

#[derive(Clone)]
pub struct RandomWalk<const CHANGE_DELAY: u8, A: Coord<K>, const K: usize> {
    tick_counter: u8,
    direction: [A; K],
}

impl<const CHANGE_DELAY: u8, A: Coord<K>, const K: usize> RandomWalk<CHANGE_DELAY, A, K> {
    pub fn new(starting_direction: [A; K]) -> Self {
        Self {
            tick_counter: 0,
            direction: starting_direction,
        }
    }
}

impl<const CHANGE_DELAY: u8, A: Coord<K>, const K: usize> MoveBehaviour<A, K>
    for RandomWalk<CHANGE_DELAY, A, K>
{
    fn tick<P: PropagationParams<A, K>>(
        self,
        data: &NodeData<A, K, P>,
        global_state_manager: &GlobalStateManager<
            impl NodeBehaviour<A, K>,
            Self,
            impl PropagationModel<A, K, P = P>,
            A,
            K,
        >,
    ) -> (Self, [A; K]) {
        let mut position = data.position;
        let mut direction = self.direction;
        if self.tick_counter % CHANGE_DELAY == 0 {
            for i in 0..K {
                let val = global_state_manager.get_random_range(
                    data.id,
                    A::from(-1.0).unwrap(),
                    A::from(1.0).unwrap(),
                );

                direction[i] = val;
            }

            let magnitude = direction
                .iter()
                .map(|x| Float::powi(*x, 2))
                .fold(A::default(), |x, y| x + y)
                .powf(A::from(0.5).unwrap());

            for i in 0..K {
                direction[i] = direction[i] / magnitude
            }
        }

        for i in 0..K {
            position[i] += direction[i]
        }

        (
            Self {
                tick_counter: self.tick_counter + 1,
                direction,
            },
            position,
        )
    }
}
