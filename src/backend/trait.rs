use crate::{
    adaptors::{
        lock_levels::{BackendConfigLevel, KeyStateLevel, TuningStateLevel},
        ConcreteLocks,
    },
    interval::stacktype::r#trait::StackType,
    msg::FromBackend,
    util::ordered_locks::{Nat, OrderedLocks, ReadAllowed},
};

pub struct BackendTag {}

impl ReadAllowed<KeyStateLevel> for BackendTag {}
impl ReadAllowed<TuningStateLevel> for BackendTag {}
impl ReadAllowed<BackendConfigLevel> for BackendTag {}

pub type BackendAdaptorNew<T, L> = OrderedLocks<BackendTag, ConcreteLocks<T>, L>;

impl<T: StackType, L: Nat> BackendAdaptorNew<T, L> {
    #[inline]
    pub fn send(&self, msg: FromBackend) {
        let _ = unsafe { self.inner() }.from_backend_tx.send(msg);
    }
}
