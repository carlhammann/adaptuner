pub mod fivelimit {
    use std::{fmt, sync::LazyLock};

    use ndarray::{arr1, arr2, linalg::general_mat_vec_mul, Array2, ArrayView1};
    use num_rational::Ratio;
    use num_traits::Zero;

    use crate::interval::{
        base::Semitones,
        stack::{semitones_from_actual, semitones_from_target, Stack},
        stacktype::r#trait::{FiveLimitIntervalBasis, StackCoeff},
    };

    #[derive(PartialEq, Debug)]
    pub struct Correction {
        comma_coeffs: Array2<Ratio<StackCoeff>>,
        semitones: Semitones,
    }

    #[derive(PartialEq, Clone, Copy)]
    pub enum CorrectionBasis {
        Semitones,
        DiesisSyntonic,
        PythagoreanSyntonic,
        PythagoreanDiesis,
    }

    const DIESIS_SYNTONIC: LazyLock<Array2<Ratio<StackCoeff>>> = LazyLock::new(|| {
        arr2(&[
            [1.into(), Ratio::new(7, 12), Ratio::new(1, 3)],
            [0.into(), Ratio::new(-1, 12), Ratio::new(-1, 3)],
            [0.into(), Ratio::new(1, 4), 0.into()],
        ])
    });

    const PYTHAGOREAN_SYNTONIC: LazyLock<Array2<Ratio<StackCoeff>>> = LazyLock::new(|| {
        arr2(&[
            [1.into(), Ratio::new(7, 12), Ratio::new(1, 3)],
            [0.into(), Ratio::new(1, 12), Ratio::new(1, 3)],
            [0.into(), 0.into(), (-1).into()],
        ])
    });

    const PYTHAGOREAN_DIESIS: LazyLock<Array2<Ratio<StackCoeff>>> = LazyLock::new(|| {
        arr2(&[
            [1.into(), Ratio::new(7, 12), Ratio::new(1, 3)],
            [0.into(), Ratio::new(1, 12), 0.into()],
            [0.into(), 0.into(), Ratio::new(-1, 3)],
        ])
    });

    impl Correction {
        pub fn is_zero(&self) -> bool {
            self.comma_coeffs
                .iter()
                .all(<Ratio<StackCoeff> as Zero>::is_zero)
        }

        pub fn new<T: FiveLimitIntervalBasis>(stack: &Stack<T>) -> Self {
            Self::from_target_and_actual::<T>((&stack.target).into(), (&stack.actual).into())
        }

        /// Like [Self::new], only taking the [Stack::target] and [Stack::actual] as separate
        /// arguments.
        pub fn from_target_and_actual<T: FiveLimitIntervalBasis>(
            target: ArrayView1<StackCoeff>,
            actual: ArrayView1<Ratio<StackCoeff>>,
        ) -> Self {
            let offset = arr1(&[
                actual[T::octave_index()] - target[T::octave_index()],
                actual[T::fifth_index()] - target[T::fifth_index()],
                actual[T::third_index()] - target[T::third_index()],
            ]);

            let mut comma_coeffs = Array2::zeros((3, 3));

            general_mat_vec_mul(
                1.into(),
                &DIESIS_SYNTONIC,
                &offset,
                0.into(),
                &mut comma_coeffs.column_mut(0),
            );

            general_mat_vec_mul(
                1.into(),
                &PYTHAGOREAN_SYNTONIC,
                &offset,
                0.into(),
                &mut comma_coeffs.column_mut(1),
            );

            general_mat_vec_mul(
                1.into(),
                &PYTHAGOREAN_DIESIS,
                &offset,
                0.into(),
                &mut comma_coeffs.column_mut(2),
            );

            Self {
                comma_coeffs,
                semitones: semitones_from_actual::<T>(actual) - semitones_from_target::<T>(target),
            }
        }
    }

    impl Correction {
        pub fn str(&self, basis: &CorrectionBasis) -> String {
            let mut res = String::new();
            // the [Write] implementation of [String] never throws any error, so this is fine:
            self.fmt(&mut res, basis).unwrap();
            res
        }

        pub fn fmt<W: fmt::Write>(&self, f: &mut W, basis: &CorrectionBasis) -> fmt::Result {
            let mut write_fraction = |x: &Ratio<StackCoeff>, suffix: &str| {
                if x.is_zero() {
                    return Ok(());
                }
                if *x > Ratio::from_integer(0) {
                    write!(f, "+{x}{suffix}")?;
                } else {
                    write!(f, "-{}{suffix}", -x)?;
                }
                Ok(())
            };

            if *basis == CorrectionBasis::Semitones {
                write!(f, "+{:.02}ct", self.semitones * 100.0)
            } else if self.comma_coeffs[(0, 0)].is_zero()
                & (self.comma_coeffs[(1, 0)].is_zero() | self.comma_coeffs[(2, 0)].is_zero())
            {
                write_fraction(&self.comma_coeffs[(1, 0)], "d")?;
                write_fraction(&self.comma_coeffs[(2, 0)], "s")
            } else if self.comma_coeffs[(0, 1)].is_zero()
                & (self.comma_coeffs[(1, 1)].is_zero() | self.comma_coeffs[(2, 1)].is_zero())
            {
                write_fraction(&self.comma_coeffs[(1, 1)], "p")?;
                write_fraction(&self.comma_coeffs[(2, 1)], "s")
            } else if self.comma_coeffs[(0, 2)].is_zero()
                & (self.comma_coeffs[(1, 2)].is_zero() | self.comma_coeffs[(2, 2)].is_zero())
            {
                write_fraction(&self.comma_coeffs[(1, 2)], "p")?;
                write_fraction(&self.comma_coeffs[(2, 2)], "d")
            } else {
                match basis {
                    CorrectionBasis::DiesisSyntonic => {
                        if self.comma_coeffs[(0, 0)].is_zero() {
                            write_fraction(&self.comma_coeffs[(1, 0)], "d")?;
                            write_fraction(&self.comma_coeffs[(2, 0)], "s")
                        } else {
                            write!(f, "+{:.02}ct", self.semitones * 100.0)
                        }
                    }
                    CorrectionBasis::PythagoreanSyntonic => {
                        if self.comma_coeffs[(0, 1)].is_zero() {
                            write_fraction(&self.comma_coeffs[(1, 1)], "p")?;
                            write_fraction(&self.comma_coeffs[(2, 1)], "s")
                        } else {
                            write!(f, "+{:.02}ct", self.semitones * 100.0)
                        }
                    }
                    CorrectionBasis::PythagoreanDiesis => {
                        if self.comma_coeffs[(0, 2)].is_zero() {
                            write_fraction(&self.comma_coeffs[(1, 2)], "p")?;
                            write_fraction(&self.comma_coeffs[(2, 2)], "d")
                        } else {
                            write!(f, "+{:.02}ct", self.semitones * 100.0)
                        }
                    }
                    CorrectionBasis::Semitones => unreachable!(),
                }
            }
        }
    }

    #[cfg(test)]
    mod test {
        use super::*;
        use crate::interval::stacktype::fivelimit::mock::MockFiveLimitStackType;

        #[test]
        fn test_correction() {
            assert_eq!(
                Correction::new(&Stack::<MockFiveLimitStackType>::new_zero())
                    .str(&CorrectionBasis::PythagoreanDiesis),
                ""
            );

            assert_eq!(
                Correction::new(&Stack::<MockFiveLimitStackType>::from_target(vec![
                    123, 234, 345
                ]))
                .str(&CorrectionBasis::PythagoreanDiesis),
                ""
            );

            assert_eq!(
                Correction::new(
                    &Stack::<MockFiveLimitStackType>::from_temperaments_and_target(
                        &[true, false],
                        vec![0, 0, 3]
                    )
                )
                .str(&CorrectionBasis::PythagoreanDiesis),
                "+1d"
            );

            assert_eq!(
                Correction::new(
                    &Stack::<MockFiveLimitStackType>::from_temperaments_and_target(
                        &[true, false],
                        vec![0, 1, 1]
                    )
                )
                .str(&CorrectionBasis::PythagoreanDiesis),
                "-1/12p+1/3d"
            );

            assert_eq!(
                Correction::new(
                    &Stack::<MockFiveLimitStackType>::from_temperaments_and_target(
                        &[false, true],
                        vec![0, 1, 0]
                    )
                )
                .str(&CorrectionBasis::PythagoreanDiesis),
                // this can be written more simply, so the basis argument to [Correction::str] is ignored.
                "-1/4s"
            );

            assert_eq!(
                Correction::new(
                    &Stack::<MockFiveLimitStackType>::from_temperaments_and_target(
                        &[false, true],
                        vec![0, 1, 0]
                    )
                )
                .str(&CorrectionBasis::PythagoreanSyntonic),
                "-1/4s"
            );

            assert_eq!(
                Correction::new(
                    &Stack::<MockFiveLimitStackType>::from_temperaments_and_target(
                        &[true, true],
                        vec![0, 1, 0]
                    )
                )
                .str(&CorrectionBasis::PythagoreanSyntonic),
                "-1/12p-1/4s"
            );

            assert_eq!(
                Correction::new(
                    &Stack::<MockFiveLimitStackType>::from_temperaments_and_target(
                        &[true, true],
                        vec![0, 1, 2]
                    )
                )
                .str(&CorrectionBasis::DiesisSyntonic),
                // the fifth is corrected by one qurter syntonic comma plus a twelfth pythagorean comma down
                // the thirds are each corrected by one third of a diesis up
                //
                // together, that makes
                //
                // -1/4 s - 1/12 p + 2/3 d = 3/4 d - 1/2 s
                "+3/4d-1/2s"
            );

            assert_eq!(
                Correction::new(&Stack::<MockFiveLimitStackType>::from_target_and_actual(
                    arr1(&[0, 0, 0]),
                    arr1(&[Ratio::new(1, 120), 0.into(), 0.into()])
                ))
                .str(&CorrectionBasis::PythagoreanSyntonic),
                "+10.00ct"
            );
        }
    }
}
