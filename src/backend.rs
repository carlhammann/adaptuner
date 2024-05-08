use std::sync::{mpsc, Arc};

use crate::{interval::StackType, msg, util::dimension::Dimension};

pub trait BackendState {
    fn handle_msg(
        &mut self,
        msg: msg::ToBackend,
        to_ui: &mpsc::Sender<msg::ToUI>,
        midi_out: &mpsc::Sender<(u64, Vec<u8>)>,
    );
}

pub struct OnlyForward<D: Dimension, T: Dimension> {
    pub st: Arc<StackType<D, T>>,
}

impl<D: Dimension, T: Dimension> BackendState for OnlyForward<D, T> {
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
