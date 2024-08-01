use std::sync::LazyLock;

use ndarray::{arr2, Array2, ArrayView2};

use crate::interval::{
    interval::{Interval, Semitones},
    stacktype::r#trait::{
        FiveLimitStackType, OctavePeriodicStackType, PeriodicStackType, StackCoeff, StackType,
    },
    temperament::Temperament,
};

#[derive(Debug, PartialEq, Clone, Copy)]
pub struct ConcreteFiveLimitStackType {}

static INTERVALS: LazyLock<[Interval; 3]> = LazyLock::new(|| {
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

static TEMPERAMENTS: LazyLock<[Temperament<StackCoeff>; 2]> = LazyLock::new(|| {
    [
        Temperament::new(
            String::from("12edo"),
            arr2(&[[0, 12, 0], [0, 0, 3], [1, 0, 0]]),
            arr2(&[[7, 0, 0], [1, 0, 0], [1, 0, 0]]).view(),
        )
        .unwrap(),
        Temperament::new(
            String::from("1/4-comma meantone"),
            arr2(&[[0, 4, 0], [1, 0, 0], [0, 0, 1]]),
            arr2(&[[2, 0, 1], [1, 0, 0], [0, 0, 1]]).view(),
        )
        .unwrap(),
    ]
});

static PRECOMPUTED_TEMPERINGS: LazyLock<Array2<Semitones>> = LazyLock::new(|| {
    Array2::from_shape_fn((3, TEMPERAMENTS.len()), |(i, t)| {
        let mut whole_comma = 0.0;
        for (i, &c) in TEMPERAMENTS[t].comma(i).iter().enumerate() {
            whole_comma += (c as Semitones) * INTERVALS[i].semitones;
        }
        whole_comma / TEMPERAMENTS[t].denominator(i) as Semitones
    })
});

impl StackType for ConcreteFiveLimitStackType {
    fn intervals() -> &'static [Interval] {
        &*INTERVALS
    }

    fn temperaments() -> &'static [Temperament<StackCoeff>] {
        &*TEMPERAMENTS
    }

    fn precomputed_temperings() -> ArrayView2<'static, Semitones> {
        PRECOMPUTED_TEMPERINGS.view()
    }
}

impl FiveLimitStackType for ConcreteFiveLimitStackType {
    fn octave_index() -> usize {
        0
    }

    fn fifth_index() -> usize {
        1
    }

    fn third_index() -> usize {
        2
    }
}

impl PeriodicStackType for ConcreteFiveLimitStackType {
    fn period_index() -> usize {
        0
    }
}

impl OctavePeriodicStackType for ConcreteFiveLimitStackType {}
