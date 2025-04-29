use std::time::Instant;

use crate::{
    interval::{stack::Stack, stacktype::r#trait::StackType},
    keystate::KeyState, msg,
};

pub trait Strategy<T: StackType> {
    /// expects the effect of the "note on" event to be alead reflected in `keys`
    fn note_on<'a>(
        &mut self,
        keys: &[KeyState; 128],
        tunings: &'a mut [Stack<T>; 128],
        note: u8,
        time: Instant,
    ) -> Vec<msg::FromStrategy<T>>;

    /// expects the effect of the "note off" event to be alead reflected in `keys`
    ///
    /// There are possibly more than one note off events becaus a pedal release my simultaneously
    /// switch off many notes.
    fn note_off<'a>(
        &mut self,
        keys: &[KeyState; 128],
        tunings: &'a mut [Stack<T>; 128],
        notes: &[u8],
        time: Instant,
    ) -> Vec<msg::FromStrategy<T>>;
}
