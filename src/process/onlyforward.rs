use std::{sync::mpsc, time::Instant};

use midi_msg::{ChannelVoiceMsg, ControlChange, MidiMsg};

use crate::{
    config::r#trait::Config,
    interval::{interval::Semitones, stack::Stack, stacktype::r#trait::StackType},
    msg,
    process::r#trait::ProcessState,
};

pub struct OnlyForward {
    sustain: [bool; 16],
}

impl OnlyForward {
    fn handle_midi_msg<T: StackType>(
        &mut self,
        time: Instant,
        bytes: &Vec<u8>,
        to_backend: &mpsc::Sender<(Instant, msg::AfterProcess<T>)>,
    ) {
        let send_to_backend =
            |msg: msg::AfterProcess<T>, time: Instant| to_backend.send((time, msg)).unwrap_or(());

        match MidiMsg::from_midi(&bytes) {
            Err(e) => send_to_backend(msg::AfterProcess::MidiParseErr(e.to_string()), time),
            Ok((msg, _number_of_bytes_parsed)) => match msg {
                MidiMsg::ChannelVoice {
                    channel,
                    msg: ChannelVoiceMsg::NoteOn { note, velocity },
                } => {
                    send_to_backend(
                        msg::AfterProcess::TunedNoteOn {
                            channel,
                            note,
                            velocity,
                            tuning: note as Semitones,
                            tuning_stack: Stack::new_zero(),
                        },
                        time,
                    );
                }

                MidiMsg::ChannelVoice {
                    channel,
                    msg: ChannelVoiceMsg::NoteOff { note, velocity },
                } => {
                    send_to_backend(
                        msg::AfterProcess::NoteOff {
                            held_by_sustain: self.sustain[channel as usize],
                            channel,
                            note,
                            velocity,
                        },
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
                    self.sustain[channel as usize] = value != 0;
                    send_to_backend(msg::AfterProcess::Sustain { channel, value }, time);
                }

                _ => match MidiMsg::from_midi(&bytes) {
                    Ok((msg, _)) => {
                        send_to_backend(
                            msg::AfterProcess::Notify {
                                line: format!("{:?}", msg),
                            },
                            time,
                        );
                        send_to_backend(msg::AfterProcess::ForwardMidi { msg }, time);
                    }
                    _ => send_to_backend(
                        msg::AfterProcess::Notify {
                            line: format!("raw midi bytes sent to backend: {:?}", &bytes),
                        },
                        time,
                    ),
                },
            },
        }
    }
}

impl<T: StackType> ProcessState<T> for OnlyForward {
    fn handle_msg(
        &mut self,
        time: Instant,
        msg: crate::msg::ToProcess,
        to_backend: &mpsc::Sender<(Instant, msg::AfterProcess<T>)>,
    ) {
        match msg {
            msg::ToProcess::IncomingMidi { bytes } => {
                self.handle_midi_msg(time, &bytes, to_backend)
            }

            _ => {}
        }
    }
}

#[derive(Clone)]
pub struct OnlyForwardConfig {}

impl Config<OnlyForward> for OnlyForwardConfig {
    fn initialise(_: &Self) -> OnlyForward {
        OnlyForward {
            sustain: [false; 16],
        }
    }
}
