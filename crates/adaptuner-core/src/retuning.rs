//! Pluggable strategies that translate backend-neutral notes into backend commands.

use crate::notes::ActiveNote;

/// Converts an active note into a command for a selected retuning backend.
///
/// The associated command keeps the engine independent of MIDI, MTS, and other
/// transports. A transport can route each command using its `NoteId` mapping.
pub trait RetuningStrategy {
    type Command;
    fn retune(&mut self, note: &ActiveNote) -> Self::Command;
}
