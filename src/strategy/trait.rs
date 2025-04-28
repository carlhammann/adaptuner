use std::time::Instant;

use midi_msg::Channel;

use crate::interval::{stack::Stack, stacktype::r#trait::StackType};

pub struct KeyState {
    last_change: Instant, // last time that the note changed between sounding and not sounding
    on_channels: u16,
    held_channels: u16,
}

impl KeyState {
    pub fn new(time: Instant) -> Self {
        Self {
            last_change: time,
            on_channels: 0,
            held_channels: 0,
        }
    }

    pub fn is_sounding(&self) -> bool {
        (self.on_channels != 0) & (self.held_channels != 0)
    }

    /// returns true iff the note state changed
    pub fn note_on(&mut self, channel: Channel, time: Instant) -> bool {
        let state_change = !self.is_sounding();
        if state_change {
            self.last_change = time;
        }
        self.on_channels |= 1 << channel as u8;
        return state_change;
    }

    /// returns true iff the note state changed
    pub fn note_off(&mut self, channel: Channel, pedal_hold: bool, time: Instant) -> bool {
        let was_sounding = self.is_sounding();
        if pedal_hold {
            self.held_channels |= self.on_channels & (1 << channel as u8);
        }
        self.on_channels -= self.on_channels & (1 << channel as u8);
        if was_sounding & !self.is_sounding() {
            self.last_change = time;
            return true;
        }
        false
    }

    /// returns true iff the note state changed
    pub fn pedal_off(&mut self, channel: Channel, time: Instant) -> bool {
        let was_sounding = self.is_sounding();
        self.held_channels -= self.held_channels | (1 << channel as u8);
        if was_sounding & !self.is_sounding() {
            self.last_change = time;
            return true;
        }
        false
    }
}

pub trait Strategy<T: StackType> {
    /// expects the effect of the "note on" event to be alead reflected in `keys`
    fn note_on<'a>(
        &mut self,
        keys: &[KeyState; 128],
        tunings: &'a mut [Stack<T>; 128],
        note: u8,
        time: Instant,
    ) -> Vec<(u8, &'a Stack<T>)>;

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
    ) -> Vec<(u8, &'a Stack<T>)>;
}
