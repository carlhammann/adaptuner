use std::{rc::Rc, time::Instant};

use serde_derive::{Deserialize, Serialize};

use crate::{
    config::{ExtractConfig, HarmonyStrategyConfig, IsHarmonyStrategyConfig},
    interval::{
        stack::{ScaledAdd, Stack},
        stacktype::r#trait::{IntervalBasis, OctavePeriodicIntervalBasis, StackCoeff, StackType},
    },
    keystate::KeyState,
    msg::{ToChordList, ToHarmony},
    neighbourhood::{Neighbourhood, Partial, PeriodicPartial, SomeNeighbourhood},
    strategy::harmony::r#trait::{Harmony, HarmonyResult, HarmonyStrategy},
    util::readerwriter::{Reader128, ReaderWriter, ReaderWriter128},
};

pub mod keyshape;
use keyshape::{active_code, Fit, HasActivationStatus, KeyShape};

#[derive(Debug, Clone, PartialEq)]
struct Pattern<T: StackType> {
    key_shape: KeyShape,
    neighbourhood: Rc<SomeNeighbourhood<T>>,
    allow_extra_high_notes: bool,
}

impl<T: StackType> Pattern<T> {
    fn new(conf: PatternConfig<T>) -> Self {
        Self {
            key_shape: conf.key_shape,
            neighbourhood: Rc::new(conf.neighbourhood),
            allow_extra_high_notes: conf.allow_extra_high_notes,
        }
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
}

impl<T: IntervalBasis> PatternConfig<T> {
    /// assumes that at least one of the `keys` is sounding.
    pub fn exact_fixed_from_current(
        keys: &[KeyState; 128],
        tunings: &impl Reader128<Stack<T>>,
        lowest_sounding: usize,
        allow_extra_high_notes: bool,
    ) -> Self {
        Self {
            key_shape: KeyShape::ExactFixed {
                keys: keys
                    .iter()
                    .enumerate()
                    .filter(|(_, k)| k.is_sounding())
                    .map(|(i, _)| i as u8)
                    .collect(),
            },
            neighbourhood: SomeNeighbourhood::Partial({
                let mut neigh = Partial::new();
                let mut tmp = Stack::new_zero();
                for (i, stack) in tunings.read_all().iter().enumerate() {
                    if keys[i].is_sounding() {
                        tmp.clone_from(stack);
                        tmp.scaled_add(-1, tunings.read(lowest_sounding));
                        let _ = neigh.insert(&tmp);
                    }
                }
                neigh
            }),
            allow_extra_high_notes,
        }
    }

    pub fn exact_relative_from_current(
        keys: &[KeyState; 128],
        tunings: &impl Reader128<Stack<T>>,
        lowest_sounding: usize,
        allow_extra_high_notes: bool,
    ) -> Self {
        Self {
            key_shape: KeyShape::ExactRelative {
                offsets: keys
                    .iter()
                    .enumerate()
                    .filter(|(_, k)| k.is_sounding())
                    .map(|(i, _)| i as u8 - lowest_sounding as u8)
                    .collect(),
            },
            neighbourhood: SomeNeighbourhood::Partial({
                let mut neigh = Partial::new();
                let mut tmp = Stack::new_zero();
                for (i, stack) in tunings.read_all().iter().enumerate() {
                    if keys[i].is_sounding() {
                        tmp.clone_from(stack);
                        tmp.scaled_add(-1, tunings.read(lowest_sounding));
                        let _ = neigh.insert(&tmp);
                    }
                }
                neigh
            }),
            allow_extra_high_notes,
        }
    }
}

/// Build a [PeriodicPartial] neighbourhood around the lowest sounding note from the other sounding
/// notes.
fn sounding_neighbourhood<T: OctavePeriodicIntervalBasis>(
    keys: &[KeyState; 128],
    tunings: &impl Reader128<Stack<T>>,
    lowest_sounding: usize,
) -> SomeNeighbourhood<T> {
    SomeNeighbourhood::PeriodicPartial({
        let mut neigh = PeriodicPartial::new_from_period_index(T::period_index());
        let mut tmp = Stack::new_zero();
        for (i, stack) in tunings.read_all().iter().enumerate() {
            if keys[i].is_sounding() {
                tmp.clone_from(stack);
                tmp.scaled_add(-1, tunings.read(lowest_sounding));
                let _ = neigh.insert(&tmp);
            }
        }
        neigh
    })
}

fn blocks_from_current(
    block_sizes: &[usize],
    keys: &[KeyState; 128],
    lowest_sounding: usize,
) -> Vec<Vec<u8>> {
    let mut encountered = [false; 12];
    let mut blocks = vec![];
    let mut i = 0;
    for &n in block_sizes {
        let mut block = vec![];
        while i < 128 && block.len() < n {
            if keys[i].is_sounding() {
                let class = (i as isize - lowest_sounding as isize).rem_euclid(12) as usize;
                if !encountered[class] {
                    block.push(class as u8);
                    encountered[class] = true;
                }
            }
            i += 1;
        }
        if !block.is_empty() {
            blocks.push(block);
        }
    }

    let mut last_block = vec![];
    while i < 128 {
        if keys[i].is_sounding() {
            let class = (i as isize - lowest_sounding as isize).rem_euclid(12) as usize;
            if !encountered[class] {
                last_block.push(class as u8);
                encountered[class] = true;
            }
        }
        i += 1;
    }
    if !last_block.is_empty() {
        blocks.push(last_block);
    }

    blocks
}

impl<T: OctavePeriodicIntervalBasis> PatternConfig<T> {
    // In principle, `lowest_sounding` is computable from the `keys` argument. The additional
    // argument thus moves the burden of this check to the caller, which might already know
    // whether there are any notes sounding.
    pub fn classes_relative_from_current(
        keys: &[KeyState; 128],
        tunings: &impl Reader128<Stack<T>>,
        lowest_sounding: usize,
        allow_extra_high_notes: bool,
    ) -> Self {
        Self {
            key_shape: KeyShape::classes_relative_from_current(keys, lowest_sounding),
            neighbourhood: sounding_neighbourhood(keys, tunings, lowest_sounding),
            allow_extra_high_notes,
        }
    }

    pub fn classes_fixed_from_current(
        keys: &[KeyState; 128],
        tunings: &impl Reader128<Stack<T>>,
        lowest_sounding: usize,
        allow_extra_high_notes: bool,
    ) -> Self {
        Self {
            key_shape: KeyShape::classes_fixed_from_current(keys),
            neighbourhood: sounding_neighbourhood(keys, tunings, lowest_sounding),
            allow_extra_high_notes,
        }
    }

    pub fn block_voicing_fixed_from_current(
        block_sizes: &[usize],
        keys: &[KeyState; 128],
        tunings: &impl Reader128<Stack<T>>,
        lowest_sounding: usize,
        allow_extra_high_notes: bool,
    ) -> Self {
        Self {
            key_shape: KeyShape::BlockVoicingFixed {
                blocks: blocks_from_current(block_sizes, keys, lowest_sounding),
            },
            neighbourhood: sounding_neighbourhood(keys, tunings, lowest_sounding),
            allow_extra_high_notes,
        }
    }

    pub fn block_voicing_relative_from_current(
        block_sizes: &[usize],
        keys: &[KeyState; 128],
        tunings: &impl Reader128<Stack<T>>,
        lowest_sounding: usize,
        allow_extra_high_notes: bool,
    ) -> Self {
        Self {
            key_shape: KeyShape::BlockVoicingRelative {
                blocks: blocks_from_current(block_sizes, keys, lowest_sounding),
            },
            neighbourhood: sounding_neighbourhood(keys, tunings, lowest_sounding),
            allow_extra_high_notes,
        }
    }
}

impl<T: StackType> ExtractConfig<PatternConfig<T>> for Pattern<T> {
    fn extract_config(&self) -> PatternConfig<T> {
        let Pattern {
            key_shape,
            neighbourhood,
            allow_extra_high_notes,
        } = self;
        PatternConfig {
            key_shape: key_shape.clone(),
            neighbourhood: (**neighbourhood).clone(),
            allow_extra_high_notes: *allow_extra_high_notes,
        }
    }
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

impl<T: StackType> ExtractConfig<ChordListConfig<T>> for ChordList<T> {
    fn extract_config(&self) -> ChordListConfig<T> {
        ChordListConfig {
            enable: self.enable,
            patterns: self.patterns.iter().map(|p| p.extract_config()).collect(),
        }
    }
}

impl<T: StackType> IsHarmonyStrategyConfig<T> for ChordListConfig<T> {
    type Realized = ChordList<T>;

    fn as_harmony_strategy_config(self) -> HarmonyStrategyConfig<T> {
        HarmonyStrategyConfig::ChordList(self)
    }
}

impl<T: StackType> HarmonyStrategy<T> for ChordList<T> {
    type Config = ChordListConfig<T>;
    type Msg = ToChordList<T>;

    fn new(mut config: ChordListConfig<T>) -> Self {
        Self {
            enable: config.enable,
            patterns: config.patterns.drain(..).map(|p| Pattern::new(p)).collect(),
            next_pattern_to_try: 0,
            best_fit: (0, Fit::Failed),
            solve_start: Instant::now(),
            active_code: 0,
        }
    }

    fn start(
        &mut self,
        time: Instant,
        keys: &impl Reader128<KeyState>,
        harmony: &impl ReaderWriter<Harmony<T>>,
    ) -> HarmonyResult {
        self.start_solve(time, keys, harmony)
    }

    fn stop(
        &mut self,
        _time: Instant,
        _keys: &impl Reader128<KeyState>,
        _harmony: &impl ReaderWriter<Harmony<T>>,
    ) {
    }

    fn start_solve(
        &mut self,
        time: Instant,
        keys: &impl Reader128<KeyState>,
        _harmony: &impl ReaderWriter<Harmony<T>>,
    ) -> HarmonyResult {
        if self.enable {
            self.next_pattern_to_try = 0;
            self.best_fit = (0, Fit::Failed);
            self.solve_start = time;
            self.active_code = active_code(keys.read_all());
        }
        HarmonyResult {
            finished: !self.enable,
            progress: false,
        }
    }

    fn step(
        &mut self,
        _keys: &impl Reader128<KeyState>,
        harmony: &impl ReaderWriter<Harmony<T>>,
    ) -> HarmonyResult {
        if self.next_pattern_to_try >= self.patterns.len() {
            if self.best_fit.1.matches_nothing() {
                return HarmonyResult {
                    finished: true,
                    progress: false,
                };
            } else {
                return HarmonyResult {
                    finished: true,
                    progress: true,
                };
            }
        }

        let the_pattern = &self.patterns[self.next_pattern_to_try];

        let fit = the_pattern.key_shape.fit_code(self.active_code);

        let update_harmony = || {
            harmony
                .write()
                .neighbourhood
                .clone_from(&the_pattern.neighbourhood);
            harmony.write().reference = fit.reference() as StackCoeff;
            harmony.write().pattern_index = Some(self.next_pattern_to_try);
        };

        if fit.is_complete() {
            update_harmony();

            self.best_fit = (self.next_pattern_to_try, fit);
            self.next_pattern_to_try = self.patterns.len(); // we won't look at more patterns.

            return HarmonyResult {
                finished: true,
                progress: true,
            };
        }

        if fit.is_better_than(&self.best_fit.1) {
            update_harmony();

            self.best_fit = (self.next_pattern_to_try, fit);
            self.next_pattern_to_try += 1;

            return HarmonyResult {
                finished: false,
                progress: true,
            };
        }

        self.next_pattern_to_try += 1;
        HarmonyResult {
            finished: false,
            progress: false,
        }
    }

    fn filter_to_harmony(msg: ToHarmony<T>) -> Option<Self::Msg> {
        match msg {
            ToHarmony::ChordList(msg) => Some(msg),
        }
    }

    fn receive_msg(
        &mut self,
        msg: Self::Msg,
        keys: &impl Reader128<KeyState>,
        harmony: &impl ReaderWriter<Harmony<T>>,
    ) -> Option<Instant> {
        let time = match &msg {
            ToChordList::ChordListAction { time, .. }
            | ToChordList::PushNewChord { time, .. }
            | ToChordList::AllowExtraHighNotes { time, .. }
            | ToChordList::ToggleEnable { time, .. } => *time,
        };

        Some(time)
    }
}
