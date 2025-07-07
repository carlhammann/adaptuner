pub mod fivelimit {
    use ndarray::ArrayView1;
    use std::fmt;

    use crate::interval::{
        stack::Stack,
        stacktype::r#trait::{FiveLimitStackType, StackCoeff, StackType},
    };

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
    use BaseName::*;

    impl std::fmt::Display for BaseName {
        fn fmt(&self, f: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
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

    #[derive(Clone)]
    pub struct Accidental {
        pub sharpflat: StackCoeff,
        pub plusminus: StackCoeff,
    }

    #[derive(Clone)]
    pub struct NoteName {
        pub basename: BaseName,
        pub octave: StackCoeff,
        pub accidental: Accidental,
    }

    const JOHNSTON_BASE_ROW: [BaseName; 7] = [F, A, C, E, G, B, D];

    impl NoteName {
        pub fn new<T: FiveLimitStackType>(s: &Stack<T>) -> Self {
            Self::new_from_indices(
                false,
                T::octave_index(),
                T::fifth_index(),
                T::third_index(),
                s,
            )
        }

        /// like [Self::new], but doesn't need a whole [Stack] as an argument, only the
        /// [Stack::target] coefficients
        pub fn new_from_coeffs<T: FiveLimitStackType>(coeffs: ArrayView1<StackCoeff>) -> Self {
            let octaves = coeffs[T::octave_index()];
            let fifths = coeffs[T::fifth_index()];
            let thirds = coeffs[T::third_index()];
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

        /// like new, but uses the [Stack::actual] instead of the [Stack::target]. Fractions are
        /// rounded in an unspecified way.
        ///
        /// This function makes sense when you know that the [Stack::actual] describes a pure
        /// interval, which is differenf from the the [Stack::target]: I.e. [Stack::is_pure()], but
        /// not [Stack::is_target()].
        pub fn new_from_actual<T: FiveLimitStackType>(s: &Stack<T>) -> Self {
            Self::new_from_indices(
                true,
                T::octave_index(),
                T::fifth_index(),
                T::third_index(),
                s,
            )
        }

        fn new_from_indices<T: StackType>(
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
        pub fn write_class<W: fmt::Write>(&self, f: &mut W) -> fmt::Result {
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
        pub fn write_full<W: fmt::Write>(&self, f: &mut W) -> fmt::Result {
            self.write_class(f)?;
            write!(f, " {}", self.octave)
        }

        /// The pitch class as [String].
        pub fn str_class(&self) -> String {
            let mut res = String::new();
            // the [Write] implementation of [String] never throws any error, so this is fine:
            self.write_class(&mut res).unwrap();
            res
        }

        /// The full note name as a [String].
        pub fn str_full(&self) -> String {
            let mut res = String::new();
            // the [Write] implementation of [String] never throws any error, so this is fine:
            self.write_full(&mut res).unwrap();
            res
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

        type MockStackType = crate::interval::stacktype::fivelimit::ConcreteFiveLimitStackType;

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
                    NoteName::new(&Stack::<MockStackType>::from_target(coeffs.to_vec())).str_full(),
                    String::from(*name)
                );
            }
        }
    }
}
