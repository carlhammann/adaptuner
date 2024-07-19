use std::sync::LazyLock;

use ndarray::{Array2, ArrayView2};

use crate::interval::{
    interval::{Interval, Semitones},
    stacktype::r#trait::{FiveLimitStackType, StackCoeff, StackType},
    temperament::Temperament,
};

#[derive(Debug, PartialEq)]
pub struct ConcreteFiveLimitStackType {
    temperaments: Vec<Temperament<StackCoeff>>,
    precomputed_temperings: Array2<Semitones>,
}

pub static FIVELIMIT_INTERVALS: LazyLock<[Interval; 3]> = LazyLock::new(|| {
    [
        Interval {
            name: "octave".into(),
            semitones: 12.0,
            key_distance: 12,
        },
        Interval {
            name: "fifth".into(),
            semitones: 12.0 * (3.0 / 2.0 as Semitones).log2(),
            key_distance: 7,
        },
        Interval {
            name: "third".into(),
            semitones: 12.0 * (5.0 / 4.0 as Semitones).log2(),
            key_distance: 4,
        },
    ]
});

impl StackType for ConcreteFiveLimitStackType {
    fn intervals(&self) -> &[Interval] {
        &*FIVELIMIT_INTERVALS
    }

    fn temperaments(&self) -> &[Temperament<StackCoeff>] {
        &self.temperaments
    }

    fn precomputed_temperings(&self) -> ArrayView2<Semitones> {
        self.precomputed_temperings.view()
    }
}

impl FiveLimitStackType for ConcreteFiveLimitStackType {
    fn octave_index(&self) -> usize {
        0
    }

    fn fifth_index(&self) -> usize {
        1
    }

    fn third_index(&self) -> usize {
        2
    }
}

impl ConcreteFiveLimitStackType {
    /// Construct a [FiveLimitStackType] from [Temperament][Temperament]s. The
    /// [dimension][Temperament::dimension] of the temperaments must be three, and the intervals
    /// are ordered as in [FIVELIMIT_INTERVALS].
    pub fn new(temperaments: Vec<Temperament<StackCoeff>>) -> Self {
        let precomputed_temperings = Array2::from_shape_fn((3, temperaments.len()), |(i, t)| {
            let mut whole_comma = 0.0;
            for (i, c) in temperaments[t].comma(i).iter().enumerate() {
                whole_comma += (*c as Semitones) * FIVELIMIT_INTERVALS[i].semitones;
            }
            whole_comma / temperaments[t].denominator(i) as Semitones
        });

        ConcreteFiveLimitStackType {
            temperaments,
            precomputed_temperings,
        }
    }
}
