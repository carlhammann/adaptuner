use std::{ops::DerefMut, time::Instant};

use crate::{interval::stacktype::r#trait::StackType, msg, notestore::TunedNoteStore};

pub trait ProcessState<T: StackType> {
    fn handle_msg<TS>(&mut self, time: Instant, msg: msg::ToProcess, tuned_store: TS)
    where
        TS: DerefMut<Target = TunedNoteStore<T>>;
}
