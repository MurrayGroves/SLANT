use log::trace;
use slant::Coord;
use slant::behaviours::{MoveBehaviour, NodeBehaviour};
use slant::builtin::node_behaviours::flood::Flood;
use slant::builtin::node_behaviours::monotonic::Monotonic;
use slant::node::NodeInit;
use slant::propagation_models::PropagationParams;
use std::sync::Arc;

pub fn generate_cloned_nodes<
    A: Coord<K>,
    const K: usize,
    NB: NodeBehaviour<A, K, P>,
    MB: MoveBehaviour<A, K>,
    P: PropagationParams<A, K>,
>(
    num_nodes: usize,
    gap: A,
    params: P,
    move_behaviour: MB,
    template: NB,
) -> Vec<NodeInit<NB, MB, P, A, K>> {
    let dim = A::from(num_nodes)
        .unwrap()
        .powf(A::one() / A::from(K).unwrap());
    let mut nodes = Vec::with_capacity(num_nodes);
    for i in 0..num_nodes {
        let mut coord = [A::zero(); K];
        let mut rem = A::from(i).unwrap();
        for j in 0..K {
            coord[j] = (rem % dim) * gap;
            rem = rem / dim;
        }
        trace!("Spawning at {:?}", coord);
        nodes.push(NodeInit {
            starting_position: coord,
            node_behaviour: template.clone(),
            move_behaviour: move_behaviour.clone(),
            propagation_params: params.clone(),
        })
    }
    nodes
}
