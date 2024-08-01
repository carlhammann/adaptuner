use std::{marker::PhantomData, mem::MaybeUninit, sync::mpsc, time::Instant};

use midi_msg::{Channel, ChannelVoiceMsg, ControlChange, MidiMsg};

use crate::{
    config::r#trait::Config,
    interval::stacktype::r#trait::StackCoeff,
    interval::{interval::Semitones, stack::Stack, stacktype::r#trait::StackType},
    msg,
    neighbourhood::{CompleteNeigbourhood, Neighbourhood},
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

pub struct Walking<T: StackType, N: CompleteNeigbourhood<T>> {
    config: WalkingConfig<T, N>,

    active_temperaments: Vec<bool>,
    neighbourhood: N,
    key_center_stack: Stack<T>,

    current_fit: Option<(usize, Stack<T>)>,

    active_notes: [NoteWithInfo<T>; 128],

    sustain: [bool; 16],

    patterns: Vec<Pattern<T>>, // must be non-empty

    temper_pattern_neighbourhoods: bool,

    tmp_work_stack: Stack<T>,
}

impl<T: StackType> HasActivationStatus for NoteWithInfo<T> {
    fn active(&self) -> bool {
        self.active
    }
}

impl<T: StackType, N: CompleteNeigbourhood<T> + Clone> Walking<T, N> {
    // returns true iff the corrent_fit changed
    fn recompute_fit(&mut self) -> bool {
        let find_fit = || -> Option<(usize, Fit)> {
            let mut index = 0;
            let mut best_fit = self.patterns[0].fit(&self.active_notes);
            for i in 1..self.patterns.len() {
                if best_fit.is_complete() {
                    break;
                }
                let fit = self.patterns[i].fit(&self.active_notes);
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

        match find_fit() {
            None => {
                if self.current_fit.is_some() {
                    self.current_fit = None;
                    true
                } else {
                    false
                }
            }
            Some((new_index, best_fit)) => match &mut self.current_fit {
                None => {
                    let reference = self
                        .neighbourhood
                        .get_relative_stack(best_fit.reference as i8);
                    self.current_fit = Some((new_index, reference));
                    true
                }
                Some((old_index, reference)) => {
                    if *old_index != new_index
                        || best_fit.reference as StackCoeff != reference.key_distance()
                    {
                        *old_index = new_index;
                        self.neighbourhood
                            .write_relative_stack(reference, best_fit.reference as i8);
                        true
                    } else {
                        false
                    }
                }
            },
        }
    }

    // returns true iff the tuning changed
    fn update_tuning(&mut self, i: u8) -> bool {
        let note = &mut self.active_notes[i as usize];

        self.tmp_work_stack.clone_from(&note.tuning_stack);

        let mut tune_using_neighbourhood_and_key_center = || {
            self.neighbourhood.write_relative_stack(
                &mut note.tuning_stack,
                (i as StackCoeff - self.key_center_stack.key_distance()) as i8,
            );
            note.tuning_stack.add_mul(1, &self.key_center_stack);
            note.tuning = note.tuning_stack.semitones(); // 60 is the tuning of the reference, which is C4
        };

        match &self.current_fit {
            None => tune_using_neighbourhood_and_key_center(),
            Some((index, relative_reference_stack)) => {
                let fit_neighbourhood = &self.patterns[*index].neighbourhood;
                let offset = (i as StackCoeff
                    - self.key_center_stack.key_distance()
                    - relative_reference_stack.key_distance()) as i8;
                if fit_neighbourhood.has_tuning_for(offset) {
                    let _ =
                        fit_neighbourhood.try_write_relative_stack(&mut note.tuning_stack, offset);
                    if self.temper_pattern_neighbourhoods {
                        note.tuning_stack.retemper(&self.active_temperaments);
                    }
                    note.tuning_stack.add_mul(1, relative_reference_stack);
                    note.tuning_stack.add_mul(1, &self.key_center_stack);
                    note.tuning = note.tuning_stack.semitones();
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
        let note = &self.active_notes[i as usize];

        if note.active && changed {
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
            if self.active_notes[i].active {
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
                    self.active_notes[new_key_number as usize].active = true;
                    self.active_notes[new_key_number as usize].sustained = false;
                    let fit_changed = self.recompute_fit();

                    if fit_changed {
                        send_to_backend(
                            msg::AfterProcess::Notify {
                                line: format!(
                                    "fit changed. reference is {}",
                                    self.current_fit.clone().expect("").1.key_distance()
                                ),
                            },
                            time,
                        );
                    }

                    self.update_tuning(new_key_number);
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
                    if fit_changed {
                        self.update_all_tunings(time, to_backend);
                    }
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
                        if self.recompute_fit() {
                            self.update_all_tunings(time, to_backend);
                        }
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
                        if self.recompute_fit() {
                            self.update_all_tunings(time, to_backend);
                        }
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
        let send_to_backend =
            |msg: msg::AfterProcess<T>, time: Instant| to_backend.send((time, msg)).unwrap_or(());

        let mut stack = Stack::new(&self.active_temperaments, coefficients);
        let normalised_stack = self.neighbourhood.insert(&stack);
        stack.clone_from(normalised_stack);
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
        self.neighbourhood
            .for_each_stack_mut(|_, stack| stack.retemper(&self.active_temperaments));
        if self.temper_pattern_neighbourhoods {
            match &mut self.current_fit {
                None => {}
                Some((_index, reference)) => {
                    reference.retemper(&self.active_temperaments);
                    // we don't have to apply anything to the neighbourhood around the reference.
                    // [update_tuning] takes temper_pattern_neighbourhoods into account.
                }
            }
        }
        self.neighbourhood.for_each_stack(|_, stack| {
            send_to_backend(
                msg::AfterProcess::Consider {
                    stack: stack.clone(),
                },
                time,
            );
        });
        self.update_all_tunings(time, to_backend);
    }
}

impl<T: StackType, N: CompleteNeigbourhood<T> + Clone> ProcessState<T> for Walking<T, N> {
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

#[derive(Clone)]
pub struct WalkingConfig<T: StackType, N: CompleteNeigbourhood<T>> {
    pub patterns: Vec<Pattern<T>>,
    pub consider_played: bool,
    pub initial_neighbourhood: N,
    pub temper_pattern_neighbourhoods: bool,
    pub _phantom: PhantomData<T>,
}

impl<T: StackType, N: CompleteNeigbourhood<T> + Clone> Config<Walking<T, N>>
    for WalkingConfig<T, N>
{
    fn initialise(config: &Self) -> Walking<T, N> {
        let mut uninit_active_notes: [MaybeUninit<NoteWithInfo<T>>; 128] =
            MaybeUninit::uninit_array();
        for i in 0..128 {
            uninit_active_notes[i].write(NoteWithInfo {
                active: false,
                sustained: false,
                channel: Channel::Ch1,
                tuning: 0.0,
                tuning_stack: Stack::new_zero(),
            });
        }
        let active_notes = unsafe { MaybeUninit::array_assume_init(uninit_active_notes) };
        Walking {
            config: config.clone(),
            active_notes,
            sustain: [false; 16],
            patterns: config.patterns.clone(),
            active_temperaments: vec![false; T::num_temperaments()],
            neighbourhood: config.initial_neighbourhood.clone(),
            key_center_stack: Stack::new_zero(),
            current_fit: None,
            temper_pattern_neighbourhoods: config.temper_pattern_neighbourhoods,
            tmp_work_stack: Stack::new_zero(),
        }
    }
}
