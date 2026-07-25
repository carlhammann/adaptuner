use std::ops::{Deref, DerefMut};

use crate::{
    interval::stacktype::r#trait::IntervalBasis, keystate::KeyState,
    process::r#trait::StackWithTuning,
};

pub trait ViewKeyStates {
    /// Index `i` must be in the range `0..128`
    fn key_state(&self, i: usize) -> KeyState;
}

pub trait ChangeKeyStates {
    /// Index `i` must be in the range `0..128`
    fn key_state_mut(&self, i: usize) -> impl DerefMut<Target = KeyState>;
}

pub trait ViewTunings<T: IntervalBasis> {
    /// Index `i` must be in the range `0..128`
    fn tuning(&self, i: usize) -> impl Deref<Target = StackWithTuning<T>>;
}

pub trait ChangeTunings<T: IntervalBasis> {
    /// Index `i` must be in the range `0..128`
    fn tuning_mut(&self, i: usize) -> impl DerefMut<Target = StackWithTuning<T>>;
}
