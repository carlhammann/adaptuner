use std::{sync::mpsc, time::Instant};

use crate::{msg, util::dimension::Dimension};

pub trait BackendState<D: Dimension, T: Dimension> {
    fn handle_msg(
        &mut self,
        time: Instant,
        msg: msg::ToBackend,
        to_ui: &mpsc::Sender<(Instant, msg::ToUI<D, T>)>,
        midi_out: &mpsc::Sender<(Instant, Vec<u8>)>,
    );
}
