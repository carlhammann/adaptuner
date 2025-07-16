use std::{
    cell::{Cell, RefCell},
    fmt,
    marker::PhantomData,
};

use ndarray::{Array1, ArrayView1};
use num_rational::Ratio;
use num_traits::Zero;

use crate::interval::{
    stack::Stack,
    stacktype::r#trait::{StackCoeff, StackType},
};

#[derive(Debug)]
pub struct Correction<T: StackType> {
    _phantom: PhantomData<T>,
    system_index: Cell<usize>,
    coeffs: RefCell<Array1<Ratio<StackCoeff>>>,
    tmp_coeffs: RefCell<Array1<Ratio<StackCoeff>>>,
}

impl<T: StackType> Stack<T> {
    pub fn apply_correction(&mut self, correction: &Correction<T>) {
        self.make_pure();
        T::correction_systems()[correction.system_index()].apply_inverse_inplace(
            correction.coeffs.borrow().view(),
            correction.tmp_coeffs.borrow_mut().view_mut(),
        );
        self.actual
            .scaled_add(1.into(), &correction.tmp_coeffs.borrow());
    }
}

impl<T: StackType> Correction<T> {
    /// Use the `system_index` you'll later want to use for other operations. That will save
    /// computation.
    pub fn new_zero(system_index: usize) -> Self {
        Self {
            _phantom: PhantomData,
            system_index: Cell::new(system_index),
            coeffs: RefCell::new(Array1::zeros(T::num_intervals())),
            tmp_coeffs: RefCell::new(Array1::zeros(T::num_intervals())),
        }
    }

    pub fn reset_to_zero(&mut self) {
        self.coeffs
            .get_mut()
            .iter_mut()
            .for_each(|x| *x = Ratio::from_integer(0));
    }

    pub fn is_zero(&self) -> bool {
        self.coeffs.borrow().iter().all(|x| x.is_zero())
    }

    pub fn set_with(&mut self, stack: &Stack<T>, system_index: usize) {
        self.tmp_coeffs
            .borrow_mut()
            .indexed_iter_mut()
            .for_each(|(i, x)| *x = stack.actual[i] - Ratio::from_integer(stack.target[i]));
        T::correction_systems()[system_index].apply_inplace(
            self.tmp_coeffs.borrow().view(),
            self.coeffs.get_mut().view_mut(),
        );
    }

    /// Use the `system_index` you'll later want to use for other operations. That will save
    /// computation.
    pub fn new(stack: &Stack<T>, system_index: usize) -> Self {
        Self::from_target_and_actual((&stack.target).into(), (&stack.actual).into(), system_index)
    }

    /// Like [Self::new], only taking the [Stack::target] and [Stack::actual] as separate
    /// arguments.
    pub fn from_target_and_actual(
        target: ArrayView1<StackCoeff>,
        actual: ArrayView1<Ratio<StackCoeff>>,
        system_index: usize,
    ) -> Self {
        let mut tmp_coeffs = actual.to_owned();
        tmp_coeffs.zip_mut_with(&target, |l, r| {
            *l -= Ratio::from_integer(*r);
        });

        let mut coeffs = Array1::zeros(tmp_coeffs.len());
        T::correction_systems()[system_index].apply_inplace(tmp_coeffs.view(), coeffs.view_mut());
        Self {
            _phantom: PhantomData,
            system_index: Cell::new(system_index),
            coeffs: RefCell::new(coeffs),
            tmp_coeffs: RefCell::new(tmp_coeffs),
        }
    }

    pub fn mutate<F: FnMut(&mut Array1<Ratio<StackCoeff>>)>(
        &mut self,
        system_index: usize,
        mut f: F,
    ) {
        self.change_to_system_mutating(system_index);
        f(self.coeffs.get_mut());
    }

    pub fn system_index(&self) -> usize {
        self.system_index.get()
    }

    /// Will write the simplest form (i.e. using as few non-zero coefficients as possible), but if
    /// all forms are equally simple, the one specified by `self`s [Self::system_index] is used.
    pub fn str(&self) -> String {
        let mut res = String::new();
        self.fmt(&mut res).unwrap();
        res
    }

    pub fn change_to_system(self, system_index: usize) -> Self {
        self.change_to_system_mutating(system_index);
        self
    }

    /// Uses interior mutability to change representation.
    fn change_to_system_mutating(&self, system_index: usize) {
        if system_index != self.system_index.get() {
            T::correction_systems()[self.system_index.get()].apply_inverse_inplace(
                self.coeffs.borrow().view(),
                self.tmp_coeffs.borrow_mut().view_mut(),
            );
            T::correction_systems()[system_index].apply_inplace(
                self.tmp_coeffs.borrow().view(),
                self.coeffs.borrow_mut().view_mut(),
            );
            self.system_index.set(system_index);
        }
    }

    /// Uses interior mutability to change to the simplest representation. helper for [Self::fmt]
    fn simplest(&self) {
        let mut smallest_count = usize::MAX;
        let mut simplest_index = self.system_index.get();
        for _ in 0..T::n_correction_systems() {
            let count = self.coeffs.borrow().iter().filter(|c| !c.is_zero()).count();
            if count <= 1 {
                return;
            }
            if count < smallest_count {
                smallest_count = count;
                simplest_index = self.system_index.get();
            }
            self.change_to_system_mutating(
                (self.system_index.get() + 1) % T::n_correction_systems(),
            );
        }
        self.change_to_system_mutating(simplest_index);
    }

    pub fn fmt<W: fmt::Write>(&self, f: &mut W) -> fmt::Result {
        self.simplest();

        for (i, x) in self.coeffs.borrow().iter().enumerate() {
            let suffix = T::correction_systems()[self.system_index.get()].short_basis_names[i];
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
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::interval::stacktype::fivelimit::mock::MockFiveLimitStackType;
    use crate::interval::stacktype::fivelimit::*;
    use ndarray::arr1;

    #[test]
    fn test_correction() {
        assert_eq!(
            Correction::<MockFiveLimitStackType>::new(&Stack::new_zero(), PYTHAGOREAN_DIESIS).str(),
            ""
        );

        assert_eq!(
            Correction::<MockFiveLimitStackType>::new(
                &Stack::from_target(vec![123, 234, 345]),
                PYTHAGOREAN_DIESIS
            )
            .str(),
            ""
        );

        assert_eq!(
            Correction::<MockFiveLimitStackType>::new(
                &Stack::from_temperaments_and_target(&[true, false], vec![0, 0, 3]),
                PYTHAGOREAN_DIESIS
            )
            .str(),
            "+1d"
        );

        assert_eq!(
            Correction::<MockFiveLimitStackType>::new(
                &Stack::from_temperaments_and_target(&[true, false], vec![0, 1, 1]),
                PYTHAGOREAN_DIESIS
            )
            .str(),
            "-1/12p+1/3d"
        );

        assert_eq!(
            Correction::<MockFiveLimitStackType>::new(
                &Stack::from_temperaments_and_target(&[false, true], vec![0, 1, 0]),
                PYTHAGOREAN_DIESIS
            )
            .str(),
            // this can be written more simply, so the basis argument to [Correction::str] is ignored.
            "-1/4s"
        );

        assert_eq!(
            Correction::<MockFiveLimitStackType>::new(
                &Stack::from_temperaments_and_target(&[false, true], vec![0, 1, 0]),
                PYTHAGOREAN_DIESIS
            )
            .str(),
            "-1/4s"
        );

        assert_eq!(
            Correction::<MockFiveLimitStackType>::new(
                &Stack::from_temperaments_and_target(&[true, true], vec![0, 1, 0]),
                PYTHAGOREAN_SYNTONIC
            )
            .str(),
            "-1/12p-1/4s"
        );

        assert_eq!(
            Correction::<MockFiveLimitStackType>::new(
                &Stack::from_temperaments_and_target(&[true, true], vec![0, 1, 2]),
                PYTHAGOREAN_DIESIS
            )
            .change_to_system(DIESIS_SYNTONIC)
            .str(),
            // the fifth is corrected by one quarter syntonic comma plus a twelfth pythagorean comma down
            // the thirds are each corrected by one third of a diesis up
            //
            // together, that makes
            //
            // -1/4 s - 1/12 p + 2/3 d = 3/4 d - 1/2 s
            "+3/4d-1/2s"
        );

        assert_eq!(
            Correction::<MockFiveLimitStackType>::new(
                &Stack::from_target_and_actual(
                    arr1(&[0, 0, 0]),
                    arr1(&[Ratio::new(1, 120), 0.into(), 0.into()])
                ),
                PYTHAGOREAN_DIESIS
            )
            .str(),
            "+1/120o"
        );
    }
}
