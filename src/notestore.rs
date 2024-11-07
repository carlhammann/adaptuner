use std::time::Instant;

use crate::{
    hashmaptree::HashMapTree,
    interval::{interval::Semitones, stack::Stack, stacktype::r#trait::StackType},
};

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct InputPort {}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum NoteOrigin {
    Reference(usize),
    FromInput {
        input: InputPort,
        channel: u8,
        midi_note: u8,
    },
}

impl NoteOrigin {
    pub fn is_from_input(&self) -> bool {
        match self {
            Self::FromInput { .. } => true,
            _ => false,
        }
    }
}

pub enum Tuning {
    FromMidiNote(Semitones),
    FromFreq(f32),
}

pub struct TunedNoteStore<T: StackType> {
    internal: HashMapTree<NoteOrigin, Stack<T>>,
    root: usize,
    root_tuning: Tuning,
    next_unused_reference_number: usize,
    last_change: Instant,
}

impl<T: StackType + 'static> TunedNoteStore<T> {
    /// Adds the given Stack and tuning for as the root node with the key
    /// `NoteOrigin::Reference(0)`
    pub fn new(reference_stack: Stack<T>, reference_tuning: Tuning) -> Self {
        let mut internal = HashMapTree::new();
        internal.add_node(NoteOrigin::Reference(0), reference_stack);
        TunedNoteStore {
            internal,
            root: 0,
            root_tuning: reference_tuning,
            next_unused_reference_number: 1,
            last_change: Instant::now(),
        }
    }

    pub fn add(
        &mut self,
        parent: &NoteOrigin,
        input: InputPort,
        channel: u8,
        midi_note: u8,
        stack: Stack<T>,
    ) {
        self.internal.add_child(
            parent,
            NoteOrigin::FromInput {
                input,
                channel,
                midi_note,
            },
            stack,
        );
    }

    /// Pre-order dfs of all keys strictly below the root
    pub fn iter(&self) -> impl Iterator<Item = NoteOrigin> + '_ {
        self.internal.descendants(NoteOrigin::Reference(self.root))
    }

    ///// iterate everything as it is stored
    //pub fn iter_relative(&self) -> impl Iterator<Item = &StackWithStatus<T>> {
    //    self.root.descendants(&self.arena).map(|i| {
    //        self.get_relative(i)
    //            .expect("this can't happen: missing node in TunedNoteStore")
    //    })
    //}

    ///// iterate everything, but convert stacks to "absolute"
    //pub fn iter_absolute(&self) -> impl Iterator<Item = StackWithStatus<T>> + '_ {
    //    let mut tmp = Stack::new_zero();
    //    self.root
    //        .traverse(&self.arena)
    //        .filter_map(move |i| match i {
    //            NodeEdge::Start(j) => {
    //                let x = self.get_relative(j).expect("");
    //                tmp += &x.stack;
    //                Some(StackWithStatus {
    //                    status: x.status.clone(),
    //                    stack: tmp.clone(),
    //                })
    //            }
    //            NodeEdge::End(j) => {
    //                let x = self.get_relative(j).expect("");
    //                tmp.add_mul(-1, &x.stack);
    //                None
    //            }
    //        })
    //}

    ///// inserts `child` bolow the node with `parent_id`. Panics if there is no node at `parent_id`.
    //pub fn insert_below(&mut self, parent_id: indextree::NodeId, child: StackWithStatus<T>) {
    //    let new_node = self.arena.new_node(child);
    //    parent_id.append(new_node, &mut self.arena);
    //}

    ///// returns the `StackWithStatus` at the given index, as it is stored. This may be a relative
    ///// stack to some tuning reference(s)
    //pub fn get_relative(&self, i: indextree::NodeId) -> Option<&StackWithStatus<T>> {
    //    self.arena.get(i).map(|n| n.get())
    //}

    // /// returns the `StackWithStatus` at the given index, as an absolute stack.
    // /// If you want to read all absolute stacks, consider using `iter_absolute`, which will need far
    // /// fewer calculations.
    //pub fn get_absolute(&self, i: indextree::NodeId) -> Option<StackWithStatus<T>> {
    //    match self.get_relative(i) {
    //        None { .. } => None,
    //        Some(StackWithStatus { stack, status }) => {
    //            let mut acc = stack.clone();
    //            let mut ancestors = i.ancestors(&self.arena);
    //            ancestors.next(); // the first element points the node itself, we don't need to add
    //                              // that
    //            for j in ancestors {
    //                acc += &self
    //                    .arena
    //                    .get(j)
    //                    .expect("this can't happen: missing node in TunedNoteStore")
    //                    .get()
    //                    .stack;
    //            }
    //            Some(StackWithStatus {
    //                status: status.clone(),
    //                stack: acc,
    //            })
    //        }
    //    }
    //}

    //pub fn iter_ancestors(
    //    &self,
    //) -> impl Iterator<Item = impl Iterator<Item = &StackWithStatus<T>>> {
    //    self.root.descendants(&self.arena).map(|i| {
    //        i.ancestors(&self.arena).map(|i| {
    //            self.arena
    //                .get(i)
    //                .expect("this can't happen: missing node in TunedNoteStore")
    //                .get()
    //        })
    //    })
    //}

    /// Helper: Pre-order dfs of all notes strictly below the root, together with their parents.
    pub fn iter_with_parent(
        &self,
    ) -> impl Iterator<Item = ((NoteOrigin, &Stack<T>), Option<(&NoteOrigin, &Stack<T>)>)> {
        self.iter().map(|i| {
            let node = self
                .internal
                .get(&i)
                .expect("iter_with_parent(): missing node in TunedNoteStore");
            let parent =
                node.parent
                    .as_ref()
                    .and_then(|i| match self.internal.get(i).map(|x| x.get()) {
                        None { .. } => None,
                        Some(p) => Some((i, p)),
                    });
            ((i, node.get()), parent)
        })
    }
}
