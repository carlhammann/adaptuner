use std::{
    sync::{mpsc, Arc},
    time::Instant,
};

use midi_msg::{ChannelVoiceMsg, MidiMsg};

use crate::{
    config::r#trait::Config,
    interval::{interval::Semitones, stack::Stack, stacktype::r#trait::StackType},
    msg,
    process::r#trait::ProcessState,
};

pub struct OnlyForward<T: StackType> {
    stacktype: Arc<T>,
}

impl<T: StackType> OnlyForward<T> {
    fn handle_midi_msg(
        &mut self,
        time: Instant,
        bytes: &Vec<u8>,
        to_backend: &mpsc::Sender<(Instant, msg::ToBackend)>,
        to_ui: &mpsc::Sender<(Instant, msg::ToUI<T>)>,
    ) {
        let send_to_backend =
            |msg: msg::ToBackend, time: Instant| to_backend.send((time, msg)).unwrap_or(());

        let send_to_ui = |msg: msg::ToUI<T>, time: Instant| to_ui.send((time, msg)).unwrap_or(());

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
                    send_to_ui(
                        msg::ToUI::TunedNoteOn {
                            note,
                            tuning: Stack::new_zero(self.stacktype.clone()),
                        },
                        time,
                    );
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

                _ => match MidiMsg::from_midi(&bytes) {
                    Ok((msg, _)) => {
                        send_to_ui(
                            msg::ToUI::Notify {
                                line: format!("{:?}", msg),
                            },
                            time,
                        );
                        send_to_backend(msg::ToBackend::ForwardMidi { msg }, time);
                    }
                    _ => send_to_ui(
                        msg::ToUI::Notify {
                            line: format!("raw midi bytes sent to backend: {:?}", &bytes),
                        },
                        time,
                    ),
                },
            },
        }
    }
}

impl<T: StackType> ProcessState<T> for OnlyForward<T> {
    fn handle_msg(
        &mut self,
        time: Instant,
        msg: crate::msg::ToProcess<T>,
        to_backend: &mpsc::Sender<(Instant, msg::ToBackend)>,
        to_ui: &mpsc::Sender<(Instant, msg::ToUI<T>)>,
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
pub struct OnlyForwardConfig<T: StackType> {
    stacktype: Arc<T>,
}

impl<T: StackType> Config<OnlyForward<T>> for OnlyForwardConfig<T> {
    fn initialise(config: &Self) -> OnlyForward<T> {
        OnlyForward {
            stacktype: config.stacktype.clone(),
        }
    }
}
