use crate::types::{GlobalStateManager, MoveBehaviour, NodeBehaviour};
use num_traits::Float;

#[derive(Clone)]
pub struct RandomWalk<const CHANGE_DELAY: u8, A: kiddo::float::kdtree::Axis, const K: usize> {
    tick_counter: u8,
    direction: [A; K],
}

impl<const CHANGE_DELAY: u8, A: kiddo::float::kdtree::Axis, const K: usize>
    RandomWalk<CHANGE_DELAY, A, K>
{
    const CHANGE_DELAY: u8 = 16;
}

impl<
    const CHANGE_DELAY: u8,
    A: kiddo::float::kdtree::Axis + num_traits::float::Float,
    const K: usize,
> MoveBehaviour<A, K> for RandomWalk<CHANGE_DELAY, A, K>
{
    fn id(&self) -> usize {
        todo!()
    }

    fn tick(
        self,
        global_state_manager: &GlobalStateManager<impl NodeBehaviour<A, K>, Self, A, K>,
        mut position: [A; K],
    ) -> (Self, [A; K]) {
        let mut direction = self.direction;
        if self.tick_counter % CHANGE_DELAY == 0 {
            for i in 0..K {
                let val = global_state_manager.sim_manager.get_random_range(
                    self.id(),
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
