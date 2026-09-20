use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};

use eframe::egui;

use crate::{
    config::{HarmonyStrategyConfig, StrategyConfig},
    gui::r#trait::{GuiShow, ReceiveToUiRef, UiAdaptor},
    interval::{
        base::Semitones,
        fundamental::{fundamental_or_overtone, HasFundamental, HasOvertone},
        stack::Stack,
        stacktype::r#trait::{StackCoeff, StackType},
    },
    msg::ToUi,
    neighbourhood::CompleteNeighbourhood,
    notename::{HasNoteNames, NoteNameStyle},
    strategy::harmony::{chordlist::ChordListConfig, r#trait::Harmony},
    util::ordered_locks::Zero,
};

pub struct Notifications<T: StackType> {
    harmony: (HarmonyNotification<T>, Instant),
    reference: (bool, Instant),
    scale_index: (Option<usize>, bool, Instant),
    detuned_notes: VecDeque<(u8, Semitones, Semitones, &'static str, Instant)>,
    cleanup_time: Duration,
}

enum HarmonyNotification<T: StackType> {
    None,
    MatchedChord {
        // The reference might be unknown, if no currently active scale can be determined.
        m_reference: Option<Stack<T>>,
        pattern_index: usize,
    },
    SpringSolution {
        // The reference might be unknown, if no currently active scale can be determined.
        m_reference: Option<Stack<T>>,
        number_of_tries: u64,
        is_utonal: bool,
        relaxed: bool,
    },
}

impl<T: StackType> HarmonyNotification<T> {
    fn is_some(&self) -> bool {
        match self {
            HarmonyNotification::None => false,
            _ => true,
        }
    }
}

impl<T: StackType + HasNoteNames> Notifications<T> {
    pub fn new() -> Self {
        Self {
            harmony: (HarmonyNotification::None, Instant::now()),
            reference: (false, Instant::now()),
            scale_index: (None {}, false, Instant::now()),
            detuned_notes: VecDeque::new(),
            cleanup_time: Duration::from_secs(2),
        }
    }

    pub fn clear_old(&mut self, time: Instant) {
        if time.duration_since(self.reference.1) > self.cleanup_time {
            self.reference.0 = false;
        }

        if let (x, true, old) = self.scale_index {
            if time.duration_since(old) > self.cleanup_time {
                self.scale_index = (x, false, time);
            }
        }

        loop {
            if let Some((_, _, _, _, old)) = self.detuned_notes.front() {
                if time.duration_since(*old) > self.cleanup_time {
                    let _ = self.detuned_notes.pop_front();
                } else {
                    break;
                }
            } else {
                break;
            }
        }
    }

    pub fn is_nonempty(&self) -> bool {
        self.harmony.0.is_some()
            || self.reference.0
            || self.scale_index.1
            || !self.detuned_notes.is_empty()
    }
}

impl<T: StackType + HasNoteNames> GuiShow<T> for Notifications<T> {
    fn show(&mut self, ui: &mut egui::Ui, mut adaptor: UiAdaptor<T, Zero>) -> UiAdaptor<T, Zero> {
        if let (Some(scale_index), true, _) = self.scale_index {
            (_, adaptor) = adaptor.scales(|m_scales, _| match m_scales {
                Some(scales) => {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 0.0;
                        ui.label("scale ");
                        ui.strong(&scales[scale_index % scales.len()].name);
                    });
                }
                None {} => {
                    ui.label("no scales for this strategy");
                }
            });
        }

        match &self.harmony.0 {
            HarmonyNotification::None => {}
            HarmonyNotification::MatchedChord {
                m_reference,
                pattern_index,
            } => {
                (_, adaptor) = adaptor.active_strategy(|strat, adaptor| {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 0.0;
                        match strat {
                            StrategyConfig::TwoStep {
                                harmony:
                                    HarmonyStrategyConfig::ChordList(ChordListConfig {
                                        patterns, ..
                                    }),
                                ..
                            } => {
                                ui.strong(&patterns[*pattern_index % patterns.len()].name);
                                if let Some(reference) = m_reference {
                                    ui.label(" on ");
                                    ui.strong(reference.corrected_notename(
                                        &NoteNameStyle::Full,
                                        adaptor.config().use_cent_values,
                                    ));
                                }
                            }
                            _ => {}
                        }
                    });
                });
            }
            HarmonyNotification::SpringSolution {
                m_reference,
                number_of_tries,
                is_utonal,
                relaxed,
            } => {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    if *relaxed {
                        ui.label("relaxed ");
                    }
                    ui.label("spring tuning");
                    if let Some(reference) = m_reference {
                        if *is_utonal {
                            ui.label(" on ");
                        } else {
                            ui.label(" below ");
                        }
                        ui.strong(reference.corrected_notename(
                            &NoteNameStyle::Full,
                            adaptor.config().use_cent_values,
                        ));
                    }
                    if *number_of_tries > 1 {
                        ui.label(format!(" ({number_of_tries} tries)"));
                    } else {
                        ui.label(" (1 try)");
                    }
                });
            }
        }

        if let (true, _) = &self.reference {
            (_, adaptor) = adaptor.reference(|reference, adaptor| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    ui.label("reference ");
                    ui.strong(reference.corrected_notename(
                        &NoteNameStyle::Full,
                        adaptor.config().use_cent_values,
                    ));
                });
            });
        }

        for (note, should_be, actual, explanation, _) in &self.detuned_notes {
            (_, adaptor) = adaptor.tuning(*note as usize, |tuning, adaptor| {
                ui.label(format!(
                "note {} not tuned correctly: should be {should_be:.02}, but is {actual:.02}: {explanation}",
                    tuning
                    .stack
                    .corrected_notename(&NoteNameStyle::Full, adaptor.config().use_cent_values,),
            ));
            });
        }

        adaptor
    }
}

impl<T: StackType + HasFundamental + HasOvertone> ReceiveToUiRef<T> for Notifications<T> {
    fn receive_to_ui_ref(
        &mut self,
        msg: &ToUi<T>,
        mut adaptor: UiAdaptor<T, Zero>,
    ) -> UiAdaptor<T, Zero> {
        match msg {
            ToUi::UpdateReference {} => {
                self.reference = (true, Instant::now());
            }
            ToUi::SelectScale { index } => {
                self.scale_index = (Some(*index), true, Instant::now());
            }
            ToUi::DetunedNote {
                note,
                should_be,
                actual,
                explanation,
            } => {
                self.detuned_notes.push_back((
                    *note,
                    *should_be,
                    *actual,
                    explanation,
                    Instant::now(),
                ));
            }
            ToUi::UpdateHarmony => {
                (_, adaptor) = adaptor.harmony(|harmony, adaptor| match harmony {
                    Harmony::None => self.harmony = (HarmonyNotification::None, Instant::now()),
                    Harmony::SpringSolution {
                        lowest_key,
                        number_of_tries,
                        neighbourhood,
                        relaxed,
                    } => {
                        let (is_utonal, reference_offset_stack) =
                            fundamental_or_overtone(&neighbourhood);
                        let reference_key =
                            *lowest_key as StackCoeff + reference_offset_stack.key_distance();
                        if let Some(scale_index) = self.scale_index.0 {
                            adaptor.scales(|m_scales, adaptor| match m_scales {
                                Some(scales) => {
                                    adaptor.reference(|adaptor_reference, _| {
                                        self.harmony = (
                                            HarmonyNotification::SpringSolution {
                                                m_reference: Some(
                                                    scales[scale_index].named.get_absolute_stack(
                                                        reference_key as StackCoeff,
                                                        adaptor_reference,
                                                    ),
                                                ),
                                                number_of_tries: *number_of_tries,
                                                is_utonal,
                                                relaxed: *relaxed,
                                            },
                                            Instant::now(),
                                        )
                                    });
                                }
                                None {} => {
                                    self.harmony = (
                                        HarmonyNotification::SpringSolution {
                                            m_reference: None {},
                                            number_of_tries: *number_of_tries,
                                            is_utonal,
                                            relaxed: *relaxed,
                                        },
                                        Instant::now(),
                                    );
                                }
                            });
                        } else {
                            self.harmony = (
                                HarmonyNotification::SpringSolution {
                                    m_reference: None {},
                                    number_of_tries: *number_of_tries,
                                    is_utonal,
                                    relaxed: *relaxed,
                                },
                                Instant::now(),
                            );
                        }
                    }
                    Harmony::MatchedChord {
                        pattern_index,
                        reference_key,
                        ..
                    } => {
                        if let Some(scale_index) = self.scale_index.0 {
                            adaptor.scales(|m_scales, adaptor| match m_scales {
                                Some(scales) => {
                                    adaptor.reference(|adaptor_reference, _| {
                                        let reference_stack = scales[scale_index]
                                            .named
                                            .get_absolute_stack(*reference_key, adaptor_reference);
                                        self.harmony = (
                                            HarmonyNotification::MatchedChord {
                                                m_reference: Some(reference_stack),
                                                pattern_index: *pattern_index,
                                            },
                                            Instant::now(),
                                        )
                                    });
                                }
                                None {} => {
                                    self.harmony = (
                                        HarmonyNotification::MatchedChord {
                                            m_reference: None {},
                                            pattern_index: *pattern_index,
                                        },
                                        Instant::now(),
                                    );
                                }
                            });
                        } else {
                            self.harmony = (
                                HarmonyNotification::MatchedChord {
                                    m_reference: None {},
                                    pattern_index: *pattern_index,
                                },
                                Instant::now(),
                            );
                        }
                    }
                });
            }

            ToUi::StartedStrategy(_) => {}
            ToUi::Notify { .. } => {} // this will only contain MIDI parse errors (which shouldn't happen?)
            _ => {}
        }

        adaptor
    }
}
