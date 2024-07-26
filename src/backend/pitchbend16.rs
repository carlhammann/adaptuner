use std::{sync::mpsc, time::Instant};

use midi_msg::{Channel, ChannelModeMsg, ChannelVoiceMsg, ControlChange, MidiMsg};

use crate::{
    backend::r#trait::BackendState,
    config::r#trait::Config,
    interval::{interval::Semitones, stacktype::r#trait::StackType},
    msg,
};

#[derive(Clone, Copy)]
struct NoteInfo {
    /// how this note should be tuned, in the best of all possible worlds. It will be tuned to the
    /// closest approximation possible using pitch bends, as long as it's not on a channel with
    /// another note that has to be tuned differently. This latter scenario will only happen when
    /// you have more than 16 sounding notes which require different pitch bends. (In "normal"
    /// music, with octave equivalence, this will never happen.)
    desired_tuning: Semitones,

    /// the channel this note is being played on, currently
    channel: Channel,

    /// the note number the note is played as on the channel. This will be the closest integer to
    /// `desired_tuning`. The pitchbend active on `channel` will bring this note closer to the
    /// ideal tuning (in the nominal case, exactly to it, whithin the presicion of pitchbend).
    mapped_to: u8,

    /// true iff the note is only held by the pedal
    sustained: bool,
}

pub struct Pitchbend16<const NCHANNELS: usize> {
    config: Pitchbend16Config<NCHANNELS>,

    /// the channels to use. Exlude CH10 for GM compatibility
    channels: [Channel; NCHANNELS],

    /// the pitch bend value of every channel
    bends: [u16; NCHANNELS],

    /// How many notes are currently sounding on each channel
    usage: [u8; NCHANNELS],

    /// which notes are currently active, and on which channel they sound, and which note they map
    /// to on that channel.
    active_notes: [Option<NoteInfo>; 128],

    /// is the sustain pedal held at the moment?
    sustained: bool,

    /// the current bend range
    bend_range: Semitones,
}

fn bend_from_semitones(bend_range: Semitones, semitones: Semitones) -> u16 {
    ((8191.0 * semitones / bend_range + 8192.0) as u16)
        .max(0)
        .min(16383)
}

fn semitones_from_bend(bend_range: Semitones, bend: u16) -> Semitones {
    (bend as Semitones - 8192.0) / 8191.0 * bend_range
}

impl<const NCHANNELS: usize, T: StackType> BackendState<T> for Pitchbend16<NCHANNELS> {
    fn handle_msg(
        &mut self,
        time: Instant,
        msg: msg::AfterProcess<T>,
        to_ui: &mpsc::Sender<(Instant, msg::AfterProcess<T>)>,
        midi_out: &mpsc::Sender<(Instant, Vec<u8>)>,
    ) {
        let send = |msg: MidiMsg, time: Instant| {
            midi_out.send((time, msg.to_midi())).unwrap_or(());
        };

        let send_to_ui = |msg: msg::AfterProcess<T>, time: Instant| to_ui.send((time, msg));

        let mapped_to_and_bend = |tuning: Semitones| {
            let mapped_to = tuning.round() as u8;
            let bend = bend_from_semitones(self.bend_range, tuning - mapped_to as Semitones);
            (mapped_to, bend)
        };

        match msg {
            msg::AfterProcess::Start | msg::AfterProcess::Reset => {
                *self = Pitchbend16Config::initialise(&self.config);
                for i in 0..NCHANNELS {
                    let channel = self.channels[i];
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
                note,
                velocity,
                tuning,
                ..
            } => {
                let (mapped_to, bend) = mapped_to_and_bend(tuning);

                let mut inserted = false;

                // Try to find a channel that already has the given pitch bend, and add the new
                // note to that channel:
                for i in 0..NCHANNELS {
                    if self.bends[i] == bend {
                        let channel = self.channels[i];
                        send(
                            MidiMsg::ChannelVoice {
                                channel,
                                msg: ChannelVoiceMsg::NoteOn {
                                    note: mapped_to,
                                    velocity,
                                },
                            },
                            time,
                        );

                        self.active_notes[note as usize] = Some(NoteInfo {
                            desired_tuning: tuning,
                            channel,
                            sustained: false,
                            mapped_to,
                        });
                        self.usage[i] += 1;

                        inserted = true;
                        break;
                    }
                }

                if !inserted {
                    // Now, we know that there was no channel with the pich bend we need. Thus, we
                    // try to find an unused channel and start using it with the add the new note
                    // with the correct pitch bend:
                    for i in 0..NCHANNELS {
                        if self.usage[i] == 0 {
                            let channel = self.channels[i];
                            send(
                                MidiMsg::ChannelVoice {
                                    channel,
                                    msg: ChannelVoiceMsg::PitchBend { bend },
                                },
                                time,
                            );
                            send(
                                MidiMsg::ChannelVoice {
                                    channel,
                                    msg: ChannelVoiceMsg::NoteOn {
                                        note: mapped_to,
                                        velocity,
                                    },
                                },
                                time,
                            );

                            self.active_notes[note as usize] = Some(NoteInfo {
                                desired_tuning: tuning,
                                channel,
                                sustained: false,
                                mapped_to,
                            });
                            self.usage[i] = 1;
                            self.bends[i] = bend;

                            inserted = true;
                            break;
                        }
                    }
                }

                if !inserted {
                    // Now, we know that all channels are used, and no channel has exactly the pitch
                    // bend we need. Thus, let's take the channel with the closest pitch bend, and send
                    // a notification to the ui about a detuned note.

                    let mut closest_channel = self.channels[0];
                    let mut dist = (bend as i32 - self.bends[0] as i32).abs();
                    for i in 1..NCHANNELS {
                        let new_dist = (bend as i32 - self.bends[i] as i32).abs();
                        if new_dist < dist {
                            dist = new_dist;
                            closest_channel = self.channels[i];
                        }
                    }

                    send(
                        MidiMsg::ChannelVoice {
                            channel: closest_channel,
                            msg: ChannelVoiceMsg::NoteOn {
                                note: mapped_to,
                                velocity,
                            },
                        },
                        time,
                    );

                    self.active_notes[note as usize] = Some(NoteInfo {
                        desired_tuning: tuning,
                        channel: closest_channel,
                        sustained: false,
                        mapped_to,
                    });
                    self.usage[closest_channel as usize] += 1;

                    let m = msg::AfterProcess::DetunedNote {
                        note,
                        should_be: tuning,
                        actual: semitones_from_bend(
                            self.bend_range,
                            self.bends[closest_channel as usize],
                        ),
                        explanation: "No more available channels on NoteOn",
                    };
                    // println!("{m:?}");
                    send_to_ui(m, time).unwrap_or(());
                }
            }

            msg::AfterProcess::NoteOff {
                held_by_sustain: _,
                channel: _,
                note,
                velocity,
            } => match self.active_notes[note as usize] {
                Some(NoteInfo {
                    channel,
                    sustained: _,
                    desired_tuning,
                    mapped_to,
                }) => {
                    send(
                        MidiMsg::ChannelVoice {
                            channel,
                            msg: ChannelVoiceMsg::NoteOff {
                                note: mapped_to,
                                velocity,
                            },
                        },
                        time,
                    );
                    if self.sustained {
                        self.active_notes[note as usize] = Some(NoteInfo {
                            desired_tuning,
                            channel,
                            mapped_to,
                            sustained: true,
                        });
                    } else {
                        self.active_notes[note as usize] = None;
                        self.usage[channel as usize] =
                            self.usage[channel as usize].saturating_sub(1);
                    }
                }
                None => {}
            },

            msg::AfterProcess::Sustain { channel: _, value } => {
                for i in 0..NCHANNELS {
                    send(
                        MidiMsg::ChannelVoice {
                            channel: self.channels[i],
                            msg: ChannelVoiceMsg::ControlChange {
                                control: ControlChange::Hold(value),
                            },
                        },
                        time,
                    );
                }
                self.sustained = value != 0;
                if value == 0 {
                    for i in 0..128 {
                        match self.active_notes[i] {
                            None => {}
                            Some(NoteInfo {
                                desired_tuning: _,
                                channel,
                                sustained,
                                mapped_to: _,
                            }) => {
                                if sustained {
                                    self.usage[channel as usize] =
                                        self.usage[channel as usize].saturating_sub(1);
                                    self.active_notes[i] = None;
                                }
                            }
                        }
                    }
                }
            }

            msg::AfterProcess::ProgramChange {
                channel: _,
                program,
            } => {
                for i in 0..NCHANNELS {
                    send(
                        MidiMsg::ChannelVoice {
                            channel: self.channels[i],
                            msg: ChannelVoiceMsg::ProgramChange { program },
                        },
                        time,
                    )
                }
            }

            msg::AfterProcess::Retune { note, tuning, .. } => {
                match self.active_notes[note as usize] {
                    None => {}
                    Some(NoteInfo {
                        desired_tuning: _,
                        channel,
                        sustained: _,
                        mapped_to,
                    }) => {
                        let bend =
                            bend_from_semitones(self.bend_range, tuning - mapped_to as Semitones);

                        if bend == self.bends[channel as usize] {
                            return;
                        }

                        send(
                            MidiMsg::ChannelVoice {
                                channel,
                                msg: ChannelVoiceMsg::PitchBend { bend },
                            },
                            time,
                        );

                        self.bends[channel as usize] = bend;

                        if (tuning - mapped_to as Semitones).abs() > self.bend_range {
                            let m = msg::AfterProcess::DetunedNote {
                                note,
                                should_be: tuning,
                                actual: mapped_to as Semitones
                                    + if tuning > note as Semitones {
                                        self.bend_range
                                    } else {
                                        -self.bend_range
                                    },
                                explanation: "Could not re-tune farther than the pitchbend range",
                            };
                            // println!("{m:?}");
                            send_to_ui(m, time).unwrap_or(());
                        }

                        if self.usage[channel as usize] > 1 {
                            for other_note in 0..128 {
                                match self.active_notes[other_note] {
                                    None => {}
                                    Some(NoteInfo {
                                        desired_tuning,
                                        channel: other_channel,
                                        mapped_to: other_mapped_to,
                                        sustained: _,
                                    }) => {
                                        if channel == other_channel && other_mapped_to != mapped_to
                                        {
                                            let other_bend = bend_from_semitones(
                                                self.bend_range,
                                                desired_tuning - other_mapped_to as Semitones,
                                            );

                                            if bend != other_bend {
                                                let m = msg::AfterProcess::DetunedNote {
                                                note: other_note as u8,
                                                should_be: desired_tuning,
                                                actual: other_mapped_to as Semitones
                                                    + semitones_from_bend( self.bend_range, bend),
                                                explanation: "Detuned because another note on the same channel was re-tuned",
                                            };
                                                // println!("{m:?}");
                                                send_to_ui(m, time).unwrap_or(());
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            msg::AfterProcess::ForwardMidi { msg } => send(msg, time),
            msg::AfterProcess::Notify { .. } => {}
            msg::AfterProcess::MidiParseErr(_) => {}
            msg::AfterProcess::DetunedNote { .. } => {}
            msg::AfterProcess::CrosstermEvent(_) => {}
            msg::AfterProcess::SetReference { .. } => {}
            msg::AfterProcess::Consider { .. } => {}
        }
    }
}

#[derive(Clone)]
pub struct Pitchbend16Config<const NCHANNELS: usize> {
    pub channels: [Channel; NCHANNELS],
    pub bend_range: Semitones,
}

impl<const NCHANNELS: usize> Config<Pitchbend16<NCHANNELS>> for Pitchbend16Config<NCHANNELS> {
    fn initialise(config: &Self) -> Pitchbend16<NCHANNELS> {
        Pitchbend16 {
            channels: config.channels,
            config: config.clone(),
            bends: [8192; NCHANNELS],
            usage: [0; NCHANNELS],
            active_notes: [None; 128],
            sustained: false,
            bend_range: config.bend_range,
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::interval::{
        stack::{stack_test_setup::init_stacktype, Stack},
        stacktype::generic::GenericStackType,
    };
    use std::sync::Arc;

    type MockStackType = GenericStackType;

    fn one_case<S>(
        state: &mut S,
        time: Instant,
        msg: msg::AfterProcess<MockStackType>,
        output_to_midi: Vec<(Instant, MidiMsg)>,
        output_to_ui: Vec<(Instant, msg::AfterProcess<MockStackType>)>,
    ) where
        S: BackendState<MockStackType>,
    {
        let (to_ui_tx, to_ui_rx) = mpsc::channel();
        let (midi_out_tx, midi_out_rx) = mpsc::channel();

        state.handle_msg(time, msg, &to_ui_tx, &midi_out_tx);

        assert_eq!(output_to_ui, to_ui_rx.try_iter().collect::<Vec<_>>());
        assert_eq!(
            output_to_midi,
            midi_out_rx
                .try_iter()
                .map(|(t, bytes)| (t, MidiMsg::from_midi(&bytes).unwrap().0))
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_sixteen_classes() {
        let mut s = Pitchbend16Config::<2>::initialise(
            &(Pitchbend16Config {
                channels: [Channel::Ch1, Channel::Ch2],
                bend_range: 2.0,
            }),
        );

        let mock_stack_type = Arc::new(init_stacktype());
        let mock_stack = Stack::new_zero(mock_stack_type.clone());

        let mut now = Instant::now();
        one_case(
            &mut s,
            now,
            msg::AfterProcess::TunedNoteOn {
                channel: Channel::Ch1,
                note: 3,
                velocity: 100,
                tuning: 3.2,
                tuning_stack: mock_stack.clone(),
            },
            vec![
                (
                    now,
                    MidiMsg::ChannelVoice {
                        channel: Channel::Ch1,
                        msg: ChannelVoiceMsg::PitchBend {
                            bend: bend_from_semitones(2.0, 0.2),
                        },
                    },
                ),
                (
                    now,
                    MidiMsg::ChannelVoice {
                        channel: Channel::Ch1,
                        msg: ChannelVoiceMsg::NoteOn {
                            note: 3,
                            velocity: 100,
                        },
                    },
                ),
            ],
            vec![],
        );

        now = Instant::now();
        one_case(
            &mut s,
            now,
            msg::AfterProcess::TunedNoteOn {
                channel: Channel::Ch1,
                note: 17,
                velocity: 101,
                tuning: 113.2,
                tuning_stack: mock_stack.clone(),
            },
            vec![(
                now,
                MidiMsg::ChannelVoice {
                    channel: Channel::Ch1,
                    msg: ChannelVoiceMsg::NoteOn {
                        note: 113,
                        velocity: 101,
                    },
                },
            )],
            vec![],
        );

        now = Instant::now();
        one_case(
            &mut s,
            now,
            msg::AfterProcess::TunedNoteOn {
                channel: Channel::Ch1,
                note: 4,
                velocity: 13,
                tuning: 3.7,
                tuning_stack: mock_stack.clone(),
            },
            vec![
                (
                    now,
                    MidiMsg::ChannelVoice {
                        channel: Channel::Ch2,
                        msg: ChannelVoiceMsg::PitchBend {
                            bend: bend_from_semitones(2.0, -0.3),
                        },
                    },
                ),
                (
                    now,
                    MidiMsg::ChannelVoice {
                        channel: Channel::Ch2,
                        msg: ChannelVoiceMsg::NoteOn {
                            note: 4,
                            velocity: 13,
                        },
                    },
                ),
            ],
            vec![],
        );

        now = Instant::now();
        one_case(
            &mut s,
            now,
            msg::AfterProcess::Sustain {
                channel: Channel::Ch1,
                value: 123,
            },
            {
                let mut many_sustains = Vec::new();
                for channel in [Channel::Ch1, Channel::Ch2] {
                    many_sustains.push((
                        now,
                        MidiMsg::ChannelVoice {
                            channel,
                            msg: ChannelVoiceMsg::ControlChange {
                                control: ControlChange::Hold(123),
                            },
                        },
                    ));
                }
                many_sustains
            },
            vec![],
        );

        now = Instant::now();
        one_case(
            &mut s,
            now,
            msg::AfterProcess::Retune {
                note: 3,
                tuning: 3.1,
                tuning_stack: mock_stack.clone(),
            },
            vec![(
                now,
                MidiMsg::ChannelVoice {
                    channel: Channel::Ch1,
                    msg: ChannelVoiceMsg::PitchBend {
                        bend: bend_from_semitones(2.0, 0.1),
                    },
                },
            )],
            vec![(
                now,
                msg::AfterProcess::DetunedNote {
                    note: 17,
                    should_be: 113.2,
                    actual: 113.0 + semitones_from_bend(2.0, bend_from_semitones(2.0, 0.1)),
                    explanation: "Detuned because another note on the same channel was re-tuned",
                },
            )],
        );

        now = Instant::now();
        one_case(
            &mut s,
            now,
            msg::AfterProcess::Retune {
                note: 4,
                tuning: 6.1,
                tuning_stack: mock_stack.clone(),
            },
            vec![(
                now,
                MidiMsg::ChannelVoice {
                    channel: Channel::Ch2,
                    msg: ChannelVoiceMsg::PitchBend { bend: 16383 },
                },
            )],
            vec![(
                now,
                msg::AfterProcess::DetunedNote {
                    note: 4,
                    should_be: 6.1,
                    actual: 6.0,
                    explanation: "Could not re-tune farther than the pitchbend range",
                },
            )],
        );
    }
}
