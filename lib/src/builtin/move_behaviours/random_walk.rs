//! Moves nodes at a constant velocity, changing direction randomly at set intervals.

use crate::behaviours::MoveBehaviour;
use crate::managers::GlobalStateManager;
use crate::node::NodeData;
use crate::propagation_models::PropagationModel;
use crate::{Coord, SimConfig};
use log::info;
use num_traits::Float;

/// Movement behaviour which moves nodes at a constant velocity, changing direction randomly at a configurable tick interval.
/// `CHANGE_DELAY` is in ticks. E.g. a change delay of 8 will cause the node to change direction every 8 ticks.
#[derive(Clone)]
pub struct RandomWalk<A: Coord<K>, const K: usize> {
    /// The delay between changes
    change_delay: f64,

    /// The time at which the direction will next change
    next_change: f64,
    /// Direction node heads in
    direction: [A; K],
    /// Magnitude that new direction vectors will have
    velocity: A,
}

impl<A: Coord<K>, const K: usize> RandomWalk<A, K> {
    /// # Arguments
    ///
    /// * `velocity`: The node will always move at this velocity
    pub fn new(velocity: A, change_delay: f64) -> Self {
        Self {
            next_change: 0.0,
            change_delay,
            direction: [A::zero(); K],
            velocity,
        }
    }
}

impl<A: Coord<K>, const K: usize> MoveBehaviour<A, K> for RandomWalk<A, K> {
    fn tick<C: SimConfig<A, K>>(
        mut self,
        data: &NodeData<A, K, <C::PM as PropagationModel<A, K>>::P>,
        global_state_manager: &GlobalStateManager<A, K, C>,
    ) -> (Self, [A; K]) {
        let mut position = data.position;
        let mut direction = self.direction;

        // Change direction if interval over
        if global_state_manager.is_time(self.next_change) {
            self.next_change += self.change_delay;
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
                direction[i] = direction[i]
                    / (magnitude
                        / (self.velocity * A::from(global_state_manager.timestep_delta()).unwrap())) // Normalise direction
            }
        }

        // Update position from direction
        for i in 0..K {
            position[i] += direction[i]
        }
        self.direction = direction;

        (self, position)
    }
}
