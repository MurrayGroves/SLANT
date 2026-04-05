use crate::types::NodeID;
use linearize::{Linearize, StaticMap};

#[derive(Linearize)]
pub enum InternalStatKey {
    PacketTransmits,
}

#[derive(Clone, Debug)]
pub enum InternalEvent {
    /// A node transmitted a packet
    PacketTransmit(NodeID),
    /// A src sent a packet to a dst
    PacketLink((NodeID, NodeID)),
}

enum StatKey<T: Linearize> {
    Internal(InternalStatKey),
    User(T),
}

/// Used to record arbitrary stats for a given timestep
/// Note you cannot retrieve stats during a tick, as stats are thread-localised
pub struct TimestepStats<T: Linearize> {
    internal: StaticMap<InternalStatKey, isize>,
    pub(crate) internal_events: Vec<InternalEvent>,
    user: StaticMap<T, isize>,
}

impl<T: Linearize> TimestepStats<T> {
    pub fn new() -> Self {
        Self {
            internal: StaticMap::default(),
            user: StaticMap::default(),
            internal_events: Vec::new(),
        }
    }

    pub fn inc(&mut self, key: T, by: isize) {
        self.user[key] += by;
    }

    pub fn dec(&mut self, key: T, by: isize) {
        self.user[key] -= by;
    }

    pub(crate) fn inc_internal(&mut self, key: InternalStatKey, by: isize) {
        self.internal[key] += by;
    }

    pub(crate) fn dec_internal(&mut self, key: InternalStatKey, by: isize) {
        self.internal[key] -= by;
    }

    pub(crate) fn add_event(&mut self, event: InternalEvent) {
        self.internal_events.push(event);
    }

    pub(crate) fn consume(&mut self, other: &Self) {
        for (key, val) in &other.internal {
            self.internal[key] += val;
        }

        for (key, val) in &other.user {
            self.user[key] += val;
        }

        self.internal_events
            .extend(other.internal_events.iter().cloned());
    }

    pub fn events(self) -> Vec<InternalEvent> {
        self.internal_events
    }
}

impl<T: Linearize> Default for TimestepStats<T> {
    fn default() -> Self {
        TimestepStats::new()
    }
}
