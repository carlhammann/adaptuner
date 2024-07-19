use std::{sync::mpsc, time::Instant};

use crate::{interval::stacktype::r#trait::StackType, msg};

pub trait BackendState<T: StackType> {
    fn handle_msg(
        &mut self,
        time: Instant,
        msg: msg::ToBackend,
        to_ui: &mpsc::Sender<(Instant, msg::ToUI<T>)>,
        midi_out: &mpsc::Sender<(Instant, Vec<u8>)>,
    );
}
