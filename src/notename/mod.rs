use std::fmt;

use crate::interval::Stack;
use crate::util::{AtLeast3, Dimension};

mod johnston;

pub enum NoteNameStyle {
    JohnstonFiveLimitFull,
    JohnstonFiveLimitClass,
}

impl<'a, D: AtLeast3 + Copy + fmt::Debug, T: Dimension + Copy> Stack<'a, D, T> {
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
