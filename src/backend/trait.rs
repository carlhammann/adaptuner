use std::{ops::Deref, sync::mpsc};

use crate::{
    interval::stacktype::r#trait::StackType,
    keystate::KeyState,
    msg::FromBackend,
    process::r#trait::StackWithTuning,
    util::readerwriter::{ConcreteReader128, Reader128},
};

#[derive(Clone)]
pub struct ConcreteBackendAdaptor<T: StackType> {
    pub forward: mpsc::Sender<FromBackend>,
    pub tunings: ConcreteReader128<StackWithTuning<T>>,
    pub key_states: ConcreteReader128<KeyState>,
}

pub trait BackendAdaptor<T: StackType>: Clone {
    fn send(&self, msg: FromBackend) -> bool;
    fn read_key_state(&self,i: usize) -> impl Deref<Target = KeyState>;
    fn read_tuning(&self,i: usize) -> impl Deref<Target = StackWithTuning<T>>;
}

impl<T: StackType> BackendAdaptor<T> for ConcreteBackendAdaptor<T> {
    fn send(&self, msg: FromBackend) -> bool {
        self.forward.send(msg).is_ok()
    }

    fn read_key_state(&self, i: usize) -> impl Deref<Target = KeyState> {
        self.key_states.read(i)
    }

    fn read_tuning(&self, i: usize) -> impl Deref<Target = StackWithTuning<T>> {
        self.tunings.read(i)
    }
}
