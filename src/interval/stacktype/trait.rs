use std::collections::HashMap;

use ndarray::{linalg::general_mat_vec_mul, Array1, Array2, ArrayView1, ArrayViewMut1};
use num_rational::Ratio;

use crate::{
    interval::{base::Interval, temperament::Temperament},
    util::lu::{lu_rational, LUErr},
};

/// The type of integer coefficients used in [Stack][crate::interval::stack::Stack]s.
pub type StackCoeff = i64;

pub trait IntervalBasis: Copy {
    fn intervals() -> &'static [Interval];

    /// Often, there's a "periodicity" in the intervals (like the octave). This function should
    /// return the index of that interval, if it exists.
    ///
    /// This interval doesn't have a logically special status, but knowing it may help in
    /// generating more user-friendly note names, animations etc.
    fn try_period_index() -> Option<usize>;

    /// Convenience: the length of the list returned by [intervals][IntervalBasis::intervals].
    fn num_intervals() -> usize {
        Self::intervals().len()
    }

    /// Convenience: At which position in the list of [IntervalBasis::intervals] is the interval with
    /// the given name?
    fn interval_positions() -> &'static HashMap<String, usize>;
}

pub struct CoordinateSystem {
    pub name: String,
    pub basis_names: Array1<String>,
    pub short_basis_names: Array1<char>,
    pub basis: Array2<Ratio<StackCoeff>>,
    pub basis_inv: Array2<Ratio<StackCoeff>>,
}

impl CoordinateSystem {
    pub fn new(
        name: String,
        basis_names: Array1<String>,
        short_basis_names: Array1<char>,
        basis_columnwise: Array2<Ratio<StackCoeff>>,
    ) -> Result<Self, LUErr> {
        let mut tmp = basis_columnwise.clone();
        let mut lu_perm = Array1::zeros(basis_columnwise.shape()[0]);
        let lu = lu_rational(tmp.view_mut(), lu_perm.view_mut())?;
        let basis_inv = lu.inverse()?;
        Ok(Self {
            name,
            basis_names,
            short_basis_names,
            basis: basis_columnwise,
            basis_inv,
        })
    }

    pub fn apply_inplace(
        &self,
        in_standard_basis: ArrayView1<Ratio<StackCoeff>>,
        mut in_new_basis: ArrayViewMut1<Ratio<StackCoeff>>,
    ) {
        general_mat_vec_mul(
            1.into(),
            &self.basis_inv,
            &in_standard_basis,
            0.into(),
            &mut in_new_basis,
        );
    }

    pub fn apply_inverse_inplace(
        &self,
        in_new_basis: ArrayView1<Ratio<StackCoeff>>,
        mut in_standard_basis: ArrayViewMut1<Ratio<StackCoeff>>,
    ) {
        general_mat_vec_mul(
            1.into(),
            &self.basis,
            &in_new_basis,
            0.into(),
            &mut in_standard_basis,
        );
    }
}

/// A description of the [Interval]s and [Temperament]s that may be used in a [Stack][crate::interval::stack::Stack]
pub trait StackType: IntervalBasis {
    /// The list of [Temperament]s that may be applied to intervals in a
    /// [Stack][crate::interval::stack::Stack] of this type. The "dimension" of the temperaments
    /// must be the [IntervalBasis::num_intervals].
    fn temperaments() -> &'static [Temperament<StackCoeff>];

    /// Convenience: the length of the list returned by [temperaments][StackType::temperaments].
    fn num_temperaments() -> usize {
        Self::temperaments().len()
    }

    /// A list of alternative Coordinate systems, to (say) write intervals in terms of commas.
    fn correction_systems() -> &'static [CoordinateSystem];

    /// Convenience: the length of the list returned by [StackType::correction_systems]
    fn n_correction_systems() -> usize {
        Self::correction_systems().len()
    }
}

pub trait FiveLimitIntervalBasis: IntervalBasis {
    fn octave_index() -> usize;
    fn fifth_index() -> usize;
    fn third_index() -> usize;
}

pub trait FiveLimitStackType: StackType + FiveLimitIntervalBasis {}

pub trait PeriodicIntervalBasis: IntervalBasis {
    fn period_index() -> usize {
        Self::try_period_index().unwrap()
    }

    fn period() -> &'static Interval {
        &Self::intervals()[Self::period_index()]
    }

    fn period_keys() -> u8 {
        Self::period().key_distance
    }
}

pub trait PeriodicStackType: StackType + PeriodicIntervalBasis {}

/// Marker trait for interval bases whose period is the octave. This means two things: the frequency
/// ratio is 2:1, and there are 12 notes in that space.
pub trait OctavePeriodicIntervalBasis: PeriodicIntervalBasis {}

pub trait OctavePeriodicStackType: StackType + OctavePeriodicIntervalBasis {}
