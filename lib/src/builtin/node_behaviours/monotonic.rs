use crate::managers::GlobalStateManager;
use crate::node::NodeData;
use crate::packets::Packet;
use crate::propagation_models::{PropagationModel, PropagationParams};
use crate::traits::{Coord, NodeBehaviour, SimConfig};
use log::trace;
use std::sync::Arc;

#[derive(Clone)]
pub struct Monotonic<
    A: Coord<K>,
    const K: usize,
    T: NodeBehaviour<A, K, PP>,
    P: Packet,
    PP: PropagationParams<A, K>,
> {
    pub ticks_per_packet: usize,
    counter: usize,
    pub received_packets: usize,
    gen_packet: Arc<dyn Fn(&mut T, &NodeData<A, K, PP>, Box<[u8]>) -> P + Send + Sync>,
    pub contained: T,
}

impl<
    A: Coord<K>,
    const K: usize,
    T: NodeBehaviour<A, K, PP>,
    PP: PropagationParams<A, K>,
    P: Packet,
> Monotonic<A, K, T, P, PP>
{
    pub fn new(
        contained: T,
        ticks_per_packet: usize,
        gen_packet: Arc<dyn Fn(&mut T, &NodeData<A, K, PP>, Box<[u8]>) -> P + Send + Sync>,
    ) -> Self {
        Monotonic {
            ticks_per_packet,
            counter: 0,
            received_packets: 0,
            gen_packet,
            contained,
        }
    }
}

impl<
    A: Coord<K>,
    const K: usize,
    T: NodeBehaviour<A, K, PP, P = P, E = E>,
    E,
    PP: PropagationParams<A, K>,
    P: Packet,
> NodeBehaviour<A, K, PP> for Monotonic<A, K, T, P, PP>
{
    type P = P;
    type E = E;

    fn tick<
        C: SimConfig<
                A,
                K,
                PM = impl PropagationModel<A, K, P = PP>,
                NB = impl NodeBehaviour<A, K, PP, P = Self::P, E = Self::E>,
                E = Self::E,
            >,
    >(
        mut self,
        node_data: &NodeData<A, K, PP>,
        global_state_manager: &GlobalStateManager<A, K, C>,
        incoming_packets: &Vec<Self::P>,
    ) -> Self {
        trace!("Ticking {}", node_data.id);
        self.received_packets += incoming_packets.len();
        trace!(
            "{} received {:?}",
            node_data.id,
            incoming_packets.iter().map(|x| x.content_ref()[0])
        );

        if self.counter % self.ticks_per_packet == 0 {
            global_state_manager.transmit_packet(
                node_data,
                (self.gen_packet)(
                    &mut self.contained,
                    node_data,
                    Box::new(node_data.id.to_be_bytes()),
                ),
            )
        }

        self.counter += 1;

        self.contained = self
            .contained
            .tick(node_data, global_state_manager, incoming_packets);

        self
    }
}
