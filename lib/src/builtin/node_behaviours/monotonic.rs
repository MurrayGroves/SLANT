//! A wrapping behaviour which encapsulates another behaviour and also generates packets every `N` ticks.
use crate::behaviours::NodeBehaviour;
use crate::managers::GlobalStateManager;
use crate::node::NodeData;
use crate::packets::Packet;
use crate::propagation_models::{PropagationModel, PropagationParams};
use crate::{Coord, SimConfig};
use log::trace;
use std::sync::Arc;

/// This behaviour can contain any type which is also a behaviour.
/// Every tick it will also tick its contained behaviour.
/// Every `N` ticks it will call a provided closure to generate a new packet, then broadcast it.
/// The closure is provided with the contained behaviour so you can use its state.
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
    /// # Arguments
    ///
    /// * `contained`: The behaviour that will be wrapped and ticked each tick.
    /// * `ticks_per_packet`: How many ticks the behaviour should wait in-between transmissions.
    /// * `gen_packet`: A closure which accepts the contained behaviour, the node's data, and some bytes the packet should contain. It should return a new packet.
    pub fn new(
        contained: T,
        ticks_per_packet: usize,
        gen_packet: Arc<dyn Fn(&mut T, &NodeData<A, K, PP>, Box<[u8]>) -> P + Send + Sync>,
    ) -> Self {
        Monotonic {
            ticks_per_packet,
            counter: 0,
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
