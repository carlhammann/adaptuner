use std::time::Instant;

use midi_msg::Channel;

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
        (self.on_channels != 0) | (self.held_channels != 0)
    }

    /// returns true iff the note state changed between "sounding on no channel" and "sounding on
    /// any channel"
    pub fn note_on(&mut self, channel: Channel, time: Instant) -> bool {
        let state_change = !self.is_sounding();
        if state_change {
            self.last_change = time;
        }
        self.on_channels |= 1 << channel as u8;
        return state_change;
    }

    /// returns true iff the note state changed between "sounding on no channel" and "sounding on
    /// any channel"
    pub fn note_off(&mut self, channel: Channel, pedal_hold: bool, time: Instant) -> bool {
        let was_sounding = self.is_sounding();
        if pedal_hold {
            self.held_channels |= self.on_channels & (1 << channel as u8);
        }
        self.on_channels &= !(1 << channel as u8);
        if was_sounding & !self.is_sounding() {
            self.last_change = time;
            return true;
        }
        false
    }

    /// returns true iff the note state changed
    pub fn pedal_off(&mut self, channel: Channel, time: Instant) -> bool {
        let was_sounding = self.is_sounding();
        self.held_channels &= !(1 << channel as u8);
        if was_sounding & !self.is_sounding() {
            self.last_change = time;
            return true;
        }
        false
    }
}

#[cfg(test)]
mod test {
    use std::time::Instant;
    use super::KeyState;
    use midi_msg::Channel::*;

    #[test]
    fn pedal_off_test() {
        let mut s = KeyState::new(Instant::now());
        s.note_on(Ch1, Instant::now());
        s.note_off(Ch1, true, Instant::now());
        assert!(s.is_sounding());
        s.pedal_off(Ch1, Instant::now());
        assert!(!s.is_sounding());
    }
}
