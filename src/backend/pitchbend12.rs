//! A backend that uses twelve midi channels and pitchbend. Works for tuning systems that have an
//! [OctavePeriodicStackType].
//!

use std::time::Instant;

use midi_msg::{Channel, ChannelVoiceMsg, ControlChange, MidiMsg};
use serde_derive::{Deserialize, Serialize};

use crate::{
    backend::r#trait::{BackendAdaptor, ConcreteBackendAdaptor},
    config::{BackendConfig, FromConfigAndState},
    custom_serde::common::{deserialize_channel, serialize_channel},
    interval::{base::Semitones, stacktype::r#trait::StackType},
    msg::{self, FromBackend, ReceiveMsg, ToBackend},
};

pub struct Pitchbend12<T: StackType> {
    /// the channels to use. Exlude CH10 for GM compatibility
    channels: [Channel; 12],

    /// invariant: the bend pertaining to `channels[i]` is in `bends[i]`
    bends: [u16; 12],

    /// the current bend range
    bend_range: Semitones,

    adaptor: ConcreteBackendAdaptor<T>,
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

impl<T: StackType> Pitchbend12<T> {
    fn send_msg(&self, msg: FromBackend) -> bool {
        self.adaptor.send(msg)
    }

    pub fn new(config: Pitchbend12Config, adaptor: ConcreteBackendAdaptor<T>) -> Self {
        Self {
            channels: core::array::from_fn(|i| config.channels[i].into()),
            bends: [8192; 12],
            bend_range: config.bend_range,
            adaptor,
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

    fn handle_note_on(&mut self, note: u8, velocity: u8, time: Instant) {
        self.handle_retune(note, time);

        let _ = self.adaptor.send(msg::FromBackend::OutgoingMidi {
            time,
            bytes: (MidiMsg::ChannelVoice {
                channel: self.channels[note as usize % 12],
                msg: ChannelVoiceMsg::NoteOn { note, velocity },
            })
            .to_midi(),
        });
    }

    fn handle_retune(&mut self, note: u8, time: Instant) {
        let tuning = self.adaptor.read_tuning(note as usize).tuning;

        let channel_index = note as usize % 12;
        let desired_bend = self.bend_from_semitones(tuning - note as Semitones);
        let current_bend = self.bends[channel_index];
        if current_bend != desired_bend {
            let _ = self.adaptor.send(msg::FromBackend::OutgoingMidi {
                time,
                bytes: (MidiMsg::ChannelVoice {
                    channel: self.channels[channel_index],
                    msg: ChannelVoiceMsg::PitchBend { bend: desired_bend },
                })
                .to_midi(),
            });
            self.bends[channel_index] = desired_bend;
        }
        if (tuning - note as Semitones).abs() > self.bend_range {
            let _ = self.send_msg(FromBackend::DetunedNote {
                note,
                actual: note as Semitones + self.semitones_from_bend(desired_bend),
                should_be: tuning,
                explanation: "exceeded bend range",
            });
        }
    }

    fn reset(&mut self, time: Instant) {
        let send_midi = |msg: MidiMsg, original_time: Instant| {
            let _ = self.adaptor.send(msg::FromBackend::OutgoingMidi {
                time: original_time,
                bytes: msg.to_midi(),
            });
        };

        // the same initialisation as in [Pitchbend12::new].
        self.bends = [8192; 12];

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
        }
    }
}

impl<T: StackType> ReceiveMsg<ToBackend> for Pitchbend12<T> {
    fn receive_msg(&mut self, msg: ToBackend) {
        let as_midi = |msg: MidiMsg, original_time: Instant| msg::FromBackend::OutgoingMidi {
            time: original_time,
            bytes: msg.to_midi(),
        };

        match msg {
            msg::ToBackend::Start { time } | msg::ToBackend::Reset { time } => {
                self.reset(time);
            }

            msg::ToBackend::Stop => {}

            ToBackend::NoteOn {
                time,
                note,
                velocity,
                ..
            } => {
                self.handle_note_on(note, velocity, time);
            }

            ToBackend::NoteOff {
                note,
                velocity,
                time,
                ..
            } => {
                self.send_msg(as_midi(
                    MidiMsg::ChannelVoice {
                        channel: self.channels[note as usize % 12],
                        msg: ChannelVoiceMsg::NoteOff { note, velocity },
                    },
                    time,
                ));
            }

            ToBackend::PedalHold { value, time, .. } => {
                for channel in self.channels {
                    self.send_msg(as_midi(
                        MidiMsg::ChannelVoice {
                            channel,
                            msg: ChannelVoiceMsg::ControlChange {
                                control: ControlChange::Hold(value),
                            },
                        },
                        time,
                    ));
                }
            }

            ToBackend::ProgramChange { program, time, .. } => {
                for channel in self.channels {
                    let _ = self.send_msg(as_midi(
                        MidiMsg::ChannelVoice {
                            channel,
                            msg: ChannelVoiceMsg::ProgramChange { program },
                        },
                        time,
                    ));
                }
            }

            ToBackend::Retune { note, time } => {
                self.handle_retune(note, time);
            }

            ToBackend::BendRange { range, time } => {
                self.bend_range = range;
                self.reset(time);
            }

            ToBackend::ChannelsToUse { channels, time } => {
                let mut i = 0;
                for (ch, used) in channels.iter().enumerate() {
                    if *used {
                        self.channels[i] = Channel::from_u8(ch as u8);
                        i += 1;
                    }
                }
                self.reset(time);
            }
        }
    }
}

impl<T: StackType> FromConfigAndState<BackendConfig, ConcreteBackendAdaptor<T>> for Pitchbend12<T> {
    fn initialise(config: BackendConfig, adaptor: ConcreteBackendAdaptor<T>) -> Self {
        match config {
            BackendConfig::Pitchbend12(config) => Self::new(config, adaptor),
        }
    }
}
