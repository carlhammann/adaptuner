use std::{
    sync::{mpsc, Arc},
    time::Instant,
};

use midi_msg::{ChannelVoiceMsg, ControlChange, MidiMsg};

use crate::{
    config::r#trait::Config,
    interval::{
        interval::Semitones,
        stack::Stack,
        stacktype::r#trait::{FiveLimitStackType, StackCoeff, StackType},
    },
    msg,
    neighbourhood::Neighbourhood,
    process::r#trait::ProcessState,
};

#[derive(Clone, Copy, PartialEq)]
enum NoteStatus {
    On,
    Off,
    Sustained,
}

pub struct Static12<T: StackType> {
    config: Static12Config<T>,
    initial_reference_key: i8,
    reference_stack: Stack<T>,
    reference_key: i8,
    neighbourhood: Neighbourhood<T>,
    active_temperaments: Vec<bool>,
    note_statuses: [(NoteStatus, Semitones); 128],
    sustain: u8,
}

/// This is the midi key number of A1. All notes below this key (that is, the lowest octave of the
/// piano) will be muted and only be used to set the `reference_key` (and `reference_stack`).
static CUTOFF_KEY: i8 = 33;

impl<T: StackType> Static12<T> {
    fn calculate_tuning_stack(&self, key: i8) -> Stack<T> {
        let mut the_stack = self
            .neighbourhood
            .relative_stack_for_key_offset(key - self.reference_key)
            .unwrap(); // this is ok, because the neighbourhood is always complete
        the_stack.add_mul(1, &self.reference_stack);
        the_stack
    }

    fn tuning_from_stack(&self, stack: &Stack<T>) -> Semitones {
        stack.semitones() + self.initial_reference_key as Semitones
    }

    fn recompute_and_send_tunings(
        &mut self,
        time: Instant,
        to_backend: &mpsc::Sender<(Instant, msg::AfterProcess<T>)>,
    ) {
        let send_to_backend =
            |msg: msg::AfterProcess<T>, time: Instant| to_backend.send((time, msg)).unwrap_or(());

        for i in 0..128 {
            match self.note_statuses[i].0 {
                NoteStatus::Off => {}
                _ => {
                    let tuning_stack = self.calculate_tuning_stack(i as i8);
                    let tuning = self.tuning_from_stack(&tuning_stack);
                    // in the presence of temperaments, the tuning may be the same, but the stack different. Hence, this test is not always corect:
                    //
                    // if self.note_statuses[i].1 != tuning {
                    self.note_statuses[i].1 = tuning;
                    send_to_backend(
                        msg::AfterProcess::Retune {
                            note: i as u8,
                            tuning,
                            tuning_stack,
                        },
                        time,
                    );
                    // }
                }
            }
        }
    }

    fn toggle_temperament(
        &mut self,
        time: Instant,
        index: usize,
        to_backend: &mpsc::Sender<(Instant, msg::AfterProcess<T>)>,
    ) {
        let send_to_backend =
            |msg: msg::AfterProcess<T>, time: Instant| to_backend.send((time, msg)).unwrap_or(());
        self.active_temperaments[index] = !self.active_temperaments[index];
        self.neighbourhood.for_each_stack_mut(|_, stack| {
            stack.retemper(&self.active_temperaments);
            send_to_backend(
                msg::AfterProcess::Consider {
                    stack: stack.clone(),
                },
                time,
            );
        });
        self.recompute_and_send_tunings(time, to_backend);
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
                    msg: ChannelVoiceMsg::NoteOn { note, velocity },
                } => {
                    if note as i8 >= CUTOFF_KEY {
                        let tuning_stack = self.calculate_tuning_stack(note as i8);
                        let tuning = self.tuning_from_stack(&tuning_stack);
                        self.note_statuses[note as usize] = (NoteStatus::On, tuning);
                        send_to_backend(
                            msg::AfterProcess::TunedNoteOn {
                                channel,
                                note,
                                velocity,
                                tuning,
                                tuning_stack,
                            },
                            time,
                        );
                    } else {
                        // here, we'll reset the reference
                        let new_reference_stack = self.calculate_tuning_stack(note as i8);
                        self.reference_stack.clone_from(&new_reference_stack);
                        self.reference_key = note as i8;
                        send_to_backend(
                            msg::AfterProcess::SetReference {
                                key: note,
                                stack: new_reference_stack,
                            },
                            time,
                        );

                        self.neighbourhood.for_each_stack(|_, stack| {
                            send_to_backend(
                                msg::AfterProcess::Consider {
                                    stack: stack.clone(),
                                },
                                time,
                            );
                        });
                        self.recompute_and_send_tunings(time, to_backend);
                    }
                }

                MidiMsg::ChannelVoice {
                    channel,
                    msg: ChannelVoiceMsg::NoteOff { note, velocity },
                } => {
                    if note as i8 >= CUTOFF_KEY {
                        send_to_backend(
                            msg::AfterProcess::NoteOff {
                                held_by_sustain: self.sustain != 0,
                                channel,
                                note,
                                velocity,
                            },
                            time,
                        );
                        if self.sustain == 0 {
                            self.note_statuses[note as usize].0 = NoteStatus::Off;
                        } else {
                            self.note_statuses[note as usize].0 = NoteStatus::Sustained;
                        }
                    }
                }

                MidiMsg::ChannelVoice {
                    channel,
                    msg:
                        ChannelVoiceMsg::ControlChange {
                            control: ControlChange::Hold(value),
                        },
                } => {
                    self.sustain = value;
                    send_to_backend(msg::AfterProcess::Sustain { channel, value }, time);
                    if value == 0 {
                        for (status, _tuning) in self.note_statuses.iter_mut() {
                            if *status == NoteStatus::Sustained {
                                *status = NoteStatus::Off;
                            }
                        }
                    }
                }

                MidiMsg::ChannelVoice {
                    channel,
                    msg: ChannelVoiceMsg::ProgramChange { program },
                } => send_to_backend(msg::AfterProcess::ProgramChange { channel, program }, time),

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
        let send_to_backend =
            |msg: msg::AfterProcess<T>, time: Instant| to_backend.send((time, msg)).unwrap_or(());

        let stack = Stack::new(
            self.config.stack_type.clone(),
            &self.active_temperaments,
            coefficients,
        );

        let normalised_stack = self.neighbourhood.insert(stack);

        send_to_backend(
            msg::AfterProcess::Consider {
                stack: normalised_stack,
            },
            time,
        );
        self.recompute_and_send_tunings(time, to_backend);
    }
}

impl<T: FiveLimitStackType> Static12<T> {
    fn reset(&mut self, time: Instant, to_backend: &mpsc::Sender<(Instant, msg::AfterProcess<T>)>) {
        let send_to_backend =
            |msg: msg::AfterProcess<T>, time: Instant| to_backend.send((time, msg)).unwrap_or(());

        send_to_backend(msg::AfterProcess::Reset, time);
        *self = Static12Config::initialise(&self.config);
        self.neighbourhood.for_each_stack(|_, stack| {
            send_to_backend(
                msg::AfterProcess::Consider {
                    stack: stack.clone(),
                },
                time,
            );
        });
    }
}

impl<T: FiveLimitStackType> ProcessState<T> for Static12<T> {
    fn handle_msg(
        &mut self,
        time: Instant,
        msg: crate::msg::ToProcess,
        to_backend: &mpsc::Sender<(Instant, msg::AfterProcess<T>)>,
    ) {
        match msg {
            msg::ToProcess::Start => {
                self.reset(time, to_backend);
            }
            msg::ToProcess::Stop => {}
            msg::ToProcess::Reset => {
                self.reset(time, to_backend);
            }
            msg::ToProcess::ToggleTemperament { index } => {
                self.toggle_temperament(time, index, to_backend)
            }
            msg::ToProcess::IncomingMidi { bytes } => self.incoming_midi(time, &bytes, to_backend),
            msg::ToProcess::Consider { coefficients } => {
                self.consider(time, coefficients, to_backend);
            }
        }
    }
}

pub struct Static12Config<T: StackType> {
    pub stack_type: Arc<T>,
    pub initial_reference_key: i8,
    pub initial_neighbourhood: Neighbourhood<T>,
}

// derive(Clone) doesn't handle cloning of `Arc` correctly
impl<T: StackType> Clone for Static12Config<T> {
    fn clone(&self) -> Self {
        Static12Config {
            stack_type: self.stack_type.clone(),
            initial_reference_key: self.initial_reference_key,
            initial_neighbourhood: self.initial_neighbourhood.clone(),
        }
    }
}

impl<T: FiveLimitStackType> Config<Static12<T>> for Static12Config<T> {
    fn initialise(config: &Self) -> Static12<T> {
        let mut note_statuses = [(NoteStatus::Off, 0.0); 128];
        for i in 0..128 {
            note_statuses[i].1 = i as Semitones;
        }
        let no_active_temperaments = vec![false; config.stack_type.num_temperaments()];
        Static12 {
            config: config.clone(),
            initial_reference_key: config.initial_reference_key,
            reference_stack: Stack::new(
                config.stack_type.clone(),
                &no_active_temperaments,
                vec![0; config.stack_type.num_intervals()],
            ),
            reference_key: config.initial_reference_key,
            neighbourhood: config.initial_neighbourhood.clone(),
            active_temperaments: no_active_temperaments,
            note_statuses,
            sustain: 0,
        }
    }
}
