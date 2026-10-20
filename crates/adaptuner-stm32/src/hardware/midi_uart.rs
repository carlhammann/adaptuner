//! MIDI 1.0 byte-stream transport helpers for Embassy UARTs.
//!
//! Configure the UART for 31,250 baud, 8 data bits, no parity and one stop bit
//! before passing its RX or TX half to these helpers. The desktop application
//! continues to use `midir`; both transports carry the same MIDI wire bytes.

use core::future::Future;

use adaptuner_core::midi::{ChannelMessage, MidiStreamDecoder};
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, channel::Channel};
use embedded_io_async::{Read, Write};

/// Decoded channel messages received from the MIDI UART.
pub static MIDI_INPUT: Channel<CriticalSectionRawMutex, ChannelMessage, 32> = Channel::new();

/// Three-byte MIDI messages waiting to be sent by the MIDI UART.
///
/// This covers the current Pitch Bend, Note On and Note Off output path.
pub static MIDI_OUTPUT: Channel<CriticalSectionRawMutex, [u8; 3], 32> = Channel::new();

/// Reads a UART byte stream and calls `on_message` for each complete channel
/// voice message. Running status is handled by the shared no_std core decoder.
pub async fn receive<R, F, Fut>(reader: &mut R, mut on_message: F) -> Result<(), R::Error>
where
    R: Read,
    F: FnMut(ChannelMessage) -> Fut,
    Fut: Future<Output = ()>,
{
    let mut decoder = MidiStreamDecoder::new();
    let mut buffer = [0; 32];

    loop {
        let count = reader.read(&mut buffer).await?;
        for &byte in &buffer[..count] {
            if let Some(message) = decoder.push(byte) {
                on_message(message).await;
            }
        }
    }
}

/// Writes one or more already encoded MIDI wire messages to a UART.
pub async fn send<W>(writer: &mut W, bytes: &[u8]) -> Result<(), W::Error>
where
    W: Write,
{
    writer.write_all(bytes).await?;
    writer.flush().await
}
