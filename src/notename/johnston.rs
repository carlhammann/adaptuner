pub mod fivelimit {
    use crate::interval::{Stack, StackCoeff};
    use crate::util::{fixed_sizes::Size3, Bounded, Dimension};
    use std::fmt;

    pub struct NoteName {
        base: char,
        sharpflat: StackCoeff,
        plusminus: StackCoeff,
        octave: StackCoeff,
    }

    const JOHNSTON_BASE_ROW: [char; 7] = ['F', 'A', 'C', 'E', 'G', 'B', 'D'];
    impl NoteName {
        /// Construct a [NoteName] from a [Stack] of intervals on middle C.
        ///
        /// It is assumed that the first three entries in the [coefficients][Stack::coefficients]
        /// of the argument denote the numbers of octaves, fifths, and thirds, in that order. (In
        /// particular, there must be at least three base intervals.)
        pub fn new<T: Dimension + Copy>(s: &Stack<Size3, T>) -> Self {
            let octaves = s.coefficients()[Bounded::new(0).unwrap()];
            let fifths = s.coefficients()[Bounded::new(1).unwrap()];
            let thirds = s.coefficients()[Bounded::new(2).unwrap()];
            let ix = 2 + 2 * fifths + thirds;
            NoteName {
                base: JOHNSTON_BASE_ROW[ix.rem_euclid(7) as usize],
                sharpflat: (1 + fifths + 4 * thirds).div_euclid(7),
                plusminus: ix.div_euclid(7),
                octave: 4 + octaves + (4 * fifths + 2 * thirds).div_euclid(7),
            }
        }

        /// Write the pitch class (i.e. the note name without the octave number)
        pub fn write_class<W: fmt::Write>(&self, f: &mut W) -> fmt::Result {
            write!(f, "{}", self.base)?;

            let sf = self.sharpflat;
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

            let pm = self.plusminus;
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
        use crate::interval::stack_test_setup::init_stacktype;
        use crate::util::vector;

        #[test]
        fn test_str_name() {
            let st = init_stacktype();

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
                    NoteName::new(
                        &Stack::new(
                            &st,
                            &vector(&[false, false]).unwrap(),
                            vector(coeffs).unwrap()
                        )
                        .unwrap()
                    )
                    .str_full(),
                    String::from(*name)
                );
            }
        }
    }
}
