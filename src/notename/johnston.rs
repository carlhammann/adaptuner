pub mod fivelimit {
    use std::fmt;

    use crate::interval::{
        stack::Stack,
        stacktype::{
            fivelimit::TheFiveLimitStackType,
            r#trait::{IntervalBasis, StackCoeff},
        },
    };

    use crate::notename::BaseName::{self, *};

    #[derive(Clone)]
    pub struct Accidental {
        sharpflat: StackCoeff,
        plusminus: StackCoeff,
    }

    #[derive(Clone)]
    pub struct NoteName {
        basename: BaseName,
        octave: StackCoeff,
        accidental: Accidental,
    }

    const JOHNSTON_BASE_ROW: [BaseName; 7] = [F, A, C, E, G, B, D];

    impl crate::notename::Accidental for Accidental {
        fn is_natural(&self) -> bool {
            self.sharpflat == 0 && self.plusminus == 0
        }

        fn sharpflat(&self) -> StackCoeff {
            self.sharpflat
        }

        fn plusminus(&self) -> StackCoeff {
            self.plusminus
        }
    }

    impl crate::notename::NoteName for NoteName {
        type Accidental = Accidental;

        fn write<W: fmt::Write>(
            &self,
            f: &mut W,
            style: &crate::notename::NoteNameStyle,
        ) -> fmt::Result {
            match style {
                crate::notename::NoteNameStyle::Full => self.write_full(f),
                crate::notename::NoteNameStyle::Class => self.write_class(f),
            }
        }

        fn base_name(&self) -> BaseName {
            self.basename
        }

        fn octave(&self) -> StackCoeff {
            self.octave
        }

        fn accidental(&self) -> &Self::Accidental {
            &self.accidental
        }

        fn middle_c() -> Self {
            NoteName {
                basename: C,
                octave: 4,
                accidental: Accidental {
                    sharpflat: 0,
                    plusminus: 0,
                },
            }
        }
    }

    impl crate::notename::NoteNameFor<TheFiveLimitStackType> for NoteName {
        fn new_from_stack(stack: &Stack<TheFiveLimitStackType>) -> Self {
            Self::new_from_indices(false, 0, 1, 2, stack)
        }

        fn new_from_stack_actual(stack: &Stack<TheFiveLimitStackType>) -> Self {
            Self::new_from_indices(true, 0, 1, 2, stack)
        }
    }

    impl NoteName {
        fn new_from_indices<T: IntervalBasis>(
            use_actual: bool,
            octave_index: usize,
            fifth_index: usize,
            third_index: usize,
            s: &Stack<T>,
        ) -> Self {
            let octaves;
            let fifths;
            let thirds;
            if use_actual {
                octaves = s.actual[octave_index].to_integer();
                fifths = s.actual[fifth_index].to_integer();
                thirds = s.actual[third_index].to_integer();
            } else {
                octaves = s.target[octave_index];
                fifths = s.target[fifth_index];
                thirds = s.target[third_index];
            }
            Self::new_from_values(octaves, fifths, thirds)
        }

        fn new_from_values(octaves: StackCoeff, fifths: StackCoeff, thirds: StackCoeff) -> Self {
            let ix = 2 + 2 * fifths + thirds;
            NoteName {
                basename: JOHNSTON_BASE_ROW[ix.rem_euclid(7) as usize],
                accidental: Accidental {
                    sharpflat: (1 + fifths + 4 * thirds).div_euclid(7),
                    plusminus: ix.div_euclid(7),
                },
                octave: 4 + octaves + (4 * fifths + 2 * thirds).div_euclid(7),
            }
        }

        /// Write the pitch class (i.e. the note name without the octave number)
        fn write_class<W: fmt::Write>(&self, f: &mut W) -> fmt::Result {
            write!(f, "{}", self.basename)?;

            let sf = self.accidental.sharpflat;
            if sf > 0 {
                for _ in 0..sf {
                    write!(f, "#")?;
                }
            }
            if sf < 0 {
                for _ in 0..-sf {
                    write!(f, "b")?;
                }
            }

            let pm = self.accidental.plusminus;
            if pm > 0 {
                for _ in 0..pm {
                    write!(f, "+")?;
                }
            }
            if pm < 0 {
                for _ in 0..-pm {
                    write!(f, "-")?;
                }
            }

            Ok(())
        }

        /// Write the full note name.
        fn write_full<W: fmt::Write>(&self, f: &mut W) -> fmt::Result {
            self.write_class(f)?;
            write!(f, " {}", self.octave)
        }
    }

    impl fmt::Display for NoteName {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            self.write_full(f)
        }
    }

    #[cfg(test)]
    mod test {
        use super::*;
        use crate::interval::stacktype::fivelimit::mock::MockFiveLimitStackType;
        use crate::notename::NoteNameStyle;

        impl crate::notename::NoteNameFor<MockFiveLimitStackType> for NoteName {
            fn new_from_stack(stack: &Stack<MockFiveLimitStackType>) -> Self {
                Self::new_from_indices(false, 0, 1, 2, stack)
            }

            fn new_from_stack_actual(stack: &Stack<MockFiveLimitStackType>) -> Self {
                Self::new_from_indices(true, 0, 1, 2, stack)
            }
        }

        impl crate::notename::HasNoteNames for MockFiveLimitStackType {
            type NoteName = NoteName;
        }

        #[test]
        fn test_str_name() {
            let examples = [
                ([0, 0, 0], "C 4"),
                ([-1, 0, 0], "C 3"),
                ([1, 0, 0], "C 5"),
                ([0, -4, 0], "Ab- 1"),
                ([0, -3, 0], "Eb- 2"),
                ([0, -2, 0], "Bb- 2"),
                ([0, -1, 0], "F 3"),
                ([0, 1, 0], "G 4"),
                ([0, 2, 0], "D 5"),
                ([0, 3, 0], "A+ 5"),
                ([0, 4, 0], "E+ 6"),
                ([0, 0, -4], "Bbbb- 2"),
                ([0, 0, -3], "Dbb- 3"),
                ([0, 0, -2], "Fb 3"),
                ([0, 0, -1], "Ab 3"),
                ([0, 0, 1], "E 4"),
                ([0, 0, 2], "G# 4"),
                ([0, 0, 3], "B# 4"),
                ([0, 0, 4], "D## 5"),
                ([0, 0, 5], "F###+ 5"),
                ([-1, 2, 1], "F#+ 4"),
                ([1, -2, 2], "F# 4"),
                ([-4, 8, -2], "C++ 4"),
            ];

            for (coeffs, name) in examples.iter() {
                assert_eq!(
                    Stack::<MockFiveLimitStackType>::from_target(coeffs.to_vec())
                        .notename(&NoteNameStyle::Full),
                    String::from(*name)
                );
            }
        }
    }
}
