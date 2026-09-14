use std::sync::mpsc;

use parking_lot::RwLock;

use crate::{
    backend::pitchbend12::Pitchbend12Config,
    config::{GuiConfig, MelodyStrategyConfig, Named, StrategyConfig},
    interval::{
        stack::Stack,
        stacktype::r#trait::{IntervalBasis, StackType},
    },
    keystate::KeyState,
    msg::{FromBackend, FromProcess, FromUi},
    neighbourhood::SomeCompleteNeighbourhood,
    process::r#trait::StackWithTuning,
    reference::Reference,
    strategy::{
        harmony::r#trait::Harmony, melody::neighbourhoods::StaticNeighbourhoodsAsMelodyConfig,
        staticneighbourhoods::StaticNeighbourhoodsConfig,
    },
    util::ordered_locks::{
        impl_access, impl_access_mut, impl_indexed_access, impl_indexed_access_mut, Access,
        AccessMut, AtMost, IndexedAccess, IndexedAccessMut, Nat, OrderedLocks, ReadAllowed, Succ,
        WriteAllowed,
    },
};

pub struct ConcreteLocks<T: StackType> {
    pub from_ui_tx: mpsc::Sender<FromUi<T>>,
    pub from_process_tx: mpsc::Sender<FromProcess<T>>,
    pub from_backend_tx: mpsc::Sender<FromBackend>,

    pub pedal_hold: RwLock<[bool; 16]>,
    pub sostenuto_hold: RwLock<[bool; 16]>,
    pub soft_hold: RwLock<[bool; 16]>,
    pub key_states: [RwLock<KeyState>; 128],
    pub tunings: [RwLock<StackWithTuning<T>>; 128],
    pub tuning_reference: RwLock<Reference<T>>,
    pub reference: RwLock<Stack<T>>,
    pub strategy_config: RwLock<Vec<StrategyConfig<T>>>,
    pub active_strategy_index: RwLock<usize>,
    /// Because this field isn't actually shared between threads, it won't be accessed through the
    /// machinery in this module, but through [UiAdaptor::config]
    pub gui_config: RwLock<GuiConfig>,
    pub backend_config: RwLock<Pitchbend12Config>,
    pub harmony: RwLock<Option<Harmony<T>>>,
}

/// The following type definitions define an ordering of locks:
///
/// In principle it should be fine to change the ordering, but:
///
/// - The functions [OrderedLocks::active_strategy] and [OrderedLocks::active_strategy_mut] assume
///   that [StrategyConfigLevel] and [ActiveStrategyIndexLevel] are immediate successors.
///
/// - The functions [OrderedLocks::for_all_sounding_tunings] and
///   [OrderedLocks::check_all_keys_and_tunings] assume that [KeyStateLevel] and [TuningStateLevel] are
///   immediate successors.
#[rustfmt::skip]
pub mod lock_levels {
    use crate::util::ordered_locks::{Zero, Succ};
    pub type HarmonyLevel             = Zero;
    pub type StrategyConfigLevel      = Succ<Zero>;
    pub type ActiveStrategyIndexLevel = Succ<Succ<Zero>>;
    pub type PedalHoldLevel           = Succ<Succ<Succ<Zero>>>;
    pub type SostenutoHoldLevel       = Succ<Succ<Succ<Succ<Zero>>>>;
    pub type SoftHoldLevel            = Succ<Succ<Succ<Succ<Succ<Zero>>>>>;
    pub type KeyStateLevel            = Succ<Succ<Succ<Succ<Succ<Succ<Zero>>>>>>;
    pub type TuningStateLevel         = Succ<Succ<Succ<Succ<Succ<Succ<Succ<Zero>>>>>>>;
    pub type TuningReferenceLevel     = Succ<Succ<Succ<Succ<Succ<Succ<Succ<Succ<Zero>>>>>>>>; 
    pub type ReferenceLevel           = Succ<Succ<Succ<Succ<Succ<Succ<Succ<Succ<Succ<Zero>>>>>>>>>;
    pub type BackendConfigLevel       = Succ<Succ<Succ<Succ<Succ<Succ<Succ<Succ<Succ<Succ<Zero>>>>>>>>>>;
    // pub type GuiConfigLevel           = Succ<Succ<Succ<Succ<Succ<Succ<Succ<Succ<Succ<Succ<Succ<Zero>>>>>>>>>>>;
}
use lock_levels::*;

impl_access! {<T:StackType>, ConcreteLocks<T>, PedalHoldLevel, [bool;16], |self| &self.pedal_hold.read()}
impl_access! {<T:StackType>, ConcreteLocks<T>, SostenutoHoldLevel, [bool;16], |self| &self.sostenuto_hold.read()}
impl_access! {<T:StackType>, ConcreteLocks<T>, SoftHoldLevel, [bool;16], |self| &self.soft_hold.read()}
impl_indexed_access! {<T:StackType>, ConcreteLocks<T>, KeyStateLevel, usize, KeyState, |self, i| &self.key_states[i].read()}
impl_indexed_access! {<T:StackType>, ConcreteLocks<T>, TuningStateLevel, usize, StackWithTuning<T>, |self, i| &self.tunings[i].read()}
impl_access! {<T:StackType>, ConcreteLocks<T>, TuningReferenceLevel, Reference<T>, |self| &self.tuning_reference.read()}
impl_access! {<T:StackType>, ConcreteLocks<T>, ReferenceLevel, Stack<T>, |self| &self.reference.read()}
impl_access! {<T:StackType>, ConcreteLocks<T>, StrategyConfigLevel, Vec<StrategyConfig<T>>, |self| &self.strategy_config.read()}
impl_access! {<T:StackType>, ConcreteLocks<T>, ActiveStrategyIndexLevel, usize, |self| &self.active_strategy_index.read()}
impl_access! {<T:StackType>, ConcreteLocks<T>, BackendConfigLevel, Pitchbend12Config, |self| &self.backend_config.read()}
// impl_access! {<T:StackType>, ConcreteLocks<T>, GuiConfigLevel, GuiConfig, |self| &self.gui_config.read()}
impl_access! {<T:StackType>, ConcreteLocks<T>, HarmonyLevel, Option<Harmony<T>>, |self| &self.harmony.read()}

impl_access_mut! {<T:StackType>, ConcreteLocks<T>, PedalHoldLevel, [bool;16], |self| &mut self.pedal_hold.write()}
impl_access_mut! {<T:StackType>, ConcreteLocks<T>, SostenutoHoldLevel, [bool;16], |self| &mut self.sostenuto_hold.write()}
impl_access_mut! {<T:StackType>, ConcreteLocks<T>, SoftHoldLevel, [bool;16], |self| &mut self.soft_hold.write()}
impl_indexed_access_mut! {<T:StackType>, ConcreteLocks<T>, KeyStateLevel, usize, KeyState, |self, i| &mut self.key_states[i].write()}
impl_indexed_access_mut! {<T:StackType>, ConcreteLocks<T>, TuningStateLevel, usize, StackWithTuning<T>, |self, i| &mut self.tunings[i].write()}
impl_access_mut! {<T:StackType>, ConcreteLocks<T>, TuningReferenceLevel, Reference<T>, |self| &mut self.tuning_reference.write()}
impl_access_mut! {<T:StackType>, ConcreteLocks<T>, ReferenceLevel, Stack<T>, |self| &mut self.reference.write()}
impl_access_mut! {<T:StackType>, ConcreteLocks<T>, StrategyConfigLevel, Vec<StrategyConfig<T>>, |self| &mut self.strategy_config.write()}
impl_access_mut! {<T:StackType>, ConcreteLocks<T>, ActiveStrategyIndexLevel, usize, |self| &mut self.active_strategy_index.write()}
impl_access_mut! {<T:StackType>, ConcreteLocks<T>, BackendConfigLevel, Pitchbend12Config, |self| &mut self.backend_config.write()}
// impl_access_mut! {<T:StackType>, ConcreteLocks<T>, GuiConfigLevel, GuiConfig, |self| &mut self.gui_config.write()}
impl_access_mut! {<T:StackType>, ConcreteLocks<T>, HarmonyLevel, Option<Harmony<T>>, |self| &mut self.harmony.write()}

// helper macro for the next impl. Only to save some writing and reading effort
macro_rules! accessor {
    ($name:ident < $($t:ident : $tr:path),* >, $tag:ty,  $domain:ty, $lowest:ty, $level:ty, $result:ty ) => {
        #[inline]
        pub fn $name<R,$($t : $tr),*>(self, f: impl FnMut(&$result, OrderedLocks<$tag, $domain, Succ<$level>>) -> R) -> (R, Self)
        where
            $domain: Access<$level, $result>,
            $lowest: AtMost<$level>,
            $tag: ReadAllowed<$level>,
        {
            self.ith::<$level, _, _>(f)
        }
    };

    (@mut $name:ident < $(  $t:ident : $tr:path  ),* >, $tag:ty, $domain:ty, $lowest:ty, $level:ty, $result:ty ) => {
        #[inline]
        pub fn $name<R,$($t : $tr),*>(self, f: impl FnMut(&mut $result, OrderedLocks<$tag, $domain, Succ<$level>>) -> R) -> (R, Self)
        where
            $domain: AccessMut<$level, $result>,
            $lowest: AtMost<$level>,
            $tag: WriteAllowed<$level>,
        {
            self.ith_mut::<$level, _, _>(f)
        }
    };

    (@indexed $name:ident < $(  $t:ident : $tr:path  ),* >, $tag:ty, $domain:ty, $lowest:ty, $level:ty, $index:ty, $result:ty ) => {
        #[inline]
        pub fn $name<R,$($t : $tr),*>(self, i: $index, f: impl FnMut(&$result, OrderedLocks<$tag, $domain, Succ<$level>>) -> R) -> (R, Self)
        where
            $domain: IndexedAccess<$level, $index, $result>,
            $lowest: AtMost<$level>,
            $tag: ReadAllowed<$level>,
        {
            self.ith_indexed::<$level, _, _, _>(i, f)
        }
    };

    (@indexed @mut $name:ident < $(  $t:ident : $tr:path  ),* >, $tag:ty, $domain:ty, $lowest:ty, $level:ty, $index:ty, $result:ty ) => {
        #[inline]
        pub fn $name<R,$($t : $tr),*>(self, i: $index, f: impl FnMut(&mut $result, OrderedLocks<$tag, $domain, Succ<$level>>) -> R) -> (R, Self)
        where
            $domain: IndexedAccessMut<$level, $index, $result>,
            $lowest: AtMost<$level>,
            $tag: WriteAllowed<$level>,
        {
            self.ith_indexed_mut::<$level, _, _, _>(i, f)
        }
    };
}

impl<X, M, L: Nat> OrderedLocks<X, M, L> {
    accessor! {pedal_hold <>, X,  M, L, PedalHoldLevel, [bool;16]}
    accessor! {@mut pedal_hold_mut <>, X,  M, L, PedalHoldLevel, [bool;16]}

    accessor! {sostenuto_hold <>, X,  M, L, SostenutoHoldLevel, [bool;16]}
    accessor! {@mut sostenuto_hold_mut <>, X,  M, L, SostenutoHoldLevel, [bool;16]}

    accessor! {soft_hold <>, X,  M, L, SoftHoldLevel, [bool;16]}
    accessor! {@mut soft_hold_mut <>, X,  M, L, SoftHoldLevel, [bool;16]}

    accessor! {@indexed key_state <>, X,  M, L, KeyStateLevel, usize, KeyState}
    accessor! {@indexed @mut key_state_mut <>, X,  M, L, KeyStateLevel, usize, KeyState}

    accessor! {@indexed tuning <T:IntervalBasis>, X,  M, L, TuningStateLevel, usize, StackWithTuning<T>}
    accessor! {@indexed @mut tuning_mut <T:IntervalBasis>, X, M, L, TuningStateLevel, usize, StackWithTuning<T>}

    accessor! {tuning_reference <T:IntervalBasis> , X,  M, L, TuningReferenceLevel, Reference<T>}
    accessor! {@mut tuning_reference_mut <T:IntervalBasis> , X, M, L, TuningReferenceLevel, Reference<T>}

    accessor! {strategy_config <T:IntervalBasis> , X,  M, L, StrategyConfigLevel, Vec<StrategyConfig<T>>}
    accessor! {@mut strategy_config_mut <T:IntervalBasis> , X,  M, L, StrategyConfigLevel, Vec<StrategyConfig<T>>}

    accessor! {active_strategy_index <> , X,  M, L, ActiveStrategyIndexLevel, usize}
    accessor! {@mut active_strategy_index_mut <>  ,  X, M, L, ActiveStrategyIndexLevel, usize}

    accessor! {reference <T:IntervalBasis> ,  X, M, L, ReferenceLevel, Stack<T>}
    accessor! {@mut reference_mut <T:IntervalBasis>  , X,  M, L, ReferenceLevel, Stack<T>}

    accessor! {backend_config <> ,  X, M, L, BackendConfigLevel,Pitchbend12Config}
    accessor! {@mut backend_config_mut <>  , X,  M, L, BackendConfigLevel, Pitchbend12Config}

    accessor! {harmony <T:IntervalBasis> ,  X, M, L, HarmonyLevel, Option<Harmony<T>>}
    accessor! {@mut harmony_mut <T:IntervalBasis>  , X,  M, L, HarmonyLevel, Option<Harmony<T>>}

    #[inline]
    pub fn active_strategy<R, T>(
        self,
        mut f: impl FnMut(&StrategyConfig<T>, OrderedLocks<X, M, Succ<ActiveStrategyIndexLevel>>) -> R,
    ) -> (R, Self)
    where
        T: IntervalBasis,
        M: Access<ActiveStrategyIndexLevel, usize>
            + Access<StrategyConfigLevel, Vec<StrategyConfig<T>>>,
        L: AtMost<StrategyConfigLevel>,
        X: ReadAllowed<StrategyConfigLevel> + ReadAllowed<ActiveStrategyIndexLevel>,
    {
        self.strategy_config(|conf, r| r.active_strategy_index(|i, s| f(&conf[*i], s)).0)
    }

    #[inline]
    pub fn active_strategy_mut<R, T>(
        self,
        mut f: impl FnMut(
            &mut StrategyConfig<T>,
            OrderedLocks<X, M, Succ<ActiveStrategyIndexLevel>>,
        ) -> R,
    ) -> (R, Self)
    where
        T: IntervalBasis,
        M: Access<ActiveStrategyIndexLevel, usize>
            + AccessMut<StrategyConfigLevel, Vec<StrategyConfig<T>>>,
        L: AtMost<StrategyConfigLevel>,
        X: WriteAllowed<StrategyConfigLevel> + ReadAllowed<ActiveStrategyIndexLevel>,
    {
        self.strategy_config_mut(|conf, r| r.active_strategy_index(|i, s| f(&mut conf[*i], s)).0)
    }

    #[inline]
    pub fn lowest_sounding_key(mut self) -> (Option<usize>, Self)
    where
        M: IndexedAccess<KeyStateLevel, usize, KeyState>,
        L: AtMost<KeyStateLevel>,
        X: ReadAllowed<KeyStateLevel>,
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
        mut f: impl FnMut(usize, &KeyState, OrderedLocks<X, M, Succ<KeyStateLevel>>),
    ) -> Self
    where
        M: IndexedAccess<KeyStateLevel, usize, KeyState>,
        L: AtMost<KeyStateLevel>,
        X: ReadAllowed<KeyStateLevel>,
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
        X: ReadAllowed<KeyStateLevel>,
    {
        let mut res = vec![];
        for i in 0..128 {
            (_, self) = self.key_state(i, |k, _| {
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
        mut f: impl FnMut(usize, &StackWithTuning<T>, OrderedLocks<X, M, Succ<TuningStateLevel>>),
    ) -> Self
    where
        T: IntervalBasis,
        M: IndexedAccess<KeyStateLevel, usize, KeyState>
            + IndexedAccess<TuningStateLevel, usize, StackWithTuning<T>>,
        L: AtMost<KeyStateLevel>,
        X: ReadAllowed<KeyStateLevel> + ReadAllowed<TuningStateLevel>,
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
    pub fn for_all_sounding_tunings_mut<T>(
        mut self,
        mut f: impl FnMut(usize, &mut StackWithTuning<T>, OrderedLocks<X, M, Succ<TuningStateLevel>>),
    ) -> Self
    where
        T: IntervalBasis,
        M: IndexedAccess<KeyStateLevel, usize, KeyState>
            + IndexedAccessMut<TuningStateLevel, usize, StackWithTuning<T>>,
        L: AtMost<KeyStateLevel>,
        X: ReadAllowed<KeyStateLevel> + WriteAllowed<TuningStateLevel>,
    {
        for i in 0..128 {
            (_, self) = self.key_state(i, |k, r| {
                if k.is_sounding() {
                    r.tuning_mut(i, |t, r| f(i, t, r));
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
            OrderedLocks<X, M, Succ<TuningStateLevel>>,
        ) -> bool,
    ) -> (bool, Self)
    where
        T: IntervalBasis,
        M: IndexedAccess<KeyStateLevel, usize, KeyState>
            + IndexedAccess<TuningStateLevel, usize, StackWithTuning<T>>,
        L: AtMost<KeyStateLevel>,
        X: ReadAllowed<KeyStateLevel> + ReadAllowed<TuningStateLevel>,
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
            OrderedLocks<X, M, Succ<TuningStateLevel>>,
        ) -> R,
    ) -> (R, Self)
    where
        T: IntervalBasis,
        M: IndexedAccess<TuningStateLevel, usize, StackWithTuning<T>>,
        L: AtMost<TuningStateLevel>,
        X: ReadAllowed<TuningStateLevel>,
    {
        self.ith_indexed_pair(i, j, |x, y, r| f(x, y, r))
    }

    #[inline]
    pub fn scales<R, T>(
        self,
        mut f: impl FnMut(
            Option<&[Named<SomeCompleteNeighbourhood<T>>]>,
            OrderedLocks<X, M, Succ<ActiveStrategyIndexLevel>>,
        ) -> R,
    ) -> (R, Self)
    where
        T: IntervalBasis,
        M: Access<ActiveStrategyIndexLevel, usize>
            + Access<StrategyConfigLevel, Vec<StrategyConfig<T>>>,
        L: AtMost<StrategyConfigLevel>,
        X: ReadAllowed<StrategyConfigLevel> + ReadAllowed<ActiveStrategyIndexLevel>,
    {
        self.active_strategy(|strat, r| match strat {
            StrategyConfig::StaticNeighbourhoods {
                config: StaticNeighbourhoodsConfig { scales, .. },
                ..
            }
            | StrategyConfig::TwoStep {
                melody:
                    MelodyStrategyConfig::StaticNeighbourhoods(StaticNeighbourhoodsAsMelodyConfig {
                        scales,
                        ..
                    }),
                ..
            } => f(Some(scales), r),
        })
    }

    #[inline]
    pub fn initial_scale_reference_mut<R, T>(
        self,
        mut f: impl FnMut(
            Option<&mut Stack<T>>,
            OrderedLocks<X, M, Succ<ActiveStrategyIndexLevel>>,
        ) -> R,
    ) -> (R, Self)
    where
        T: IntervalBasis,
        M: Access<ActiveStrategyIndexLevel, usize>
            + AccessMut<StrategyConfigLevel, Vec<StrategyConfig<T>>>,
        L: AtMost<StrategyConfigLevel>,
        X: WriteAllowed<StrategyConfigLevel> + ReadAllowed<ActiveStrategyIndexLevel>,
    {
        self.active_strategy_mut(|strat, r| match strat {
            StrategyConfig::StaticNeighbourhoods {
                config:
                    StaticNeighbourhoodsConfig {
                        initial_reference, ..
                    },
                ..
            }
            | StrategyConfig::TwoStep {
                melody:
                    MelodyStrategyConfig::StaticNeighbourhoods(StaticNeighbourhoodsAsMelodyConfig {
                        initial_reference,
                        ..
                    }),
                ..
            } => f(Some(initial_reference), r),
        })
    }
}

/// Helper function to apply the accessors in this module to the `Option<...>`. This makes it possible
/// to have a struct field with an adaptor (wrapped in `Option`).
///
/// Panics if `a` is `None`.
#[inline]
pub fn take_replace<A, R>(a: &mut Option<A>, mut f: impl FnMut(A) -> (R, A)) -> R {
    let (res, a_new) = f(a.take().unwrap());
    *a = Some(a_new);
    res
}
