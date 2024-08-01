use std::{
    mem::MaybeUninit,
    sync::{mpsc, Arc},
    time::Instant,
};

use midi_msg::{Channel, ChannelVoiceMsg, ControlChange, MidiMsg};

use crate::{
    config::r#trait::Config,
    interval::stacktype::r#trait::StackCoeff,
    interval::{interval::Semitones, stack::Stack, stacktype::r#trait::StackType},
    msg,
    neighbourhood::Neighbourhood,
    pattern::*,
    process::r#trait::ProcessState,
};

struct NoteWithInfo<T: StackType> {
    active: bool,
    sustained: bool,
    channel: Channel,
    tuning: Semitones,
    tuning_stack: Stack<T>,
}

impl<T: StackType> Clone for NoteWithInfo<T> {
    fn clone(&self) -> Self {
        NoteWithInfo {
            active: self.active,
            sustained: self.sustained,
            channel: self.channel,
            tuning: self.tuning,
            tuning_stack: self.tuning_stack.clone(),
        }
    }
}

pub struct TuningFrame<T: StackType> {
    initial_reference_key: i8,
    reference_key: i8,
    reference_stack: Stack<T>,

    neighbourhood: Neighbourhood<T>,
}

impl<T: StackType> TuningFrame<T> {
    fn write_tuning_stack(&self, target: &mut Stack<T>, key: i8) -> bool {
        if self
            .neighbourhood
            .write_relative_stack_for_key_offset(target, key - self.reference_key)
        {
            target.add_mul(1, &self.reference_stack);
            true
        } else {
            false
        }
    }

    fn calculate_tuning_stack(&self, key: i8) -> Option<Stack<T>> {
        match self
            .neighbourhood
            .relative_stack_for_key_offset(key - self.reference_key)
        {
            None => None,
            Some(mut the_stack) => {
                the_stack.add_mul(1, &self.reference_stack);
                Some(the_stack)
            }
        }
    }

    fn tuning_from_stack(&self, stack: &Stack<T>) -> Semitones {
        stack.semitones() + self.initial_reference_key as Semitones
    }
}

pub struct Walking<T: StackType> {
    config: WalkingConfig<T>,

    tuningframe: TuningFrame<T>,

    active_notes: [NoteWithInfo<T>; 128],

    // active_temperaments: Vec<bool>,
    sustain: [bool; 16],

    patterns: Vec<Pattern<T>>,

    /// whether the neighbourhood should be updated with the (possibly partial) neighbourhoods from the
    /// currently matched pattern
    consider_played: bool,
}

impl<T: StackType> HasActivationStatus for NoteWithInfo<T> {
    fn active(&self) -> bool {
        self.active
    }
}

impl<T: StackType> Walking<T> {
    fn recompute_fit(&mut self) -> (usize, Fit) {
        let mut best_index = 0;
        let mut best_fit = self.patterns[0].fit(&self.active_notes);
        for i in 1..self.patterns.len() {
            let fit = self.patterns[i].fit(&self.active_notes);
            if fit.is_better_than(&best_fit) {
                best_fit = fit;
                best_index = i;
            }
        }
        (best_index, best_fit)
    }

    fn update_all_tunings(
        &mut self,
        time: Instant,
        to_backend: &mpsc::Sender<(Instant, msg::AfterProcess<T>)>,
    ) {
        let send_to_backend =
            |msg: msg::AfterProcess<T>, time: Instant| to_backend.send((time, msg)).unwrap_or(());
        for (i, note) in self.active_notes.iter_mut().enumerate() {
            if note.active {
                self.tuningframe
                    .write_tuning_stack(&mut note.tuning_stack, i as i8);
                note.tuning = self.tuningframe.tuning_from_stack(&note.tuning_stack);
                send_to_backend(
                    msg::AfterProcess::Retune {
                        note: i as u8,
                        tuning: note.tuning,
                        tuning_stack: note.tuning_stack.clone(),
                    },
                    time,
                );
            }
        }
    }

    fn start(&mut self, time: Instant, to_backend: &mpsc::Sender<(Instant, msg::AfterProcess<T>)>) {
    }

    fn stop(&mut self, time: Instant, to_backend: &mpsc::Sender<(Instant, msg::AfterProcess<T>)>) {}

    fn reset(&mut self, time: Instant, to_backend: &mpsc::Sender<(Instant, msg::AfterProcess<T>)>) {
        let send_to_backend =
            |msg: msg::AfterProcess<T>, time: Instant| to_backend.send((time, msg)).unwrap_or(());

        send_to_backend(msg::AfterProcess::Reset, time);
        *self = WalkingConfig::initialise(&self.config);
        self.tuningframe.neighbourhood.for_each_stack(|_, stack| {
            send_to_backend(
                msg::AfterProcess::Consider {
                    stack: stack.clone(),
                },
                time,
            );
        });
    }

    fn incoming_midi(
        &mut self,
        time: Instant,
        bytes: &[u8],
        to_backend: &mpsc::Sender<(Instant, msg::AfterProcess<T>)>,
    ) {
        let send_to_backend =
            |msg: msg::AfterProcess<T>, time: Instant| to_backend.send((time, msg)).unwrap_or(());

        match MidiMsg::from_midi(bytes) {
            Err(err) => send_to_backend(msg::AfterProcess::MidiParseErr(err.to_string()), time),
            Ok((msg, _nbtyes)) => match msg {
                MidiMsg::ChannelVoice {
                    channel,
                    msg:
                        ChannelVoiceMsg::NoteOn {
                            note: new_key_number,
                            velocity,
                        },
                } => {
                    // set the active status of the new note, the tuning data may be set again,
                    // depending on the new pattern below.
                    self.active_notes[new_key_number as usize].active = true;
                    self.active_notes[new_key_number as usize].channel = channel;
                    self.active_notes[new_key_number as usize].sustained = false;
                    self.tuningframe.write_tuning_stack(
                        &mut self.active_notes[new_key_number as usize].tuning_stack,
                        new_key_number as i8,
                    );
                    self.active_notes[new_key_number as usize].tuning =
                        self.tuningframe.tuning_from_stack(
                            &self.active_notes[new_key_number as usize].tuning_stack,
                        );

                    let (pattern_index, fit) = self.recompute_fit();
                    if fit.is_at_least_partial() {
                        let fit_reference_stack = self
                            .tuningframe
                            .calculate_tuning_stack(fit.reference as i8)
                            .unwrap(); // this is OK, I'm expecting the neighbourhood to be always complete
                        let extra_neighbourhood = &self.patterns[pattern_index].neighbourhood;
                        if self.consider_played {
                            self.tuningframe.neighbourhood.extend_with_constant_offset(
                                &fit_reference_stack,
                                extra_neighbourhood,
                                |stack| {
                                    send_to_backend(
                                        msg::AfterProcess::Consider {
                                            stack: stack.clone(),
                                        },
                                        time,
                                    );
                                },
                            );
                            self.update_all_tunings(time, to_backend);
                        } else {
                            // only update the tunings of the notes from the fit
                            for (key_number, note) in self.active_notes.iter_mut().enumerate() {
                                if note.active {
                                    match extra_neighbourhood.relative_stack_for_key_offset(
                                        key_number as i8 - fit.reference as i8,
                                    ) {
                                        None => {}
                                        Some(mut tuning_stack) => {
                                            tuning_stack.add_mul(1, &fit_reference_stack);
                                            let tuning =
                                                self.tuningframe.tuning_from_stack(&tuning_stack);

                                            note.tuning = tuning;
                                            note.tuning_stack.clone_from(&tuning_stack);

                                            if key_number as u8 != new_key_number {
                                                send_to_backend(
                                                    msg::AfterProcess::Retune {
                                                        note: key_number as u8,
                                                        tuning,
                                                        tuning_stack,
                                                    },
                                                    time,
                                                );
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    send_to_backend(
                        msg::AfterProcess::TunedNoteOn {
                            channel,
                            note: new_key_number,
                            velocity,
                            tuning: self.active_notes[new_key_number as usize].tuning,
                            tuning_stack: self.active_notes[new_key_number as usize]
                                .tuning_stack
                                .clone(),
                        },
                        time,
                    );
                }

                MidiMsg::ChannelVoice {
                    channel,
                    msg: ChannelVoiceMsg::NoteOff { note, velocity },
                } => {
                    if self.sustain[channel as usize] {
                        self.active_notes[note as usize].sustained = true;
                    } else {
                        self.active_notes[note as usize].active = false;
                        self.active_notes[note as usize].sustained = false;
                        // deactivate the note
                        // recompute the reference and update tunings of active notes
                    }
                    send_to_backend(
                        msg::AfterProcess::NoteOff {
                            channel,
                            note,
                            velocity,
                            held_by_sustain: self.sustain[channel as usize],
                        },
                        time,
                    );
                }

                MidiMsg::ChannelVoice {
                    channel,
                    msg:
                        ChannelVoiceMsg::ControlChange {
                            control: ControlChange::Hold(value),
                        },
                } => {
                    self.sustain[channel as usize] = value != 0;
                    if value == 0 {
                        // deactivate all notes on this channel that are only held by sustain
                        for note in &mut self.active_notes {
                            if note.channel == channel {
                                note.active = false;
                                note.sustained = false;
                            }
                        }
                        // recompute the reference and update tunings of active notes
                    }
                    send_to_backend(msg::AfterProcess::Sustain { channel, value }, time);
                }

                MidiMsg::ChannelVoice {
                    channel,
                    msg: ChannelVoiceMsg::ProgramChange { program },
                } => {
                    send_to_backend(msg::AfterProcess::ProgramChange { channel, program }, time);
                }

                _ => {
                    send_to_backend(msg::AfterProcess::ForwardMidi { msg }, time);
                }
            },
        }
    }

    fn consider(
        &mut self,
        time: Instant,
        coefficients: Vec<StackCoeff>,
        to_backend: &mpsc::Sender<(Instant, msg::AfterProcess<T>)>,
    ) {
        todo!()
    }

    fn toggle_temperament(
        &mut self,
        time: Instant,
        index: usize,
        to_backend: &mpsc::Sender<(Instant, msg::AfterProcess<T>)>,
    ) {
        todo!()
    }
}

impl<T: StackType> ProcessState<T> for Walking<T> {
    fn handle_msg(
        &mut self,
        time: Instant,
        msg: msg::ToProcess,
        to_backend: &mpsc::Sender<(Instant, msg::AfterProcess<T>)>,
    ) {
        match msg {
            msg::ToProcess::Start => self.start(time, to_backend),
            msg::ToProcess::Stop => self.stop(time, to_backend),
            msg::ToProcess::Reset => self.reset(time, to_backend),
            msg::ToProcess::IncomingMidi { bytes } => self.incoming_midi(time, &bytes, to_backend),
            msg::ToProcess::Consider { coefficients } => {
                self.consider(time, coefficients, to_backend)
            }
            msg::ToProcess::ToggleTemperament { index } => {
                self.toggle_temperament(time, index, to_backend)
            }
        }
    }
}

pub struct WalkingConfig<T: StackType> {
    pub stacktype: Arc<T>,
    pub patterns: Vec<Pattern<T>>,
    pub consider_played: bool,
    pub initial_neighbourhood: Neighbourhood<T>,
}

impl<T: StackType> Clone for WalkingConfig<T> {
    fn clone(&self) -> Self {
        WalkingConfig {
            stacktype: self.stacktype.clone(),
            patterns: self.patterns.clone(),
            consider_played: self.consider_played,
            initial_neighbourhood: self.initial_neighbourhood.clone(),
        }
    }
}

impl<T: StackType> Config<Walking<T>> for WalkingConfig<T> {
    fn initialise(config: &Self) -> Walking<T> {
        let mut uninit_active_notes: [MaybeUninit<NoteWithInfo<T>>; 128] =
            MaybeUninit::uninit_array();
        for i in 0..128 {
            uninit_active_notes[i].write(NoteWithInfo {
                active: false,
                sustained: false,
                channel: Channel::Ch1,
                tuning: 0.0,
                tuning_stack: Stack::new_zero(config.stacktype.clone()),
            });
        }
        let active_notes = unsafe { MaybeUninit::array_assume_init(uninit_active_notes) };
        Walking {
            config: config.clone(),
            tuningframe: TuningFrame {
                initial_reference_key: 60,
                reference_key: 60,
                reference_stack: Stack::new_zero(config.stacktype.clone()),
                neighbourhood: config.initial_neighbourhood.clone(),
            },
            active_notes,
            sustain: [false; 16],
            patterns: config.patterns.clone(),
            consider_played: config.consider_played,
        }
    }
}
