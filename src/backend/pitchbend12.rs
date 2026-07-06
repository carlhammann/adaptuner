//! A backend that uses twelve midi channels and pitchbend. Works for tuning systems that have an
//! [OctavePeriodicStackType].
//!

use std::time::Instant;

use midi_msg::{Channel, ChannelVoiceMsg, ControlChange, MidiMsg};
use serde_derive::{Deserialize, Serialize};

use crate::{
    backend::r#trait::{BackendAdaptor, ConcretePitchbend12Adaptor, Pitchbend12Adaptor},
    custom_serde::common::{deserialize_channels, serialize_channels},
    interval::{base::Semitones, stacktype::r#trait::StackType},
    msg::{self, FromBackend, ReceiveMsg, ToBackend},
};

pub struct Pitchbend12<T: StackType> {
    /// invariant: the bend pertaining to `channels[i]` is in `bends[i]`
    bends: [u16; 12],

    adaptor: ConcretePitchbend12Adaptor<T>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "kebab-case")]
#[derive(Clone)]
pub struct Pitchbend12Config {
    pub bend_range: Semitones,

    #[serde(
        serialize_with = "serialize_channels",
        deserialize_with = "deserialize_channels"
    )]
    pub channels: [Channel; 12],
}

impl Pitchbend12Config {
    pub fn uses_channels(&self, used_channels_map: u16) -> bool {
        match self {
            Pitchbend12Config { channels, .. } => {
                let mut actual_used_channels_map = 0;
                for channel in channels {
                    actual_used_channels_map |= 1 << Channel::from(*channel) as u8;
                }
                actual_used_channels_map == used_channels_map
            }
        }
    }
}

impl<T: StackType> Pitchbend12<T> {
    pub fn new(adaptor: ConcretePitchbend12Adaptor<T>) -> Self {
        Self {
            bends: [8192; 12],
            adaptor,
        }
    }

    fn bend_from_semitones(&self, semitones: Semitones) -> u16 {
        ((8191.0 * semitones / self.adaptor.config().bend_range + 8192.0) as u16)
            .max(0)
            .min(16383)
    }

    fn semitones_from_bend(&self, bend: u16) -> Semitones {
        (bend as Semitones - 8192.0) / 8191.0 * self.adaptor.config().bend_range
    }

    fn handle_note_on(&mut self, note: u8, velocity: u8, time: Instant) {
        self.handle_retune(note, time);

        let _ = self.adaptor.send(msg::FromBackend::OutgoingMidi {
            time,
            bytes: (MidiMsg::ChannelVoice {
                channel: self.adaptor.config().channels[note as usize % 12],
                msg: ChannelVoiceMsg::NoteOn { note, velocity },
            })
            .to_midi(),
        });
    }

    fn handle_retune(&mut self, note: u8, time: Instant) {
        let tuning: Semitones = self.adaptor.tuning(note as usize).semitones;

        let channel_index = note as usize % 12;
        let desired_bend = self.bend_from_semitones(tuning - note as Semitones);
        let current_bend = self.bends[channel_index];
        if current_bend != desired_bend {
            let _ = self.adaptor.send(msg::FromBackend::OutgoingMidi {
                time,
                bytes: (MidiMsg::ChannelVoice {
                    channel: self.adaptor.config().channels[channel_index],
                    msg: ChannelVoiceMsg::PitchBend { bend: desired_bend },
                })
                .to_midi(),
            });
            self.bends[channel_index] = desired_bend;
        }
        if (tuning - note as Semitones).abs() > self.adaptor.config().bend_range {
            let _ = self.adaptor.send(FromBackend::DetunedNote {
                note,
                actual: note as Semitones + self.semitones_from_bend(desired_bend),
                should_be: tuning,
                explanation: "exceeded bend range",
            });
        }
    }

    fn reset(&mut self, time: Instant) {
        for note in 0..128 {
            if self.adaptor.key_state(note).is_sounding() {
                self.handle_retune(note as u8, time);
            }
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
                self.adaptor.send(as_midi(
                    MidiMsg::ChannelVoice {
                        channel: self.adaptor.config().channels[note as usize % 12],
                        msg: ChannelVoiceMsg::NoteOff { note, velocity },
                    },
                    time,
                ));
            }

            ToBackend::PedalHold { value, time, .. } => {
                for channel in self.adaptor.config().channels {
                    self.adaptor.send(as_midi(
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
                for channel in self.adaptor.config().channels {
                    let _ = self.adaptor.send(as_midi(
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
                self.adaptor.config().bend_range = range;
                self.reset(time);
            }

            ToBackend::ChannelsToUse { channels, time } => {
                let mut i = 0;
                for ch in 0..16 {
                    if 0 != channels & 1 << i {
                        self.adaptor.config().channels[i] = Channel::from_u8(ch as u8);
                        i += 1;
                    }
                }
                self.reset(time);
            }
        }
    }
}
