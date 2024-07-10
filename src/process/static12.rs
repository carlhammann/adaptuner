use std::{
    fmt,
    sync::{mpsc, Arc},
    time::Instant,
};

use midi_msg::{ChannelVoiceMsg, ControlChange, MidiMsg};

use crate::{
    config::r#trait::Config,
    interval::{Semitones, Stack, StackCoeff, StackType},
    msg,
    neighbourhood::Neighbourhood,
    process::r#trait::ProcessState,
    util::dimension::{vector_from_elem, AtLeast, Bounded, Dimension, Vector},
};

#[derive(Clone, Copy, PartialEq)]
enum NoteStatus {
    On,
    Off,
    Sustained,
}

pub struct Static12<D: Dimension, T: Dimension> {
    config: Static12Config<D, T>,
    initial_reference_key: i8,
    reference_stack: Stack<D, T>,
    reference_key: i8,
    neighbourhood: Neighbourhood<D>,
    active_temperaments: Vector<T, bool>,
    note_statuses: [(NoteStatus, Semitones); 128],
    sustain: u8,
}

/// This is the midi key number of A1. All notes below this key (that is, the lowest octave of the
/// piano) will be muted and only be used to set the `reference_key` (and `reference_stack`).
static CUTOFF_KEY: i8 = 33;

impl<D: Dimension + Copy + fmt::Debug + AtLeast<1>, T: Dimension + Copy> Static12<D, T> {
    fn calculate_tuning_stack(&self, key: i8) -> Stack<D, T> {
        let mut the_stack = self.reference_stack.clone();
        let rem = (key - self.reference_key).rem_euclid(12);
        let quot = (key - self.reference_key).div_euclid(12);
        the_stack.increment(
            &self.active_temperaments,
            &self.neighbourhood.coefficients[rem as usize],
        );
        the_stack.increment_at_index(
            &self.active_temperaments,
            Bounded::new(0).unwrap(),
            quot as StackCoeff,
        );
        the_stack
    }

    fn calculate_tuning(&mut self, key: i8) -> Semitones {
        let the_stack = self.calculate_tuning_stack(key);
        the_stack.semitones() + self.initial_reference_key as Semitones
    }

    fn recompute_and_send_tunings_to_backend(
        &mut self,
        time: Instant,
        to_backend: &mpsc::Sender<(Instant, msg::ToBackend)>,
    ) {
        let send_to_backend =
            |msg: msg::ToBackend, time: Instant| to_backend.send((time, msg)).unwrap_or(());

        for i in 0..128 {
            match self.note_statuses[i].0 {
                NoteStatus::Off => {}
                _ => {
                    let tuning = self.calculate_tuning(i as i8);
                    if self.note_statuses[i].1 != tuning {
                        self.note_statuses[i].1 = tuning;
                        send_to_backend(
                            msg::ToBackend::Retune {
                                note: i as u8,
                                tuning,
                            },
                            time,
                        );
                    }
                }
            }
        }
    }

    fn set_neighborhood(
        &mut self,
        time: Instant,
        new_neighbourhood: Neighbourhood<D>,
        to_backend: &mpsc::Sender<(Instant, msg::ToBackend)>,
    ) {
        self.neighbourhood = new_neighbourhood;
        self.recompute_and_send_tunings_to_backend(time, to_backend);
    }

    fn toggle_temperament(
        &mut self,
        time: Instant,
        index: Bounded<T>,
        to_backend: &mpsc::Sender<(Instant, msg::ToBackend)>,
    ) {
        self.active_temperaments[index] = !self.active_temperaments[index];
        self.recompute_and_send_tunings_to_backend(time, to_backend);
    }

    fn incoming_midi(
        &mut self,
        time: Instant,
        bytes: &[u8],
        to_backend: &mpsc::Sender<(Instant, msg::ToBackend)>,
        to_ui: &mpsc::Sender<(Instant, msg::ToUI<D, T>)>,
    ) {
        let send_to_backend =
            |msg: msg::ToBackend, time: Instant| to_backend.send((time, msg)).unwrap_or(());

        let send_to_ui =
            |msg: msg::ToUI<D, T>, time: Instant| to_ui.send((time, msg)).unwrap_or(());

        match MidiMsg::from_midi(bytes) {
            Err(err) => send_to_ui(msg::ToUI::MidiParseErr(err), time),
            Ok((msg, _nbtyes)) => match msg {
                MidiMsg::ChannelVoice {
                    channel,
                    msg: ChannelVoiceMsg::NoteOn { note, velocity },
                } => {
                    if note as i8 >= CUTOFF_KEY {
                        let tuning = self.calculate_tuning(note as i8);
                        self.note_statuses[note as usize] = (NoteStatus::On, tuning);
                        send_to_backend(
                            msg::ToBackend::TunedNoteOn {
                                channel,
                                note,
                                velocity,
                                tuning,
                            },
                            time,
                        );
                        send_to_ui(msg::ToUI::TunedNoteOn { note, tuning }, time);
                    } else {
                        // here, we'll reset the reference
                        let new_reference_stack = self.calculate_tuning_stack(note as i8);
                        self.reference_stack.clone_from(&new_reference_stack); // TODO: do this without cloning
                        self.reference_key = note as i8;
                        send_to_ui(
                            msg::ToUI::SetReference {
                                key: note,
                                stack: new_reference_stack,
                            },
                            time,
                        );
                    }
                }

                MidiMsg::ChannelVoice {
                    channel,
                    msg: ChannelVoiceMsg::NoteOff { note, velocity },
                } => {
                    if note as i8 >= CUTOFF_KEY {
                        send_to_backend(
                            msg::ToBackend::NoteOff {
                                channel,
                                note,
                                velocity,
                            },
                            time,
                        );
                        if self.sustain == 0 {
                            self.note_statuses[note as usize].0 = NoteStatus::Off;
                            send_to_ui(msg::ToUI::NoteOff { note }, time);
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
                    send_to_backend(msg::ToBackend::Sustain { channel, value }, time);
                    if value == 0 {
                        for (note, (status, _tuning)) in self.note_statuses.iter_mut().enumerate() {
                            if *status == NoteStatus::Sustained {
                                *status = NoteStatus::Off;
                                send_to_ui(msg::ToUI::NoteOff { note: note as u8 }, time);
                            }
                        }
                    }
                }

                MidiMsg::ChannelVoice {
                    channel,
                    msg: ChannelVoiceMsg::ProgramChange { program },
                } => send_to_backend(msg::ToBackend::ProgramChange { channel, program }, time),

                _ => {
                    send_to_backend(msg::ToBackend::ForwardMidi { msg }, time);
                }
            },
        }
    }
}

impl<D, T> ProcessState<D, T> for Static12<D, T>
where
    D: Dimension + Copy + fmt::Debug + AtLeast<3> + AtLeast<1>, // TODO this should not be like nhat
    T: Dimension + Copy,
{
    fn handle_msg(
        &mut self,
        time: Instant,
        msg: crate::msg::ToProcess<D, T>,
        to_backend: &mpsc::Sender<(Instant, msg::ToBackend)>,
        to_ui: &mpsc::Sender<(Instant, msg::ToUI<D, T>)>,
    ) {
        let send_to_backend =
            |msg: msg::ToBackend, time: Instant| to_backend.send((time, msg)).unwrap_or(());

        match msg {
            msg::ToProcess::Start => {}
            msg::ToProcess::Stop => {}
            msg::ToProcess::Reset => {
                send_to_backend(msg::ToBackend::Reset, time);
                *self = Static12Config::<D, T>::initialise(&self.config);
            }
            msg::ToProcess::SetNeighboughood { neighbourhood } => {
                self.set_neighborhood(time, neighbourhood, to_backend)
            }
            msg::ToProcess::ToggleTemperament { index } => {
                self.toggle_temperament(time, index, to_backend)
            }
            msg::ToProcess::IncomingMidi { bytes } => {
                self.incoming_midi(time, &bytes, to_backend, to_ui)
            }
        }
    }
}

#[derive(Clone)]
pub struct Static12Config<D: Dimension, T: Dimension> {
    pub stack_type: Arc<StackType<D, T>>,
    pub initial_reference_key: i8,
    pub initial_neighbourhood_width: StackCoeff,
    pub initial_neighbourhood_index: StackCoeff,
    pub initial_neighbourhood_offset: StackCoeff,
}

impl<D, T> Config<Static12<D, T>> for Static12Config<D, T>
where
    D: Dimension + Copy + fmt::Debug + AtLeast<3>,
    T: Dimension + Copy,
{
    fn initialise(config: &Self) -> Static12<D, T> {
        let mut note_statuses = [(NoteStatus::Off, 0.0); 128];
        for i in 0..128 {
            note_statuses[i].1 = i as Semitones;
        }
        Static12 {
            config: config.clone(),
            initial_reference_key: config.initial_reference_key,
            reference_stack: Stack::new(
                config.stack_type.clone(),
                &vector_from_elem(false),
                vector_from_elem(0),
            ),
            reference_key: config.initial_reference_key,
            neighbourhood: Neighbourhood::fivelimit_new(
                config.initial_neighbourhood_width,
                config.initial_neighbourhood_index,
                config.initial_neighbourhood_offset,
            ),
            active_temperaments: vector_from_elem(false),
            note_statuses,
            sustain: 0,
        }
    }
}
