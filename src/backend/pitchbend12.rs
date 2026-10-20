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

use adaptuner_core::{
    notes::{ActiveNote, ActiveNotes, NoteId},
    pitch_bend_strategy::PitchBendStrategy,
    retuning::RetuningEngine,
};

fn changed_pitch_bend(channel: Channel, current: u16, desired: u16) -> Option<[u8; 3]> {
    if current == desired {
        None
    } else {
        Some(
            adaptuner_core::midi::pitch_bend(channel as u8, desired)
                .expect("valid MIDI channel and pitch bend"),
        )
    }
}

fn emit_note_on_sequence(
    bend_message: Option<[u8; 3]>,
    note_on_message: [u8; 3],
    mut emit: impl FnMut([u8; 3]),
) {
    if let Some(bend_message) = bend_message {
        emit(bend_message);
    }
    emit(note_on_message);
}

struct ActiveNoteTracker {
    notes: ActiveNotes<128>,
}

impl ActiveNoteTracker {
    fn new() -> Self {
        Self {
            notes: ActiveNotes::new(),
        }
    }

    fn note_on_or_retune(&mut self, note: u8, target_pitch: Semitones) -> NoteId {
        let id = NoteId::new(note as u32);
        if let Some(active_note) = self.notes.get_mut(id) {
            active_note.set_target_pitch(target_pitch);
        } else {
            self.notes
                .insert(ActiveNote::new(id, note as Semitones, target_pitch))
                .expect("one active-note slot exists for each MIDI key");
        }
        id
    }

    fn get(&self, id: NoteId) -> Option<&ActiveNote> {
        self.notes.get(id)
    }

    fn note_off(&mut self, note: u8, still_sounding: bool) {
        if !still_sounding {
            self.notes.remove(NoteId::new(note as u32));
        }
    }

    fn remove_silent(&mut self, mut is_sounding: impl FnMut(u8) -> bool) {
        for note in 0..128 {
            self.note_off(note, is_sounding(note));
        }
    }

    fn clear(&mut self) {
        self.notes.clear();
    }
}

pub struct Pitchbend12<T: StackType> {
    /// invariant: the bend pertaining to `channels[i]` is in `bends[i]`
    bends: [u16; 12],
    active_notes: ActiveNoteTracker,
    retuning: RetuningEngine<PitchBendStrategy>,

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
            active_notes: ActiveNoteTracker::new(),
            retuning: RetuningEngine::new(PitchBendStrategy::new(0.0)),
            adaptor: Some(adaptor),
        }
    }

    fn send(&self, msg: FromBackend) {
        self.adaptor.as_ref().unwrap().send(msg)
    }

    fn handle_note_on(&mut self, note: u8, velocity: u8, time: Instant) {
        // A changed bend must reach the channel before the newly tuned note starts.
        let bend_message = self.handle_retune(note);
        let mut channel = Channel::Ch1; // dummy initialisation
        take_replace(&mut self.adaptor, |mut adaptor| {
            (channel, adaptor) =
                adaptor.backend_config(|conf, _| conf.channels[note as usize % 12]);
            ((), adaptor)
        });
        let note_on_message = adaptuner_core::midi::note_on(channel as u8, note, velocity)
            .expect("valid MIDI channel, note, and velocity");
        emit_note_on_sequence(bend_message, note_on_message, |bytes| {
            self.send(msg::FromBackend::OutgoingMidi {
                time,
                bytes: bytes.to_vec(),
            });
        });
    }

    fn handle_retune(&mut self, note: u8) -> Option<[u8; 3]> {
        let mut tuning = 0.0; // dummy initialisation
        let mut bend_range = 0.0; // dummy initialisation;
        let mut channel = Channel::Ch1; // dummy initialisation
        take_replace(&mut self.adaptor, |mut adaptor| {
            (tuning, adaptor) = adaptor
                .tuning(note as usize, |StackWithTuning { semitones, .. }, _| {
                    *semitones
                });

            (bend_range, adaptor) = adaptor.backend_config(|conf, _| conf.bend_range);
            ((), adaptor)
        });
        // The process layer models one sounding state per MIDI key, so the key
        // is the stable identity for this adapter's active-note lifecycle.
        let note_id = self.active_notes.note_on_or_retune(note, tuning);

        *self.retuning.strategy_mut() = PitchBendStrategy::new(bend_range);
        let command = self
            .active_notes
            .get(note_id)
            .map(|active_note| self.retuning.retune(active_note))
            .expect("active note was inserted before retuning");
        let channel_index = command.note_id.get() as usize % 12;
        take_replace(&mut self.adaptor, |mut adaptor| {
            (channel, adaptor) = adaptor.backend_config(|conf, _| conf.channels[channel_index]);
            ((), adaptor)
        });

        let bend_message = changed_pitch_bend(channel, self.bends[channel_index], command.value);
        if bend_message.is_some() {
            self.bends[channel_index] = command.value;
        }

        if command.out_of_range {
            let _ = self.send(FromBackend::DetunedNote {
                note,
                actual: command.actual_pitch,
                should_be: tuning,
                explanation: "exceeded bend range",
            });
        }

        bend_message
    }

    fn reset(&mut self, time: Instant) {
        self.bends = [8192; 12];
        self.active_notes.clear();
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
                if let Some(bytes) = self.handle_retune(note as u8) {
                    self.send(msg::FromBackend::OutgoingMidi {
                        time,
                        bytes: bytes.to_vec(),
                    });
                }
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

                let mut still_sounding = false;
                take_replace(&mut self.adaptor, |mut adaptor| {
                    (still_sounding, adaptor) =
                        adaptor.key_state(note as usize, |key_state, _| key_state.is_sounding());
                    ((), adaptor)
                });
                self.active_notes.note_off(note, still_sounding);
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

                if value == 0 {
                    let mut sounding_by_key = [false; 128];
                    for note in 0..128 {
                        take_replace(&mut self.adaptor, |mut adaptor| {
                            (sounding_by_key[note], adaptor) =
                                adaptor.key_state(note, |key_state, _| key_state.is_sounding());
                            ((), adaptor)
                        });
                    }
                    self.active_notes
                        .remove_silent(|note| sounding_by_key[note as usize]);
                }
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
                if let Some(bytes) = self.handle_retune(note) {
                    self.send(msg::FromBackend::OutgoingMidi {
                        time,
                        bytes: bytes.to_vec(),
                    });
                }
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
    use std::time::Instant;

    use super::{changed_pitch_bend, emit_note_on_sequence, ActiveNoteTracker, NoteId};
    use crate::keystate::KeyState;

    #[test]
    fn note_on_retune_and_multiple_input_channels_keep_one_active_key() {
        let now = Instant::now();
        let mut key_state = KeyState::new(now);
        let mut active = ActiveNoteTracker::new();
        key_state.note_on(Channel::Ch1, now);
        active.note_on_or_retune(60, 60.0);

        key_state.note_on(Channel::Ch2, now);
        active.note_on_or_retune(60, 60.25);
        assert_eq!(active.notes.len(), 1);
        assert_eq!(active.get(NoteId::new(60)).unwrap().target_pitch(), 60.25);

        key_state.note_off(Channel::Ch1, false, now);
        active.note_off(60, key_state.is_sounding());
        assert!(active.get(NoteId::new(60)).is_some());

        key_state.note_off(Channel::Ch2, false, now);
        active.note_off(60, key_state.is_sounding());
        assert!(active.get(NoteId::new(60)).is_none());
    }

    #[test]
    fn sustain_keeps_note_active_until_pedal_release() {
        let now = Instant::now();
        let mut key_state = KeyState::new(now);
        let mut active = ActiveNoteTracker::new();
        key_state.note_on(Channel::Ch1, now);
        active.note_on_or_retune(64, 64.0);

        key_state.note_off(Channel::Ch1, true, now);
        active.note_off(64, key_state.is_sounding());
        assert!(active.get(NoteId::new(64)).is_some());

        key_state.pedal_off(Channel::Ch1, now);
        active.remove_silent(|_| key_state.is_sounding());
        assert!(active.get(NoteId::new(64)).is_none());
    }

    #[test]
    fn reset_clears_active_note_tracking() {
        let mut active = ActiveNoteTracker::new();
        active.note_on_or_retune(60, 60.0);
        active.note_on_or_retune(67, 67.0);
        active.clear();
        assert!(active.notes.is_empty());
    }

    #[test]
    fn changed_pitch_bend_is_emitted_before_note_on() {
        let bend = changed_pitch_bend(Channel::Ch2, 8192, 10239);
        let note_on = adaptuner_core::midi::note_on(Channel::Ch2 as u8, 60, 100).unwrap();
        let mut emitted = Vec::new();

        emit_note_on_sequence(bend, note_on, |message| emitted.push(message));

        assert_eq!(
            emitted,
            vec![
                adaptuner_core::midi::pitch_bend(Channel::Ch2 as u8, 10239).unwrap(),
                note_on,
            ]
        );
    }

    #[test]
    fn unchanged_pitch_bend_is_not_repeated_before_note_on() {
        let bend = changed_pitch_bend(Channel::Ch2, 8192, 8192);
        let note_on = adaptuner_core::midi::note_on(Channel::Ch2 as u8, 60, 100).unwrap();
        let mut emitted = Vec::new();

        emit_note_on_sequence(bend, note_on, |message| emitted.push(message));

        assert_eq!(emitted, vec![note_on]);
    }

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
