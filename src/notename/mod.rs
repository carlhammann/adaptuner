use crate::interval::Stack;

mod johnston;

pub enum NoteNameStyle {
    JohnstonFiveLimitClass,
    JohnstonFiveLimitFull,
}

impl<'a> Stack<'a> {
  pub fn notename(&self, style: &NoteNameStyle) -> String {
      match style {
          NoteNameStyle::JohnstonFiveLimitFull =>  johnston::fivelimit::NoteName::new(&self).str_full(),
          NoteNameStyle::JohnstonFiveLimitClass => johnston::fivelimit::NoteName::new(&self).str_class(),
      }

  }
}
