use std::{
    ops::{Deref, DerefMut},
    sync::{mpsc, Arc},
};

use parking_lot::RwLock;

use crate::{
    adaptors::{ViewKeyStates, ViewTunings},
    backend::pitchbend12::Pitchbend12Config,
    interval::stacktype::r#trait::StackType,
    keystate::KeyState,
    msg::FromBackend,
    process::r#trait::StackWithTuning,
};

/// todo: remove the generic? -- this is only possible if we somehow take sub-views of the 'tunings'
/// field.
#[derive(Clone)]
pub struct ConcretePitchbend12Adaptor<T: StackType> {
    pub forward: mpsc::Sender<FromBackend>,
    pub key_states: [Arc<RwLock<KeyState>>; 128],
    pub tunings: [Arc<RwLock<StackWithTuning<T>>>; 128],
    pub config: Arc<RwLock<Pitchbend12Config>>,
}

pub trait BackendAdaptor<T: StackType>: Clone + ViewKeyStates + ViewTunings<T> {
    fn send(&self, msg: FromBackend) -> bool;
}

pub trait Pitchbend12Adaptor<T: StackType>: BackendAdaptor<T> {
    fn config(&self) -> impl DerefMut<Target = Pitchbend12Config>;
}

impl<T: StackType> ViewKeyStates for ConcretePitchbend12Adaptor<T> {
    #[inline]
    fn key_state(&self, i: usize) -> KeyState {
        *self.key_states[i].read()
    }
}

impl<T: StackType> ViewTunings<T> for ConcretePitchbend12Adaptor<T> {
    #[inline]
    fn tuning(&self, i: usize) -> impl Deref<Target = StackWithTuning<T>> {
        self.tunings[i].read()
    }
}

impl<T: StackType> BackendAdaptor<T> for ConcretePitchbend12Adaptor<T> {
    #[inline]
    fn send(&self, msg: FromBackend) -> bool {
        self.forward.send(msg).is_ok()
    }
}

impl<T: StackType> Pitchbend12Adaptor<T> for ConcretePitchbend12Adaptor<T> {
    #[inline]
    fn config(&self) -> impl DerefMut<Target = Pitchbend12Config> {
        self.config.write()
    }
}
