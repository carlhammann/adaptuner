use std::{sync::mpsc, time::Instant};

use midi_msg::{ChannelVoiceMsg, ControlChange, MidiMsg};

use crate::{
    backend::r#trait::BackendState, config::r#trait::Config, msg, util::dimension::Dimension,
};

pub struct OnlyForward {}

impl<D: Dimension, T: Dimension> BackendState<D, T> for OnlyForward {
    fn handle_msg(
        &mut self,
        time: Instant,
        msg: msg::ToBackend,
        _to_ui: &mpsc::Sender<(Instant, msg::ToUI<D, T>)>,
        midi_out: &mpsc::Sender<(Instant, Vec<u8>)>,
    ) {
        let send = |msg: MidiMsg, time: Instant| midi_out.send((time, msg.to_midi())).unwrap_or(());

        match msg {
            msg::ToBackend::Start => {}
            msg::ToBackend::Stop => {}
            msg::ToBackend::Reset => {}
            msg::ToBackend::TunedNoteOn {
                channel,
                note,
                velocity,
                tuning: _,
            } => send(
                MidiMsg::ChannelVoice {
                    channel,
                    msg: ChannelVoiceMsg::NoteOn { note, velocity },
                },
                time,
            ),

            msg::ToBackend::NoteOff {
                channel,
                note,
                velocity,
            } => send(
                MidiMsg::ChannelVoice {
                    channel,
                    msg: ChannelVoiceMsg::NoteOff { note, velocity },
                },
                time,
            ),

            msg::ToBackend::Sustain { channel, value } => send(
                MidiMsg::ChannelVoice {
                    channel,
                    msg: ChannelVoiceMsg::ControlChange {
                        control: ControlChange::Hold(value),
                    },
                },
                time,
            ),

            msg::ToBackend::ProgramChange { channel, program } => send(
                MidiMsg::ChannelVoice {
                    channel,
                    msg: ChannelVoiceMsg::ProgramChange { program },
                },
                time,
            ),

            msg::ToBackend::Retune { .. } => {}

            msg::ToBackend::ForwardMidi { msg } => send(msg, time),
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
