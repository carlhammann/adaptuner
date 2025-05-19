//! A backend that uses twelve midi channels and pitchbend. Works for tuning systems that have an
//! [OctavePeriodicStackType].
//!

use std::{sync::mpsc, time::Instant};

use midi_msg::{Channel, ChannelModeMsg, ChannelVoiceMsg, ControlChange, MidiMsg};

use crate::{
    backend::r#trait::BackendState,
    config::r#trait::Config,
    interval::{base::Semitones, stacktype::r#trait::OctavePeriodicStackType},
    keystate::KeyState,
    msg,
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
        midi_out: &mpsc::Sender<(Instant, msg::ToMidiOut)>,
    ) {
        let send_to_ui =
            |msg: msg::AfterProcess<T>, time: Instant| to_ui.send((time, msg)).unwrap_or(());

        let send_midi = |msg: MidiMsg, time: Instant| {
            midi_out
                .send((
                    time,
                    msg::ToMidiOut::OutgoingMidi {
                        bytes: msg.to_midi(),
                    },
                ))
                .unwrap_or(());
        };

        match msg {
            msg::AfterProcess::Start | msg::AfterProcess::Reset => {
                *self = Pitchbend12Config::initialise(&self.config);
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
            msg::AfterProcess::Stop => {}
            msg::AfterProcess::ForwardMidi { msg } => match msg {
                MidiMsg::ChannelVoice {
                    channel,
                    msg: ChannelVoiceMsg::NoteOn { note, velocity },
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

                MidiMsg::ChannelVoice {
                    channel,
                    msg: ChannelVoiceMsg::NoteOff { note, velocity },
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
                            time,
                        )
                    }
                }

                _ => send_midi(msg, time),
            },
            msg::AfterProcess::FromStrategy(msg) => match msg {
                msg::FromStrategy::Retune { note, tuning, .. } => {
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
                        send_to_ui(
                            msg::AfterProcess::DetunedNote {
                                note,
                                actual: note as Semitones + self.semitones_from_bend(desired_bend),
                                should_be: tuning,
                                explanation: "Exceeded bend range",
                            },
                            time,
                        );
                    }
                }
                _ => {}
            },

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
        let now = Instant::now();
        Pitchbend12 {
            config: config.clone(),
            channels: config.channels.clone(),
            bends: [8192; 12],
            key_state: core::array::from_fn(|_| KeyState::new(now)),
            pedal_hold: [false; 16],
            bend_range: config.bend_range,
        }
    }
}
