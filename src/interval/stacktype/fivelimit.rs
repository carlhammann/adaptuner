use std::sync::LazyLock;

use ndarray::arr2;
use serde_derive::{Deserialize, Serialize};

use crate::interval::{
    base::{Interval, Semitones},
    fundamental::HasFundamental,
    stack::Stack,
    stacktype::r#trait::{
        FiveLimitStackType, OctavePeriodicStackType, PeriodicStackType, StackCoeff, StackType,
    },
    temperament::Temperament,
};

#[derive(Debug, Hash, Eq, PartialEq, Clone, Copy, Serialize, Deserialize)]
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
            String::from("equal temperament"),
            arr2(&[[0, 12, 0], [0, 0, 3], [1, 0, 0]]).view(),
            arr2(&[[7, 0, 0], [1, 0, 0], [1, 0, 0]]).view(),
        )
        .unwrap(),
        Temperament::new(
            String::from("1/4-comma meantone fifths"),
            arr2(&[[0, 4, 0], [1, 0, 0], [0, 0, 1]]).view(),
            arr2(&[[2, 0, 1], [1, 0, 0], [0, 0, 1]]).view(),
        )
        .unwrap(),
    ]
});

impl StackType for ConcreteFiveLimitStackType {
    fn intervals() -> &'static [Interval] {
        &*INTERVALS
    }

    fn temperaments() -> &'static [Temperament<StackCoeff>] {
        &*TEMPERAMENTS
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

impl HasFundamental for ConcreteFiveLimitStackType {
    fn fundamental_inplace(a: &Stack<Self>, b: &mut Stack<Self>) {
        let mut exponents = [0, 0, 0];

        exponents[0] += a.target[Self::octave_index()];
        exponents[1] += a.target[Self::fifth_index()];
        exponents[0] -= a.target[Self::fifth_index()];
        exponents[2] += a.target[Self::third_index()];
        exponents[0] -= a.target[Self::third_index()] * 2;

        exponents[0] -= b.target[Self::octave_index()];
        exponents[1] -= b.target[Self::fifth_index()];
        exponents[0] += b.target[Self::fifth_index()];
        exponents[2] -= b.target[Self::third_index()];
        exponents[0] += b.target[Self::third_index()] * 2;

        for n in exponents.iter_mut() {
            if *n > 0 {
                *n = 0;
            }
        }

        exponents[0] += exponents[1];
        exponents[0] += exponents[2] * 2;

        b.increment_at_index_pure(Self::octave_index(), exponents[0]);
        b.increment_at_index_pure(Self::fifth_index(), exponents[1]);
        b.increment_at_index_pure(Self::third_index(), exponents[2]);
    }
}

#[cfg(test)]
mod test {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn test_target_fundamental() {
        assert_eq!(
            <ConcreteFiveLimitStackType as HasFundamental>::fundamental(
                &Stack::from_target(vec![0, 0, 0]),
                &Stack::from_target(vec![0, 0, 0])
            ),
            Stack::from_target(vec![0, 0, 0])
        );

        assert_eq!(
            <ConcreteFiveLimitStackType as HasFundamental>::fundamental(
                &Stack::from_target(vec![0, 0, 0]),
                &Stack::from_target(vec![1, 0, 0])
            ),
            Stack::from_target(vec![0, 0, 0])
        );

        assert_eq!(
            <ConcreteFiveLimitStackType as HasFundamental>::fundamental(
                &Stack::from_target(vec![1, 0, 0]),
                &Stack::from_target(vec![0, 0, 0])
            ),
            Stack::from_target(vec![0, 0, 0])
        );

        assert_eq!(
            <ConcreteFiveLimitStackType as HasFundamental>::fundamental(
                &Stack::from_target(vec![0, 0, 0]),
                &Stack::from_target(vec![1, 1, 0])
            ),
            Stack::from_target(vec![0, 0, 0])
        );

        assert_eq!(
            <ConcreteFiveLimitStackType as HasFundamental>::fundamental(
                &Stack::from_target(vec![0, 0, 0]),
                &Stack::from_target(vec![2, 0, 1])
            ),
            Stack::from_target(vec![0, 0, 0])
        );

        assert_eq!(
            <ConcreteFiveLimitStackType as HasFundamental>::fundamental(
                &Stack::from_target(vec![0, 0, 0]),
                &Stack::from_target(vec![0, 0, 1])
            ),
            Stack::from_target(vec![-2, 0, 0])
        );

        assert_eq!(
            <ConcreteFiveLimitStackType as HasFundamental>::fundamental(
                &Stack::from_target(vec![0, 1, 0]),
                &Stack::from_target(vec![0, 0, 1])
            ),
            Stack::from_target(vec![-2, 0, 0])
        );

        assert_eq!(
            <ConcreteFiveLimitStackType as HasFundamental>::fundamental(
                &Stack::from_target(vec![0, 0, 0]),
                &Stack::from_target(vec![-1, 2, 0])
            ),
            Stack::from_target(vec![-3, 0, 0])
        );

        assert_eq!(
            <ConcreteFiveLimitStackType as HasFundamental>::fundamental(
                &Stack::from_target(vec![0, -1, 0]),
                &Stack::from_target(vec![0, 0, 1])
            ),
            Stack::from_target(vec![-3, -1, 0])
        );

        assert_eq!(
            <ConcreteFiveLimitStackType as HasFundamental>::fundamental(
                &Stack::from_target(vec![0, -1, 0]),
                &Stack::from_target(vec![1, -1, 0])
            ),
            Stack::from_target(vec![0, -1, 0])
        );
    }
}
