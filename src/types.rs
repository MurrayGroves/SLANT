use kiddo::immutable::float::kdtree::ImmutableKdTree;

/// Describes a node behaviour which performs some processing each tick to produce a new node behaviour
pub trait NodeBehaviour<A: kiddo::float::kdtree::Axis, const K: usize>
where
    Self: Sized + Send + Clone,
{
    fn tick(
        self,
        global_state_manager: &GlobalStateManager<Self, impl MoveBehaviour<A, K>, A, K>,
    ) -> Self;
}

pub trait MoveBehaviour<A: kiddo::float::kdtree::Axis, const K: usize>
where
    Self: Sized + Send + Clone,
{
    fn tick(
        self,
        global_state_manager: &GlobalStateManager<impl NodeBehaviour<A, K>, Self, A, K>,
        position: [A; K],
    ) -> (Self, [A; K]);
}

#[derive(Clone)]
struct Node<
    NodeBehaviourType: NodeBehaviour<A, K>,
    MoveBehaviourType: MoveBehaviour<A, K>,
    A: kiddo::float::kdtree::Axis,
    const K: usize,
> {
    behaviour: NodeBehaviourType,
    move_behaviour: MoveBehaviourType,
    position: [A; K],
}

impl<
    NodeBehaviourType: NodeBehaviour<A, K>,
    MoveBehaviourType: MoveBehaviour<A, K>,
    A: kiddo::float::kdtree::Axis,
    const K: usize,
> Node<NodeBehaviourType, MoveBehaviourType, A, K>
{
    fn tick_behaviour(
        self,
        global_state_manager: &GlobalStateManager<NodeBehaviourType, MoveBehaviourType, A, K>,
    ) -> Self {
        Self {
            behaviour: self.behaviour.tick(global_state_manager),
            move_behaviour: self.move_behaviour,
            position: self.position,
        }
    }

    fn tick_movement(
        self,
        global_state_manager: &GlobalStateManager<NodeBehaviourType, MoveBehaviourType, A, K>,
    ) -> Self {
        let mut new = self.clone();
        let (move_behaviour, position) = self
            .move_behaviour
            .tick(global_state_manager, self.position);
        new.move_behaviour = move_behaviour;
        new.position = position;
        new
    }
}

#[derive(Clone)]
pub struct GlobalStateManager<
    NodeBehaviourType: NodeBehaviour<A, K>,
    MoveBehaviourType: MoveBehaviour<A, K>,
    A: kiddo::float::kdtree::Axis,
    const K: usize,
> {
    nodes: Vec<Node<NodeBehaviourType, MoveBehaviourType, A, K>>,
    /// 32 is the bucket size, might be worth profiling different values (see https://github.com/sdd/kiddo/blob/20560517c7e06d71a6887a7662b89b70091ef8db/examples/cities.rs#L96)
    tree: ImmutableKdTree<A, u32, K, 32>,
}

impl<
    NodeBehaviourType: NodeBehaviour<A, K>,
    MoveBehaviourType: MoveBehaviour<A, K>,
    A: kiddo::float::kdtree::Axis,
    const K: usize,
> GlobalStateManager<NodeBehaviourType, MoveBehaviourType, A, K>
{
    fn tick(self) -> Self {
        let mut new = self.clone();
        let nodes = new
            .nodes
            .into_iter()
            .map(|x| x.tick_movement(&self))
            .collect();

        new.nodes = nodes;

        let mut new_2 = new.clone();
        let nodes = new_2
            .nodes
            .into_iter()
            .map(|x| x.tick_behaviour(&new))
            .collect();

        new_2.nodes = nodes;
        new_2
    }
}
