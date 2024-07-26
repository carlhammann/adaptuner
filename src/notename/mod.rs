use crate::interval::{
    stack::Stack,
    stacktype::r#trait::{FiveLimitStackType},
};

mod johnston;

#[derive(Clone)]
pub enum NoteNameStyle {
    JohnstonFiveLimitFull,
    JohnstonFiveLimitClass,
}

impl<T: FiveLimitStackType> Stack<T> {
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

// impl<T: StackType> Stack<T> {
//     pub fn indexed_notename(
//         &self,
//         fifth_index: usize,
//         third_index: usize,
//         style: &NoteNameStyle,
//     ) -> String {
//         match style {
//             NoteNameStyle::JohnstonFiveLimitFull => {
//                 johnston::fivelimit::NoteName::new_with_indices(fifth_index, third_index, &self)
//                     .str_full()
//             }
//             NoteNameStyle::JohnstonFiveLimitClass => {
//                 johnston::fivelimit::NoteName::new_with_indices(fifth_index, third_index, &self)
//                     .str_class()
//             }
//         }
//     }
// }
