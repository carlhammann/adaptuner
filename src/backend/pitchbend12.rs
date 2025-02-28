//! A backend that uses twelve midi channels and pitchbend. Works for tuning systems that have an
//! [OctavePeriodicStackType].
//!

use std::{mem::MaybeUninit, sync::mpsc, time::Instant};

use midi_msg::{Channel, ChannelModeMsg, ChannelVoiceMsg, ControlChange, MidiMsg};

use crate::{
    backend::r#trait::BackendState,
    config::r#trait::Config,
    interval::{base::Semitones, stacktype::r#trait::OctavePeriodicStackType},
    msg,
};

#[derive(PartialEq, Clone, Copy)]
enum NoteState {
    Pressed,
    Sustained,
    Off,
}

#[derive(Clone, Copy)]
struct NoteInfo {
    desired_tuning: Semitones,
    state_by_input_channel: [NoteState; 16],
}

impl NoteInfo {
    fn not_pressed(&self) -> bool {
        for state in self.state_by_input_channel {
            if state == NoteState::Pressed {
                return false;
            }
        }
        true
    }
}

pub struct Pitchbend12 {
    config: Pitchbend12Config,

    /// the channels to use. Exlude CH10 for GM compatibility
    channels: [Channel; 12],

    /// invariant: the bend pertaining to `channels[i]` is in `bends[i]`
    bends: [u16; 12],

    active_notes: [NoteInfo; 128],

    /// is the sustain pedal held at the moment?
    sustain_by_input_channel: [bool; 16],

    /// the current bend range
    bend_range: Semitones,
}

impl Pitchbend12 {
    fn bend_from_semitones(&self, semitones: Semitones) -> u16 {
        ((8191.0 * semitones / self.bend_range + 8192.0) as u16)
            .max(0)
            .min(16383)
    }

    fn semitones_from_bend(&self, bend: u16) -> Semitones {
        (bend as Semitones - 8192.0) / 8191.0 * self.bend_range
    }
}

impl<T: OctavePeriodicStackType> BackendState<T> for Pitchbend12 {
    fn handle_msg(
        &mut self,
        time: Instant,
        msg: msg::AfterProcess<T>,
        to_ui: &mpsc::Sender<(Instant, msg::AfterProcess<T>)>,
        midi_out: &mpsc::Sender<(Instant, Vec<u8>)>,
    ) {
        let send_to_ui =
            |msg: msg::AfterProcess<T>, time: Instant| to_ui.send((time, msg)).unwrap_or(());

        let send = |msg: MidiMsg, time: Instant| {
            midi_out.send((time, msg.to_midi())).unwrap_or(());
        };

        match msg {
            msg::AfterProcess::Start | msg::AfterProcess::Reset => {
                *self = Pitchbend12Config::initialise(&self.config);
                for (i, &channel) in self.channels.iter().enumerate() {
                    send(
                        MidiMsg::ChannelVoice {
                            channel,
                            msg: ChannelVoiceMsg::PitchBend {
                                bend: self.bends[i],
                            },
                        },
                        time,
                    );
                    send(
                        MidiMsg::ChannelVoice {
                            channel,
                            msg: ChannelVoiceMsg::ControlChange {
                                control: ControlChange::Hold(0),
                            },
                        },
                        time,
                    );
                    send(
                        MidiMsg::ChannelMode {
                            channel,
                            msg: ChannelModeMsg::AllSoundOff,
                        },
                        time,
                    );
                }
            }

            msg::AfterProcess::Stop => {}

            msg::AfterProcess::TunedNoteOn {
                channel,
                note,
                velocity,
                tuning,
                ..
            } => {
                let mut the_note = self.active_notes[note as usize];
                the_note.desired_tuning = tuning;
                the_note.state_by_input_channel[channel as usize] = NoteState::Pressed;
                let channel_index = note as usize % 12;
                let old_bend = self.bends[channel_index];
                let bend = self.bend_from_semitones(tuning - note as Semitones);
                send(
                    MidiMsg::ChannelVoice {
                        channel: self.channels[channel_index],
                        msg: ChannelVoiceMsg::NoteOn { note, velocity },
                    },
                    time,
                );
                if old_bend != bend {
                    send(
                        MidiMsg::ChannelVoice {
                            channel: self.channels[channel_index],
                            msg: ChannelVoiceMsg::PitchBend { bend },
                        },
                        time,
                    );
                    self.bends[channel_index] = bend;
                }
                if (tuning - note as Semitones).abs() > self.bend_range {
                    send_to_ui(
                        msg::AfterProcess::DetunedNote {
                            note,
                            actual: note as Semitones + self.semitones_from_bend(bend),
                            should_be: tuning,
                            explanation: "Exceeded bend range",
                        },
                        time,
                    );
                }
            }

            msg::AfterProcess::NoteOff {
                channel,
                note,
                velocity,
                ..
            } => {
                let mut the_note = self.active_notes[note as usize];
                let old_state = the_note.state_by_input_channel[channel as usize];
                the_note.state_by_input_channel[channel as usize] = match old_state {
                    NoteState::Off => NoteState::Off,
                    NoteState::Pressed => {
                        if self.sustain_by_input_channel[channel as usize] {
                            NoteState::Sustained
                        } else {
                            NoteState::Off
                        }
                    }
                    NoteState::Sustained => NoteState::Sustained,
                };
                let channel_index = note as usize % 12;
                if the_note.not_pressed() {
                    send(
                        MidiMsg::ChannelVoice {
                            channel: self.channels[channel_index],
                            msg: ChannelVoiceMsg::NoteOff { note, velocity },
                        },
                        time,
                    );
                }
            }

            msg::AfterProcess::Sustain { channel, value } => {
                self.sustain_by_input_channel[channel as usize] = value != 0;
                if value == 0 {
                    for note in &mut self.active_notes {
                        note.state_by_input_channel[channel as usize] =
                            match note.state_by_input_channel[channel as usize] {
                                NoteState::Off => NoteState::Off,
                                NoteState::Pressed => NoteState::Pressed,
                                NoteState::Sustained => NoteState::Off,
                            };
                    }
                }
                for channel in self.channels {
                    send(
                        MidiMsg::ChannelVoice {
                            channel,
                            msg: ChannelVoiceMsg::ControlChange {
                                control: ControlChange::Hold(value),
                            },
                        },
                        time,
                    );
                }
            }

            msg::AfterProcess::Retune { note, tuning, .. } => {
                let mut the_note = self.active_notes[note as usize];
                the_note.desired_tuning = tuning;
                let channel_index = note as usize % 12;
                let old_bend = self.bends[channel_index];
                let bend = self.bend_from_semitones(tuning - note as Semitones);
                if old_bend != bend {
                    send(
                        MidiMsg::ChannelVoice {
                            channel: self.channels[channel_index],
                            msg: ChannelVoiceMsg::PitchBend { bend },
                        },
                        time,
                    );
                    self.bends[channel_index] = bend;
                }
                if (tuning - note as Semitones).abs() > self.bend_range {
                    send_to_ui(
                        msg::AfterProcess::DetunedNote {
                            note,
                            actual: note as Semitones + self.semitones_from_bend(bend),
                            should_be: tuning,
                            explanation: "Exceeded bend range",
                        },
                        time,
                    );
                }
            }

            msg::AfterProcess::ForwardMidi { msg } => send(msg, time),

            msg::AfterProcess::ProgramChange { program, .. } => {
                for channel in self.channels {
                    send(
                        MidiMsg::ChannelVoice {
                            channel,
                            msg: ChannelVoiceMsg::ProgramChange { program },
                        },
                        time,
                    )
                }
            }

            _ => {}
        }
    }
}

#[derive(Clone)]
pub struct Pitchbend12Config {
    pub bend_range: Semitones,
    pub channels: [Channel; 12],
}

impl Config<Pitchbend12> for Pitchbend12Config {
    fn initialise(config: &Self) -> Pitchbend12 {
        let mut uninit_active_notes: [MaybeUninit<NoteInfo>; 128] = MaybeUninit::uninit_array();
        for i in 0..128 {
            uninit_active_notes[i].write(NoteInfo {
                desired_tuning: i as Semitones,
                state_by_input_channel: [NoteState::Off; 16],
            });
        }
        let active_notes = unsafe { MaybeUninit::array_assume_init(uninit_active_notes) };
        Pitchbend12 {
            config: config.clone(),
            channels: config.channels.clone(),
            bends: [8192; 12],
            active_notes,
            sustain_by_input_channel: [false; 16],
            bend_range: config.bend_range,
        }
    }
}
