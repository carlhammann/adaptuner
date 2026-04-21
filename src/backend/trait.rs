use std::{
    ops::Deref,
    sync::{mpsc, Arc},
};

use parking_lot::RwLock;

use crate::{
    interval::stacktype::r#trait::StackType,
    keystate::KeyState,
    msg::FromBackend,
    process::r#trait::StackWithTuning,
};

/// todo: remove the generic?
#[derive(Clone)]
pub struct ConcreteBackendAdaptor<T: StackType> {
    pub forward: mpsc::Sender<FromBackend>,
    pub tunings: Arc<RwLock<[StackWithTuning<T>; 128]>>,
    pub key_states: Arc<RwLock<[KeyState; 128]>>,
}

pub trait BackendAdaptor<T: StackType>: Clone {
    fn send(&self, msg: FromBackend) -> bool;
    fn key_states(&self) -> impl Deref<Target = [KeyState; 128]>;
    fn tunings(&self) -> impl Deref<Target = [StackWithTuning<T>; 128]>;
}

impl<T: StackType> BackendAdaptor<T> for ConcreteBackendAdaptor<T> {
    fn send(&self, msg: FromBackend) -> bool {
        self.forward.send(msg).is_ok()
    }

    fn key_states(&self) -> impl Deref<Target = [KeyState; 128]> {
        self.key_states.read()
    }

    fn tunings(&self) -> impl Deref<Target = [StackWithTuning<T>; 128]> {
        self.tunings.read()
    }
}
