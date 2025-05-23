//! A backend that uses twelve midi channels and pitchbend. Works for tuning systems that have an
//! [OctavePeriodicStackType].
//!

use std::{sync::mpsc, time::Instant};

use midi_msg::{Channel, ChannelModeMsg, ChannelVoiceMsg, ControlChange, MidiMsg};

use crate::{
    interval::base::Semitones,
    keystate::KeyState,
    msg::{self, FromBackend, HandleMsg, ToBackend},
};

pub struct Pitchbend12 {
    config: Pitchbend12Config,

    /// the channels to use. Exlude CH10 for GM compatibility
    channels: [Channel; 12],

    /// invariant: the bend pertaining to `channels[i]` is in `bends[i]`
    bends: [u16; 12],

    key_state: [KeyState; 128],

    /// is the sustain pedal held at the moment? (for each channel)
    pedal_hold: [bool; 16],

    /// the current bend range
    bend_range: Semitones,
}

#[derive(Clone)]
pub struct Pitchbend12Config {
    pub bend_range: Semitones,
    pub channels: [Channel; 12],
}

impl Pitchbend12 {
    pub fn new(config: &Pitchbend12Config) -> Self {
        let now = Instant::now();
        Self {
            config: config.clone(),
            channels: config.channels.clone(),
            bends: [8192; 12],
            key_state: core::array::from_fn(|_| KeyState::new(now)),
            pedal_hold: [false; 16],
            bend_range: config.bend_range,
        }
    }

    fn bend_from_semitones(&self, semitones: Semitones) -> u16 {
        ((8191.0 * semitones / self.bend_range + 8192.0) as u16)
            .max(0)
            .min(16383)
    }

    fn semitones_from_bend(&self, bend: u16) -> Semitones {
        (bend as Semitones - 8192.0) / 8191.0 * self.bend_range
    }

    fn handle_retune(
        &mut self,
        note: u8,
        tuning: Semitones,
        time: Instant,
        forward: &mpsc::Sender<FromBackend>,
    ) {
        let send_midi = |msg: MidiMsg, original_time: Instant| {
            let _ = forward.send(msg::FromBackend::OutgoingMidi {
                time: original_time,
                bytes: msg.to_midi(),
            });
        };

        let channel_index = note as usize % 12;
        let desired_bend = self.bend_from_semitones(tuning - note as Semitones);
        let current_bend = self.bends[channel_index];
        if current_bend != desired_bend {
            send_midi(
                MidiMsg::ChannelVoice {
                    channel: self.channels[channel_index],
                    msg: ChannelVoiceMsg::PitchBend { bend: desired_bend },
                },
                time,
            );
            self.bends[channel_index] = desired_bend;
        }
        if (tuning - note as Semitones).abs() > self.bend_range {
            let _ = forward.send(FromBackend::DetunedNote {
                note,
                actual: note as Semitones + self.semitones_from_bend(desired_bend),
                should_be: tuning,
                explanation: "Exceeded bend range",
            });
        }
    }
}

impl HandleMsg<ToBackend, FromBackend> for Pitchbend12 {
    fn handle_msg(&mut self, msg: ToBackend, forward: &mpsc::Sender<FromBackend>) {
        // let send_to_ui =
        //     |msg: msg::AfterProcess<T>, time: Instant| to_ui.send((time, msg)).unwrap_or(());

        let send_midi = |msg: MidiMsg, original_time: Instant| {
            let _ = forward.send(msg::FromBackend::OutgoingMidi {
                time: original_time,
                bytes: msg.to_midi(),
            });
        };

        match msg {
            msg::ToBackend::Start { time } | msg::ToBackend::Reset { time } => {
                *self = Pitchbend12::new(&self.config);
                for (i, &channel) in self.channels.iter().enumerate() {
                    send_midi(
                        MidiMsg::ChannelVoice {
                            channel,
                            msg: ChannelVoiceMsg::PitchBend {
                                bend: self.bends[i],
                            },
                        },
                        time,
                    );
                    send_midi(
                        MidiMsg::ChannelVoice {
                            channel,
                            msg: ChannelVoiceMsg::ControlChange {
                                control: ControlChange::Hold(0),
                            },
                        },
                        time,
                    );
                    send_midi(
                        MidiMsg::ChannelMode {
                            channel,
                            msg: ChannelModeMsg::AllSoundOff,
                        },
                        time,
                    );
                }
            }
            msg::ToBackend::Stop => {}
            ToBackend::ForwardMidi {
                msg,
                time: original_time,
            } => match msg {
                MidiMsg::ChannelVoice {
                    channel,
                    msg: ChannelVoiceMsg::NoteOn { note, velocity },
                } => {
                    send_midi(
                        MidiMsg::ChannelVoice {
                            channel: self.channels[note as usize % 12],
                            msg: ChannelVoiceMsg::NoteOn { note, velocity },
                        },
                        original_time,
                    );

                    self.key_state[note as usize].note_on(channel, original_time);
                }

                MidiMsg::ChannelVoice {
                    channel,
                    msg: ChannelVoiceMsg::NoteOff { note, velocity },
                } => {
                    send_midi(
                        MidiMsg::ChannelVoice {
                            channel: self.channels[note as usize % 12],
                            msg: ChannelVoiceMsg::NoteOff { note, velocity },
                        },
                        original_time,
                    );

                    self.key_state[note as usize].note_off(
                        channel,
                        self.pedal_hold[channel as usize],
                        original_time,
                    );
                }

                MidiMsg::ChannelVoice {
                    channel,
                    msg:
                        ChannelVoiceMsg::ControlChange {
                            control: ControlChange::Hold(value),
                        },
                } => {
                    for channel in self.channels {
                        send_midi(
                            MidiMsg::ChannelVoice {
                                channel,
                                msg: ChannelVoiceMsg::ControlChange {
                                    control: ControlChange::Hold(value),
                                },
                            },
                            original_time,
                        );
                    }

                    self.pedal_hold[channel as usize] = value != 0;

                    if value == 0 {
                        for s in self.key_state.iter_mut() {
                            s.pedal_off(channel, original_time);
                        }
                    }
                }

                MidiMsg::ChannelVoice {
                    channel: _,
                    msg: ChannelVoiceMsg::ProgramChange { program },
                } => {
                    for channel in self.channels {
                        send_midi(
                            MidiMsg::ChannelVoice {
                                channel,
                                msg: ChannelVoiceMsg::ProgramChange { program },
                            },
                            original_time,
                        )
                    }
                }

                _ => send_midi(msg, original_time),
            },
            ToBackend::Retune { note, tuning, time } => {
                self.handle_retune(note, tuning, time, forward);
            }
            ToBackend::TunedNoteOn {
                channel,
                note,
                velocity,
                tuning,
                time,
            } => {
                send_midi(
                    MidiMsg::ChannelVoice {
                        channel: self.channels[note as usize % 12],
                        msg: ChannelVoiceMsg::NoteOn { note, velocity },
                    },
                    time,
                );
                self.handle_retune(note, tuning, time, forward);

                self.key_state[note as usize].note_on(channel, time);
            }
        }
    }
}
