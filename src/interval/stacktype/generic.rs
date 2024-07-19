use ndarray::*;

use crate::interval::{interval::*, stacktype::r#trait::*, temperament::*};

#[derive(Debug, PartialEq)]
pub struct GenericStackType {
    intervals: Vec<Interval>,
    temperaments: Vec<Temperament<StackCoeff>>,
    precomputed_temperings: Array2<Semitones>,
}

impl StackType for GenericStackType {
    fn intervals(&self) -> &[Interval] {
        &self.intervals
    }

    fn temperaments(&self) -> &[Temperament<StackCoeff>] {
        &self.temperaments
    }

    fn precomputed_temperings(&self) -> ArrayView2<Semitones> {
        self.precomputed_temperings.view()
    }
}

impl GenericStackType {
    /// Construct a [GenericStackType] from [Interval][Interval]s and
    /// [Temperament][Temperament]s. The [dimension][Temperament::dimension] of the temperaments
    /// must be the number of intervals.
    pub fn new(intervals: Vec<Interval>, temperaments: Vec<Temperament<StackCoeff>>) -> Self {
        let precomputed_temperings =
            Array2::from_shape_fn((intervals.len(), temperaments.len()), |(i, t)| {
                let mut whole_comma = 0.0;
                for (i, c) in temperaments[t].comma(i).iter().enumerate() {
                    whole_comma += (*c as Semitones) * intervals[i].semitones;
                }
                whole_comma / temperaments[t].denominator(i) as Semitones
            });

        GenericStackType {
            intervals,
            temperaments,
            precomputed_temperings,
        }
    }
}
