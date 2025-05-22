use std::{sync::mpsc, time::Instant};

use crate::{
    interval::{stack::Stack, stacktype::r#trait::StackType},
    keystate::KeyState,
    msg::{FromProcess, ToStrategy},
};

pub trait Strategy<T: StackType> {
    /// expects the effect of the "note on" event to be alead reflected in `keys`
    fn note_on(
        &mut self,
        keys: &[KeyState; 128],
        tunings: &mut [Stack<T>; 128],
        note: u8,
        time: Instant,
        forward: &mpsc::Sender<FromProcess<T>>,
    ) -> bool;

    /// expects the effect of the "note off" event to be alead reflected in `keys`
    ///
    /// There are possibly more than one note off events becaus a pedal release my simultaneously
    /// switch off many notes.
    fn note_off(
        &mut self,
        keys: &[KeyState; 128],
        tunings: &mut [Stack<T>; 128],
        notes: &[u8],
        time: Instant,
        forward: &mpsc::Sender<FromProcess<T>>,
    ) -> bool;

    fn handle_msg(
        &mut self,
        keys: &[KeyState; 128],
        tunings: &mut [Stack<T>; 128],
        msg: ToStrategy<T>,
        forward: &mpsc::Sender<FromProcess<T>>,
    ) -> bool;
}
