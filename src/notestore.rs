use std::time::Instant;

use midi_msg::Channel;

use indextree::{self, Arena};

use crate::interval::{interval::Semitones, stack::Stack, stacktype::r#trait::StackType};

enum OnInfo {
    On { since: Instant, channel: Channel },
    Reference { since: Instant },
}

struct DetunedInfo {}

pub struct NoteStatus {
    on: OnInfo,
    detuned: DetunedInfo,
}

impl NoteStatus {
    pub fn is_on(&self) -> bool {
        match self.on {
            OnInfo::On { .. } => true,
            _ => false,
        }
    }
}

pub struct StackWithStatus<T: StackType> {
    pub stack: Stack<T>,
    pub status: NoteStatus,
}

pub enum Tuning {
    FromMidiNote(Semitones),
    FromFreq(f32),
}

pub struct TunedNoteStore<T: StackType> {
    arena: indextree::Arena<StackWithStatus<T>>,
    root: indextree::NodeId,
    root_tuning: Tuning,
}

impl<T: StackType> TunedNoteStore<T> {
    pub fn new(reference_stack: Stack<T>, reference_tuning: Tuning) -> Self {
        let mut arena = Arena::new();
        let root = arena.new_node(StackWithStatus {
            stack: reference_stack,
            status: NoteStatus {
                on: OnInfo::Reference {
                    since: Instant::now(),
                },
                detuned: DetunedInfo {},
            },
        });
        TunedNoteStore {
            arena,
            root,
            root_tuning: reference_tuning,
        }
    }
    pub fn iter(&self) -> impl Iterator<Item = &StackWithStatus<T>> {
        self.root.descendants(&self.arena).map(|i| {
            self.arena
                .get(i)
                .expect("this can't happen: missing node in TunedNoteStore")
                .get()
        })
    }
    pub fn iter_ancestors(
        &self,
    ) -> impl Iterator<Item = impl Iterator<Item = &StackWithStatus<T>>> {
        self.root.descendants(&self.arena).map(|i| {
            i.ancestors(&self.arena).map(|i| {
                self.arena
                    .get(i)
                    .expect("this can't happen: missing node in TunedNoteStore")
                    .get()
            })
        })
    }
    pub fn iter_with_parent(
        &self,
    ) -> impl Iterator<Item = (&StackWithStatus<T>, Option<&StackWithStatus<T>>)> {
        self.root.descendants(&self.arena).map(|i| {
            let node = self
                .arena
                .get(i)
                .expect("this can't happen: missing node in TunedNoteStore");
            let parent = node
                .parent()
                .and_then(|i| self.arena.get(i))
                .map(|x| x.get());
            (node.get(), parent)
        })
    }
}
