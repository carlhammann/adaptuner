use std::fmt;

use crate::interval::{stack::Stack, stacktype::r#trait::FiveLimitStackType};

pub mod correction;
pub mod johnston;

#[derive(Clone, Copy)]
pub enum NoteNameStyle {
    JohnstonFiveLimitFull,
    JohnstonFiveLimitClass,
}

impl<T: FiveLimitStackType> Stack<T> {
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
        self.write_notename(&mut res, style).unwrap();
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

    pub fn write_corrected_notename<W: fmt::Write>(
        &self,
        f: &mut W,
        style: &NoteNameStyle,
        preference_order: &[usize],
        use_cent_values: bool,
    ) -> fmt::Result {
        self.write_notename(f, style)?;
        if !self.is_target() {
            write!(f, "  ")?;
            let mut write_cents = || {
                let d = self.semitones() - self.target_semitones();
                if d > 0.0 {
                    write!(f, "+")?;
                }
                write!(f, "{:.02}ct", d * 100.0)
            };
            if use_cent_values {
                write_cents()?;
            } else {
                if let Some(corr) = correction::Correction::new(self, preference_order) {
                    corr.fmt(f)?;
                } else {
                    write_cents()?;
                }
            }
            if self.is_pure() {
                write!(f, " = ")?;
                self.write_actual_notename(f, style)?;
            }
        }
        Ok(())
    }

    pub fn corrected_notename(
        &self,
        style: &NoteNameStyle,
        preference_order: &[usize],
        use_cent_values: bool,
    ) -> String {
        let mut res = String::new();
        // the [Write] implementation of [String] never throws any error, so this is fine:
        self.write_corrected_notename(&mut res, style, preference_order, use_cent_values)
            .unwrap();
        res
    }
}
