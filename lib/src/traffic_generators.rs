use crate::packets::{MulticastOrUnicast, MulticastPacket, Packet, UnicastPacket};
use crate::propagation_models::{PropagationModel, PropagationParams};
use crate::types::{Coord, GlobalStateManager, MoveBehaviour, NodeBehaviour, NodeID, SimConfig};
use rand::RngExt;

pub fn mixed_multicast_and_random_target_unicast<
    A: Coord<K>,
    const K: usize,
    C: SimConfig<
            A,
            K,
            NB = impl NodeBehaviour<
                A,
                K,
                <<C as SimConfig<A, K>>::PM as PropagationModel<A, K>>::P,
                P = MulticastOrUnicast,
            >,
        >,
>(
    rng: &mut impl rand::Rng,
    global_state_manager: &GlobalStateManager<A, K, C>,
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
        global_state_manager.transmit_packet(
            &global_state_manager.nodes[index].data,
            MulticastOrUnicast::MulticastPacket(packet),
        )
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
        global_state_manager.transmit_packet(
            &global_state_manager.nodes[index].data,
            MulticastOrUnicast::UnicastPacket(packet),
        )
    }
}
