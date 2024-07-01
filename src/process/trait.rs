use std::{sync::mpsc, time::Instant};

use crate::{msg, util::dimension::Dimension};

pub trait ProcessState<D: Dimension, T: Dimension> {
    fn handle_msg(
        &mut self,
        time: Instant,
        msg: msg::ToProcess<D, T>,
        to_backend: &mpsc::Sender<(Instant, msg::ToBackend)>,
        to_ui: &mpsc::Sender<(Instant, msg::ToUI<D, T>)>,
    );
}
