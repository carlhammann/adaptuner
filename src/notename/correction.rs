pub mod fivelimit {
    use std::{fmt, sync::LazyLock};

    use ndarray::{arr1, arr2, Array2};
    use num_rational::Ratio;
    use num_traits::Zero;

    use crate::interval::{
        base::Semitones,
        stack::Stack,
        stacktype::r#trait::{FiveLimitStackType, StackCoeff},
    };

    #[derive(PartialEq, Debug)]
    pub enum Correction {
        Semitones(Semitones),
        DiesisSyntonic([Ratio<StackCoeff>; 2]),
        PythagoreanSyntonic([Ratio<StackCoeff>; 2]),
        PythagoreanDiesis([Ratio<StackCoeff>; 2]),
    }

    #[derive(Clone, Copy)]
    pub enum CorrectionBasis {
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
        pub fn new<T: FiveLimitStackType>(stack: &Stack<T>, basis: CorrectionBasis) -> Self {
            let offset = arr1(&[
                stack.actual[T::octave_index()] - stack.target[T::octave_index()],
                stack.actual[T::fifth_index()] - stack.target[T::fifth_index()],
                stack.actual[T::third_index()] - stack.target[T::third_index()],
            ]);

            let the_semitones = || stack.semitones() - stack.target_semitones();

            match basis {
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
            match self {
                Correction::Semitones(s) => write!(f, "{:.02} ct", s * 100.0),
                Correction::DiesisSyntonic([d, s]) => {
                    if *s > 0.into() {
                        write!(f, "{d} d + {s} s")
                    } else {
                        write!(f, "{d} d - {} s", -s)
                    }
                }
                Correction::PythagoreanSyntonic([p, s]) => {
                    if *s > 0.into() {
                        write!(f, "{p} p + {s} s")
                    } else {
                        write!(f, "{p} p - {} s", -s)
                    }
                }
                Correction::PythagoreanDiesis([p, d]) => {
                    if *d > 0.into() {
                        write!(f, "{p} p + {d} d")
                    } else {
                        write!(f, "{p} p - {} d", -d)
                    }
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
