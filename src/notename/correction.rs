use std::{fmt, marker::PhantomData};

use adaptuner_core::StackTypeConfig;
use ndarray::Array1;
use num_rational::Ratio;
use num_traits::Zero;

use crate::{
    interval::{
        base::Semitones,
        stack::{semitones_from_actual, Stack},
        stacktype::r#trait::{StackCoeff, StackType},
    },
    util::subsequences::Subsequences,
};

#[derive(Debug)]
pub struct Correction<T: StackType> {
    _phantom: PhantomData<T>,
    pub coeffs: Array1<Ratio<StackCoeff>>,
}

pub trait StackCorrectionExt<T: StackType> {
    fn apply_correction(&mut self, correction: &Correction<T>, config: &StackTypeConfig<T>);
}

impl<T: StackType> StackCorrectionExt<T> for Stack<T> {
    fn apply_correction(&mut self, correction: &Correction<T>, config: &StackTypeConfig<T>) {
        self.make_pure();

        for (i, coefficient) in correction.coeffs.iter().enumerate() {
            self.actual
                .scaled_add(*coefficient, &config.named_intervals()[i].coeffs);
        }
    }
}

impl<T: StackType> Correction<T> {
    pub fn new(stack: &Stack<T>, config: &StackTypeConfig<T>) -> Option<Self> {
        let mut correction = Self::new_zero(config);

        if correction.set_with(stack, config) {
            Some(correction)
        } else {
            None
        }
    }

    pub fn new_zero(config: &StackTypeConfig<T>) -> Self {
        Self {
            _phantom: PhantomData,
            coeffs: Array1::zeros(config.num_named_intervals()),
        }
    }

    pub fn semitones(&self, config: &StackTypeConfig<T>) -> Semitones {
        let mut result = 0.0;

        for (i, &coefficient) in self.coeffs.iter().enumerate() {
            result += *coefficient.numer() as Semitones / *coefficient.denom() as Semitones
                * semitones_from_actual::<T>(config.named_intervals()[i].coeffs.view());
        }

        result
    }

    pub fn is_nonzero(&self) -> bool {
        self.coeffs.iter().any(|coefficient| !coefficient.is_zero())
    }

    pub fn set_with(&mut self, stack: &Stack<T>, config: &StackTypeConfig<T>) -> bool {
        let count_nonzero = |coefficients: &Array1<Ratio<StackCoeff>>| -> usize {
            coefficients
                .iter()
                .filter(|coefficient| !coefficient.is_zero())
                .count()
        };

        let offset = {
            let mut offset = stack.actual.to_owned();

            offset.zip_mut_with(&stack.target, |actual, target| {
                *actual -= Ratio::from_integer(*target);
            });

            offset
        };

        let mut tmp: Array1<Ratio<StackCoeff>> = Array1::zeros(stack.actual.len());
        let mut lowest_score = usize::MAX;

        let mut subsequences = Subsequences::new(config.num_named_intervals(), stack.actual.len());

        while let Some(basis_indices) = subsequences.next() {
            if config.with_coordinate_system(&basis_indices, |result| {
                if let Some((ordered_basis_indices, coordinate_system)) = result {
                    coordinate_system.apply_inplace(offset.view(), tmp.view_mut());

                    let score = count_nonzero(&tmp);

                    if score < lowest_score {
                        self.reset_to_zero();

                        tmp.iter().enumerate().for_each(|(i, coefficient)| {
                            self.coeffs[ordered_basis_indices[i]] = *coefficient;
                        });
                    }

                    lowest_score = lowest_score.min(score);

                    if lowest_score <= 1 {
                        return true;
                    }
                }

                false
            }) {
                break;
            }
        }

        lowest_score < usize::MAX
    }

    pub fn reset_to_zero(&mut self) {
        self.coeffs
            .iter_mut()
            .for_each(|coefficient| *coefficient = Ratio::from_integer(0));
    }

    pub fn fmt<W: fmt::Write>(&self, f: &mut W, config: &StackTypeConfig<T>) -> fmt::Result {
        for (i, coefficient) in self.coeffs.iter().enumerate() {
            if coefficient.is_zero() {
                continue;
            }

            let suffix = &config.named_intervals()[i].short_name;

            if *coefficient > Ratio::from_integer(0) {
                write!(f, "+{coefficient}{suffix}")?;
            } else {
                write!(f, "-{}{suffix}", -coefficient)?;
            }
        }

        Ok(())
    }

    pub fn str(&self, config: &StackTypeConfig<T>) -> String {
        let mut result = String::new();
        self.fmt(&mut result, config).unwrap();
        result
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::interval::stacktype::fivelimit::mock::MockFiveLimitStackType;
    use crate::interval::temperament::TemperamentDefinition;
    use ndarray::{arr1, arr2};

    fn mock_config() -> StackTypeConfig<MockFiveLimitStackType> {
        let temperament_definitions = vec![
            TemperamentDefinition::new(
                "equal temperament".into(),
                arr2(&[[0, 12, 0], [0, 0, 3], [1, 0, 0]]),
                arr2(&[[7, 0, 0], [1, 0, 0], [1, 0, 0]]),
            ),
            TemperamentDefinition::new(
                "1/4-comma meantone fifths".into(),
                arr2(&[[0, 4, 0], [1, 0, 0], [0, 0, 1]]),
                arr2(&[[2, 0, 1], [1, 0, 0], [0, 0, 1]]),
            ),
        ];

        StackTypeConfig::new(
            temperament_definitions,
            crate::interval::stacktype::fivelimit::mock::named_intervals(),
        )
        .unwrap()
    }

    #[test]
    fn test_correction() {
        // Preference order: pythagorean comma, diesis, syntonic comma, octave.
        let config = mock_config();

        assert_eq!(
            Correction::<MockFiveLimitStackType>::new(&Stack::new_zero(), &config)
                .unwrap()
                .coeffs,
            arr1(&[0.into(), 0.into(), 0.into(), 0.into()])
        );

        assert_eq!(
            Correction::<MockFiveLimitStackType>::new(
                &Stack::from_target(vec![123, 234, 345]),
                &config,
            )
            .unwrap()
            .coeffs,
            arr1(&[0.into(), 0.into(), 0.into(), 0.into()])
        );

        assert_eq!(
            Correction::<MockFiveLimitStackType>::new(
                &Stack::from_target_and_actual(
                    vec![0, 0, 3].into(),
                    vec![1.into(), 0.into(), 0.into()].into(),
                ),
                &config,
            )
            .unwrap()
            .coeffs,
            arr1(&[0.into(), 1.into(), 0.into(), 0.into()])
        );

        assert_eq!(
            Correction::<MockFiveLimitStackType>::new(
                &Stack::from_temperaments_and_target_with_config(
                    &config,
                    &[true, false],
                    vec![0, 1, 1],
                ),
                &config,
            )
            .unwrap()
            .coeffs,
            arr1(&[Ratio::new(-1, 12), Ratio::new(1, 3), 0.into(), 0.into(),])
        );

        assert_eq!(
            Correction::<MockFiveLimitStackType>::new(
                &Stack::from_temperaments_and_target_with_config(
                    &config,
                    &[false, true],
                    vec![0, 1, 0],
                ),
                &config,
            )
            .unwrap()
            .coeffs,
            arr1(&[0.into(), 0.into(), Ratio::new(-1, 4), 0.into()])
        );
    }
}
