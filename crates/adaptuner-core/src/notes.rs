//! Backend-neutral active-note state for retuning.

use crate::interval::base::Semitones;

/// Stable identity for one sounding note, including repeated pitches.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NoteId(u32);

impl NoteId {
    pub const fn new(value: u32) -> Self {
        Self(value)
    }
    pub const fn get(self) -> u32 {
        self.0
    }
}

/// A note currently held by the engine and the pitch it should be retuned to.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ActiveNote {
    id: NoteId,
    source_pitch: Semitones,
    target_pitch: Semitones,
}

impl ActiveNote {
    pub const fn new(id: NoteId, source_pitch: Semitones, target_pitch: Semitones) -> Self {
        Self {
            id,
            source_pitch,
            target_pitch,
        }
    }
    pub const fn id(&self) -> NoteId {
        self.id
    }
    pub const fn source_pitch(&self) -> Semitones {
        self.source_pitch
    }
    pub const fn target_pitch(&self) -> Semitones {
        self.target_pitch
    }
    pub fn set_target_pitch(&mut self, target_pitch: Semitones) {
        self.target_pitch = target_pitch;
    }
}

/// Result of inserting a note into a fixed-capacity active-note collection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InsertError {
    DuplicateId,
    Full,
}

/// A no-allocation collection suitable for embedded `no_std` targets.
#[derive(Debug, Clone, Copy)]
pub struct ActiveNotes<const N: usize> {
    slots: [Option<ActiveNote>; N],
    len: usize,
}

impl<const N: usize> Default for ActiveNotes<N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const N: usize> ActiveNotes<N> {
    pub const fn new() -> Self {
        Self {
            slots: [None; N],
            len: 0,
        }
    }
    pub const fn len(&self) -> usize {
        self.len
    }
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }
    pub const fn capacity(&self) -> usize {
        N
    }

    pub fn insert(&mut self, note: ActiveNote) -> Result<(), InsertError> {
        if self.get(note.id()).is_some() {
            return Err(InsertError::DuplicateId);
        }
        let Some(slot) = self.slots.iter_mut().find(|slot| slot.is_none()) else {
            return Err(InsertError::Full);
        };
        *slot = Some(note);
        self.len += 1;
        Ok(())
    }

    pub fn get(&self, id: NoteId) -> Option<&ActiveNote> {
        self.slots.iter().flatten().find(|note| note.id() == id)
    }

    pub fn get_mut(&mut self, id: NoteId) -> Option<&mut ActiveNote> {
        self.slots.iter_mut().flatten().find(|note| note.id() == id)
    }

    pub fn remove(&mut self, id: NoteId) -> Option<ActiveNote> {
        let slot = self
            .slots
            .iter_mut()
            .find(|slot| slot.as_ref().is_some_and(|note| note.id() == id))?;
        self.len -= 1;
        slot.take()
    }

    pub fn iter(&self) -> impl Iterator<Item = &ActiveNote> {
        self.slots.iter().flatten()
    }

    pub fn clear(&mut self) {
        self.slots.fill(None);
        self.len = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tracks_notes_by_unique_id_even_when_pitches_match() {
        let mut notes = ActiveNotes::<2>::new();
        notes
            .insert(ActiveNote::new(NoteId::new(1), 60.0, 60.2))
            .unwrap();
        notes
            .insert(ActiveNote::new(NoteId::new(2), 60.0, 59.8))
            .unwrap();
        assert_eq!(notes.len(), 2);
        assert_eq!(notes.get(NoteId::new(1)).unwrap().target_pitch(), 60.2);
        assert_eq!(notes.get(NoteId::new(2)).unwrap().target_pitch(), 59.8);
    }

    #[test]
    fn rejects_duplicate_ids_and_full_capacity() {
        let mut notes = ActiveNotes::<1>::new();
        let note = ActiveNote::new(NoteId::new(7), 64.0, 64.0);
        notes.insert(note).unwrap();
        assert_eq!(notes.insert(note), Err(InsertError::DuplicateId));
        assert_eq!(
            notes.insert(ActiveNote::new(NoteId::new(8), 65.0, 65.0)),
            Err(InsertError::Full)
        );
    }

    #[test]
    fn clear_removes_all_active_notes() {
        let mut notes = ActiveNotes::<2>::new();
        notes
            .insert(ActiveNote::new(NoteId::new(1), 60.0, 60.25))
            .unwrap();
        notes
            .insert(ActiveNote::new(NoteId::new(2), 64.0, 63.75))
            .unwrap();
        notes.clear();
        assert!(notes.is_empty());
        assert_eq!(notes.len(), 0);
        assert!(notes.get(NoteId::new(1)).is_none());
        assert!(notes.get(NoteId::new(2)).is_none());
    }

    #[test]
    fn updates_and_removes_notes_by_id() {
        let mut notes = ActiveNotes::<1>::new();
        let id = NoteId::new(42);
        notes.insert(ActiveNote::new(id, 67.0, 67.0)).unwrap();
        notes.get_mut(id).unwrap().set_target_pitch(67.25);
        assert_eq!(notes.remove(id).unwrap().target_pitch(), 67.25);
        assert!(notes.is_empty());
        assert_eq!(notes.remove(id), None);
    }
}
