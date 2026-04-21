use std::{
    ops::Deref,
    sync::{mpsc, Arc},
};

use parking_lot::RwLock;

use crate::{
    interval::stacktype::r#trait::StackType, keystate::KeyState, msg::FromBackend,
    process::r#trait::StackWithTuning,
};

/// todo: remove the generic?
#[derive(Clone)]
pub struct ConcreteBackendAdaptor<T: StackType> {
    pub forward: mpsc::Sender<FromBackend>,
    pub tunings: [Arc<RwLock<StackWithTuning<T>>>; 128],
    pub key_states: [Arc<RwLock<KeyState>>; 128],
}

pub trait BackendAdaptor<T: StackType>: Clone {
    fn send(&self, msg: FromBackend) -> bool;
    /// index `i` must be in the range `0..128`
    fn key_state(&self, i: usize) -> impl Deref<Target = KeyState>;
    /// index `i` must be in the range `0..128`
    fn tuning(&self, i: usize) -> impl Deref<Target = StackWithTuning<T>>;
}

impl<T: StackType> BackendAdaptor<T> for ConcreteBackendAdaptor<T> {
    fn send(&self, msg: FromBackend) -> bool {
        self.forward.send(msg).is_ok()
    }

    fn key_state(&self, i: usize) -> impl Deref<Target = KeyState> {
        self.key_states[i].read()
    }

    fn tuning(&self, i: usize) -> impl Deref<Target = StackWithTuning<T>> {
        self.tunings[i].read()
    }
}
