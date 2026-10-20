//! A backend that uses twelve midi channels and pitchbend. Works for tuning systems that have an
//! [OctavePeriodicStackType].
//!

use std::time::Instant;

use midi_msg::{Channel, ChannelVoiceMsg, ControlChange, MidiMsg};
use serde_derive::{Deserialize, Serialize};

use crate::{
    adaptors::take_replace,
    backend::r#trait::BackendAdaptorNew,
    custom_serde::common::{deserialize_channels, serialize_channels},
    interval::{base::Semitones, stacktype::r#trait::StackType},
    msg::{self, FromBackend, ReceiveMsg, ToBackend},
    process::r#trait::StackWithTuning,
    util::ordered_locks::Zero,
};

use adaptuner_core::pitch_bend::decide_retuning;

pub struct Pitchbend12<T: StackType> {
    /// invariant: the bend pertaining to `channels[i]` is in `bends[i]`
    bends: [u16; 12],

    adaptor: Option<BackendAdaptorNew<T, Zero>>,
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
    pub fn new(adaptor: BackendAdaptorNew<T, Zero>) -> Self {
        Self {
            bends: [8192; 12],
            adaptor: Some(adaptor),
        }
    }

    fn send(&self, msg: FromBackend) {
        self.adaptor.as_ref().unwrap().send(msg)
    }

    fn handle_note_on(&mut self, note: u8, velocity: u8, time: Instant) {
        self.handle_retune(note, time);

        take_replace(&mut self.adaptor, |mut adaptor| {
            let channel;
            (channel, adaptor) =
                adaptor.backend_config(|conf, _| conf.channels[note as usize % 12]);
            adaptor.send(msg::FromBackend::OutgoingMidi {
                time,
                bytes: adaptuner_core::midi::note_on(channel as u8, note, velocity)
                    .expect("valid MIDI channel, note, and velocity")
                    .to_vec(),
            });
            ((), adaptor)
        });
    }

    fn handle_retune(&mut self, note: u8, time: Instant) {
        let mut tuning = 0.0; // dummy initialisation
        let mut bend_range = 0.0; // dummy initialisation;
        let channel_index = note as usize % 12;
        let mut channel = Channel::Ch1; // dummy initialisation
        take_replace(&mut self.adaptor, |mut adaptor| {
            (tuning, adaptor) = adaptor
                .tuning(note as usize, |StackWithTuning { semitones, .. }, _| {
                    *semitones
                });

            ((bend_range, channel), adaptor) =
                adaptor.backend_config(|conf, _| (conf.bend_range, conf.channels[channel_index]));
            ((), adaptor)
        });
        let decision = decide_retuning(note, tuning, bend_range, self.bends[channel_index]);

        if decision.should_send {
            self.send(msg::FromBackend::OutgoingMidi {
                time,
                bytes: adaptuner_core::midi::pitch_bend(channel as u8, decision.desired_bend)
                    .expect("valid MIDI channel and pitch bend")
                    .to_vec(),
            });

            self.bends[channel_index] = decision.desired_bend;
        }

        if decision.out_of_range {
            let _ = self.send(FromBackend::DetunedNote {
                note,
                actual: decision.actual_semitones,
                should_be: tuning,
                explanation: "exceeded bend range",
            });
        }
    }

    fn reset(&mut self, time: Instant) {
        self.bends = [8192; 12];
        take_replace(&mut self.adaptor, |adaptor| {
            adaptor.backend_config(|config, adaptor| {
                for channel in config.channels {
                    adaptor.send(msg::FromBackend::OutgoingMidi {
                        time,
                        bytes: adaptuner_core::midi::pitch_bend(channel as u8, 8192)
                            .expect("valid MIDI channel and pitch bend")
                            .to_vec(),
                    });
                }
            })
        });

        for note in 0..128 {
            let mut sounding = false; // dummy initialisation
            take_replace(&mut self.adaptor, |mut adaptor| {
                (sounding, adaptor) = adaptor.key_state(note, |k, _| k.is_sounding());
                ((), adaptor)
            });
            if sounding {
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

            msg::ToBackend::Stop { .. } => {}

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
                take_replace(&mut self.adaptor, |adaptor| {
                    adaptor.backend_config(|conf, adaptor| {
                        let channel = conf.channels[note as usize % 12];

                        let bytes = adaptuner_core::midi::note_off(channel as u8, note, velocity)
                            .expect("valid MIDI channel, note, and velocity")
                            .to_vec();

                        adaptor.send(msg::FromBackend::OutgoingMidi { time, bytes });
                    })
                });
            }

            ToBackend::PedalHold { value, time, .. } => {
                take_replace(&mut self.adaptor, |adaptor| {
                    adaptor.backend_config(|conf, adaptor| {
                        for channel in conf.channels {
                            adaptor.send(as_midi(
                                MidiMsg::ChannelVoice {
                                    channel,
                                    msg: ChannelVoiceMsg::ControlChange {
                                        control: ControlChange::Hold(value),
                                    },
                                },
                                time,
                            ));
                        }
                    })
                });
            }

            ToBackend::ProgramChange { program, time, .. } => {
                take_replace(&mut self.adaptor, |adaptor| {
                    adaptor.backend_config(|conf, adaptor| {
                        for channel in conf.channels {
                            adaptor.send(as_midi(
                                MidiMsg::ChannelVoice {
                                    channel,
                                    msg: ChannelVoiceMsg::ProgramChange { program },
                                },
                                time,
                            ));
                        }
                    })
                });
            }

            ToBackend::Retune { note, time } => {
                self.handle_retune(note, time);
            }

            ToBackend::UpdateBendRange { time } => {
                self.reset(time);
            }

            ToBackend::UpdateChannelsToUse { time } => {
                self.reset(time);
            }
        }
    }
}

#[cfg(test)]
mod test {
    use midi_msg::{Channel, ChannelVoiceMsg, MidiMsg};

    #[test]
    fn pitch_bend_encoding_matches_midi_msg() {
        for channel in [Channel::Ch1, Channel::Ch2, Channel::Ch12] {
            for bend in [0, 1, 8192, 16383] {
                let expected = MidiMsg::ChannelVoice {
                    channel,
                    msg: ChannelVoiceMsg::PitchBend { bend },
                }
                .to_midi();

                let actual = adaptuner_core::midi::pitch_bend(channel as u8, bend)
                    .unwrap()
                    .to_vec();

                assert_eq!(actual, expected);
            }
        }
    }
}
