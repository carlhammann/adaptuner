//! This backend uses Pitchbends, but doesn't assign pitch classes to channels before the fact. It
//! therefore makes it possible to tune as many notes independently as there are channels (i.e. at
//! most
//! 16). Thus, this is the correct backend if you want to work in tuning systems that are not
//!    octave-periodic, for example.
//!
//! This entails some subtleties with regard to input and output channels:
//!
//! - Notes are set to their correct tuning at the "note on". If you retune, It might be possible
//! that (a) you can't because you'll exceed the pitch bend range or (b) you inadvertently detune
//! other notes which were assigned the same channel during their note on.
//!
//! - Notes will be considered "on" exactly if there's at least one input channel on which they are
//! sounding. Thus, If you send a "note on" on Channel 1 and one on Channel 2, you'll also have to
//! send two corresponding "note off" events, or the note will continue sounding.
use std::{sync::mpsc, time::Instant};

use midi_msg::{Channel, ChannelModeMsg, ChannelVoiceMsg, ControlChange, MidiMsg};

use crate::{
    interval::base::Semitones,
    keystate::KeyState,
    msg::{FromBackend, HandleMsg, ToBackend},
};

struct AssignedNoteAndChannel {
    channel_index: usize,
    note: u8,
}

struct ChannelWithInfo {
    channel: Channel,
    bend: u16,
    n_active_notes: u8,
}

pub struct Pitchbend {
    config: PitchbendConfig,
    bend_range: Semitones,
    key_state: [KeyState; 128],
    pedal_hold: [bool; 16], //pertains to input channels
    channels_with_info: Vec<ChannelWithInfo>,
    assigned: [AssignedNoteAndChannel; 128],
}

#[derive(Clone)]
pub struct PitchbendConfig {
    pub bend_range: Semitones,
    pub channels: Vec<Channel>,
}

impl Pitchbend {
    pub fn new(config: &PitchbendConfig) -> Self {
        let now = Instant::now();
        Self {
            config: config.clone(),
            bend_range: config.bend_range,
            key_state: core::array::from_fn(|_| KeyState::new(now)),
            pedal_hold: [false; 16],
            channels_with_info: config
                .channels
                .iter()
                .map(|&channel| ChannelWithInfo {
                    channel,
                    bend: 8192,
                    n_active_notes: 0,
                })
                .collect(),
            assigned: core::array::from_fn(|i| AssignedNoteAndChannel {
                note: i as u8,
                channel_index: 0,
            }),
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

    fn closest_note_and_bend(&self, desired_tuning: Semitones) -> (u8, u16) {
        let closest_whole_note = desired_tuning.round();
        let remainder = desired_tuning - closest_whole_note;
        (
            closest_whole_note as u8,
            self.bend_from_semitones(remainder),
        )
    }

    fn send_retune(
        &mut self,
        note: u8,
        desired_tuning: Semitones,
        time: Instant,
        forward: &mpsc::Sender<FromBackend>,
    ) {
        let send_midi = |msg: MidiMsg, time: Instant| {
            let _ = forward.send(FromBackend::OutgoingMidi {
                time,
                bytes: msg.to_midi(),
            });
        };

        if !self.key_state[note as usize].is_sounding() {
            return;
        }

        let AssignedNoteAndChannel {
            channel_index: assigned_channel_index,
            note: assigned_note,
        } = self.assigned[note as usize];

        let desired_bend = self.bend_from_semitones(desired_tuning - assigned_note as Semitones);
        let output_channel_with_info = &self.channels_with_info[assigned_channel_index];
        let actual_bend = output_channel_with_info.bend;

        if actual_bend == desired_bend {
            return;
        }

        send_midi(
            MidiMsg::ChannelVoice {
                channel: output_channel_with_info.channel,
                msg: ChannelVoiceMsg::PitchBend { bend: desired_bend },
            },
            time,
        );

        if (desired_tuning - note as Semitones).abs() > self.bend_range {
            let _ = forward.send(FromBackend::DetunedNote {
                note,
                should_be: desired_tuning,
                actual: assigned_note as Semitones + self.semitones_from_bend(desired_bend),
                explanation: "outside of bend range",
            });
        }
    }

    fn send_tuned_note_on(
        &mut self,
        input_channel: Channel,
        note: u8,
        velocity: u8,
        desired_tuning: Semitones,
        time: Instant,
        forward: &mpsc::Sender<FromBackend>,
    ) {
        let send_midi = |msg: MidiMsg, time: Instant| {
            let _ = forward.send(FromBackend::OutgoingMidi {
                time,
                bytes: msg.to_midi(),
            });
        };

        if self.key_state[note as usize].is_sounding() {
            let AssignedNoteAndChannel {
                channel_index: assigned_channel_index,
                note: assigned_note,
            } = self.assigned[note as usize];
            send_midi(
                MidiMsg::ChannelVoice {
                    channel: self.channels_with_info[assigned_channel_index].channel,
                    msg: ChannelVoiceMsg::NoteOn {
                        note: assigned_note,
                        velocity,
                    },
                },
                time,
            );
            self.send_retune(note, desired_tuning, time, forward);
            return;
        }

        self.key_state[note as usize].note_on(input_channel, time);

        let (closest_note, desired_bend) = self.closest_note_and_bend(desired_tuning);

        // first try: try to find a channel that has the right bend
        for (i, channel_with_info) in self.channels_with_info.iter().enumerate() {
            if channel_with_info.bend == desired_bend {
                send_midi(
                    MidiMsg::ChannelVoice {
                        channel: self.channels_with_info[i].channel,
                        msg: ChannelVoiceMsg::NoteOn {
                            note: closest_note,
                            velocity,
                        },
                    },
                    time,
                );
                self.assigned[note as usize].note = closest_note;
                self.assigned[note as usize].channel_index = i;
                self.channels_with_info[i].n_active_notes += 1;
                return;
            }
        }

        // second try: try to find an unused channel
        for (i, channel_with_info) in self.channels_with_info.iter_mut().enumerate() {
            if channel_with_info.n_active_notes == 0 {
                send_midi(
                    MidiMsg::ChannelVoice {
                        channel: channel_with_info.channel,
                        msg: ChannelVoiceMsg::PitchBend { bend: desired_bend },
                    },
                    time,
                );
                send_midi(
                    MidiMsg::ChannelVoice {
                        channel: channel_with_info.channel,
                        msg: ChannelVoiceMsg::NoteOn {
                            note: closest_note,
                            velocity,
                        },
                    },
                    time,
                );
                channel_with_info.bend = desired_bend;
                self.assigned[note as usize].note = closest_note;
                self.assigned[note as usize].channel_index = i;
                self.channels_with_info[i].n_active_notes += 1;
                return;
            }
        }

        // if the first two methods failed: use the channel that has the closest bend
        let mut best_bend_index = 0;
        let mut best_bend = self.channels_with_info[0].bend;
        for i in 1..self.channels_with_info.len() {
            let new_bend = self.channels_with_info[i].bend;
            if (new_bend as i32 - desired_bend as i32).abs()
                < (best_bend as i32 - desired_bend as i32).abs()
            {
                best_bend_index = i;
                best_bend = new_bend;
            }
        }
        send_midi(
            MidiMsg::ChannelVoice {
                channel: self.channels_with_info[best_bend_index].channel,
                msg: ChannelVoiceMsg::NoteOn {
                    note: closest_note,
                    velocity,
                },
            },
            time,
        );
        self.assigned[note as usize].note = closest_note;
        self.assigned[note as usize].channel_index = best_bend_index;
        self.channels_with_info[best_bend_index].n_active_notes += 1;
        let _ = forward.send(FromBackend::DetunedNote {
            note,
            should_be: desired_tuning,
            actual: closest_note as Semitones + self.semitones_from_bend(best_bend),
            explanation:
                "all channels are used; I used the one with the closest possible pitch bend",
        });
    }

    fn send_note_off(
        &mut self,
        input_channel: Channel,
        note: u8,
        velocity: u8,
        time: Instant,
        forward: &mpsc::Sender<FromBackend>,
    ) {
        let send_midi = |msg: MidiMsg, time: Instant| {
            let _ = forward.send(FromBackend::OutgoingMidi {
                time,
                bytes: msg.to_midi(),
            });
        };

        let output_channel_index = self.assigned[note as usize].channel_index;
        let output_note = self.assigned[note as usize].note;
        let output_channel = self.channels_with_info[output_channel_index].channel;

        if self.key_state[note as usize].note_off(
            input_channel,
            self.pedal_hold[input_channel as usize],
            time,
        ) {
            send_midi(
                MidiMsg::ChannelVoice {
                    channel: output_channel,
                    msg: ChannelVoiceMsg::NoteOff {
                        note: output_note,
                        velocity,
                    },
                },
                time,
            );
            self.channels_with_info[output_channel_index].n_active_notes -= 1;
        }
    }
}

impl HandleMsg<ToBackend, FromBackend> for Pitchbend {
    fn handle_msg(&mut self, msg: ToBackend, forward: &mpsc::Sender<FromBackend>) {
        // let send_to_ui =
        //     |msg: AfterProcess<T>, time: Instant| to_ui.send((time, msg)).unwrap_or(());

        let send_midi = |msg: MidiMsg, time: Instant| {
            let _ = forward.send(FromBackend::OutgoingMidi {
                time,
                bytes: msg.to_midi(),
            });
        };

        match msg {
            ToBackend::Start { time } | ToBackend::Reset { time } => {
                *self = Pitchbend::new(&self.config);
                for channel_with_info in self.channels_with_info.iter() {
                    send_midi(
                        MidiMsg::ChannelVoice {
                            channel: channel_with_info.channel,
                            msg: ChannelVoiceMsg::PitchBend {
                                bend: channel_with_info.bend,
                            },
                        },
                        time,
                    );
                    send_midi(
                        MidiMsg::ChannelVoice {
                            channel: channel_with_info.channel,
                            msg: ChannelVoiceMsg::ControlChange {
                                control: ControlChange::Hold(0),
                            },
                        },
                        time,
                    );
                    send_midi(
                        MidiMsg::ChannelMode {
                            channel: channel_with_info.channel,
                            msg: ChannelModeMsg::AllSoundOff,
                        },
                        time,
                    );
                }
            }

            ToBackend::Stop => {}

            ToBackend::NoteOn {
                channel,
                time,
                note,
                velocity,
            } => {
                self.send_tuned_note_on(channel, note, velocity, note as Semitones, time, forward);
            }

            ToBackend::NoteOff {
                time,
                channel,
                note,
                velocity,
            } => {
                self.send_note_off(channel, note, velocity, time, forward);
            }

            ToBackend::PedalHold {
                channel,
                value,
                time,
            } => {
                for channel_with_info in self.channels_with_info.iter() {
                    send_midi(
                        MidiMsg::ChannelVoice {
                            channel: channel_with_info.channel,
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
                for channel_with_info in self.channels_with_info.iter() {
                    send_midi(
                        MidiMsg::ChannelVoice {
                            channel: channel_with_info.channel,
                            msg: ChannelVoiceMsg::ProgramChange { program },
                        },
                        time,
                    )
                }
            }

            ToBackend::Retune {
                note,
                tuning: desired_tuning,
                time,
            } => {
                if self.key_state[note as usize].is_sounding() {
                    self.send_retune(note, desired_tuning, time, forward);
                }
            }

            ToBackend::TunedNoteOn {
                channel,
                note,
                velocity,
                tuning,
                time,
            } => {
                self.send_tuned_note_on(channel, note, velocity, tuning, time, forward);
            }
        }
    }
}
