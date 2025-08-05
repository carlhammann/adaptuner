use serde_derive::{Deserialize, Serialize};

/// invariant: the first entry of, `offsets`, `classes`, or `blocks` fields must always be 0.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, std::hash::Hash)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "kebab-case")]
pub enum KeyShape {
    #[serde(rename_all = "kebab-case")]
    ExactFixed {
        /// this is the only Vec<u8> field whose first entry may be nonzero
        keys: Vec<u8>,
    },
    #[serde(rename_all = "kebab-case")]
    ExactRelative { offsets: Vec<u8> },
    #[serde(rename_all = "kebab-case")]
    ClassesFixed { classes: Vec<u8>, zero: u8 },
    #[serde(rename_all = "kebab-case")]
    ClassesRelative { classes: Vec<u8> },
    #[serde(rename_all = "kebab-case")]
    BlockVoicingFixed { blocks: Vec<Vec<u8>>, zero: u8 },
    #[serde(rename_all = "kebab-case")]
    BlockVoicingRelative { blocks: Vec<Vec<u8>> },
}

#[derive(Debug, PartialEq)]
pub struct Fit {
    pub zero: u8,
    next: usize,
}

impl Fit {
    pub fn new_worst() -> Self {
        Self { zero: 0, next: 0 }
    }
    pub fn is_complete(&self) -> bool {
        self.next == 128
    }
    pub fn matches_nothing(&self) -> bool {
        self.next == 0
    }
    pub fn is_better_than(&self, other: &Self) -> bool {
        self.next > other.next
    }
}

pub trait HasActivationStatus {
    fn active(&self) -> bool;
}

fn classes_relative_to_lowest_sounding<N: HasActivationStatus>(
    keys: &[N; 128],
    lowest_sounding: usize,
) -> Vec<u8> {
    let mut active = [false; 12];
    for (i, k) in keys.iter().enumerate() {
        if k.active() {
            active[i % 12] = true;
        }
    }
    let mut classes = vec![];
    for (i, b) in active.iter().enumerate() {
        if *b {
            classes.push((i as isize - lowest_sounding as isize).rem_euclid(12) as u8);
        }
    }
    classes
}

impl KeyShape {
    /// Returns a [KeyShape::ClassesRelative] that fits the currently active notes.
    ///
    /// The `lowest_sounding` argument must be the index of the lowest sounding note in `keys`.
    ///
    /// It is ensured that `lowest_sounding % 12` is mapped to `0` in the returned
    /// [KeyShape::ClassesRelative::classes].
    pub fn classes_relative_from_current<N: HasActivationStatus>(
        keys: &[N; 128],
        lowest_sounding: usize,
    ) -> Self {
        Self::ClassesRelative {
            classes: classes_relative_to_lowest_sounding(keys, lowest_sounding),
        }
    }

    /// Returns a [KeyShape::ClassesFixed] that fits the currently active notes.
    ///
    /// The `lowest_sounding` argument must be the index of the lowest sounding note in `keys`.
    ///
    /// The returned [KeyShape::ClassesFixed::zero] will be `lowest_sounding % 12`
    pub fn classes_fixed_from_current<N: HasActivationStatus>(
        keys: &[N; 128],
        lowest_sounding: usize,
    ) -> Self {
        Self::ClassesFixed {
            classes: classes_relative_to_lowest_sounding(keys, lowest_sounding),
            zero: (lowest_sounding % 12) as u8,
        }
    }

    pub fn fit<N: HasActivationStatus>(&self, notes: &[N; 128]) -> Fit {
        match self {
            Self::ExactFixed { keys } => fit_exact_fixed(keys, notes),
            Self::ExactRelative { offsets } => fit_exact_relative(offsets, notes),
            _ => self.fit_from(notes, 0),
        }
    }

    fn fit_from<N: HasActivationStatus>(&self, notes: &[N; 128], start: usize) -> Fit {
        match self {
            Self::ClassesFixed { classes, zero } => fit_classes_fixed(classes, *zero, notes, start),
            Self::ClassesRelative { classes } => fit_classes_relative(classes, notes, start),
            Self::BlockVoicingFixed { blocks, zero } => {
                fit_voicing_fixed(blocks, *zero, notes, start)
            }
            Self::BlockVoicingRelative { blocks } => fit_voicing_relative(blocks, notes, start),
            Self::ExactFixed { .. } | Self::ExactRelative { .. } => unreachable!(),
        }
    }
}

/// Assumes that the `keys` argument is nonemtpy (it comes from [KeyShape::ExactFixed])
fn fit_exact_fixed<N: HasActivationStatus>(keys: &[u8], notes: &[N; 128]) -> Fit {
    let mut active_code: u128 = 0;
    for (i, n) in notes.iter().enumerate() {
        if n.active() {
            active_code |= 1 << i;
        }
    }

    if active_code == 0 {
        return Fit { zero: 0, next: 0 };
    }
    let lowest_sounding = (active_code & active_code.wrapping_neg()).ilog2();

    let mut pattern_code = 0;
    for k in keys {
        pattern_code |= 1 << k;
    }

    let diff = active_code ^ pattern_code;
    if diff == 0 {
        return Fit {
            zero: lowest_sounding as u8,
            next: 128,
        };
    }

    let lowest_different = diff & diff.wrapping_neg();
    if lowest_different > pattern_code {
        // by assumption pattern_code>0, and hence the ilog2 won't panic
        let highest_set_in_pattern = pattern_code.ilog2();
        Fit {
            zero: lowest_sounding as u8,
            next: 1 + highest_set_in_pattern as usize,
        }
    } else {
        Fit { zero: 0, next: 0 }
    }
}

/// Assumes that the `offsets` argument is nonemtpy (it comes from [KeyShape::ExactRelative])
fn fit_exact_relative<N: HasActivationStatus>(offsets: &[u8], notes: &[N; 128]) -> Fit {
    let mut active_code: u128 = 0;
    for (i, n) in notes.iter().enumerate() {
        if n.active() {
            active_code |= 1 << i;
        }
    }

    if active_code == 0 {
        return Fit { zero: 0, next: 0 };
    }
    let lowest_sounding = (active_code & active_code.wrapping_neg()).ilog2();
    active_code >>= lowest_sounding;

    let mut pattern_code = 0;
    for k in offsets {
        pattern_code |= 1 << (k - offsets[0]);
    }

    let diff = active_code ^ pattern_code;
    if diff == 0 {
        return Fit {
            zero: lowest_sounding as u8,
            next: 128,
        };
    }

    let lowest_different = diff & diff.wrapping_neg();
    if lowest_different > pattern_code {
        let highest_set_in_pattern = pattern_code.ilog2();
        Fit {
            zero: lowest_sounding as u8,
            next: 1 + lowest_sounding as usize + highest_set_in_pattern as usize,
        }
    } else {
        Fit { zero: 0, next: 0 }
    }
}

fn fit_classes_fixed<N: HasActivationStatus>(
    classes: &[u8],
    zero: u8,
    notes: &[N; 128],
    start: usize,
) -> Fit {
    let mut matched_zero = u8::MAX;
    let mut used = vec![false; classes.len()];
    let mut i = start;
    while i < 128 {
        if !notes[i].active() {
            i += 1;
            continue;
        }
        if i as u8 % 12 == zero % 12 {
            matched_zero = matched_zero.min(i as u8);
        }
        match classes
            .iter()
            .position(|&x| (x + zero) % 12 == i as u8 % 12)
        {
            Some(j) => {
                i += 1;
                used[j] = true
            }
            None {} => break,
        }
    }
    if used.iter().any(|&u| !u) {
        Fit { zero, next: start }
    } else {
        Fit {
            zero: matched_zero,
            next: i,
        }
    }
}

fn fit_classes_relative<N: HasActivationStatus>(
    classes: &[u8],
    notes: &[N; 128],
    start: usize,
) -> Fit {
    let period_keys = 12;
    for zero in 0..period_keys {
        let res = fit_classes_fixed(classes, zero, notes, start);
        match res {
            Fit { next, .. } => {
                if next > start {
                    return res;
                }
            }
        }
    }
    Fit { zero: 0, next: 0 }
}

fn fit_voicing_fixed<N: HasActivationStatus>(
    blocks: &[Vec<u8>],
    zero: u8,
    notes: &[N; 128],
    start: usize,
) -> Fit {
    let mut matched_zero = u8::MAX;
    let mut next = start;
    let mut i = 0;
    while i < blocks.len() {
        match fit_classes_fixed(&blocks[i], zero, notes, next) {
            Fit {
                next: new_next,
                zero: new_matched_zero,
            } => {
                matched_zero = matched_zero.min(new_matched_zero);
                if new_next > next {
                    next = new_next;
                    i += 1;
                } else {
                    break;
                }
            }
        }
    }
    if i == blocks.len() {
        Fit {
            zero: matched_zero,
            next,
        }
    } else {
        Fit { zero, next: start }
    }
}

fn fit_voicing_relative<N: HasActivationStatus>(
    blocks: &[Vec<u8>],
    notes: &[N; 128],
    start: usize,
) -> Fit {
    for zero in 0..12 {
        let res = fit_voicing_fixed(blocks, zero, notes, start);
        match res {
            Fit { next, .. } => {
                if next > start {
                    return res;
                }
            }
        }
    }
    Fit { zero: 0, next: 0 }
}

#[cfg(test)]
mod test {
    use super::*;

    impl HasActivationStatus for bool {
        fn active(&self) -> bool {
            *self
        }
    }

    fn one_case(active: &[u8], pat: KeyShape, expect: Fit) {
        let mut active_notes = [false; 128];
        for i in active {
            active_notes[*i as usize] = true;
        }
        let actual = pat.fit(&active_notes);
        assert!(actual == expect, "for\npattern: {pat:?}\nactive: {active:?}\n\nexpected: {expect:?}\n     got: {actual:?}");
    }

    fn one_classes_fixed(active: &[u8], classes: Vec<u8>, zero: u8, reference: u8, next: usize) {
        one_case(
            active,
            KeyShape::ClassesFixed { classes, zero },
            Fit {
                zero: reference,
                next,
            },
        );
    }

    #[test]
    fn test_classes_fixed() {
        let examples = [
            (vec![0], vec![0], 0, 0, 128),
            (vec![0, 1], vec![0], 0, 0, 1),
            (vec![1], vec![0], 1, 1, 128),
            (vec![1], vec![0], 0, 0, 0),
            (vec![0], vec![0], 1, 1, 0),
            (vec![0, 5], vec![0, 5], 0, 0, 128),
            (vec![0, 4], vec![0, 5], 0, 0, 0),
            (vec![0, 5], vec![0, 4], 0, 0, 0),
            (vec![1, 5], vec![0, 4], 1, 1, 128),
            (vec![0, 5, 6], vec![0, 5], 0, 0, 6),
            // the order doesn't matter, as long as the "matching" keys come first:
            (vec![8, 3, 11], vec![0, 5], 3, 3, 11),
            (vec![8, 3, 4], vec![0, 5], 3, 3, 0),
            // permutations (active notes)
            (vec![0, 1, 2], vec![0, 1, 2], 0, 0, 128),
            (vec![0, 2, 1], vec![0, 1, 2], 0, 0, 128),
            (vec![1, 0, 2], vec![0, 1, 2], 0, 0, 128),
            (vec![1, 2, 0], vec![0, 1, 2], 0, 0, 128),
            (vec![2, 0, 1], vec![0, 1, 2], 0, 0, 128),
            (vec![2, 1, 0], vec![0, 1, 2], 0, 0, 128),
            // permutations (pattern)
            (vec![0, 1, 2], vec![0, 1, 2], 0, 0, 128),
            (vec![0, 1, 2], vec![0, 2, 1], 0, 0, 128),
            // longer than one octave
            (vec![0, 13], vec![0, 1], 0, 0, 128),
            (vec![20, 7], vec![0, 1], 7, 7, 128),
        ];

        for (active, classes, zero, reference, next) in examples {
            one_classes_fixed(&active, classes, zero, reference, next);
        }
    }

    fn one_classes_relative(active: &[u8], classes: Vec<u8>, reference: u8, next: usize) {
        one_case(
            active,
            KeyShape::ClassesRelative { classes },
            Fit {
                zero: reference,
                next,
            },
        );
    }

    #[test]
    fn test_classes_relative() {
        let examples = [
            (vec![0], vec![0], 0, 128),
            (vec![1], vec![0], 1, 128),
            (vec![1, 5], vec![0, 4], 1, 128),
            (vec![0, 5, 6], vec![0, 5], 0, 6),
            // the order doesn't matter, as long as the "matching" keys come first:
            (vec![8, 3, 11], vec![0, 5], 3, 11),
            (vec![8, 3, 4], vec![0, 5], 0, 0),
            // big major chord with octave doublings
            (vec![1, 13, 18, 22, 34], vec![0, 4, 7], 18, 128),
            // a few illustrative examples: inversions of a major chord
            (vec![60, 64, 67], vec![0, 4, 7], 60, 128),
            (vec![60, 64, 67], vec![0, 3, 8], 64, 128),
            (vec![60, 64, 67], vec![0, 5, 9], 67, 128),
        ];

        for (active, classes, reference, next) in examples {
            one_classes_relative(&active, classes, reference, next);
        }
    }

    fn one_voicing_fixed(
        active: &[u8],
        blocks: Vec<Vec<u8>>,
        zero: u8,
        reference: u8,
        next: usize,
    ) {
        one_case(
            active,
            KeyShape::BlockVoicingFixed { blocks, zero },
            Fit {
                zero: reference,
                next,
            },
        );
    }

    #[test]
    fn test_voicing_fixed() {
        let examples = [
            (vec![1, 2, 3, 4], vec![vec![0, 1], vec![3, 2]], 1, 1, 128),
            (vec![3, 4, 5, 6], vec![vec![0], vec![2, 1]], 3, 3, 6),
            (vec![0, 1, 2], vec![vec![0, 2], vec![1]], 0, 0, 0),
            (vec![25, 26, 27], vec![vec![0, 1], vec![2]], 1, 25, 128),
            (vec![25, 26, 27], vec![vec![0, 1], vec![2]], 0, 0, 0),
        ];

        for (active, blocks, zero, reference, next) in examples {
            one_voicing_fixed(&active, blocks, zero, reference, next);
        }
    }

    fn one_voicing_relative(active: &[u8], blocks: Vec<Vec<u8>>, reference: u8, next: usize) {
        one_case(
            active,
            KeyShape::BlockVoicingRelative { blocks },
            Fit {
                zero: reference,
                next,
            },
        );
    }

    #[test]
    fn test_voicing_relative() {
        let examples = [
            (vec![4, 5, 6, 7], vec![vec![0, 1], vec![3, 2]], 4, 128),
            (vec![0, 1, 2, 3], vec![vec![0], vec![2, 1]], 0, 3),
            (vec![1, 2, 3], vec![vec![0, 2], vec![1]], 0, 0),
            (
                vec![25 + 1, 25 + 2, 25 + 3],
                vec![vec![0, 1], vec![2]],
                25 + 1,
                128,
            ),
        ];

        for (active, blocks, reference, next) in examples {
            one_voicing_relative(&active, blocks, reference, next);
        }
    }

    fn one_exact_fixed(active: &[u8], keys: Vec<u8>, zero: u8, next: usize) {
        one_case(active, KeyShape::ExactFixed { keys }, Fit { zero, next });
    }

    #[test]
    fn test_exact_fixed() {
        let examples = [
            (vec![0], vec![0], 0, 128),
            (vec![1], vec![0], 0, 0),
            (vec![0], vec![1], 0, 0),
            (vec![0, 1], vec![0], 0, 1),
            (vec![0, 2, 3], vec![0, 2], 0, 3),
            (vec![10, 32, 45], vec![10, 32, 45], 10, 128),
            (vec![10, 32, 45], vec![11, 32, 45], 0, 0),
            (vec![10, 32], vec![10, 32, 45], 0, 0),
            (vec![10, 32, 45], vec![10, 32], 10, 33),
        ];

        for (active, keys, zero, next) in examples {
            one_exact_fixed(&active, keys, zero, next);
        }
    }

    fn one_exact_relative(active: &[u8], offsets: Vec<u8>, zero: u8, next: usize) {
        one_case(
            active,
            KeyShape::ExactRelative { offsets },
            Fit { zero, next },
        );
    }

    #[test]
    fn test_exact_relative() {
        let examples = [
            (vec![0], vec![0], 0, 128),
            (vec![1], vec![0], 1, 128),
            (vec![0, 1], vec![0], 0, 1),
            (vec![0, 2, 3], vec![0, 2], 0, 3),
            (vec![10, 32, 45], vec![0, 22, 35], 10, 128),
            (vec![10, 32, 45], vec![1, 22, 35], 0, 0),
            (vec![10, 32], vec![0, 22, 35], 0, 0),
            (vec![10, 32, 45], vec![0, 22], 10, 33),
            (vec![20, 42, 55], vec![0, 22], 20, 43),
        ];

        for (active, offsets, zero, next) in examples {
            one_exact_relative(&active, offsets, zero, next);
        }
    }
}
