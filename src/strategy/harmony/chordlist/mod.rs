use std::{ops::Deref, time::Instant};

use serde_derive::{Deserialize, Serialize};

use crate::{
    adaptors::ViewKeyStates,
    bindable::BindableStrategyAction,
    config::{HarmonyStrategyConfig, IsHarmonyStrategyConfig},
    interval::{
        stack::Stack,
        stacktype::r#trait::{IntervalBasis, StackCoeff, StackType},
    },
    keystate::KeyState,
    msg::{ToChordList, ToHarmony},
    neighbourhood::SomeNeighbourhood,
    strategy::harmony::r#trait::{HarmonyResult, HarmonyStrategy, HarmonyStrategyAdaptor},
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
pub fn blocks_from_current(
    block_sizes: &[usize],
    adaptor: &impl ViewKeyStates,
    lowest_sounding: usize,
) -> Vec<Vec<u8>> {
    let mut encountered = [false; 12];
    let mut blocks = vec![];
    let mut i = 0;
    for &n in block_sizes {
        let mut block = vec![];
        while i < 128 && block.len() < n {
            if adaptor.key_state(i).is_sounding() {
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
        if adaptor.key_state(i).is_sounding() {
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

pub trait ChordListAdaptor<T: StackType>: HarmonyStrategyAdaptor<T> {
    /// This function is allowed to be not extremely fast; it's only called in situations where we
    /// want to reload (parts of) the configuration.
    fn config(&self) -> impl Deref<Target = ChordListConfig<T>>;
}

impl<T: StackType, A: ChordListAdaptor<T>> HarmonyStrategy<T, A> for ChordList<T> {
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

    fn start(&mut self, time: Instant, adaptor: &A) -> HarmonyResult {
        self.start_solve(time, adaptor)
    }

    fn stop(&mut self, _time: Instant, _adaptor: &A) {}

    fn reset(&mut self, adaptor: &A) {
        self.enable = adaptor.config().enable;
        self.patterns = adaptor
            .config()
            .patterns
            .iter()
            .map(|p| Pattern::new(&p))
            .collect();
    }

    fn start_solve(&mut self, time: Instant, adaptor: &A) -> HarmonyResult {
        if self.enable {
            self.next_pattern_to_try = 0;
            self.best_fit = (0, Fit::Failed);
            self.solve_start = time;
            self.active_code = active_code(|i| adaptor.key_state(i));
        }
        adaptor.harmony().valid = false;
        HarmonyResult {
            finished: !self.enable,
            progress: false,
        }
    }

    fn step(&mut self, adaptor: &A) -> HarmonyResult {
        if self.next_pattern_to_try >= self.patterns.len() {
            let progress = self.best_fit.1.matches_something();
            adaptor.harmony().valid = progress;
            return HarmonyResult {
                finished: true,
                progress,
            };
        }

        let the_pattern = &self.patterns[self.next_pattern_to_try];

        let fit = the_pattern.key_shape.fit_code(self.active_code);

        let update_harmony = || {
            adaptor
                .harmony()
                .neighbourhood
                .clone_from(&the_pattern.neighbourhood);
            adaptor.harmony().reference = fit.reference() as StackCoeff;
            adaptor.harmony().pattern_index = Some(self.next_pattern_to_try);
            adaptor.harmony().valid = true;
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

    fn filter_to_harmony(msg: ToHarmony) -> Option<Self::Msg> {
        match msg {
            ToHarmony::ChordList(msg) => Some(msg),
        }
    }

    fn receive_msg(&mut self, msg: Self::Msg, adaptor: &A) -> Option<Instant> {
        match msg {
            ToChordList::ToggleEnable { time } => {
                self.enable = !self.enable;
                Some(time)
            }
            ToChordList::ChordListAction { list_action, time } => {
                list_action.apply_to_no_select(&mut self.patterns, |x| x.clone());
                Some(time)
            }
            ToChordList::UpdateChord { index, time } => {
                self.patterns[index].update_from_config(&adaptor.config().patterns[index]);
                Some(time)
            }
            ToChordList::PushNewChord { time } => {
                self.patterns
                    .push(Pattern::new(adaptor.config().patterns.last().unwrap()));
                Some(time)
            }
        }
    }

    fn handle_bound_action(
        &mut self,
        action: BindableStrategyAction,
        time: Instant,
        adaptor: &A,
    ) -> Option<Instant> {
        match action {
            BindableStrategyAction::Reset => {
                self.stop(time, adaptor);
                self.reset(adaptor);
                Some(time)
                // returning this will make sure that we call
                // self.start(time, adaptor)
                // next
            }
            _ => None {},
        }
    }
}
