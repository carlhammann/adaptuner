use std::sync::mpsc;

use crate::{msg, util::dimension::Dimension};

pub trait ProcessState<D: Dimension, T: Dimension> {
    fn handle_msg(
        &mut self,
        time: u64,
        msg: msg::ToProcess<D, T>,
        to_backend: &mpsc::Sender<(u64, msg::ToBackend)>,
        to_ui: &mpsc::Sender<(u64, msg::ToUI<D, T>)>,
    );
}
