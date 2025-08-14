//! A backend that uses twelve midi channels and pitchbend. Works for tuning systems that have an
//! [OctavePeriodicStackType].
//!

use std::{sync::mpsc, time::Instant};

use midi_msg::{Channel, ChannelModeMsg, ChannelVoiceMsg, ControlChange, MidiMsg};
use serde_derive::{Deserialize, Serialize};

use crate::{
    config::{BackendConfig, ExtractConfig, FromConfigAndState},
    custom_serde::common::{deserialize_channel, serialize_channel},
    interval::base::Semitones,
    keystate::KeyState,
    msg::{self, FromBackend, HandleMsg, ToBackend},
};

pub struct Pitchbend12 {
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

#[derive(Serialize, Deserialize, Debug)]
#[serde(deny_unknown_fields)]
#[derive(Clone, Copy)]
pub struct WrappedChannel(
    #[serde(
        deserialize_with = "deserialize_channel",
        serialize_with = "serialize_channel"
    )]
    Channel,
);

impl From<WrappedChannel> for Channel {
    fn from(x: WrappedChannel) -> Self {
        let WrappedChannel(x) = x;
        x
    }
}

impl From<Channel> for WrappedChannel {
    fn from(x: Channel) -> Self {
        WrappedChannel(x)
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "kebab-case")]
#[derive(Clone)]
pub struct Pitchbend12Config {
    pub bend_range: Semitones,
    pub channels: [WrappedChannel; 12],
}

impl Pitchbend12 {
    pub fn new(config: Pitchbend12Config) -> Self {
        let now = Instant::now();
        Self {
            channels: core::array::from_fn(|i| config.channels[i].into()),
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
                explanation: "exceeded bend range",
            });
        }
    }

    fn reset(&mut self, time: Instant, forward: &mpsc::Sender<FromBackend>) {
        let send_midi = |msg: MidiMsg, original_time: Instant| {
            let _ = forward.send(msg::FromBackend::OutgoingMidi {
                time: original_time,
                bytes: msg.to_midi(),
            });
        };

        // the same initialisations as in [Pitchbend12::new].
        self.bends = [8192; 12];
        self.key_state = core::array::from_fn(|_| KeyState::new(time));
        self.pedal_hold = [false; 16];

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
}

impl HandleMsg<ToBackend, FromBackend> for Pitchbend12 {
    fn handle_msg(&mut self, msg: ToBackend, forward: &mpsc::Sender<FromBackend>) {
        let send_midi = |msg: MidiMsg, original_time: Instant| {
            let _ = forward.send(msg::FromBackend::OutgoingMidi {
                time: original_time,
                bytes: msg.to_midi(),
            });
        };

        match msg {
            msg::ToBackend::Start { time } | msg::ToBackend::Reset { time } => {
                self.reset(time, forward);
            }

            msg::ToBackend::Stop => {}

            ToBackend::NoteOn {
                time,
                channel,
                note,
                velocity,
            } => {
                send_midi(
                    MidiMsg::ChannelVoice {
                        channel: self.channels[note as usize % 12],
                        msg: ChannelVoiceMsg::NoteOn { note, velocity },
                    },
                    time,
                );

                self.key_state[note as usize].note_on(channel, time);
            }

            ToBackend::NoteOff {
                channel,
                note,
                velocity,
                time,
            } => {
                send_midi(
                    MidiMsg::ChannelVoice {
                        channel: self.channels[note as usize % 12],
                        msg: ChannelVoiceMsg::NoteOff { note, velocity },
                    },
                    time,
                );

                self.key_state[note as usize].note_off(
                    channel,
                    self.pedal_hold[channel as usize],
                    time,
                );
            }

            ToBackend::PedalHold {
                channel,
                value,
                time,
            } => {
                for channel in self.channels {
                    send_midi(
                        MidiMsg::ChannelVoice {
                            channel,
                            msg: ChannelVoiceMsg::ControlChange {
                                control: ControlChange::Hold(value),
                            },
                        },
                        time,
                    );
                }

                self.pedal_hold[channel as usize] = value != 0;

                if value == 0 {
                    for s in self.key_state.iter_mut() {
                        s.pedal_off(channel, time);
                    }
                }
            }

            ToBackend::ProgramChange {
                channel: _,
                program,
                time,
            } => {
                for channel in self.channels {
                    send_midi(
                        MidiMsg::ChannelVoice {
                            channel,
                            msg: ChannelVoiceMsg::ProgramChange { program },
                        },
                        time,
                    )
                }
            }

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

            ToBackend::BendRange { range, time } => {
                self.bend_range = range;
                self.reset(time, forward);
            }

            ToBackend::ChannelsToUse { channels, time } => {
                let mut i = 0;
                for (ch, used) in channels.iter().enumerate() {
                    if *used {
                        self.channels[i] = Channel::from_u8(ch as u8);
                        i += 1;
                    }
                }
                self.reset(time, forward);
            }
            ToBackend::GetCurrentConfig => {
                let _ = forward.send(FromBackend::CurrentConfig(self.extract_config()));
            }
            ToBackend::RestartWithConfig { config, time } => {
                *self = <Self as FromConfigAndState<_, _>>::initialise(config, ());
                self.reset(time, forward);
            }
            ToBackend::RestartWithCurrentConfig { time } => {
                *self = <Self as FromConfigAndState<_, _>>::initialise(self.extract_config(), ());
                self.reset(time, forward);
            }
        }
    }
}

impl ExtractConfig<BackendConfig> for Pitchbend12 {
    fn extract_config(&self) -> BackendConfig {
        BackendConfig::Pitchbend12(Pitchbend12Config {
            bend_range: self.bend_range,
            channels: core::array::from_fn(|i| WrappedChannel(self.channels[i])),
        })
    }
}

impl<S> FromConfigAndState<BackendConfig, S> for Pitchbend12 {
    fn initialise(config: BackendConfig, _state: S) -> Self {
        match config {
            BackendConfig::Pitchbend12(config) => Self::new(config),
        }
    }
}
