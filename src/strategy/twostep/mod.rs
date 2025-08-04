use std::{collections::VecDeque, rc::Rc, time::Instant};

use harmony::chordlist::{ChordList, PatternConfig};

use crate::{
    config::{ExtractConfig, HarmonyStrategyConfig, MelodyStrategyConfig, StrategyConfig},
    interval::{
        base::Semitones,
        stack::Stack,
        stacktype::r#trait::{IntervalBasis, StackCoeff, StackType},
    },
    keystate::KeyState,
    msg::{FromStrategy, ToStrategy},
    neighbourhood::SomeNeighbourhood,
    util::list_action::ListAction,
};

use super::{r#static::StaticTuning, r#trait::Strategy};

pub mod harmony;
pub mod melody;

#[derive(Clone)]
pub struct Harmony<T: IntervalBasis> {
    pub neighbourhood: Rc<SomeNeighbourhood<T>>,
    /// MIDI key number of the reference note, but may be outside the MIDI range
    pub reference: StackCoeff,
}

pub trait HarmonyStrategy<T: IntervalBasis>: ExtractConfig<HarmonyStrategyConfig<T>> {
    fn solve(&mut self, keys: &[KeyState; 128]) -> (Option<usize>, Option<Harmony<T>>);
    fn handle_chord_list_action(&mut self, action: ListAction) -> bool;
    fn push_new_chord(&mut self, chord: PatternConfig<T>) -> bool;
    fn allow_extra_high_notes(&mut self, pattern_index: usize, allow: bool);
    fn enable_chord_list(&mut self, enable: bool);
}

pub trait MelodyStrategy<T: StackType>: ExtractConfig<MelodyStrategyConfig<T>> {
    /// returns a boolean signalling success and an optional stack that is the tuning of the
    /// `harmony.reference`
    fn solve(
        &mut self,
        keys: &[KeyState; 128],
        tunings: &mut [Stack<T>; 128],
        harmony: Option<Harmony<T>>,
        time: Instant,
        forward: &mut VecDeque<FromStrategy<T>>,
    ) -> (bool, Option<Stack<T>>);

    fn handle_msg(
        &mut self,
        keys: &[KeyState; 128],
        tunings: &mut [Stack<T>; 128],
        harmony: Option<Harmony<T>>,
        msg: ToStrategy<T>,
        forward: &mut VecDeque<FromStrategy<T>>,
    ) -> (bool, Option<Stack<T>>);

    fn start(
        &mut self,
        keys: &[KeyState; 128],
        tunings: &mut [Stack<T>; 128],
        harmony: Option<Harmony<T>>,
        time: Instant,
        forward: &mut VecDeque<FromStrategy<T>>,
    ) -> Option<Stack<T>>;

    fn absolute_semitones(&self, stack: &Stack<T>) -> Semitones;
}

pub struct TwoStep<T: StackType> {
    harmony: Box<dyn HarmonyStrategy<T>>,
    melody: Box<dyn MelodyStrategy<T>>,
}

impl<T: StackType> TwoStep<T> {
    pub fn new(
        harmony_config: HarmonyStrategyConfig<T>,
        melody_config: MelodyStrategyConfig<T>,
    ) -> Self {
        Self {
            harmony: match harmony_config {
                HarmonyStrategyConfig::ChordList(c) => Box::new(ChordList::new(c)),
            },
            melody: match melody_config {
                MelodyStrategyConfig::StaticTuning(c) => Box::new(StaticTuning::new(c)),
            },
        }
    }

    fn solve(
        &mut self,
        keys: &[KeyState; 128],
        tunings: &mut [Stack<T>; 128],
        time: Instant,
        forward: &mut VecDeque<FromStrategy<T>>,
    ) -> bool {
        let (pattern_index, harmony) = self.harmony.solve(keys);
        let (success, reference) = self.melody.solve(keys, tunings, harmony, time, forward);
        forward.push_back(FromStrategy::CurrentHarmony {
            pattern_index,
            reference,
        });
        success
    }
}

impl<T: StackType> Strategy<T> for TwoStep<T> {
    fn note_on<'a>(
        &mut self,
        keys: &[KeyState; 128],
        tunings: &'a mut [Stack<T>; 128],
        note: u8,
        time: Instant,
        forward: &mut VecDeque<FromStrategy<T>>,
    ) -> Option<(Semitones, &'a Stack<T>)> {
        if self.solve(keys, tunings, time, forward) {
            let stack = &tunings[note as usize];
            Some((self.melody.absolute_semitones(stack), stack))
        } else {
            None {}
        }
    }

    fn note_off(
        &mut self,
        keys: &[KeyState; 128],
        tunings: &mut [Stack<T>; 128],
        _note: u8,
        time: Instant,
        forward: &mut VecDeque<FromStrategy<T>>,
    ) -> bool {
        self.solve(keys, tunings, time, forward)
    }

    fn handle_msg(
        &mut self,
        keys: &[KeyState; 128],
        tunings: &mut [Stack<T>; 128],
        msg: ToStrategy<T>,
        forward: &mut VecDeque<FromStrategy<T>>,
    ) -> bool {
        match msg {
            ToStrategy::ChordListAction { action, time } => {
                if self.harmony.handle_chord_list_action(action) {
                    self.solve(keys, tunings, time, forward)
                } else {
                    false
                }
            }
            ToStrategy::PushNewChord { pattern, time } => {
                if self.harmony.push_new_chord(pattern) {
                    self.solve(keys, tunings, time, forward)
                } else {
                    false
                }
            }
            ToStrategy::AllowExtraHighNotes {
                pattern_index,
                allow,
                time,
            } => {
                self.harmony.allow_extra_high_notes(pattern_index, allow);
                self.solve(keys, tunings, time, forward)
            }
            ToStrategy::EnableChordList { enable, time } => {
                self.harmony.enable_chord_list(enable);
                self.solve(keys, tunings, time, forward)
            }
            _ => {
                let (pattern_index, harmony) = self.harmony.solve(keys);
                let (success, reference) =
                    self.melody.handle_msg(keys, tunings, harmony, msg, forward);
                forward.push_back(FromStrategy::CurrentHarmony {
                    pattern_index,
                    reference,
                });
                success
            }
        }
    }

    fn start(
        &mut self,
        keys: &[KeyState; 128],
        tunings: &mut [Stack<T>; 128],
        time: Instant,
        forward: &mut VecDeque<FromStrategy<T>>,
    ) {
        let (pattern_index, harmony) = self.harmony.solve(keys);
        let reference = self.melody.start(keys, tunings, harmony, time, forward);
        forward.push_back(FromStrategy::CurrentHarmony {
            pattern_index,
            reference,
        });
    }
}

impl<T: StackType> ExtractConfig<StrategyConfig<T>> for TwoStep<T> {
    fn extract_config(&self) -> StrategyConfig<T> {
        StrategyConfig::TwoStep(self.harmony.extract_config(), self.melody.extract_config())
    }
}
