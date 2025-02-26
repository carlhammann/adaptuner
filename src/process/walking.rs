use std::{fmt, marker::PhantomData, mem::MaybeUninit, sync::mpsc, time::Instant};

use midi_msg::{ChannelVoiceMsg, ControlChange, MidiMsg};

use crate::{
    config::r#trait::Config,
    interval::stacktype::r#trait::StackCoeff,
    interval::{interval::Semitones, stack::Stack, stacktype::r#trait::StackType},
    msg,
    neighbourhood::{CompleteNeigbourhood, Neighbourhood},
    pattern::*,
    process::r#trait::ProcessState,
};

#[derive(PartialEq, Clone, Copy)]
enum NoteState {
    Pressed,
    Sustained,
    Off,
}

struct NoteInfo<T: StackType> {
    state_per_channel: [NoteState; 16],
    tuning: Semitones,
    tuning_stack: Stack<T>,
}

pub struct Walking<T: StackType, N: CompleteNeigbourhood<T>> {
    config: WalkingConfig<T, N>,

    active_temperaments: Vec<bool>,
    neighbourhood: N,
    key_center_stack: Stack<T>,

    current_fit: Option<(usize, Stack<T>)>,

    current_notes: [NoteInfo<T>; 128],

    sustain: [bool; 16],

    patterns: Vec<Pattern<T>>, // must be non-empty

    temper_pattern_neighbourhoods: bool,
    use_patterns: bool,

    tmp_work_stack: Stack<T>,
}

impl<T: StackType> HasActivationStatus for NoteInfo<T> {
    fn active(&self) -> bool {
        for state in self.state_per_channel {
            if state != NoteState::Off {
                return true;
            }
        }
        false
    }
}

// `code`s sent in [Special][msg::AfterProcess::Special] messages by this process.
pub static PATTERNS_ENABLED: u8 = 0;
pub static PATTERNS_DISABLED: u8 = 1;

// `code`s received in [Special][msg::ToProcess::Special] messages to this process.
pub static TOGGLE_PATTERNS: u8 = 0;
pub static UPDATE_KEY_CENTER: u8 = 1;
pub static TOGGLE_TEMPER_PATTERN_NEIGHBOURHOODS: u8 = 2;

impl<T: StackType + fmt::Debug, N: CompleteNeigbourhood<T> + Clone> Walking<T, N> {
    // returns true iff the current_fit changed
    fn recompute_fit(
        &mut self,
        time: Instant,
        to_backend: &mpsc::Sender<(Instant, msg::AfterProcess<T>)>,
    ) -> bool {
        let send_to_backend =
            |msg: msg::AfterProcess<T>, time: Instant| to_backend.send((time, msg)).unwrap_or(());

        let find_fit = || -> Option<(usize, Fit)> {
            let mut index = 0;
            let mut best_fit = self.patterns[0].fit(&self.current_notes);
            for i in 1..self.patterns.len() {
                if best_fit.is_complete() {
                    break;
                }
                let fit = self.patterns[i].fit(&self.current_notes);
                if fit.is_better_than(&best_fit) {
                    best_fit = fit;
                    index = i;
                }
            }
            if best_fit.is_at_least_partial() {
                Some((index, best_fit))
            } else {
                None
            }
        };

        let mut updated = false;
        match find_fit() {
            None => {
                if self.current_fit.is_some() {
                    self.current_fit = None;
                    updated = true;
                }
            }
            Some((new_index, best_fit)) => {
                let best_fit_offset =
                    best_fit.reference as StackCoeff - self.key_center_stack.key_number();
                match &mut self.current_fit {
                    None => {
                        let offset = self.neighbourhood.get_relative_stack(best_fit_offset as i8);

                        self.current_fit = Some((new_index, offset));
                        updated = true;
                    }
                    Some((old_index, offset)) => {
                        if *old_index != new_index || best_fit_offset != offset.key_distance() {
                            *old_index = new_index;
                            self.neighbourhood
                                .write_relative_stack(offset, best_fit_offset as i8);
                            updated = true;
                        }
                    }
                }
            }
        }

        match &self.current_fit {
            Some((index, offset)) => {
                let pattern_name = self.patterns[*index].name.clone();
                let mut reference_stack = offset.clone();
                reference_stack.scaled_add(1, &self.key_center_stack);
                send_to_backend(
                    msg::AfterProcess::NotifyFit {
                        pattern_name,
                        reference_stack,
                    },
                    time,
                );
            }
            None => send_to_backend(msg::AfterProcess::NotifyNoFit, time),
        }

        updated
    }

    // returns true iff the tuning changed
    fn update_tuning(&mut self, i: u8) -> bool {
        let note = &mut self.current_notes[i as usize];

        self.tmp_work_stack.clone_from(&note.tuning_stack);

        let mut tune_using_neighbourhood_and_key_center = || {
            self.neighbourhood.write_relative_stack(
                &mut note.tuning_stack,
                (i as StackCoeff - self.key_center_stack.key_number()) as i8,
            );
            note.tuning_stack.scaled_add(1, &self.key_center_stack);
            note.tuning = note.tuning_stack.absolute_semitones();
        };

        if !self.use_patterns {
            tune_using_neighbourhood_and_key_center();
            return note.tuning_stack != self.tmp_work_stack;
        }

        match &self.current_fit {
            None => tune_using_neighbourhood_and_key_center(),
            Some((index, relative_reference_stack)) => {
                let fit_neighbourhood = &self.patterns[*index].neighbourhood;
                let offset = (i as StackCoeff
                    - self.key_center_stack.key_number()
                    - relative_reference_stack.key_distance()) as i8;
                if fit_neighbourhood.has_tuning_for(offset) {
                    let _ =
                        fit_neighbourhood.try_write_relative_stack(&mut note.tuning_stack, offset);
                    if self.temper_pattern_neighbourhoods {
                        note.tuning_stack.retemper(&self.active_temperaments);
                    }
                    note.tuning_stack.scaled_add(1, relative_reference_stack);
                    note.tuning_stack.scaled_add(1, &self.key_center_stack);
                    note.tuning = note.tuning_stack.absolute_semitones();
                } else {
                    tune_using_neighbourhood_and_key_center();
                }
            }
        }

        note.tuning_stack != self.tmp_work_stack
    }

    fn update_tuning_and_send(
        &mut self,
        time: Instant,
        i: u8,
        to_backend: &mpsc::Sender<(Instant, msg::AfterProcess<T>)>,
    ) {
        let send_to_backend =
            |msg: msg::AfterProcess<T>, time: Instant| to_backend.send((time, msg)).unwrap_or(());

        let changed = self.update_tuning(i);
        let note = &self.current_notes[i as usize];

        if changed {
            send_to_backend(
                msg::AfterProcess::Retune {
                    note: i,
                    tuning: note.tuning,
                    tuning_stack: note.tuning_stack.clone(),
                },
                time,
            );
        }
    }

    fn update_all_tunings(
        &mut self,
        time: Instant,
        to_backend: &mpsc::Sender<(Instant, msg::AfterProcess<T>)>,
    ) {
        for i in 0..128 {
            if self.current_notes[i].active() {
                self.update_tuning_and_send(time, i as u8, to_backend);
            }
        }
    }

    fn start(
        &mut self,
        _time: Instant,
        _to_backend: &mpsc::Sender<(Instant, msg::AfterProcess<T>)>,
    ) {
    }

    fn stop(
        &mut self,
        _time: Instant,
        _to_backend: &mpsc::Sender<(Instant, msg::AfterProcess<T>)>,
    ) {
    }

    fn reset(&mut self, time: Instant, to_backend: &mpsc::Sender<(Instant, msg::AfterProcess<T>)>) {
        let send_to_backend =
            |msg: msg::AfterProcess<T>, time: Instant| to_backend.send((time, msg)).unwrap_or(());

        send_to_backend(msg::AfterProcess::Reset, time);
        *self = WalkingConfig::initialise(&self.config);
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
                    self.current_notes[new_key_number as usize].state_per_channel
                        [channel as usize] = NoteState::Pressed;
                    let fit_changed = self.recompute_fit(time, to_backend);

                    self.update_tuning(new_key_number);
                    send_to_backend(
                        msg::AfterProcess::TunedNoteOn {
                            channel,
                            note: new_key_number,
                            velocity,
                            tuning: self.current_notes[new_key_number as usize].tuning,
                            tuning_stack: self.current_notes[new_key_number as usize]
                                .tuning_stack
                                .clone(),
                        },
                        time,
                    );
                    if fit_changed {
                        self.update_all_tunings(time, to_backend);
                    }
                }

                MidiMsg::ChannelVoice {
                    channel,
                    msg: ChannelVoiceMsg::NoteOff { note, velocity },
                } => {
                    let held_by_sustain = self.sustain[channel as usize];
                    self.current_notes[note as usize].state_per_channel[channel as usize] =
                        if held_by_sustain {
                            NoteState::Sustained
                        } else {
                            NoteState::Off
                        };
                    send_to_backend(
                        msg::AfterProcess::NoteOff {
                            channel,
                            note,
                            velocity,
                            held_by_sustain,
                        },
                        time,
                    );
                    if self.recompute_fit(time, to_backend) {
                        self.update_all_tunings(time, to_backend);
                    }
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
                        for note in &mut self.current_notes {
                            note.state_per_channel[channel as usize] =
                                match note.state_per_channel[channel as usize] {
                                    NoteState::Sustained => NoteState::Off,
                                    NoteState::Pressed => NoteState::Pressed,
                                    NoteState::Off => NoteState::Off,
                                };
                        }
                    }

                    send_to_backend(msg::AfterProcess::Sustain { channel, value }, time);
                    if self.recompute_fit(time, to_backend) {
                        self.update_all_tunings(time, to_backend);
                    }
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
        let send_to_backend =
            |msg: msg::AfterProcess<T>, time: Instant| to_backend.send((time, msg)).unwrap_or(());

        let mut stack = Stack::new(&self.active_temperaments, coefficients);
        let normalised_stack = self.neighbourhood.insert(&stack);
        stack.clone_from(normalised_stack);
        match &mut self.current_fit {
            None => {}
            Some((_, reference)) => {
                let dist = reference.key_distance();
                self.neighbourhood
                    .write_relative_stack(reference, dist as i8);
            }
        };
        send_to_backend(msg::AfterProcess::Consider { stack }, time);

        self.update_all_tunings(time, to_backend); // TODO make this affect  only the changed notes?
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
        });
        match &mut self.current_fit {
            None => {}
            Some((_index, reference)) => {
                reference.retemper(&self.active_temperaments);
                // we don't have to apply anything to the neighbourhood around the reference.
                // [update_tuning] takes temper_pattern_neighbourhoods into account.
            }
        }
        self.update_all_tunings(time, to_backend);
        self.neighbourhood.for_each_stack(|_, stack| {
            send_to_backend(
                msg::AfterProcess::Consider {
                    stack: stack.clone(),
                },
                time,
            );
        });
    }

    fn toggle_temper_pattern_neighbourhoods(
        &mut self,
        time: Instant,
        to_backend: &mpsc::Sender<(Instant, msg::AfterProcess<T>)>,
    ) {
        let send_to_backend =
            |msg: msg::AfterProcess<T>, time: Instant| to_backend.send((time, msg)).unwrap_or(());

        self.temper_pattern_neighbourhoods = !self.temper_pattern_neighbourhoods;
        self.update_all_tunings(time, to_backend);
        self.neighbourhood.for_each_stack(|_, stack| {
            send_to_backend(
                msg::AfterProcess::Consider {
                    stack: stack.clone(),
                },
                time,
            );
        });
    }

    fn update_key_center(
        &mut self,
        time: Instant,
        to_backend: &mpsc::Sender<(Instant, msg::AfterProcess<T>)>,
    ) {
        let send_to_backend =
            |msg: msg::AfterProcess<T>, time: Instant| to_backend.send((time, msg)).unwrap_or(());

        match &mut self.current_fit {
            None => {}
            Some((_, reference)) => {
                self.tmp_work_stack.clone_from(&self.key_center_stack);
                self.neighbourhood.write_relative_stack(
                    &mut self.key_center_stack,
                    reference.key_distance() as i8,
                );
                self.key_center_stack.scaled_add(1, &self.tmp_work_stack);
                reference.reset_to_zero();
            }
        }

        self.update_all_tunings(time, to_backend);

        send_to_backend(
            msg::AfterProcess::SetReference {
                key: self.key_center_stack.key_number() as u8,
                stack: self.key_center_stack.clone(),
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
    }

    fn toggle_patterns(
        &mut self,
        time: Instant,
        to_backend: &mpsc::Sender<(Instant, msg::AfterProcess<T>)>,
    ) {
        let send_to_backend =
            |msg: msg::AfterProcess<T>, time: Instant| to_backend.send((time, msg)).unwrap_or(());
        self.use_patterns = !self.use_patterns;
        self.update_all_tunings(time, to_backend);
        send_to_backend(
            msg::AfterProcess::Special {
                code: if self.use_patterns {
                    PATTERNS_ENABLED
                } else {
                    PATTERNS_DISABLED
                },
            },
            time,
        );
    }
}

impl<T: StackType + fmt::Debug, N: CompleteNeigbourhood<T> + Clone> ProcessState<T>
    for Walking<T, N>
{
    fn handle_msg(
        &mut self,
        time: Instant,
        msg: msg::ToProcess,
        to_backend: &mpsc::Sender<(Instant, msg::AfterProcess<T>)>,
    ) {
        match msg {
            msg::ToProcess::Special { code } => {
                if code == TOGGLE_TEMPER_PATTERN_NEIGHBOURHOODS {
                    self.toggle_temper_pattern_neighbourhoods(time, to_backend)
                } else if code == UPDATE_KEY_CENTER {
                    self.update_key_center(time, to_backend);
                } else if code == TOGGLE_PATTERNS {
                    self.toggle_patterns(time, to_backend);
                }
            }
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

#[derive(Clone)]
pub struct WalkingConfig<T: StackType, N: CompleteNeigbourhood<T>> {
    pub patterns: Vec<Pattern<T>>,
    pub consider_played: bool,
    pub initial_neighbourhood: N,
    pub temper_pattern_neighbourhoods: bool,
    pub use_patterns: bool,
    pub _phantom: PhantomData<T>,
}

impl<T: StackType, N: CompleteNeigbourhood<T> + Clone> Config<Walking<T, N>>
    for WalkingConfig<T, N>
{
    fn initialise(config: &Self) -> Walking<T, N> {
        let mut uninit_current_notes: [MaybeUninit<NoteInfo<T>>; 128] = MaybeUninit::uninit_array();
        for i in 0..128 {
            uninit_current_notes[i].write(NoteInfo {
                state_per_channel: [NoteState::Off; 16],
                tuning: 0.0,
                tuning_stack: Stack::new_zero(),
            });
        }
        let current_notes = unsafe { MaybeUninit::array_assume_init(uninit_current_notes) };
        Walking {
            config: config.clone(),
            current_notes,
            sustain: [false; 16],
            patterns: config.patterns.clone(),
            active_temperaments: vec![false; T::num_temperaments()],
            neighbourhood: config.initial_neighbourhood.clone(),
            key_center_stack: Stack::new_zero(),
            current_fit: None,
            temper_pattern_neighbourhoods: config.temper_pattern_neighbourhoods,
            use_patterns: config.use_patterns,
            tmp_work_stack: Stack::new_zero(),
        }
    }
}
