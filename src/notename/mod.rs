use crate::interval::Stack;

mod johnston;

pub enum NoteNameStyle {
    JohnstonFiveLimitClass,
    JohnstonFiveLimitFull,
}

impl<'a, const T: usize> Stack<'a, 3, T> {
  pub fn notename(&self, style: &NoteNameStyle) -> String {
      match style {
          NoteNameStyle::JohnstonFiveLimitFull =>  johnston::fivelimit::NoteName::new(&self).str_full(),
          NoteNameStyle::JohnstonFiveLimitClass => johnston::fivelimit::NoteName::new(&self).str_class(),
      }

  }
}
