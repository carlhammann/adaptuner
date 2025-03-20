use std::{hash::Hash, sync::mpsc, time::Instant};

use crate::{interval::stacktype::r#trait::StackType, msg};

pub trait BackendState<T: StackType + Eq + Hash> {
    fn handle_msg(
        &mut self,
        time: Instant,
        msg: msg::AfterProcess<T>,
        to_ui: &mpsc::Sender<(Instant, msg::AfterProcess<T>)>,
        midi_out: &mpsc::Sender<(Instant, Vec<u8>)>,
    );
}
