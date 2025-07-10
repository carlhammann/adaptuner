pub mod fivelimit {
    use std::{fmt, sync::LazyLock};

    use ndarray::{arr1, arr2, linalg::general_mat_vec_mul, Array2, ArrayView1};
    use num_rational::Ratio;
    use num_traits::Zero;

    use crate::interval::{
        base::Semitones,
        stack::{semitones_from_actual, semitones_from_target, Stack},
        stacktype::r#trait::{FiveLimitStackType, StackCoeff},
    };

    #[derive(PartialEq, Debug)]
    pub enum Correction {
        Semitones(Semitones),

        /// invariant: only at most two entries will ever be non-zero
        Commas {
            diesis: Ratio<StackCoeff>,
            pythagorean: Ratio<StackCoeff>,
            syntonic: Ratio<StackCoeff>,
        },
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
            match self {
                Correction::Semitones(x) => *x == 0.0,
                Correction::Commas {
                    diesis,
                    pythagorean,
                    syntonic,
                } => diesis.is_zero() & pythagorean.is_zero() & syntonic.is_zero(),
            }
        }

        /// Try to express the deviation of the [Stack::actual] from the [Stack::target] in the
        /// given basis of two commas:
        ///
        /// - If there's a way to write the deviation as a multiple of only one of the three commas
        ///   (syntonic, pythagorean, (lesser) diesis), use that representation, disregarding the
        ///   basis.
        /// - If the deviation cannot be written in terms of the basis, return [Correction::Semitones].
        pub fn new<T: FiveLimitStackType>(stack: &Stack<T>, basis: CorrectionBasis) -> Self {
            Self::from_target_and_actual::<T>((&stack.target).into(), (&stack.actual).into(), basis)
        }

        /// Like [Self::new], only taking the [Stack::target] and [Stack::actual] as separate
        /// arguments.
        pub fn from_target_and_actual<T: FiveLimitStackType>(
            target: ArrayView1<StackCoeff>,
            actual: ArrayView1<Ratio<StackCoeff>>,
            basis: CorrectionBasis,
        ) -> Self {
            let offset = arr1(&[
                actual[T::octave_index()] - target[T::octave_index()],
                actual[T::fifth_index()] - target[T::fifth_index()],
                actual[T::third_index()] - target[T::third_index()],
            ]);

            let the_semitones =
                || semitones_from_actual::<T>(actual) - semitones_from_target::<T>(target);

            if basis == CorrectionBasis::Semitones {
                Self::Semitones(the_semitones())
            } else {
                let mut coeffs = Array2::zeros((3, 3));
                general_mat_vec_mul(
                    1.into(),
                    &DIESIS_SYNTONIC,
                    &offset,
                    0.into(),
                    &mut coeffs.column_mut(0),
                );
                if coeffs[(0, 0)].is_zero() & (coeffs[(1, 0)].is_zero() | coeffs[(2, 0)].is_zero())
                {
                    return Self::Commas {
                        diesis: coeffs[(1, 0)],
                        pythagorean: 0.into(),
                        syntonic: coeffs[(2, 0)],
                    };
                }

                general_mat_vec_mul(
                    1.into(),
                    &PYTHAGOREAN_SYNTONIC,
                    &offset,
                    0.into(),
                    &mut coeffs.column_mut(1),
                );
                if coeffs[(0, 1)].is_zero() & (coeffs[(1, 1)].is_zero() | coeffs[(2, 1)].is_zero())
                {
                    return Self::Commas {
                        diesis: 0.into(),
                        pythagorean: coeffs[(1, 1)],
                        syntonic: coeffs[(2, 1)],
                    };
                }

                general_mat_vec_mul(
                    1.into(),
                    &PYTHAGOREAN_DIESIS,
                    &offset,
                    0.into(),
                    &mut coeffs.column_mut(2),
                );
                if coeffs[(0, 2)].is_zero() & (coeffs[(1, 2)].is_zero() | coeffs[(2, 2)].is_zero())
                {
                    return Self::Commas {
                        pythagorean: coeffs[(1, 2)],
                        diesis: coeffs[(2, 2)],
                        syntonic: 0.into(),
                    };
                }

                match basis {
                    CorrectionBasis::DiesisSyntonic => {
                        if coeffs[(0, 0)].is_zero() {
                            Self::Commas {
                                diesis: coeffs[(1, 0)],
                                pythagorean: 0.into(),
                                syntonic: coeffs[(2, 0)],
                            }
                        } else {
                            Self::Semitones(the_semitones())
                        }
                    }
                    CorrectionBasis::PythagoreanSyntonic => {
                        if coeffs[(0, 1)].is_zero() {
                            Self::Commas {
                                diesis: 0.into(),
                                pythagorean: coeffs[(1, 1)],
                                syntonic: coeffs[(2, 1)],
                            }
                        } else {
                            Self::Semitones(the_semitones())
                        }
                    }
                    CorrectionBasis::PythagoreanDiesis => {
                        if coeffs[(0, 2)].is_zero() {
                            Self::Commas {
                                pythagorean: coeffs[(1, 2)],
                                diesis: coeffs[(2, 2)],
                                syntonic: 0.into(),
                            }
                        } else {
                            Self::Semitones(the_semitones())
                        }
                    }
                    CorrectionBasis::Semitones => unreachable!(),
                }
            }
        }
    }

    impl fmt::Display for Correction {
        fn fmt(&self, f: &mut std::fmt::Formatter) -> fmt::Result {
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
            match self {
                Correction::Semitones(s) => {
                    if *s > 0.0 {
                        write!(f, "+{:.02}ct", s * 100.0)
                    } else if *s < 0.0 {
                        write!(f, "-{:.02}ct", -s * 100.0)
                    } else {
                        Ok(())
                    }
                }
                Correction::Commas {
                    diesis,
                    pythagorean,
                    syntonic,
                } => {
                    write_fraction(diesis, "d")?;
                    write_fraction(pythagorean, "p")?;
                    write_fraction(syntonic, "s")
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
                Correction::new(
                    &Stack::<MockFiveLimitStackType>::new_zero(),
                    CorrectionBasis::PythagoreanDiesis
                ),
                Correction::Commas {
                    diesis: 0.into(),
                    pythagorean: 0.into(),
                    syntonic: 0.into()
                }
            );

            assert_eq!(
                Correction::new(
                    &Stack::<MockFiveLimitStackType>::from_target(vec![123, 234, 345]),
                    CorrectionBasis::PythagoreanDiesis
                ),
                Correction::Commas {
                    diesis: 0.into(),
                    pythagorean: 0.into(),
                    syntonic: 0.into()
                }
            );

            assert_eq!(
                Correction::new(
                    &Stack::<MockFiveLimitStackType>::from_temperaments_and_target(
                        &[true, false],
                        vec![0, 0, 3]
                    ),
                    CorrectionBasis::PythagoreanDiesis
                ),
                Correction::Commas {
                    diesis: 1.into(),
                    pythagorean: 0.into(),
                    syntonic: 0.into()
                }
            );

            assert_eq!(
                Correction::new(
                    &Stack::<MockFiveLimitStackType>::from_temperaments_and_target(
                        &[true, false],
                        vec![0, 1, 1]
                    ),
                    CorrectionBasis::PythagoreanDiesis
                ),
                Correction::Commas {
                    diesis: Ratio::new(1, 3),
                    pythagorean: Ratio::new(-1, 12),
                    syntonic: 0.into()
                }
            );

            assert_eq!(
                Correction::new(
                    &Stack::<MockFiveLimitStackType>::from_temperaments_and_target(
                        &[false, true],
                        vec![0, 1, 0]
                    ),
                    CorrectionBasis::PythagoreanDiesis
                ),
                // this can be written more simply, so the basis is ignored.
                Correction::Commas {
                    diesis: 0.into(),
                    pythagorean: 0.into(),
                    syntonic: Ratio::new(-1, 4)
                }
            );

            assert_eq!(
                Correction::new(
                    &Stack::<MockFiveLimitStackType>::from_temperaments_and_target(
                        &[false, true],
                        vec![0, 1, 0]
                    ),
                    CorrectionBasis::PythagoreanSyntonic
                ),
                Correction::Commas {
                    diesis: 0.into(),
                    pythagorean: 0.into(),
                    syntonic: Ratio::new(-1, 4)
                }
            );

            assert_eq!(
                Correction::new(
                    &Stack::<MockFiveLimitStackType>::from_temperaments_and_target(
                        &[true, true],
                        vec![0, 1, 0]
                    ),
                    CorrectionBasis::PythagoreanSyntonic
                ),
                Correction::Commas {
                    diesis: 0.into(),
                    pythagorean: Ratio::new(-1, 12),
                    syntonic: Ratio::new(-1, 4)
                }
            );

            assert_eq!(
                Correction::new(
                    &Stack::<MockFiveLimitStackType>::from_temperaments_and_target(
                        &[true, true],
                        vec![0, 1, 2]
                    ),
                    CorrectionBasis::DiesisSyntonic
                ),
                // the fifth is corrected by one qurter syntonic comma plus a twelfth pythagorean comma down
                // the thirds are each corrected by one third of a diesis up
                //
                // together, that makes
                //
                // -1/4 s - 1/12 p + 2/3 d = 3/4 d - 1/2 s
                Correction::Commas {
                    diesis: Ratio::new(3,4),
                    pythagorean: 0.into(),
                    syntonic: Ratio::new(-1, 2)
                }
            );

            assert_eq!(
                Correction::new(
                    &Stack::<MockFiveLimitStackType>::from_target_and_actual(
                        arr1(&[0, 0, 0]),
                        arr1(&[Ratio::new(1, 120), 0.into(), 0.into()])
                    ),
                    CorrectionBasis::PythagoreanSyntonic
                ),
                Correction::Semitones(0.1)
            );
        }
    }
}
