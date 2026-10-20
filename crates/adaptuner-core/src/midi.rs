//! Minimal MIDI 1.0 channel-voice message encoding.
//!
//! Channel numbers use the MIDI wire representation: 0..=15.

/// Encodes a 14-bit MIDI Pitch Bend message.
pub fn pitch_bend(channel: u8, bend: u16) -> Option<[u8; 3]> {
    if channel > 15 || bend > 16383 {
        return None;
    }

    Some([
        0xE0 | channel,
        (bend & 0x7F) as u8,
        ((bend >> 7) & 0x7F) as u8,
    ])
}

/// Encodes a MIDI Note-On message.
pub fn note_on(channel: u8, note: u8, velocity: u8) -> Option<[u8; 3]> {
    if channel > 15 || note > 127 || velocity > 127 {
        return None;
    }

    Some([0x90 | channel, note, velocity])
}

/// Encodes a MIDI Note-Off message.
pub fn note_off(channel: u8, note: u8, velocity: u8) -> Option<[u8; 3]> {
    if channel > 15 || note > 127 || velocity > 127 {
        return None;
    }

    Some([0x80 | channel, note, velocity])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_pitch_bend_center() {
        assert_eq!(pitch_bend(0, 8192), Some([0xE0, 0x00, 0x40]));
    }

    #[test]
    fn encodes_pitch_bend_endpoints() {
        assert_eq!(pitch_bend(0, 0), Some([0xE0, 0x00, 0x00]));
        assert_eq!(pitch_bend(0, 16383), Some([0xE0, 0x7F, 0x7F]));
    }

    #[test]
    fn encodes_channel_and_note() {
        assert_eq!(note_on(1, 60, 100), Some([0x91, 60, 100]));
        assert_eq!(note_off(1, 60, 0), Some([0x81, 60, 0]));
    }

    #[test]
    fn rejects_invalid_values() {
        assert_eq!(pitch_bend(16, 8192), None);
        assert_eq!(pitch_bend(0, 16384), None);
        assert_eq!(note_on(0, 128, 100), None);
    }
}
