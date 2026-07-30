use std::{
    ops::Deref,
    sync::{mpsc, Arc},
};

use parking_lot::RwLock;

use crate::{
    adaptors::{
        lock_levels::{BackendConfigLevel, KeyStateLevel, TuningStateLevel},
    },
    backend::pitchbend12::Pitchbend12Config,
    interval::stacktype::r#trait::StackType,
    keystate::KeyState,
    msg::FromBackend,
    process::r#trait::StackWithTuning,
    util::ordered_locks::{impl_access, impl_indexed_access, Access, IndexedAccess},
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

pub trait BackendAdaptor:
    IndexedAccess<KeyStateLevel, usize, KeyState>
    + IndexedAccess<TuningStateLevel, usize, StackWithTuning<Self::StackType>>
    + Access<BackendConfigLevel, Pitchbend12Config>
{
    type StackType: StackType;
    fn send(&self, msg: FromBackend) -> bool;
}

pub trait Pitchbend12Adaptor<T: StackType>: BackendAdaptor<StackType = T> {
    fn config(&self) -> impl Deref<Target = Pitchbend12Config>;
}

impl_indexed_access! {<T:StackType>, ConcretePitchbend12Adaptor<T>, KeyStateLevel, usize, KeyState, |self, i| &self.key_states[i].read()}
impl_indexed_access! {<T:StackType>, ConcretePitchbend12Adaptor<T>, TuningStateLevel, usize, StackWithTuning<T>, |self, i| &self.tunings[i].read()}
impl_access! {<T:StackType>, ConcretePitchbend12Adaptor<T>, BackendConfigLevel, Pitchbend12Config, |self| &self.config.read()}

impl<T: StackType> BackendAdaptor for ConcretePitchbend12Adaptor<T> {
    type StackType = T;
    #[inline]
    fn send(&self, msg: FromBackend) -> bool {
        self.forward.send(msg).is_ok()
    }
}

impl<T: StackType> Pitchbend12Adaptor<T> for ConcretePitchbend12Adaptor<T> {
    #[inline]
    fn config(&self) -> impl Deref<Target = Pitchbend12Config> {
        self.config.read()
    }
}
