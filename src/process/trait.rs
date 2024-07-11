use std::{sync::mpsc, time::Instant};

use crate::{interval::StackType, msg};

pub trait ProcessState<T: StackType> {
    fn handle_msg(
        &mut self,
        time: Instant,
        msg: msg::ToProcess,
        to_backend: &mpsc::Sender<(Instant, msg::ToBackend)>,
        to_ui: &mpsc::Sender<(Instant, msg::ToUI<T>)>,
    );
}
