use std::sync::mpsc;

use crate::msg;

pub trait BackendState {
    fn handle_msg(
        &mut self,
        msg: msg::ToBackend,
        to_ui: &mpsc::Sender<msg::ToUI>,
        midi_out: &mpsc::Sender<(u64, Vec<u8>)>,
    );
}

pub struct OnlyForward {}

impl BackendState for OnlyForward {
    fn handle_msg(
        &mut self,
        msg: msg::ToBackend,
        _to_ui: &mpsc::Sender<msg::ToUI>,
        midi_out: &mpsc::Sender<(u64, Vec<u8>)>,
    ) {
        match msg {
            msg::ToBackend::ForwardMidi { msg, time } => {
                midi_out.send((time, msg.to_midi())).unwrap_or(())
            }
            _ => {}
        }
    }
}
