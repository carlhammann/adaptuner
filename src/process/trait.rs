use crate::{
    adaptors::{
        lock_levels::{
            ActiveStrategyIndexLevel, KeyStateLevel, PedalHoldLevel, ReferenceLevel, SoftHoldLevel,
            SostenutoHoldLevel, StrategyConfigLevel, TuningReferenceLevel, TuningStateLevel,
        },
        ConcreteLocks,
    },
    interval::{
        base::Semitones,
        stack::Stack,
        stacktype::r#trait::{IntervalBasis, StackType},
    },
    msg::FromProcess,
    util::ordered_locks::{Nat, OrderedLocks, ReadAllowed, WriteAllowed},
};

pub struct StackWithTuning<T: IntervalBasis> {
    pub stack: Stack<T>,
    pub semitones: Semitones,
}

pub struct ProcessTag {}

impl ReadAllowed<PedalHoldLevel> for ProcessTag {}
impl ReadAllowed<SostenutoHoldLevel> for ProcessTag {}
impl ReadAllowed<SoftHoldLevel> for ProcessTag {}
impl ReadAllowed<KeyStateLevel> for ProcessTag {}
impl ReadAllowed<TuningStateLevel> for ProcessTag {}
impl ReadAllowed<StrategyConfigLevel> for ProcessTag {}
impl ReadAllowed<ActiveStrategyIndexLevel> for ProcessTag {}
impl ReadAllowed<TuningReferenceLevel> for ProcessTag {}
impl ReadAllowed<ReferenceLevel> for ProcessTag {}

impl WriteAllowed<PedalHoldLevel> for ProcessTag {}
impl WriteAllowed<SostenutoHoldLevel> for ProcessTag {}
impl WriteAllowed<SoftHoldLevel> for ProcessTag {}
impl WriteAllowed<KeyStateLevel> for ProcessTag {}
impl WriteAllowed<TuningStateLevel> for ProcessTag {}
impl WriteAllowed<ReferenceLevel> for ProcessTag {}

pub type ProcessAdaptor<T, L> = OrderedLocks<ProcessTag, ConcreteLocks<T>, L>;

impl<T: StackType, L: Nat> ProcessAdaptor<T, L> {
    #[inline]
    pub fn send(&self, msg: FromProcess<T>) {
        let _ = unsafe { self.inner() }.from_process_tx.send(msg);
    }
}
