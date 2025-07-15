use std::{sync::mpsc, time::Instant};

use crate::{
    config::StrategyConfig, interval::{base::Semitones, stack::Stack, stacktype::r#trait::StackType}, keystate::KeyState, msg::{FromProcess, ToStrategy}
};

pub trait Strategy<T: StackType> {
    /// expects the effect of the "note on" event to be alead reflected in `keys`.
    ///
    /// May only send [FromProcess::FromStrategy] messages.
    ///
    /// Returns the tuning of the note that was turned on, if it was successfully tuned.
    fn note_on<'a>(
        &mut self,
        keys: &[KeyState; 128],
        tunings: &'a mut [Stack<T>; 128],
        note: u8,
        time: Instant,
        forward: &mpsc::Sender<FromProcess<T>>,
    ) -> Option<(Semitones, &'a Stack<T>)>;

    /// expects the effect of the "note off" event to be alead reflected in `keys`
    ///
    /// May only send [FromProcess::FromStrategy] messages.
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

    /// May only send [FromProcess::FromStrategy] messages.
    fn handle_msg(
        &mut self,
        keys: &[KeyState; 128],
        tunings: &mut [Stack<T>; 128],
        msg: ToStrategy<T>,
        forward: &mpsc::Sender<FromProcess<T>>,
    ) -> bool;

    fn next_neighbourhood(
        &mut self,
        keys: &[KeyState; 128],
        tunings: &mut [Stack<T>; 128],
        time: Instant,
        forward: &mpsc::Sender<FromProcess<T>>,
    ) -> bool;

    fn set_reference(
        &mut self,
        keys: &[KeyState; 128],
        tunings: &mut [Stack<T>; 128],
        time: Instant,
        forward: &mpsc::Sender<FromProcess<T>>,
    ) -> bool;

    fn start(
        &mut self,
        keys: &[KeyState; 128],
        tunings: &mut [Stack<T>; 128],
        time: Instant,
        forward: &mpsc::Sender<FromProcess<T>>,
    );

    fn extract_config(&self) -> StrategyConfig<T>;
}
