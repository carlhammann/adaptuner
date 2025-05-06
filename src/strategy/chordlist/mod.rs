use std::time::Instant;

use crate::{
    interval::stacktype::r#trait::StackType,
    keystate::KeyState,
    neighbourhood::Neighbourhood,
    strategy::twostep::{IntervalSolution, IntervalStrategy},
};

mod pattern;
use pattern::*;

pub struct Chordlist<T: StackType> {
    patterns: Vec<Pattern<T>>,
}

impl HasActivationStatus for KeyState {
    fn active(&self) -> bool {
        self.is_sounding()
    }
}

impl<T: StackType> IntervalStrategy<T> for Chordlist<T> {
    fn solve(&mut self, keys: &[crate::keystate::KeyState; 128]) -> Option<IntervalSolution<T>> {
        let mut pattern_iter = self.patterns.iter();

        let p = pattern_iter.next().expect("empty chord list");
        let mut fit = p.fit(keys);
        let mut index = 0;

        let mut i = 1;
        for p in pattern_iter {
            if fit.is_complete() {
                break;
            }
            let new_fit = p.fit(keys);
            if new_fit.is_better_than(&fit) {
                fit = new_fit;
                index = i;
            }
            i += 1;
        }

        if !fit.is_complete() {
            return None {};
        }

        let best_neighbourhood = &self.patterns[index].neighbourhood;
        let mut intervals = vec![];
        let reference = fit.reference;

        for (note, state) in keys.iter().enumerate().rev() {
            if state.is_sounding() {
                intervals.push((
                    note as u8,
                    best_neighbourhood
                        .try_get_relative_stack(note as i8 - reference as i8)
                        .expect("fit was complete, but note not found in neighbourhood"),
                ));
            }
        }

        Some(IntervalSolution {
            intervals,
            reference,
        })
    }
}
