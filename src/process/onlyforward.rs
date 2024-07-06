use std::{sync::mpsc, time::Instant};

use midi_msg::{ChannelVoiceMsg, MidiMsg};

use crate::{
    config::r#trait::Config, interval::Semitones, msg, process::r#trait::ProcessState,
    util::dimension::Dimension,
};

pub struct OnlyForward {}

impl OnlyForward {
    fn handle_midi_msg<D: Dimension, T: Dimension>(
        &mut self,
        time: Instant,
        bytes: &Vec<u8>,
        to_backend: &mpsc::Sender<(Instant, msg::ToBackend)>,
        to_ui: &mpsc::Sender<(Instant, msg::ToUI<D, T>)>,
    ) {
        let send_to_backend =
            |msg: msg::ToBackend, time: Instant| to_backend.send((time, msg)).unwrap_or(());

        let send_to_ui =
            |msg: msg::ToUI<D, T>, time: Instant| to_ui.send((time, msg)).unwrap_or(());

        match MidiMsg::from_midi(&bytes) {
            Err(e) => send_to_ui(msg::ToUI::MidiParseErr(e), time),
            Ok((msg, _number_of_bytes_parsed)) => match msg {
                MidiMsg::ChannelVoice {
                    channel,
                    msg: ChannelVoiceMsg::NoteOn { note, velocity },
                } => {
                    send_to_backend(
                        msg::ToBackend::TunedNoteOn {
                            channel,
                            note,
                            velocity,
                            tuning: note as Semitones,
                        },
                        time,
                    );
                    send_to_ui(msg::ToUI::NoteOn { note }, time);
                }

                MidiMsg::ChannelVoice {
                    channel,
                    msg: ChannelVoiceMsg::NoteOff { note, velocity },
                } => {
                    send_to_backend(
                        msg::ToBackend::NoteOff {
                            channel,
                            note,
                            velocity,
                        },
                        time,
                    );
                    send_to_ui(msg::ToUI::NoteOff { note }, time);
                }

                _ => {
                    send_to_ui(
                        msg::ToUI::Notify {
                            line: match MidiMsg::from_midi(&bytes) {
                                Ok((m, _)) => format!("{:?}", m),
                                _ => format!("raw midi bytes sent to backend: {:?}", &bytes),
                            },
                        },
                        time,
                    );
                    send_to_backend(
                        msg::ToBackend::ForwardBytes {
                            bytes: bytes.to_vec(),
                        },
                        time,
                    );
                }
            },
        }
    }
}

impl<D: Dimension, T: Dimension> ProcessState<D, T> for OnlyForward {
    fn handle_msg(
        &mut self,
        time: Instant,
        msg: crate::msg::ToProcess<D, T>,
        to_backend: &mpsc::Sender<(Instant, msg::ToBackend)>,
        to_ui: &mpsc::Sender<(Instant, msg::ToUI<D, T>)>,
    ) {
        match msg {
            msg::ToProcess::IncomingMidi { bytes } => {
                self.handle_midi_msg(time, &bytes, to_backend, to_ui)
            }

            _ => {}
        }
    }
}

#[derive(Clone)]
pub struct OnlyForwardConfig {}

impl Config<OnlyForward> for OnlyForwardConfig {
    fn initialise(_: &Self) -> OnlyForward {
        OnlyForward {}
    }
}
