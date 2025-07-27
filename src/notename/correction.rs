use std::{fmt, marker::PhantomData};

use ndarray::Array1;
use num_rational::Ratio;
use num_traits::Zero;

use crate::{
    interval::{
        stack::Stack,
        stacktype::r#trait::{StackCoeff, StackType},
    },
    util::subsequences::Subsequences,
};

#[derive(Debug)]
pub struct Correction<T: StackType> {
    _phantom: PhantomData<T>,
    pub coeffs: Array1<Ratio<StackCoeff>>,
}

impl<T: StackType> Stack<T> {
    pub fn apply_correction(&mut self, correction: &Correction<T>) {
        self.make_pure();
        for (i, c) in correction.coeffs.iter().enumerate() {
            self.actual.scaled_add(*c, &T::named_intervals()[i].coeffs)
        }
    }
}

impl<T: StackType> Correction<T> {
    /// `preference_order` contains indices into ´T::named_intervals()`
    pub fn new(stack: &Stack<T>, preference_order: &[usize]) -> Option<Self> {
        let mut res = Self::new_zero();
        if res.set_with(stack, preference_order) {
            Some(res)
        } else {
            None {}
        }
    }

    pub fn new_zero() -> Self {
        Self {
            _phantom: PhantomData,
            coeffs: Array1::zeros(T::num_named_intervals()),
        }
    }

    pub fn set_with(&mut self, stack: &Stack<T>, preference_order: &[usize]) -> bool {
        let count_nonzero = |coeffs: &Array1<Ratio<StackCoeff>>| -> usize {
            coeffs.iter().filter(|x| !x.is_zero()).count()
        };

        let offset = {
            let mut offset = stack.actual.to_owned();
            offset.zip_mut_with(&stack.target, |l, r| {
                *l -= Ratio::from_integer(*r);
            });
            offset
        };
        let mut tmp: Array1<Ratio<StackCoeff>> = Array1::zeros(T::num_intervals());
        let mut lowest_score = usize::MAX;

        let mut subsequences = Subsequences::new(preference_order, T::num_intervals());
        while let Some(basis_indices) = subsequences.next() {
            if T::with_coordinate_system(basis_indices, |x| {
                if let Some((ordered_basis_indices, coordinate_system)) = x {
                    coordinate_system.apply_inplace(offset.view(), tmp.view_mut());
                    let score = count_nonzero(&tmp);
                    if score < lowest_score {
                        self.coeffs.iter_mut().for_each(|c| *c = 0.into());
                        tmp.iter()
                            .enumerate()
                            .for_each(|(i, c)| self.coeffs[ordered_basis_indices[i]] = *c);
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
            .for_each(|x| *x = Ratio::from_integer(0));
    }

    pub fn fmt<W: fmt::Write>(&self, f: &mut W) -> fmt::Result {
        for (i, x) in self.coeffs.iter().enumerate() {
            let suffix = T::named_intervals()[i].short_name;
            if x.is_zero() {
                continue;
            }
            if *x > Ratio::from_integer(0) {
                write!(f, "+{x}{suffix}")?;
            } else {
                write!(f, "-{}{suffix}", -x)?;
            }
        }

        Ok(())
    }

    pub fn str(&self) -> String {
        let mut res = String::new();
        self.fmt(&mut res).unwrap();
        res
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::interval::stacktype::fivelimit::mock::MockFiveLimitStackType;
    use ndarray::arr1;

    #[test]
    fn test_correction() {
        // preference order: pythagorean comma, diesis, syntonic comma, octave
        let pdso: [usize; 4] = [2, 3, 1, 0];

        // preference order: pythagorean comma, syntonic comma, diesis octave
        let psdo: [usize; 4] = [2, 1, 3, 0];

        assert_eq!(
            Correction::<MockFiveLimitStackType>::new(&Stack::new_zero(), &pdso)
                .unwrap()
                .coeffs,
            arr1(&[0.into(), 0.into(), 0.into(), 0.into()])
        );

        assert_eq!(
            Correction::<MockFiveLimitStackType>::new(
                &Stack::from_target(vec![123, 234, 345]),
                &pdso
            )
            .unwrap()
            .coeffs,
            arr1(&[0.into(), 0.into(), 0.into(), 0.into()])
        );

        assert_eq!(
            Correction::<MockFiveLimitStackType>::new(
                &Stack::from_temperaments_and_target(&[true, false], vec![0, 0, 3]),
                &pdso
            )
            .unwrap()
            .coeffs,
            arr1(&[0.into(), 0.into(), 0.into(), 1.into()])
        );

        assert_eq!(
            Correction::<MockFiveLimitStackType>::new(
                &Stack::from_temperaments_and_target(&[true, false], vec![0, 1, 1]),
                &pdso
            )
            .unwrap()
            .coeffs,
            arr1(&[0.into(), 0.into(), Ratio::new(-1, 12), Ratio::new(1, 3)])
        );

        assert_eq!(
            Correction::<MockFiveLimitStackType>::new(
                &Stack::from_temperaments_and_target(&[false, true], vec![0, 1, 0]),
                &pdso
            )
            .unwrap()
            .coeffs,
            arr1(&[0.into(), Ratio::new(-1, 4), 0.into(), 0.into()])
        );

        assert_eq!(
            Correction::<MockFiveLimitStackType>::new(
                &Stack::from_temperaments_and_target(&[true, true], vec![0, 1, 0]),
                &psdo
            )
            .unwrap()
            .coeffs,
            arr1(&[0.into(), Ratio::new(-1, 4), Ratio::new(-1, 12), 0.into()])
        );

        assert_eq!(
            Correction::<MockFiveLimitStackType>::new(
                &Stack::from_target_and_actual(
                    arr1(&[0, 0, 0]),
                    arr1(&[Ratio::new(1, 120), 0.into(), 0.into()])
                ),
                &pdso
            )
            .unwrap()
            .coeffs,
            arr1(&[Ratio::new(1, 120), 0.into(), 0.into(), 0.into()]),
        );
    }
}
