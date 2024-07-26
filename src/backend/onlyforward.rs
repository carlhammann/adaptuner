use std::{sync::mpsc, time::Instant};

use midi_msg::{ChannelVoiceMsg, ControlChange, MidiMsg};

use crate::{
    backend::r#trait::BackendState, config::r#trait::Config,
    interval::stacktype::r#trait::StackType, msg,
};

pub struct OnlyForward {}

impl<T: StackType> BackendState<T> for OnlyForward {
    fn handle_msg(
        &mut self,
        time: Instant,
        msg: msg::AfterProcess<T>,
        _to_ui: &mpsc::Sender<(Instant, msg::AfterProcess<T>)>,
        midi_out: &mpsc::Sender<(Instant, Vec<u8>)>,
    ) {
        let send = |msg: MidiMsg, time: Instant| midi_out.send((time, msg.to_midi())).unwrap_or(());

        match msg {
            msg::AfterProcess::Start => {}
            msg::AfterProcess::Stop => {}
            msg::AfterProcess::Reset => {}
            msg::AfterProcess::TunedNoteOn {
                channel,
                note,
                velocity,
                ..
            } => send(
                MidiMsg::ChannelVoice {
                    channel,
                    msg: ChannelVoiceMsg::NoteOn { note, velocity },
                },
                time,
            ),

            msg::AfterProcess::NoteOff {
                channel,
                note,
                velocity,
                ..
            } => send(
                MidiMsg::ChannelVoice {
                    channel,
                    msg: ChannelVoiceMsg::NoteOff { note, velocity },
                },
                time,
            ),

            msg::AfterProcess::Sustain { channel, value } => send(
                MidiMsg::ChannelVoice {
                    channel,
                    msg: ChannelVoiceMsg::ControlChange {
                        control: ControlChange::Hold(value),
                    },
                },
                time,
            ),

            msg::AfterProcess::ProgramChange { channel, program } => send(
                MidiMsg::ChannelVoice {
                    channel,
                    msg: ChannelVoiceMsg::ProgramChange { program },
                },
                time,
            ),

            msg::AfterProcess::Retune { .. } => {}

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
pub struct OnlyForwardConfig {}

impl Config<OnlyForward> for OnlyForwardConfig {
    fn initialise(_config: &Self) -> OnlyForward {
        OnlyForward {}
    }
}
