use std::time::Instant;

use crate::{
    interval::{stack::Stack, stacktype::r#trait::StackType},
    keystate::KeyState,
    msg,
};

pub trait Strategy<T: StackType> {
    /// expects the effect of the "note on" event to be alead reflected in `keys`
    fn note_on(
        &mut self,
        keys: &[KeyState; 128],
        tunings: &mut [Stack<T>; 128],
        note: u8,
        time: Instant,
    ) -> Option<Vec<msg::FromStrategy<T>>>;

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
    ) -> Option<Vec<msg::FromStrategy<T>>>;

    fn handle_msg(
        &mut self,
        keys: &[KeyState; 128],
        tunings: &mut [Stack<T>; 128],
        msg: msg::ToStrategy,
    ) -> Option<Vec<msg::FromStrategy<T>>>;
}
