use std::{
    ops::{Deref, DerefMut},
    sync::mpsc,
};

use crate::{
    interval::{
        base::Semitones,
        stack::Stack,
        stacktype::r#trait::{IntervalBasis, StackType},
    },
    keystate::KeyState,
    msg::FromProcess,
    util::readerwriter::{ConcreteReader128, ConcreteReaderWriter128, Reader128, ReaderWriter128},
};

pub struct StackWithTuning<T: IntervalBasis> {
    pub stack: Stack<T>,
    pub tuning: Semitones,
}

// impl<T: IntervalBasis> Reader<Stack<T>> for ConcreteReader128<StackWithTuning<T>> {
//     fn read(&self, i: usize) -> impl Deref<Target = Stack<T>> {
//        <Self as Reader<StackWithTuning<T>>>::read(self, i).stack
//     }
// }

#[derive(Clone)]
pub struct ConcreteProcessAdaptor<T: StackType> {
    pub forward: mpsc::Sender<FromProcess<T>>,
    pub tunings: ConcreteReaderWriter128<StackWithTuning<T>>,
    pub key_states: ConcreteReaderWriter128<KeyState>,
}

pub trait ProcessAdaptor<T: StackType>: Clone {
    fn send(&self, msg: FromProcess<T>) -> bool;
    fn read_tuning(&self, i: usize) -> impl Deref<Target = StackWithTuning<T>>;
    fn write_tuning(&self, i: usize) -> impl DerefMut<Target = StackWithTuning<T>>;
    fn read_key_state(&self, i: usize) -> impl Deref<Target = KeyState>;
    fn write_key_state(&self, i: usize) -> impl DerefMut<Target = KeyState>;
}

impl<T: StackType> ProcessAdaptor<T> for ConcreteProcessAdaptor<T> {
    fn send(&self, msg: FromProcess<T>) -> bool {
        self.forward.send(msg).is_ok()
    }

    fn read_tuning(&self, i: usize) -> impl Deref<Target = StackWithTuning<T>> {
        self.tunings.read(i)
    }

    fn write_tuning(&self, i: usize) -> impl DerefMut<Target = StackWithTuning<T>> {
        self.tunings.write(i)
    }

    fn read_key_state(&self, i: usize) -> impl Deref<Target = KeyState> {
        self.key_states.read(i)
    }

    fn write_key_state(&self, i: usize) -> impl DerefMut<Target = KeyState> {
        self.key_states.write(i)
    }
}
