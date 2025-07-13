use std::fmt;

use correction::fivelimit::CorrectionBasis;

use crate::interval::{stack::Stack, stacktype::r#trait::FiveLimitIntervalBasis};

pub mod correction;
pub mod johnston;

#[derive(Clone, Copy)]
pub enum NoteNameStyle {
    JohnstonFiveLimitFull,
    JohnstonFiveLimitClass,
}

impl<T: FiveLimitIntervalBasis> Stack<T> {
    pub fn write_notename<W: fmt::Write>(&self, f: &mut W, style: &NoteNameStyle) -> fmt::Result {
        match style {
            NoteNameStyle::JohnstonFiveLimitFull => {
                johnston::fivelimit::NoteName::new(&self).write_full(f)
            }
            NoteNameStyle::JohnstonFiveLimitClass => {
                johnston::fivelimit::NoteName::new(&self).write_class(f)
            }
        }
    }

    pub fn notename(&self, style: &NoteNameStyle) -> String {
        let mut res = String::new();
        // the [Write] implementation of [String] never throws any error, so this is fine:
        self.write_actual_notename(&mut res, style).unwrap();
        res
    }

    pub fn write_actual_notename<W: fmt::Write>(
        &self,
        f: &mut W,
        style: &NoteNameStyle,
    ) -> fmt::Result {
        match style {
            NoteNameStyle::JohnstonFiveLimitFull => {
                johnston::fivelimit::NoteName::new_from_actual(&self).write_full(f)
            }
            NoteNameStyle::JohnstonFiveLimitClass => {
                johnston::fivelimit::NoteName::new_from_actual(&self).write_class(f)
            }
        }
    }

    pub fn actual_notename(&self, style: &NoteNameStyle) -> String {
        let mut res = String::new();
        // the [Write] implementation of [String] never throws any error, so this is fine:
        self.write_actual_notename(&mut res, style).unwrap();
        res
    }

    pub fn write_corrected_name<W: fmt::Write>(
        &self,
        f: &mut W,
        style: &NoteNameStyle,
        basis: &CorrectionBasis,
    ) -> fmt::Result {
        self.write_notename(f, style)?;
        if !self.is_target() {
            write!(f, "  ")?;
            correction::fivelimit::Correction::new(self).fmt(f, basis)?;
            if self.is_pure() {
                write!(f, " = ")?;
                self.write_actual_notename(f, style)?;
            }
        }
        Ok(())
    }

    pub fn corrected_name(&self, style: &NoteNameStyle, basis: &CorrectionBasis) -> String {
        let mut res = String::new();
        // the [Write] implementation of [String] never throws any error, so this is fine:
        self.write_corrected_name(&mut res, style, basis).unwrap();
        res
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
