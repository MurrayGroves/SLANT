use crate::propagation_models::PropagationModel;
use crate::types::{Coord, GlobalStateManager, MoveBehaviour, NodeData, SimConfig};
use num_traits::Float;

/// Movement behaviour which moves nodes at a constant behaviour, changing direction randomly at a configurable tick interval
#[derive(Clone)]
pub struct RandomWalk<const CHANGE_DELAY: u8, A: Coord<K>, const K: usize> {
    /// Incremented each tick
    tick_counter: u8,
    /// Direction node heads in
    direction: [A; K],
    /// Magnitude that new direction vectors will have
    velocity: A,
}

impl<const CHANGE_DELAY: u8, A: Coord<K>, const K: usize> RandomWalk<CHANGE_DELAY, A, K> {
    /// # Arguments
    ///
    /// * `velocity`: The node will always move at this velocity
    ///
    /// returns: RandomWalk<{ CHANGE_DELAY }, A, { K }>
    pub fn new(velocity: A) -> Self {
        Self {
            tick_counter: 0,
            direction: [A::zero(); K],
            velocity,
        }
    }
}

impl<const CHANGE_DELAY: u8, A: Coord<K>, const K: usize> MoveBehaviour<A, K>
    for RandomWalk<CHANGE_DELAY, A, K>
{
    fn tick<C: SimConfig<A, K>>(
        self,
        data: &NodeData<A, K, <C::PM as PropagationModel<A, K>>::P>,
        global_state_manager: &GlobalStateManager<A, K, C>,
    ) -> (Self, [A; K]) {
        let mut position = data.position;
        let mut direction = self.direction;

        // Change direction if interval over
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
                direction[i] = direction[i] / (magnitude / self.velocity) // Normalise direction
            }
        }

        // Update position from direction
        for i in 0..K {
            position[i] += direction[i]
        }

        (
            Self {
                tick_counter: self.tick_counter + 1,
                direction,
                velocity: self.velocity,
            },
            position,
        )
    }
}
