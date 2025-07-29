use std::fmt;

use serde_derive::{Deserialize, Serialize};
use crate::interval::{
    stack::Stack,
    stacktype::{
        fivelimit::TheFiveLimitStackType,
        r#trait::{IntervalBasis, StackCoeff, StackType},
    },
};

pub mod correction;
pub mod johnston;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[derive(Clone, Copy)]
pub enum NoteNameStyle {
    Full,
    Class,
}

#[derive(Clone, Copy)]
pub enum BaseName {
    C,
    D,
    E,
    F,
    G,
    A,
    B,
}

impl std::fmt::Display for BaseName {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
        use BaseName::*;
        match self {
            C => f.write_str(&"C"),
            D => f.write_str(&"D"),
            E => f.write_str(&"E"),
            F => f.write_str(&"F"),
            G => f.write_str(&"G"),
            A => f.write_str(&"A"),
            B => f.write_str(&"B"),
        }
    }
}

pub trait Accidental {
    fn is_natural(&self) -> bool;
    fn sharpflat(&self) -> StackCoeff;
    fn plusminus(&self) -> StackCoeff;
}

pub trait NoteName {
    type Accidental: Accidental;
    fn write<W: fmt::Write>(&self, f: &mut W, style: &NoteNameStyle) -> fmt::Result;

    fn base_name(&self) -> BaseName;

    fn octave(&self) -> StackCoeff;

    fn accidental(&self) -> &Self::Accidental;

    fn has_accidental(&self) -> bool {
        !self.accidental().is_natural()
    }

    fn middle_c() -> Self;
}

pub trait NoteNameFor<T: IntervalBasis>: NoteName {
    fn new_from_stack(stack: &Stack<T>) -> Self;
    fn new_from_stack_actual(stack: &Stack<T>) -> Self;
}

pub trait HasNoteNames: IntervalBasis {
    type NoteName: Clone + NoteNameFor<Self>;

    fn notename(stack: &Stack<Self>) -> Self::NoteName {
        Self::NoteName::new_from_stack(stack)
    }

    fn actual_notename(stack: &Stack<Self>) -> Self::NoteName {
        Self::NoteName::new_from_stack_actual(stack)
    }

    fn write_notename<W: fmt::Write>(
        stack: &Stack<Self>,
        f: &mut W,
        style: &NoteNameStyle,
    ) -> fmt::Result {
        Self::notename(stack).write(f, style)
    }

    fn write_actual_notename<W: fmt::Write>(
        stack: &Stack<Self>,
        f: &mut W,
        style: &NoteNameStyle,
    ) -> fmt::Result {
        Self::actual_notename(stack).write(f, style)
    }

    fn write_corrected_notename<W: fmt::Write>(
        stack: &Stack<Self>,
        f: &mut W,
        style: &NoteNameStyle,
        preference_order: &[usize],
        use_cent_values: bool,
    ) -> fmt::Result
    where
        Self: StackType,
    {
        Self::write_notename(stack, f, style)?;
        if !stack.is_target() {
            write!(f, "  ")?;
            let mut write_cents = || {
                let d = stack.semitones() - stack.target_semitones();
                if d > 0.0 {
                    write!(f, "+")?;
                }
                write!(f, "{:.02}ct", d * 100.0)
            };
            if use_cent_values {
                write_cents()?;
            } else {
                if let Some(corr) = correction::Correction::new(stack, preference_order) {
                    corr.fmt(f)?;
                } else {
                    write_cents()?;
                }
            }
            if stack.is_pure() {
                write!(f, " = ")?;
                Self::write_actual_notename(stack, f, style)?;
            }
        }
        Ok(())
    }
}

impl HasNoteNames for TheFiveLimitStackType {
    type NoteName = johnston::fivelimit::NoteName;
}

impl<T: StackType + HasNoteNames> Stack<T> {
    pub fn write_notename<W: fmt::Write>(&self, f: &mut W, style: &NoteNameStyle) -> fmt::Result {
        T::write_notename(self, f, style)
    }

    pub fn notename(&self, style: &NoteNameStyle) -> String {
        let mut res = String::new();
        self.write_notename(&mut res, style).unwrap();
        res
    }

    pub fn write_actual_notename<W: fmt::Write>(
        &self,
        f: &mut W,
        style: &NoteNameStyle,
    ) -> fmt::Result {
        T::write_actual_notename(self, f, style)
    }

    pub fn actual_notename(&self, style: &NoteNameStyle) -> String {
        let mut res = String::new();
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
        T::write_corrected_notename(self, f, style, preference_order, use_cent_values)
    }

    pub fn corrected_notename(
        &self,
        style: &NoteNameStyle,
        preference_order: &[usize],
        use_cent_values: bool,
    ) -> String {
        let mut res = String::new();
        self.write_corrected_notename(&mut res, style, preference_order, use_cent_values)
            .unwrap();
        res
    }
}
