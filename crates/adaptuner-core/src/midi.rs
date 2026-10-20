//! Minimal MIDI 1.0 channel-voice message encoding.
//!
//! Channel numbers use the MIDI wire representation: 0..=15.

/// A complete MIDI 1.0 channel-voice message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChannelMessage {
    status: u8,
    data: [u8; 2],
    len: u8,
}

impl ChannelMessage {
    /// Status byte, including the message kind and zero-based channel.
    pub const fn status(&self) -> u8 {
        self.status
    }

    /// Zero-based MIDI channel.
    pub const fn channel(&self) -> u8 {
        self.status & 0x0f
    }

    /// Message kind, e.g. `0x90` for Note On or `0xe0` for Pitch Bend.
    pub const fn kind(&self) -> u8 {
        self.status & 0xf0
    }

    /// Data bytes. The second value is zero for two-byte channel messages.
    pub const fn data(&self) -> [u8; 2] {
        self.data
    }

    /// Number of data bytes in this message.
    pub const fn data_len(&self) -> u8 {
        self.len
    }
}

/// Stateful decoder for MIDI 1.0 channel messages on a serial byte stream.
///
/// It supports running status and ignores system-common, SysEx and real-time
/// messages. Real-time bytes may appear between the bytes of a channel message
/// without interrupting its decoding.
#[derive(Debug, Clone, Copy)]
pub struct MidiStreamDecoder {
    running_status: Option<u8>,
    data: [u8; 2],
    data_len: u8,
    expected_len: u8,
    in_sysex: bool,
}

impl Default for MidiStreamDecoder {
    fn default() -> Self {
        Self::new()
    }
}

impl MidiStreamDecoder {
    /// Creates a decoder with no running status.
    pub const fn new() -> Self {
        Self {
            running_status: None,
            data: [0; 2],
            data_len: 0,
            expected_len: 0,
            in_sysex: false,
        }
    }

    /// Feeds one serial byte and returns a message once it is complete.
    pub fn push(&mut self, byte: u8) -> Option<ChannelMessage> {
        // MIDI real-time messages are independent and may be interleaved with
        // any other message, including SysEx.
        if byte >= 0xf8 {
            return None;
        }

        if byte & 0x80 != 0 {
            self.data_len = 0;
            self.expected_len = 0;
            if byte == 0xf0 {
                self.running_status = None;
                self.in_sysex = true;
                return None;
            }
            if self.in_sysex {
                if byte == 0xf7 {
                    self.in_sysex = false;
                }
                return None;
            }
            if byte >= 0xf0 {
                // System-common messages cancel channel running status.
                self.running_status = None;
                return None;
            }

            self.running_status = Some(byte);
            self.expected_len = if matches!(byte & 0xf0, 0xc0 | 0xd0) {
                1
            } else {
                2
            };
            return None;
        }

        if self.in_sysex {
            return None;
        }

        let status = self.running_status?;
        if self.expected_len == 0 {
            self.expected_len = if matches!(status & 0xf0, 0xc0 | 0xd0) {
                1
            } else {
                2
            };
        }
        self.data[self.data_len as usize] = byte;
        self.data_len += 1;
        if self.data_len != self.expected_len {
            return None;
        }

        let message = ChannelMessage {
            status,
            data: self.data,
            len: self.expected_len,
        };
        self.data_len = 0;
        Some(message)
    }
}

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
    fn decodes_channel_messages_across_arbitrary_byte_chunks() {
        let mut decoder = MidiStreamDecoder::new();
        assert_eq!(decoder.push(0x92), None);
        assert_eq!(decoder.push(60), None);
        assert_eq!(
            decoder.push(100),
            Some(ChannelMessage {
                status: 0x92,
                data: [60, 100],
                len: 2,
            })
        );
    }

    #[test]
    fn supports_running_status_and_one_data_byte_messages() {
        let mut decoder = MidiStreamDecoder::new();
        assert_eq!(decoder.push(0xc3), None);
        assert_eq!(
            decoder.push(7),
            Some(ChannelMessage {
                status: 0xc3,
                data: [7, 0],
                len: 1,
            })
        );
        assert_eq!(
            decoder.push(8),
            Some(ChannelMessage {
                status: 0xc3,
                data: [8, 0],
                len: 1,
            })
        );
    }

    #[test]
    fn ignores_realtime_bytes_without_interrupting_a_message() {
        let mut decoder = MidiStreamDecoder::new();
        decoder.push(0x90);
        decoder.push(60);
        assert_eq!(decoder.push(0xf8), None);
        assert_eq!(decoder.push(100).unwrap().data(), [60, 100]);
    }

    #[test]
    fn system_common_and_sysex_cancel_running_status() {
        let mut decoder = MidiStreamDecoder::new();
        decoder.push(0x90);
        decoder.push(60);
        decoder.push(0xf1);
        assert_eq!(decoder.push(100), None);

        decoder.push(0x91);
        decoder.push(61);
        decoder.push(0xf0);
        assert_eq!(decoder.push(100), None);
        decoder.push(0xf7);
        assert_eq!(decoder.push(100), None);
    }

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
