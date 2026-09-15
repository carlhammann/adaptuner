use std::time::Instant;

use serde_derive::{Deserialize, Serialize};

use crate::{
    adaptors::lock_levels::{ActiveStrategyIndexLevel, KeyStateLevel, StrategyConfigLevel},
    bindable::BindableStrategyAction,
    config::{HarmonyStrategyConfig, IsHarmonyStrategyConfig, StrategyConfig},
    interval::{
        stack::Stack,
        stacktype::r#trait::{IntervalBasis, StackCoeff, StackType},
    },
    keystate::KeyState,
    msg::{ToChordList, ToHarmony},
    neighbourhood::SomeNeighbourhood,
    strategy::harmony::r#trait::{Harmony, HarmonyAdaptor, HarmonyResult, HarmonyStrategy},
    util::ordered_locks::{AtMost, IndexedAccess, OrderedLocks, ReadAllowed, Succ, Zero},
};

pub mod keyshape;
use keyshape::{active_code, Fit, HasActivationStatus, KeyShape};

#[derive(Debug, Clone, PartialEq)]
struct Pattern<T: StackType> {
    key_shape: KeyShape,
    neighbourhood: SomeNeighbourhood<T>,
    allow_extra_high_notes: bool,
}

impl<T: StackType> Pattern<T> {
    fn new(conf: &PatternConfig<T>) -> Self {
        Self {
            key_shape: conf.key_shape.clone(),
            neighbourhood: conf.neighbourhood.clone(),
            allow_extra_high_notes: conf.allow_extra_high_notes,
        }
    }

    fn update_from_config(&mut self, conf: &PatternConfig<T>) {
        self.key_shape.clone_from(&conf.key_shape);
        self.neighbourhood.clone_from(&conf.neighbourhood);
        self.allow_extra_high_notes = conf.allow_extra_high_notes;
    }
}

impl HasActivationStatus for KeyState {
    fn active(&self) -> bool {
        self.is_sounding()
    }
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "kebab-case")]
pub struct PatternConfig<T: IntervalBasis> {
    pub key_shape: KeyShape,
    pub neighbourhood: SomeNeighbourhood<T>,
    pub allow_extra_high_notes: bool,
    pub name: String,
    pub original_reference: Stack<T>,
}

/// Compute blocks for the [KeyShape::BlockVoicingFixed] and [KeyShape::BlockVoicingRelative] from
/// the currently sounding notes.
pub fn blocks_from_current<T, A, L>(
    block_sizes: &[usize],
    mut adaptor: OrderedLocks<T, A, L>,
    lowest_sounding: usize,
) -> (Vec<Vec<u8>>, OrderedLocks<T, A, L>)
where
    A: IndexedAccess<KeyStateLevel, usize, KeyState>,
    L: AtMost<KeyStateLevel>,
    T: ReadAllowed<KeyStateLevel>,
{
    let mut encountered = [false; 12];
    let mut blocks = vec![];
    let mut i = 0;
    for &n in block_sizes {
        let mut block = vec![];
        while i < 128 && block.len() < n {
            (_, adaptor) = adaptor.key_state(i, |key_state, _| {
                if key_state.is_sounding() {
                    let class = (i as isize - lowest_sounding as isize).rem_euclid(12) as usize;
                    if !encountered[class] {
                        block.push(class as u8);
                        encountered[class] = true;
                    }
                }
                i += 1;
            });
        }
        if !block.is_empty() {
            blocks.push(block);
        }
    }

    let mut last_block = vec![];
    while i < 128 {
        (_, adaptor) = adaptor.key_state(i, |key_state, _| {
            if key_state.is_sounding() {
                let class = (i as isize - lowest_sounding as isize).rem_euclid(12) as usize;
                if !encountered[class] {
                    last_block.push(class as u8);
                    encountered[class] = true;
                }
            }
            i += 1;
        });
    }
    if !last_block.is_empty() {
        blocks.push(last_block);
    }

    (blocks, adaptor)
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "kebab-case")]
pub struct ChordListConfig<T: IntervalBasis> {
    pub enable: bool,
    pub patterns: Vec<PatternConfig<T>>,
}

pub struct ChordList<T: StackType> {
    enable: bool,
    patterns: Vec<Pattern<T>>,
    next_pattern_to_try: usize,
    best_fit: (usize, Fit),
    solve_start: Instant,
    active_code: u128,
}

impl<T: StackType> IsHarmonyStrategyConfig<T> for ChordListConfig<T> {
    fn as_harmony_strategy_config(self) -> HarmonyStrategyConfig<T> {
        HarmonyStrategyConfig::ChordList(self)
    }
}

impl<T: StackType, L: AtMost<StrategyConfigLevel>> HarmonyAdaptor<T, ChordList<T>, L> {
    pub fn config<R>(
        self,
        mut f: impl FnMut(
            &ChordListConfig<T>,
            HarmonyAdaptor<T, ChordList<T>, Succ<ActiveStrategyIndexLevel>>,
        ) -> R,
    ) -> (R, Self) {
        self.active_strategy(|strat, adaptor| match strat {
            StrategyConfig::TwoStep {
                harmony: HarmonyStrategyConfig::ChordList(conf),
                ..
            } => f(conf, adaptor),
            _ => panic!(
                "config() method on HarmonyAdaptor: expected ChordListConfig, got something else"
            ),
        })
    }
}

impl<T: StackType> HarmonyStrategy<T> for ChordList<T> {
    type Config = ChordListConfig<T>;
    type Msg = ToChordList;

    fn new(mut config: ChordListConfig<T>) -> Self {
        Self {
            enable: config.enable,
            patterns: config
                .patterns
                .drain(..)
                .map(|p| Pattern::new(&p))
                .collect(),
            next_pattern_to_try: 0,
            best_fit: (0, Fit::Failed),
            solve_start: Instant::now(),
            active_code: 0,
        }
    }

    fn start(
        &mut self,
        time: Instant,
        adaptor: HarmonyAdaptor<T, Self, Zero>,
    ) -> (HarmonyResult, HarmonyAdaptor<T, Self, Zero>) {
        self.start_solve(time, adaptor)
    }

    fn stop(
        &mut self,
        _time: Instant,
        adaptor: HarmonyAdaptor<T, Self, Zero>,
    ) -> HarmonyAdaptor<T, Self, Zero> {
        adaptor
    }

    fn start_solve(
        &mut self,
        time: Instant,
        mut adaptor: HarmonyAdaptor<T, Self, Zero>,
    ) -> (HarmonyResult, HarmonyAdaptor<T, Self, Zero>) {
        if self.enable {
            self.next_pattern_to_try = 0;
            self.best_fit = (0, Fit::Failed);
            self.solve_start = time;
            (self.active_code, adaptor) = active_code(adaptor);
        }
        (_, adaptor) = adaptor.harmony_mut(|h, _| {
            if let Some(h) = h {
                h.valid = false
            }
        });
        (
            HarmonyResult {
                finished: !self.enable,
                progress: false,
            },
            adaptor,
        )
    }

    fn step(
        &mut self,
        mut adaptor: HarmonyAdaptor<T, Self, Zero>,
    ) -> (HarmonyResult, HarmonyAdaptor<T, Self, Zero>) {
        if self.next_pattern_to_try >= self.patterns.len() {
            let progress = self.best_fit.1.matches_something();

            (_, adaptor) = adaptor.harmony_mut(|h, _| {
                if let Some(h) = h {
                    h.valid = progress
                }
            });
            return (
                HarmonyResult {
                    finished: true,
                    progress,
                },
                adaptor,
            );
        }

        let the_pattern = &self.patterns[self.next_pattern_to_try];

        let fit = the_pattern.key_shape.fit_code(self.active_code);

        let update_harmony = |mut adaptor: HarmonyAdaptor<T, Self, Zero>| {
            (_, adaptor) = adaptor.harmony_mut(|h, _| {
                if let Some(h) = h {
                    h.neighbourhood.clone_from(&the_pattern.neighbourhood);
                    h.reference_key = fit.reference() as StackCoeff;
                    h.pattern_index = Some(self.next_pattern_to_try);
                    h.valid = true;
                } else {
                    *h = Some(Harmony {
                        neighbourhood: the_pattern.neighbourhood.clone(),
                        reference_key: fit.reference() as StackCoeff,
                        pattern_index: Some(self.next_pattern_to_try),
                        valid: true,
                    });
                }
            });
            adaptor
        };

        if fit.is_complete() {
            adaptor = update_harmony(adaptor);

            self.best_fit = (self.next_pattern_to_try, fit);
            self.next_pattern_to_try = self.patterns.len(); // we won't look at more patterns.

            return (
                HarmonyResult {
                    finished: true,
                    progress: true,
                },
                adaptor,
            );
        }

        if fit.is_better_than(&self.best_fit.1) {
            adaptor = update_harmony(adaptor);

            self.best_fit = (self.next_pattern_to_try, fit);
            self.next_pattern_to_try += 1;

            return (
                HarmonyResult {
                    finished: false,
                    progress: true,
                },
                adaptor,
            );
        }

        self.next_pattern_to_try += 1;
        (
            HarmonyResult {
                finished: false,
                progress: false,
            },
            adaptor,
        )
    }

    fn filter_to_harmony(msg: ToHarmony) -> Option<Self::Msg> {
        match msg {
            ToHarmony::ChordList(msg) => Some(msg),
            _ => None {},
        }
    }

    fn receive_msg(
        &mut self,
        msg: Self::Msg,
        mut adaptor: HarmonyAdaptor<T, Self, Zero>,
    ) -> (Option<Instant>, HarmonyAdaptor<T, Self, Zero>) {
        match msg {
            ToChordList::ToggleEnable { time } => {
                self.enable = !self.enable;
                (Some(time), adaptor)
            }
            ToChordList::ChordListAction { list_action, time } => {
                list_action.apply_to_no_select(&mut self.patterns, |x| x.clone());
                (Some(time), adaptor)
            }
            ToChordList::UpdateChord { index, time } => {
                (_, adaptor) = adaptor.config(|conf, _| {
                    self.patterns[index].update_from_config(&conf.patterns[index]);
                });
                (Some(time), adaptor)
            }
            ToChordList::PushNewChord { time } => {
                (_, adaptor) = adaptor.config(|conf, _| {
                    self.patterns
                        .push(Pattern::new(conf.patterns.last().unwrap()))
                });
                (Some(time), adaptor)
            }
        }
    }

    fn handle_bound_action(
        &mut self,
        action: BindableStrategyAction,
        _time: Instant,
        adaptor: HarmonyAdaptor<T, Self, Zero>,
    ) -> (Option<Instant>, HarmonyAdaptor<T, Self, Zero>) {
        match action {
            _ => (None {}, adaptor),
        }
    }
}
