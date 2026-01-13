use kiddo::immutable::float::kdtree::ImmutableKdTree;

/// Describes a node behaviour which performs some processing each tick to produce a new node behaviour
pub trait NodeBehaviour<A: kiddo::float::kdtree::Axis, const K: usize>
where
    Self: Sized + Send,
{
    fn tick(
        self,
        global_state_manager: &GlobalStateManager<Self, impl MoveBehaviour<A, K>, A, K>,
    ) -> Self;
}

pub trait MoveBehaviour<A: kiddo::float::kdtree::Axis, const K: usize>
where
    Self: Sized + Send,
{
    fn tick(
        self,
        global_state_manager: &GlobalStateManager<impl NodeBehaviour<A, K>, Self, A, K>,
        position: [A; K],
    ) -> (Self, [A; K]);
}

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
        todo!()
    }
}
