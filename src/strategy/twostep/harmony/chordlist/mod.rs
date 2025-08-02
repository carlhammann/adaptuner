use std::rc::Rc;

use serde_derive::{Deserialize, Serialize};

use crate::{
    config::{ExtractConfig, HarmonyStrategyConfig},
    interval::{
        stack::{ScaledAdd, Stack},
        stacktype::r#trait::{IntervalBasis, StackCoeff, StackType},
    },
    keystate::KeyState,
    neighbourhood::{Neighbourhood, Partial, PeriodicPartial, SomeNeighbourhood},
    strategy::twostep::{Harmony, HarmonyStrategy},
    util::list_action::ListAction,
};

pub mod keyshape;
use keyshape::{Fit, HasActivationStatus, KeyShape};

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

/// build a partial neighbourhood around the current lowest soundign note from the other sounding
/// notes.
///
/// If [IntervalBasis::try_period_index] returns `Some` for `T`, build a [PeriodicPartial],
/// otherwise a [Partial].
fn sounding_neighbourhood<T: IntervalBasis>(
    keys: &[KeyState; 128],
    tunings: &[Stack<T>; 128],
    lowest_sounding: usize,
) -> SomeNeighbourhood<T> {
    if let Some(period_index) = T::try_period_index() {
        SomeNeighbourhood::PeriodicPartial({
            let mut neigh = PeriodicPartial::new_from_period_index(period_index);
            let mut tmp = Stack::new_zero();
            for (i, stack) in tunings.iter().enumerate() {
                if keys[i].is_sounding() {
                    tmp.clone_from(stack);
                    tmp.scaled_add(-1, &tunings[lowest_sounding]);
                    let _ = neigh.insert(&tmp);
                }
            }
            neigh
        })
    } else {
        SomeNeighbourhood::Partial({
            let mut neigh = Partial::new();
            let mut tmp = Stack::new_zero();
            for (i, stack) in tunings.iter().enumerate() {
                if keys[i].is_sounding() {
                    tmp.clone_from(stack);
                    tmp.scaled_add(-1, &tunings[lowest_sounding]);
                    let _ = neigh.insert(&tmp);
                }
            }
            neigh
        })
    }
}

impl<T: StackType> PatternConfig<T> {
    // In principle, `lowest_sounding` is computable from the `keys` argument. The additional
    // argument thus moves the burden of this check to the caller, which might already know
    // whether there are any notes sounding.
    pub fn classes_relative_from_current(
        keys: &[KeyState; 128],
        tunings: &[Stack<T>; 128],
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
        tunings: &[Stack<T>; 128],
        lowest_sounding: usize,
        allow_extra_high_notes: bool,
    ) -> Self {
        Self {
            key_shape: KeyShape::classes_fixed_from_current(keys, lowest_sounding),
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
            allow_extra_high_notes: allow_extra_high_notes,
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
    pub patterns: Vec<PatternConfig<T>>,
}

pub struct ChordList<T: StackType> {
    patterns: Vec<Pattern<T>>,
}

impl<T: StackType> ChordList<T> {
    pub fn new(mut conf: ChordListConfig<T>) -> Self {
        Self {
            patterns: conf.patterns.drain(..).map(|c| Pattern::new(c)).collect(),
        }
    }
}

impl<T: StackType> HarmonyStrategy<T> for ChordList<T> {
    fn solve(&mut self, keys: &[KeyState; 128]) -> (Option<usize>, Option<Harmony<T>>) {
        if self.patterns.is_empty() {
                return (None {}, None {});
        }

        let mut fit = Fit::new_worst();
        let mut index = 0;
        for (i, p) in self.patterns.iter().enumerate() {
            if fit.is_complete() {
                break;
            }
            let new_fit = p.key_shape.fit(keys);
            if new_fit.is_better_than(&fit) {
                fit = new_fit;
                index = i;
            }
        }

        let selected = &self.patterns[index];

        if selected.allow_extra_high_notes {
            if fit.matches_nothing() {
                return (None {}, None {});
            }
        } else if !fit.is_complete() {
            return (None {}, None {});
        }

        (
            Some(index),
            Some(Harmony {
                neighbourhood: selected.neighbourhood.clone(),
                reference: fit.zero as StackCoeff,
            }),
        )
    }

    fn handle_chord_list_action(&mut self, action: ListAction) -> bool {
        let mut dummy = Some(0);
        action.apply_to(|p| p.clone(), &mut self.patterns, &mut dummy);
        true
    }

    fn push_new_chord(&mut self, chord: PatternConfig<T>) -> bool {
        self.patterns.push(Pattern::new(chord));
        true
    }

    fn allow_extra_high_notes(&mut self, pattern_index: usize, allow: bool) {
        self.patterns[pattern_index].allow_extra_high_notes = allow;
    }
}

impl<T: StackType> ExtractConfig<HarmonyStrategyConfig<T>> for ChordList<T> {
    fn extract_config(&self) -> HarmonyStrategyConfig<T> {
        HarmonyStrategyConfig::ChordList(ChordListConfig {
            patterns: self.patterns.iter().map(|p| p.extract_config()).collect(),
        })
    }
}
