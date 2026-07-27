use std::ops::{Deref, DerefMut};

use crate::{
    backend::pitchbend12::Pitchbend12Config,
    config::StrategyConfig,
    interval::{stack::Stack, stacktype::r#trait::IntervalBasis},
    keystate::KeyState,
    process::r#trait::StackWithTuning,
    reference::Reference,
    util::ordered_locks::{
        Access, AccessMut, AtMost, IndexedAccess, IndexedAccessMut, Nat, OrderedLocks, Succ,
    },
};

#[deprecated]
pub trait ViewKeyStates {
    /// Index `i` must be in the range `0..128`
    fn key_state(&self, i: usize) -> KeyState;
}

#[deprecated]
pub trait ChangeKeyStates {
    /// Index `i` must be in the range `0..128`
    fn key_state_mut(&self, i: usize) -> impl DerefMut<Target = KeyState>;
}

#[deprecated]
pub trait ViewTunings<T: IntervalBasis> {
    /// Index `i` must be in the range `0..128`
    fn tuning(&self, i: usize) -> impl Deref<Target = StackWithTuning<T>>;
}

#[deprecated]
pub trait ChangeTunings<T: IntervalBasis> {
    /// Index `i` must be in the range `0..128`
    fn tuning_mut(&self, i: usize) -> impl DerefMut<Target = StackWithTuning<T>>;
}

/// The following type definitions define an ordering of locks:
///
/// In principle it should be fine to change the ordering, but:
///
/// - The functions [OrderedLocks::active_strategy] and [OrderedLocks::active_strategy_mut] assumes
///   that [StrategyConfigLevel] and [ActiveStrategyIndexLevel] are immediate successors.
///
/// - The functions [OrderedLocks::for_all_sounding_tunings] and
///   [OrderedLocks::check_all_keys_and_tunings] assume that [KeyStateLevel] and [TuningStateLevel] are
///   immediate successors.
#[rustfmt::skip]
pub mod lock_levels {
    use crate::util::ordered_locks::{Zero, Succ};
    pub type StrategyConfigLevel      = Zero;
    pub type ActiveStrategyIndexLevel = Succ<Zero>;
    pub type KeyStateLevel            = Succ<Succ<Zero>>; 
    pub type TuningStateLevel         = Succ<Succ<Succ<Zero>>>;
    pub type TuningReferenceLevel     = Succ<Succ<Succ<Succ<Zero>>>>;
    pub type ReferenceLevel           = Succ<Succ<Succ<Succ<Succ<Zero>>>>>;
    pub type BackendConfigLevel       = Succ<Succ<Succ<Succ<Succ<Succ<Zero>>>>>>;
}
use lock_levels::*;

// helper macro for the next impl. Only to save some writing and reading effort
macro_rules! accessor {
    ($name:ident < $($t:ident : $tr:path),* >,  $domain:ty, $lowest:ty, $level:ty, $result:ty ) => {
        #[inline]
        pub fn $name<R,$($t : $tr),*>(self, f: impl FnMut(&$result, OrderedLocks< $domain, Succ<$level>>) -> R) -> (R, Self)
        where
            $domain: Access<$level, $result>,
            $lowest: AtMost<$level>,
        {
            self.ith::<$level, _, _>(f)
        }
    };

    (@mut $name:ident < $(  $t:ident : $tr:path  ),* >, $domain:ty, $lowest:ty, $level:ty, $result:ty ) => {
        #[inline]
        pub fn $name<R,$($t : $tr),*>(self, f: impl FnMut(&mut $result, OrderedLocks< $domain, Succ<$level>>) -> R) -> (R, Self)
        where
            $domain: AccessMut<$level, $result>,
            $lowest: AtMost<$level>,
        {
            self.ith_mut::<$level, _, _>(f)
        }
    };

    (@indexed $name:ident < $(  $t:ident : $tr:path  ),* >, $domain:ty, $lowest:ty, $level:ty, $index:ty, $result:ty ) => {
        #[inline]
        pub fn $name<R,$($t : $tr),*>(self, i: $index, f: impl FnMut(&$result,  OrderedLocks< $domain, Succ<$level>>) -> R) -> (R, Self)
        where
            $domain: IndexedAccess<$level, $index, $result>,
            $lowest: AtMost<$level>,
        {
            self.ith_indexed::<$level, _, _, _>(i, f)
        }
    };

    (@indexed @mut $name:ident < $(  $t:ident : $tr:path  ),* >, $domain:ty, $lowest:ty, $level:ty, $index:ty, $result:ty ) => {
        #[inline]
        pub fn $name<R,$($t : $tr),*>(self, i: $index, f: impl FnMut(&mut $result, OrderedLocks< $domain, Succ<$level>>) -> R) -> (R, Self)
        where
            $domain: IndexedAccessMut<$level, $index, $result>,
            $lowest: AtMost<$level>,
        {
            self.ith_indexed_mut::<$level, _, _, _>(i, f)
        }
    };
}

impl<M, L: Nat> OrderedLocks<M, L> {
    accessor! {@indexed key_state <>,  M, L, KeyStateLevel, usize, KeyState}
    accessor! {@indexed @mut key_state_mut <>,  M, L, KeyStateLevel, usize, KeyState}

    accessor! {@indexed tuning <T:IntervalBasis>,  M, L, TuningStateLevel, usize, StackWithTuning<T>}
    accessor! {@indexed @mut tuning_mut <T:IntervalBasis>,  M, L, TuningStateLevel, usize, StackWithTuning<T>}

    accessor! {tuning_reference <T:IntervalBasis> ,  M, L, TuningReferenceLevel, Reference<T>}
    accessor! {@mut tuning_reference_mut <T:IntervalBasis> ,  M, L, TuningReferenceLevel, Reference<T>}

    accessor! {strategy_config <T:IntervalBasis> ,  M, L, StrategyConfigLevel, Vec<StrategyConfig<T>>}
    accessor! {@mut strategy_config_mut <T:IntervalBasis> ,  M, L, StrategyConfigLevel, Vec<StrategyConfig<T>>}

    accessor! {active_strategy_index <> ,  M, L, ActiveStrategyIndexLevel, usize}
    accessor! {@mut active_strategy_index_mut <>  ,  M, L, ActiveStrategyIndexLevel, usize}

    accessor! {reference <T:IntervalBasis> ,  M, L, ReferenceLevel, Stack<T>}
    accessor! {@mut reference_mut <T:IntervalBasis>  ,  M, L, ReferenceLevel, Stack<T>}

    accessor! {backend_config <> ,  M, L, BackendConfigLevel,Pitchbend12Config}
    accessor! {@mut backend_config_mut <>  ,  M, L, BackendConfigLevel, Pitchbend12Config}

    #[inline]
    pub fn active_strategy<R, T>(
        self,
        mut f: impl FnMut(&StrategyConfig<T>, OrderedLocks<M, Succ<ActiveStrategyIndexLevel>>) -> R,
    ) -> (R, Self)
    where
        T: IntervalBasis,
        M: Access<ActiveStrategyIndexLevel, usize>
            + Access<StrategyConfigLevel, Vec<StrategyConfig<T>>>,
        L: AtMost<StrategyConfigLevel>,
    {
        self.strategy_config(|conf, r| r.active_strategy_index(|i, s| f(&conf[*i], s)).0)
    }

    #[inline]
    pub fn active_strategy_mut<R, T>(
        self,
        mut f: impl FnMut(&mut StrategyConfig<T>, OrderedLocks<M, Succ<ActiveStrategyIndexLevel>>) -> R,
    ) -> (R, Self)
    where
        T: IntervalBasis,
        M: Access<ActiveStrategyIndexLevel, usize>
            + AccessMut<StrategyConfigLevel, Vec<StrategyConfig<T>>>,
        L: AtMost<StrategyConfigLevel>,
    {
        self.strategy_config_mut(|conf, r| r.active_strategy_index(|i, s| f(&mut conf[*i], s)).0)
    }

    #[inline]
    pub fn lowest_sounding_key(mut self) -> (Option<usize>, Self)
    where
        M: IndexedAccess<KeyStateLevel, usize, KeyState>,
        L: AtMost<KeyStateLevel>,
    {
        let mut res = None {};
        for i in 0..128 {
            (_, self) = self.key_state(i, |k, _| {
                if k.is_sounding() {
                    res = Some(i);
                }
            });
            if res.is_some() {
                break;
            }
        }
        (res, self)
    }

    #[inline]
    pub fn for_all_sounding_keys(
        mut self,
        mut f: impl FnMut(usize, &KeyState, OrderedLocks<M, Succ<KeyStateLevel>>),
    ) -> Self
    where
        M: IndexedAccess<KeyStateLevel, usize, KeyState>,
        L: AtMost<KeyStateLevel>,
    {
        for i in 0..128 {
            (_, self) = self.key_state(i, |k, r| {
                if k.is_sounding() {
                    f(i, k, r);
                }
            });
        }
        self
    }

    #[inline]
    pub fn collect_sounding_keys(mut self) -> (Vec<u8>, Self)
    where
        M: IndexedAccess<KeyStateLevel, usize, KeyState>,
        L: AtMost<KeyStateLevel>,
    {
        let mut res = vec![];
        for i in 0..128 {
            (_, self) = self.key_state(i, |k, r| {
                if k.is_sounding() {
                    res.push(i as u8);
                }
            });
        }
        (res, self)
    }

    #[inline]
    pub fn for_all_sounding_tunings<T>(
        mut self,
        mut f: impl FnMut(usize, &StackWithTuning<T>, OrderedLocks<M, Succ<TuningStateLevel>>),
    ) -> Self
    where
        T: IntervalBasis,
        M: IndexedAccess<KeyStateLevel, usize, KeyState>
            + IndexedAccess<TuningStateLevel, usize, StackWithTuning<T>>,
        L: AtMost<KeyStateLevel>,
    {
        for i in 0..128 {
            (_, self) = self.key_state(i, |k, r| {
                if k.is_sounding() {
                    r.tuning(i, |t, r| f(i, t, r));
                }
            });
        }
        self
    }

    #[inline]
    pub fn check_all_keys_and_tunings<T>(
        mut self,
        mut f: impl FnMut(
            usize,
            &KeyState,
            &StackWithTuning<T>,
            OrderedLocks<M, Succ<TuningStateLevel>>,
        ) -> bool,
    ) -> (bool, Self)
    where
        T: IntervalBasis,
        M: IndexedAccess<KeyStateLevel, usize, KeyState>
            + IndexedAccess<TuningStateLevel, usize, StackWithTuning<T>>,
        L: AtMost<KeyStateLevel>,
    {
        let mut res = true;
        for i in 0..128 {
            (_, self) = self.key_state(i, |k, r| {
                (res, _) = r.tuning(i, |t, r| f(i, k, t, r));
            });
            if !res {
                return (false, self);
            }
        }
        (true, self)
    }

    #[inline]
    pub fn pair_of_tunings<R, T>(
        self,
        i: usize,
        j: usize,
        mut f: impl FnMut(
            &StackWithTuning<T>,
            &StackWithTuning<T>,
            OrderedLocks<M, Succ<TuningStateLevel>>,
        ) -> R,
    ) -> (R, Self)
    where
        T: IntervalBasis,
        M: IndexedAccess<TuningStateLevel, usize, StackWithTuning<T>>,
        L: AtMost<TuningStateLevel>,
    {
        self.ith_indexed_pair(i, j, |x, y, r| f(x, y, r))
    }
}
