use std::{time::Instant, sync::mpsc};

use midi_msg::MidiMsg;

use crate::{
    config::r#trait::Config, msg, process::r#trait::ProcessState, util::dimension::Dimension,
};

pub struct OnlyForward {}

impl<D: Dimension, T: Dimension> ProcessState<D, T> for OnlyForward {
    fn handle_msg(
        &mut self,
        time: Instant,
        msg: crate::msg::ToProcess<D, T>,
        to_backend: &mpsc::Sender<(Instant, msg::ToBackend)>,
        to_ui: &mpsc::Sender<(Instant, msg::ToUI<D, T>)>,
    ) {
        let send_to_backend =
            |msg: msg::ToBackend, time: Instant| to_backend.send((time, msg)).unwrap_or(());

        let send_to_ui = |msg: msg::ToUI<D, T>, time: Instant| to_ui.send((time, msg)).unwrap_or(());

        match msg {
            msg::ToProcess::IncomingMidi { bytes } => {
                send_to_ui(
                    msg::ToUI::Notify {
                        line: match MidiMsg::from_midi(&bytes) {
                            Ok((m, _)) => format!("{:?}", m),
                            _ => format!("raw midi bytes sent to backend: {:?}", &bytes),
                        },
                    },
                    time,
                );
                send_to_backend(msg::ToBackend::ForwardBytes { bytes }, time);
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
