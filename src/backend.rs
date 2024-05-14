use std::sync::mpsc;

use midi_msg::{Channel, ChannelVoiceMsg, ControlChange, MidiMsg};

use crate::{interval::Semitones, msg, util::mod12::PitchClass};

pub trait BackendState {
    fn handle_msg(
        &mut self,
        time: u64,
        msg: msg::ToBackend,
        to_ui: &mpsc::Sender<msg::ToUI>,
        midi_out: &mpsc::Sender<(u64, Vec<u8>)>,
    );
}

pub struct OnlyForward {}

impl BackendState for OnlyForward {
    fn handle_msg(
        &mut self,
        time: u64,
        msg: msg::ToBackend,
        _to_ui: &mpsc::Sender<msg::ToUI>,
        midi_out: &mpsc::Sender<(u64, Vec<u8>)>,
    ) {
        match msg {
            msg::ToBackend::ForwardMidi { msg } => {
                //println!("{time}: {msg:?}");
                midi_out.send((time, msg.to_midi())).unwrap_or(())
            }
            _ => {}
        }
    }
}

pub struct PitchbendClasses {
    pub bends: [u16; 12],
    pub channels: [Channel; 12],
    pub bend_range: Semitones,
}

impl BackendState for PitchbendClasses {
    fn handle_msg(
        &mut self,
        time: u64,
        msg: msg::ToBackend,
        to_ui: &mpsc::Sender<msg::ToUI>,
        midi_out: &mpsc::Sender<(u64, Vec<u8>)>,
    ) {
        let send = |msg: MidiMsg, time: u64| midi_out.send((time, msg.to_midi())).unwrap_or(());
        match msg {
            msg::ToBackend::ForwardMidi { msg } => {
                self.send_channelshifted(msg, time, midi_out);
            }

            msg::ToBackend::RetuneClass { class, target } => {
                let bend = (8191.0 * (target - class as u8 as Semitones) / self.bend_range + 8192.0)
                    as u16;
                // if bend == self.bends[class as usize] {
                //     return;
                // }
                self.bends[class as usize] = bend;
                send(
                    MidiMsg::ChannelVoice {
                        channel: self.channels[class as usize],
                        msg: ChannelVoiceMsg::PitchBend { bend },
                    },
                    time,
                );
            }

            msg::ToBackend::RetuneNote { note, target } => {
                self.handle_msg(
                    time,
                    msg::ToBackend::RetuneClass {
                        class: PitchClass::from(note),
                        target: target % 12.0,
                    },
                    to_ui,
                    midi_out,
                );
            }
        }
    }
}

impl PitchbendClasses {
    fn send_channelshifted(
        &self,
        msg: MidiMsg,
        time: u64,
        midi_out: &mpsc::Sender<(u64, Vec<u8>)>,
    ) {
        let send = |msg: MidiMsg| midi_out.send((time, msg.to_midi())).unwrap_or(());
        match msg {
            MidiMsg::ChannelVoice {
                channel: _,
                msg: ChannelVoiceMsg::NoteOn { note, velocity },
            } => send(MidiMsg::ChannelVoice {
                channel: self.channels[note as usize % 12],
                msg: ChannelVoiceMsg::NoteOn { note, velocity },
            }),

            MidiMsg::ChannelVoice {
                channel: _,
                msg: ChannelVoiceMsg::NoteOff { note, velocity },
            } => send(MidiMsg::ChannelVoice {
                channel: self.channels[note as usize % 12],
                msg: ChannelVoiceMsg::NoteOff { note, velocity },
            }),

            MidiMsg::ChannelVoice {
                channel: _,
                msg:
                    ChannelVoiceMsg::ControlChange {
                        control: ControlChange::Hold(value),
                    },
            } => {
                for channel in self.channels {
                    send(MidiMsg::ChannelVoice {
                        channel,
                        msg: ChannelVoiceMsg::ControlChange {
                            control: ControlChange::Hold(value),
                        },
                    })
                }
            }

            MidiMsg::ChannelVoice {
                channel: _,
                msg: ChannelVoiceMsg::ProgramChange { program },
            } => {
                for channel in self.channels {
                    send(MidiMsg::ChannelVoice {
                        channel,
                        msg: ChannelVoiceMsg::ProgramChange { program },
                    })
                }
            }

            _ => {}
        }
    }
}
