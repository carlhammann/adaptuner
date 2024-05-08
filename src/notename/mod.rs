use std::fmt;

use crate::interval::Stack;
use crate::util::dimension::{AtLeast, Dimension};

mod johnston;

pub enum NoteNameStyle {
    JohnstonFiveLimitFull,
    JohnstonFiveLimitClass,
}

impl<D: AtLeast<3> + Copy + fmt::Debug, T: Dimension + Copy> Stack<D, T> {
    pub fn notename(&self, style: &NoteNameStyle) -> String {
        match style {
            NoteNameStyle::JohnstonFiveLimitFull => {
                johnston::fivelimit::NoteName::new(&self).str_full()
            }
            NoteNameStyle::JohnstonFiveLimitClass => {
                johnston::fivelimit::NoteName::new(&self).str_class()
            }
        }
    }
}
