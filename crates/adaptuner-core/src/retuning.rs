//! Pluggable strategies and engine for retuning active notes.

use crate::notes::{ActiveNote, ActiveNotes};

/// Converts an active note into a command for a selected retuning backend.
///
/// The associated command keeps the engine independent of MIDI, MTS, and other
/// transports. A transport can route each command using its `NoteId` mapping.
pub trait RetuningStrategy {
    type Command;

    fn retune(&mut self, note: &ActiveNote) -> Self::Command;
}

/// Applies one interchangeable strategy to the current active-note set.
#[derive(Debug, Clone)]
pub struct RetuningEngine<S> {
    strategy: S,
}

impl<S: RetuningStrategy> RetuningEngine<S> {
    pub const fn new(strategy: S) -> Self {
        Self { strategy }
    }

    pub fn strategy(&self) -> &S {
        &self.strategy
    }
    pub fn strategy_mut(&mut self) -> &mut S {
        &mut self.strategy
    }

    /// Calculates a strategy-specific command for one active note.
    pub fn retune(&mut self, note: &ActiveNote) -> S::Command {
        self.strategy.retune(note)
    }

    /// Calculates one strategy-specific command per active note and streams it
    /// to `emit`, without allocating or depending on a device backend.
    pub fn retune_all<const N: usize>(
        &mut self,
        notes: &ActiveNotes<N>,
        mut emit: impl FnMut(S::Command),
    ) {
        for note in notes.iter() {
            emit(self.retune(note));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::notes::{ActiveNote, NoteId};

    struct IdentityStrategy;

    impl RetuningStrategy for IdentityStrategy {
        type Command = NoteId;
        fn retune(&mut self, note: &ActiveNote) -> Self::Command {
            note.id()
        }
    }

    #[test]
    fn engine_runs_the_selected_strategy_for_each_active_note() {
        let mut notes = ActiveNotes::<2>::new();
        notes
            .insert(ActiveNote::new(NoteId::new(5), 60.0, 60.25))
            .unwrap();
        notes
            .insert(ActiveNote::new(NoteId::new(9), 64.0, 63.75))
            .unwrap();
        let mut engine = RetuningEngine::new(IdentityStrategy);
        let mut ids = [None; 2];
        let mut index = 0;
        engine.retune_all(&notes, |command| {
            ids[index] = Some(command);
            index += 1;
        });
        assert_eq!(index, 2);
        assert!(ids.contains(&Some(NoteId::new(5))));
        assert!(ids.contains(&Some(NoteId::new(9))));
    }
}
