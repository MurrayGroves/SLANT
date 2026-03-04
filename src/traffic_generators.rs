use crate::packets::{MulticastPacket, UnicastPacket};
use crate::propagation_models::PropagationModel;
use crate::types::{Coord, GlobalStateManager, MoveBehaviour, NodeBehaviour, NodeID};

pub fn mixed_multicast_and_random_target_unicast<
    NodeBehaviourType: NodeBehaviour<A, K>,
    MoveBehaviourType: MoveBehaviour<A, K>,
    A: Coord<K>,
    const K: usize,
>(
    rng: &mut impl rand::Rng,
    global_state_manager: &GlobalStateManager<
        NodeBehaviourType,
        MoveBehaviourType,
        impl PropagationModel<A, K>,
        A,
        K,
    >,
    num_multicast: usize,
    num_unicast: usize,
) {
    for _ in 0..num_multicast {
        let index = rng.random_range(0..global_state_manager.nodes.len());
        let mut bytes: [u8; 32] = [0; 32];
        rng.fill(&mut bytes);
        let packet = MulticastPacket {
            content: Box::new(bytes),
        };
        global_state_manager.transmit_packet(&global_state_manager.nodes[index].data, packet)
    }

    for _ in 0..num_unicast {
        let index = rng.random_range(0..global_state_manager.nodes.len());
        let target_id: NodeID = rng.random_range(0..global_state_manager.nodes.len());
        let mut bytes: [u8; 32] = [0; 32];
        rng.fill(&mut bytes);
        let packet = UnicastPacket {
            content: Box::new(bytes),
            target: target_id,
        };
        global_state_manager.transmit_packet(&global_state_manager.nodes[index].data, packet)
    }
}
