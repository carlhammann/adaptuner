use std::{hash::Hash, sync::mpsc, time::Instant};

use crate::{interval::stacktype::r#trait::StackType, msg};

pub trait ProcessState<T: StackType + Eq + Hash> {
    fn handle_msg(
        &mut self,
        time: Instant,
        msg: msg::ToProcess,
        to_backend: &mpsc::Sender<(Instant, msg::AfterProcess<T>)>,
    );
}
