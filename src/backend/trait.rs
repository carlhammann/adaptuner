use std::sync::mpsc;

use crate::{msg, util::dimension::Dimension};

pub trait BackendState<D: Dimension, T: Dimension> {
    fn handle_msg(
        &mut self,
        time: u64,
        msg: msg::ToBackend,
        to_ui: &mpsc::Sender<(u64, msg::ToUI<D, T>)>,
        midi_out: &mpsc::Sender<(u64, Vec<u8>)>,
    );
}
