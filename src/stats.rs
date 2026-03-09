use linearize::{Linearize, StaticMap};

#[derive(Linearize)]
enum InternalStatKey {}

enum StatKey<T: Linearize> {
    Internal(InternalStatKey),
    User(T),
}

/// Used to record arbitrary stats for a given timestep
/// Note you cannot retrieve stats during a tick, as stats are thread-localised
pub struct TimestepStats<T: Linearize> {
    internal: StaticMap<InternalStatKey, isize>,
    user: StaticMap<T, isize>,
}

impl<T: Linearize> TimestepStats<T> {
    pub fn new() -> Self {
        Self {
            internal: StaticMap::default(),
            user: StaticMap::default(),
        }
    }

    pub fn inc(&mut self, key: T, by: isize) {
        self.user[key] += by;
    }

    pub fn dec(&mut self, key: T, by: isize) {
        self.user[key] -= by;
    }

    pub(crate) fn inc_internal(&mut self, key: T, by: isize) {
        self.user[key] += by;
    }

    pub(crate) fn dec_internal(&mut self, key: T, by: isize) {
        self.user[key] -= by;
    }
}
