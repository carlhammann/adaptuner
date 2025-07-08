pub mod fivelimit {
    use std::{fmt, sync::LazyLock};

    use ndarray::{arr1, arr2, Array2, ArrayView1};
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
        DiesisSyntonic([Ratio<StackCoeff>; 2]),
        PythagoreanSyntonic([Ratio<StackCoeff>; 2]),
        PythagoreanDiesis([Ratio<StackCoeff>; 2]),
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
                Correction::DiesisSyntonic([a, b]) => a.is_zero() & b.is_zero(),
                Correction::PythagoreanSyntonic([a, b]) => a.is_zero() & b.is_zero(),
                Correction::PythagoreanDiesis([a, b]) => a.is_zero() & b.is_zero(),
            }
        }

        pub fn new<T: FiveLimitStackType>(stack: &Stack<T>, basis: CorrectionBasis) -> Self {
            Self::from_target_and_actual::<T>((&stack.target).into(), (&stack.actual).into(), basis)
        }

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

            match basis {
                CorrectionBasis::Semitones => Self::Semitones(the_semitones()),
                CorrectionBasis::PythagoreanDiesis => {
                    let coeffs = PYTHAGOREAN_DIESIS.dot(&offset);
                    if coeffs[0].is_zero() {
                        Self::PythagoreanDiesis([coeffs[1], coeffs[2]])
                    } else {
                        Self::Semitones(the_semitones())
                    }
                }

                CorrectionBasis::PythagoreanSyntonic => {
                    let coeffs = PYTHAGOREAN_SYNTONIC.dot(&offset);
                    if coeffs[0].is_zero() {
                        Self::PythagoreanSyntonic([coeffs[1], coeffs[2]])
                    } else {
                        Self::Semitones(the_semitones())
                    }
                }

                CorrectionBasis::DiesisSyntonic => {
                    let coeffs = DIESIS_SYNTONIC.dot(&offset);
                    if coeffs[0].is_zero() {
                        Self::DiesisSyntonic([coeffs[1], coeffs[2]])
                    } else {
                        Self::Semitones(the_semitones())
                    }
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
                Correction::DiesisSyntonic([d, s]) => {
                    write_fraction(d, "d")?;
                    write_fraction(s, "s")
                }
                Correction::PythagoreanSyntonic([p, s]) => {
                    write_fraction(p, "p")?;
                    write_fraction(s, "s")
                }
                Correction::PythagoreanDiesis([p, d]) => {
                    write_fraction(p, "p")?;
                    write_fraction(d, "d")
                }
            }
        }
    }

    #[cfg(test)]
    mod test {
        use super::*;
        use crate::interval::stacktype::fivelimit::ConcreteFiveLimitStackType;

        #[test]
        fn test_correction() {
            assert_eq!(
                Correction::new(
                    &Stack::<ConcreteFiveLimitStackType>::new_zero(),
                    CorrectionBasis::PythagoreanDiesis
                ),
                Correction::PythagoreanDiesis([0.into(), 0.into()])
            );

            assert_eq!(
                Correction::new(
                    &Stack::<ConcreteFiveLimitStackType>::from_target(vec![123, 234, 345]),
                    CorrectionBasis::PythagoreanDiesis
                ),
                Correction::PythagoreanDiesis([0.into(), 0.into()])
            );

            assert_eq!(
                Correction::new(
                    &Stack::<ConcreteFiveLimitStackType>::from_temperaments_and_target(
                        &[true, false],
                        vec![0, 0, 3]
                    ),
                    CorrectionBasis::PythagoreanDiesis
                ),
                Correction::PythagoreanDiesis([0.into(), 1.into()])
            );

            assert_eq!(
                Correction::new(
                    &Stack::<ConcreteFiveLimitStackType>::from_temperaments_and_target(
                        &[true, false],
                        vec![0, 1, 1]
                    ),
                    CorrectionBasis::PythagoreanDiesis
                ),
                Correction::PythagoreanDiesis([Ratio::new(-1, 12), Ratio::new(1, 3)])
            );

            assert_eq!(
                Correction::new(
                    &Stack::<ConcreteFiveLimitStackType>::from_temperaments_and_target(
                        &[false, true],
                        vec![0, 1, 0]
                    ),
                    CorrectionBasis::PythagoreanDiesis
                ),
                // indeed: a quarter syntonic comma is a twelfth diesis plus a twelfth pythagorean comma
                Correction::PythagoreanDiesis([Ratio::new(-1, 12), Ratio::new(-1, 12)])
            );

            assert_eq!(
                Correction::new(
                    &Stack::<ConcreteFiveLimitStackType>::from_temperaments_and_target(
                        &[false, true],
                        vec![0, 1, 0]
                    ),
                    CorrectionBasis::PythagoreanSyntonic
                ),
                Correction::PythagoreanSyntonic([0.into(), Ratio::new(-1, 4)])
            );

            assert_eq!(
                Correction::new(
                    &Stack::<ConcreteFiveLimitStackType>::from_temperaments_and_target(
                        &[true, true],
                        vec![0, 1, 0]
                    ),
                    CorrectionBasis::PythagoreanSyntonic
                ),
                Correction::PythagoreanSyntonic([Ratio::new(-1, 12), Ratio::new(-1, 4)])
            );

            assert_eq!(
                Correction::new(
                    &Stack::<ConcreteFiveLimitStackType>::from_temperaments_and_target(
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
                Correction::DiesisSyntonic([Ratio::new(3, 4), Ratio::new(-1, 2)])
            );

            assert_eq!(
                Correction::new(
                    &Stack::<ConcreteFiveLimitStackType>::from_target_and_actual(
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
