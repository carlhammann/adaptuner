use crate::interval::Stack;
use crate::util::{fixed_sizes::Size3, Dimension};

mod johnston;

pub enum NoteNameStyle {
    JohnstonFull,
    JohnstonClass,
}

impl<'a, T: Dimension + Copy> Stack<'a, Size3, T> {
    pub fn notename(&self, style: &NoteNameStyle) -> String {
        match style {
            NoteNameStyle::JohnstonFull => johnston::fivelimit::NoteName::new(&self).str_full(),
            NoteNameStyle::JohnstonClass => johnston::fivelimit::NoteName::new(&self).str_class(),
        }
    }
}
