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
    PP: PropagationParams<A, K>,
> {
    /// Interval between each packet generation in sim time.
    /// E.g. with a value of `5` a packet will be generated every 5 time units.
    pub generation_interval: f64,
    next_packet: f64,
    gen_packet: Arc<
        dyn Fn(
                &mut T,
                &NodeData<A, K, PP>,
                Arc<Box<[u8]>>,
            ) -> <Monotonic<A, K, T, PP> as NodeBehaviour<A, K, PP>>::P
            + Send
            + Sync,
    >,
    /// The contained behaviour which is ticked by Monotonic.
    pub contained: T,
}

impl<A: Coord<K>, const K: usize, T: NodeBehaviour<A, K, PP>, PP: PropagationParams<A, K>>
    Monotonic<A, K, T, PP>
{
    /// # Arguments
    ///
    /// * `contained`: The behaviour that will be wrapped and ticked each tick.
    /// * `generation_interval`: Interval between transmission in simulation time.
    /// * `gen_packet`: A closure which accepts the contained behaviour, the node's data, and some bytes the packet should contain. It should return a new packet.
    pub fn new(
        contained: T,
        generation_interval: f64,
        gen_packet: Arc<
            dyn Fn(
                    &mut T,
                    &NodeData<A, K, PP>,
                    Arc<Box<[u8]>>,
                ) -> <Monotonic<A, K, T, PP> as NodeBehaviour<A, K, PP>>::P
                + Send
                + Sync,
        >,
    ) -> Self {
        Monotonic {
            generation_interval,
            next_packet: generation_interval,
            gen_packet,
            contained,
        }
    }
}

impl<A: Coord<K>, const K: usize, T: NodeBehaviour<A, K, PP>, PP: PropagationParams<A, K>>
    NodeBehaviour<A, K, PP> for Monotonic<A, K, T, PP>
{
    type P = T::P;
    type E = T::E;

    /// Ticks contained behaviour.
    /// Generates a new packet from the given closure every `ticks_per_packet` ticks.
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

        if global_state_manager.is_time(self.next_packet) {
            self.next_packet += self.generation_interval;
            global_state_manager.transmit_packet(
                node_data,
                (self.gen_packet)(
                    &mut self.contained,
                    node_data,
                    Arc::new(Box::new(node_data.id.to_be_bytes())),
                ),
            )
        }

        self.contained = self
            .contained
            .tick(node_data, global_state_manager, incoming_packets);

        self
    }
}

#[cfg(test)]
mod tests {
    use crate::SimConfig;
    use crate::builtin::move_behaviours::static_movement::StaticMovement;
    use crate::builtin::node_behaviours::empty_behaviour::EmptyBehaviour;
    use crate::builtin::node_behaviours::monotonic::Monotonic;
    use crate::builtin::packets::multicast::MulticastPacket;
    use crate::builtin::propagation_models::simple_distance::{
        SimpleDistance, SimpleDistanceParams,
    };
    use crate::managers::SimManager;
    use crate::node::NodeInit;
    use crate::stats::InternalStatKey;
    use std::sync::Arc;

    #[test]
    fn monotonic() {
        struct Config;
        impl SimConfig<f32, 2> for Config {
            type MB = StaticMovement;
            type NB =
                Monotonic<f32, 2, EmptyBehaviour<MulticastPacket>, SimpleDistanceParams<f32, 2>>;
            type PM = SimpleDistance;
        }

        let interval = 1.0;
        let tick_time = 100.0;

        let mut sim_manager: SimManager<f32, 2, Config> = SimManager::new(
            vec![NodeInit {
                node_behaviour: Monotonic::new(
                    EmptyBehaviour::new(),
                    interval,
                    Arc::new(|_, _, content| MulticastPacket { content }),
                ),
                starting_position: [0.0, 0.0],
                move_behaviour: StaticMovement {},
                propagation_params: SimpleDistanceParams {
                    transmit_distance: 0.0,
                },
            }],
            123,
            SimpleDistance,
            0.3,
        );

        let stats = sim_manager.tick_time(tick_time);
        let packets_transmitted = stats
            .into_iter()
            .map(|mut stat| stat.internal_stats()[InternalStatKey::PacketTransmits])
            .reduce(std::ops::Add::add)
            .unwrap_or_default();

        assert_eq!(packets_transmitted, (tick_time / interval) as isize);
    }
}
