use serde_derive::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, std::hash::Hash)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "kebab-case")]
pub enum KeyShape {
    #[serde(rename_all = "kebab-case")]
    ExactFixed { keys: Vec<u8> },
    #[serde(rename_all = "kebab-case")]
    ExactRelative {
        /// first entry must be zero
        offsets: Vec<u8>,
    },
    #[serde(rename_all = "kebab-case")]
    ClassesFixed { classes: Vec<u8> },
    #[serde(rename_all = "kebab-case")]
    ClassesRelative {
        /// first entry must be zero
        classes: Vec<u8>,
    },
    #[serde(rename_all = "kebab-case")]
    BlockVoicingFixed { blocks: Vec<Vec<u8>> },
    #[serde(rename_all = "kebab-case")]
    BlockVoicingRelative {
        /// first entry of first entry must be zero
        blocks: Vec<Vec<u8>>,
    },
}

#[derive(Debug, PartialEq)]
pub enum Fit {
    Failed,
    Partial { reference: u8, next: usize },
    Complete { reference: u8 },
}

impl Fit {
    pub fn is_complete(&self) -> bool {
        match self {
            Fit::Complete { .. } => true,
            _ => false,
        }
    }

    pub fn matches_nothing(&self) -> bool {
        match self {
            Fit::Failed => true,
            _ => false,
        }
    }

    pub fn reference(&self) -> u8 {
        match self {
            Fit::Failed => 0,
            Fit::Partial { reference, .. } => *reference,
            Fit::Complete { reference } => *reference,
        }
    }

    fn is_better_than(&self, other: &Self) -> bool {
        match (self, other) {
            (Fit::Failed, _) => false,
            (_, Fit::Failed) => true,
            (_, Fit::Complete { .. }) => false,
            (Fit::Complete { .. }, _) => true,
            (Fit::Partial { next: a, .. }, Fit::Partial { next: b, .. }) => a > b,
        }
    }
}

pub trait HasActivationStatus {
    fn active(&self) -> bool;
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
            classes: {
                let mut active = [false; 12];
                for (i, k) in keys.iter().enumerate() {
                    if k.active() {
                        active[((i as isize - lowest_sounding as isize) % 12) as usize] = true;
                    }
                }
                let mut classes = vec![];
                for (i, b) in active.iter().enumerate() {
                    if *b {
                        classes.push((i as isize - lowest_sounding as isize).rem_euclid(12) as u8);
                    }
                }
                classes
            },
        }
    }

    /// Returns a [KeyShape::ClassesFixed] that fits the currently active notes.
    pub fn classes_fixed_from_current<N: HasActivationStatus>(keys: &[N; 128]) -> Self {
        Self::ClassesFixed {
            classes: {
                let mut active = [false; 12];
                for (i, k) in keys.iter().enumerate() {
                    if k.active() {
                        active[i % 12] = true;
                    }
                }

                let mut classes = vec![];
                for (i, b) in active.iter().enumerate() {
                    if *b {
                        classes.push(i.rem_euclid(12) as u8);
                    }
                }
                classes
            },
        }
    }

    /// Only use this on an active_code that you know is nonzero
    fn fit_code(&self, active_code: u128) -> Fit {
        match self {
            Self::ClassesFixed { classes } => fit_classes_fixed(classes, 0, active_code),
            Self::ClassesRelative { classes } => fit_classes_relative(classes, active_code),
            Self::BlockVoicingFixed { blocks } => fit_block_voicing_fixed(blocks, 0, active_code),
            Self::BlockVoicingRelative { blocks } => {
                fit_block_voicing_relative(blocks, active_code)
            }
            Self::ExactFixed { keys } => fit_exact_fixed(keys, active_code),
            Self::ExactRelative { offsets } => fit_exact_relative(offsets, active_code),
        }
    }
}


/// returns the index of either the first [Fit::Complete] fit or the best [Fit::Partial] fit.
pub fn first_complete_fit_or_best<'a, N: HasActivationStatus>(
    notes: &[N; 128],
    shapes: impl Iterator<Item = &'a KeyShape>,
) -> (usize, Fit) {
    let mut active_code: u128 = 0;
    for (i, n) in notes.iter().enumerate() {
        if n.active() {
            active_code |= 1 << i;
        }
    }
    if active_code == 0 {
        return (0, Fit::Failed);
    }
    let mut best = (0, Fit::Failed);
    for (i, shape) in shapes.enumerate() {
        let new = shape.fit_code(active_code);
        if new.is_complete() {
            return (i, new);
        }
        if new.is_better_than(&best.1) {
            best = (i, new);
        }
    }
    best
}

/// Assumes that the `keys` argument is nonemtpy (it comes from [KeyShape::ExactFixed])
fn fit_exact_fixed(keys: &[u8], active: u128) -> Fit {
    let lowest_sounding = (active & active.wrapping_neg()).ilog2();

    let mut pattern = 0;
    for k in keys {
        pattern |= 1 << k;
    }

    let diff = active ^ pattern;
    if diff == 0 {
        return Fit::Complete {
            reference: lowest_sounding as u8,
        };
    }

    let lowest_different = diff & diff.wrapping_neg();
    if lowest_different > pattern {
        // by assumption pattern_code>0, and hence the ilog2 won't panic
        let highest_set_in_pattern = pattern.ilog2();
        Fit::Partial {
            reference: lowest_sounding as u8,
            next: 1 + highest_set_in_pattern as usize,
        }
    } else {
        Fit::Failed
    }
}

/// Assumes that the `offsets` argument is nonemtpy (it comes from [KeyShape::ExactRelative])
fn fit_exact_relative(offsets: &[u8], mut active: u128) -> Fit {
    let lowest_sounding = (active & active.wrapping_neg()).ilog2();
    active >>= lowest_sounding;

    let mut pattern = 0;
    for k in offsets {
        pattern |= 1 << (k - offsets[0]);
    }

    let diff = active ^ pattern;
    if diff == 0 {
        return Fit::Complete {
            reference: lowest_sounding as u8,
        };
    }

    let lowest_different = diff & diff.wrapping_neg();
    if lowest_different > pattern {
        let highest_set_in_pattern = pattern.ilog2();
        Fit::Partial {
            reference: lowest_sounding as u8,
            next: 1 + lowest_sounding as usize + highest_set_in_pattern as usize,
        }
    } else {
        Fit::Failed
    }
}

/// `first_class` must be in the range 0..12
///
/// The `first_class`-th bit of pattern must be set, and all other bits at distances a multiple of
/// 12 from it as well.
fn fit_classes_bittwiddling(pattern: u128, mut first_class: u8, active: u128) -> Fit {
    let lowest_match_of_first_class = {
        let mut first_class_pattern = 0;
        while first_class < 128 {
            first_class_pattern |= 1 << first_class;
            first_class += 12;
        }
        let matches = active & first_class_pattern;
        if matches == 0 {
            return Fit::Failed;
        }
        (matches & matches.wrapping_neg()).ilog2() as u8
    };

    let matches = active & pattern;
    // we know that matches != 0, because pattern contains at least all the ones in
    // first_class_pattern

    let success_if_all_were_matched = |mut windowed_matches: u128, next: usize| {
        let mut check_all_present = 0;
        while windowed_matches != 0 {
            check_all_present |= windowed_matches;
            windowed_matches >>= 12;
        }
        check_all_present &= (1 << 12) - 1;
        if check_all_present == pattern & ((1 << 12) - 1) {
            if next >= 128 {
                Fit::Complete {
                    reference: lowest_match_of_first_class,
                }
            } else {
                Fit::Partial {
                    reference: lowest_match_of_first_class,
                    next,
                }
            }
        } else {
            Fit::Failed
        }
    };

    let nonmatches = active & !pattern;
    if nonmatches == 0 {
        return success_if_all_were_matched(matches, 128);
    }
    let lowest_nonmatch = (nonmatches & nonmatches.wrapping_neg()).ilog2();

    success_if_all_were_matched(
        matches & ((1 << lowest_nonmatch) - 1),
        lowest_nonmatch as usize,
    )
}

fn fit_classes_fixed(classes: &[u8], offset: u8, active: u128) -> Fit {
    let mut octave_pattern = 0;
    for c in classes {
        octave_pattern |= 1 << ((c + offset) % 12);
    }

    let mut pattern = 0;
    let mut tmp = octave_pattern;
    while tmp != 0 {
        pattern |= tmp;
        tmp <<= 12;
    }

    fit_classes_bittwiddling(pattern, (classes[0] + offset) % 12, active)
}

fn fit_classes_relative(classes: &[u8], active: u128) -> Fit {
    let mut best = Fit::Failed;
    for offset in 0..12 {
        let new = fit_classes_fixed(classes, offset, active);
        if new.is_complete() {
            return new;
        }
        if new.is_better_than(&best) {
            best = new;
        }
    }
    best
}

/// offset must be in the range 0..12
fn fit_block_voicing_fixed(blocks: &[Vec<u8>], offset: u8, active: u128) -> Fit {
    let mut fit = Fit::Failed;
    let mut octave_pattern = 0;
    let mut pattern = 0;
    for (i, block) in blocks.iter().enumerate() {
        for c in block {
            octave_pattern |= 1 << ((c + offset) % 12);
        }
        let mut tmp = octave_pattern;
        while tmp != 0 {
            pattern |= tmp;
            tmp <<= 12;
        }
        let new = fit_classes_bittwiddling(pattern, (blocks[0][0] + offset) % 12, active);
        if new.is_complete() {
            if i == blocks.len() - 1 {
                return new;
            } else {
                return Fit::Failed;
            }
        }
        if new.is_better_than(&fit) {
            fit = new;
        } else {
            return Fit::Failed;
        }
    }

    fit
}

fn fit_block_voicing_relative(blocks: &[Vec<u8>], active: u128) -> Fit {
    let mut best = Fit::Failed;
    for offset in 0..12 {
        let new = fit_block_voicing_fixed(blocks, offset, active);
        if new.is_complete() {
            return new;
        }
        if new.is_better_than(&best) {
            best = new;
        }
    }
    best
}

#[cfg(test)]
mod test {
    use super::*;

    impl KeyShape {
        fn fit<N: HasActivationStatus>(&self, notes: &[N; 128]) -> Fit {
            let mut active_code: u128 = 0;
            for (i, n) in notes.iter().enumerate() {
                if n.active() {
                    active_code |= 1 << i;
                }
            }
            if active_code == 0 {
                return Fit::Failed;
            }
            self.fit_code(active_code)
        }
    }

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
        assert!(
            actual == expect,
            "pattern: {pat:?}\n\
            active: {active:?}\n\
            expected: {expect:?}\n\
            got: {actual:?}"
        );
    }

    fn one_classes_fixed(active: &[u8], classes: Vec<u8>, expect: Fit) {
        one_case(active, KeyShape::ClassesFixed { classes }, expect);
    }

    #[test]
    fn test_classes_fixed() {
        let examples = [
            (vec![0], vec![0], Fit::Complete { reference: 0 }),
            (vec![12], vec![0], Fit::Complete { reference: 12 }),
            (
                vec![0, 1],
                vec![0],
                Fit::Partial {
                    reference: 0,
                    next: 1,
                },
            ),
            (vec![1], vec![0], Fit::Failed),
            (vec![0], vec![1], Fit::Failed),
            (vec![0, 5], vec![0, 5], Fit::Complete { reference: 0 }),
            (vec![24, 29], vec![0, 5], Fit::Complete { reference: 24 }),
            (vec![30, 35], vec![6, 11], Fit::Complete { reference: 30 }),
            (vec![0, 4], vec![0, 5], Fit::Failed),
            (vec![0, 5], vec![0, 4], Fit::Failed),
            (vec![1, 5], vec![0, 4], Fit::Failed),
            (
                vec![0, 5, 6],
                vec![0, 5],
                Fit::Partial {
                    reference: 0,
                    next: 6,
                },
            ),
            // testing octave doublings
            (vec![96], vec![0], Fit::Complete { reference: 96 }),
            (vec![0, 12, 24, 96], vec![0], Fit::Complete { reference: 0 }),
            (vec![12, 24, 96], vec![0], Fit::Complete { reference: 12 }),
            (vec![12, 24, 96], vec![0, 4], Fit::Failed),
            (
                vec![16, 24, 100],
                vec![0, 4],
                Fit::Complete { reference: 24 },
            ),
            // permutations (active notes)
            (vec![0, 1, 2], vec![0, 1, 2], Fit::Complete { reference: 0 }),
            (vec![0, 2, 1], vec![0, 1, 2], Fit::Complete { reference: 0 }),
            (vec![1, 0, 2], vec![0, 1, 2], Fit::Complete { reference: 0 }),
            (vec![1, 2, 0], vec![0, 1, 2], Fit::Complete { reference: 0 }),
            (vec![2, 0, 1], vec![0, 1, 2], Fit::Complete { reference: 0 }),
            (vec![2, 1, 0], vec![0, 1, 2], Fit::Complete { reference: 0 }),
            // permutations (pattern)
            (vec![0, 1, 2], vec![0, 1, 2], Fit::Complete { reference: 0 }),
            (vec![0, 1, 2], vec![0, 2, 1], Fit::Complete { reference: 0 }),
            (vec![0, 1, 2], vec![1, 0, 2], Fit::Complete { reference: 1 }),
            (vec![0, 1, 2], vec![1, 2, 0], Fit::Complete { reference: 1 }),
            (vec![0, 1, 2], vec![2, 0, 1], Fit::Complete { reference: 2 }),
            (vec![0, 1, 2], vec![2, 1, 0], Fit::Complete { reference: 2 }),
            // longer than one octave
            (vec![0, 13], vec![0, 1], Fit::Complete { reference: 0 }),
            (vec![20, 7], vec![7, 8], Fit::Complete { reference: 7 }),
            // getting a reference that is not the lowest note
            (
                vec![64, 67, 72],
                vec![0, 4, 7],
                Fit::Complete { reference: 72 },
            ),
        ];

        for (active, classes, expect) in examples {
            one_classes_fixed(&active, classes, expect);
        }
    }

    fn one_classes_relative(active: &[u8], classes: Vec<u8>, expect: Fit) {
        one_case(active, KeyShape::ClassesRelative { classes }, expect);
    }

    #[test]
    fn test_classes_relative() {
        let examples = [
            (vec![0], vec![0], Fit::Complete { reference: 0 }),
            (vec![1], vec![0], Fit::Complete { reference: 1 }),
            (vec![1, 5], vec![0, 4], Fit::Complete { reference: 1 }),
            (
                vec![0, 5, 6],
                vec![0, 5],
                Fit::Partial {
                    reference: 0,
                    next: 6,
                },
            ),
            // the order doesn't matter, as long as the "matching" keys come first:
            (
                vec![8, 3, 11],
                vec![0, 5],
                Fit::Partial {
                    reference: 3,
                    next: 11,
                },
            ),
            (vec![8, 3, 4], vec![0, 5], Fit::Failed),
            // big major chord with octave doublings
            (
                vec![1, 13, 18, 22, 34],
                vec![0, 4, 7],
                Fit::Complete { reference: 18 },
            ),
            // a few illustrative examples: inversions of a major chord
            (
                vec![60, 64, 67],
                vec![0, 4, 7],
                Fit::Complete { reference: 60 },
            ),
            (
                vec![60, 64, 67],
                vec![0, 3, 8],
                Fit::Complete { reference: 64 },
            ),
            (
                vec![60, 64, 67],
                vec![0, 5, 9],
                Fit::Complete { reference: 67 },
            ),
        ];

        for (active, classes, expect) in examples {
            one_classes_relative(&active, classes, expect);
        }
    }

    fn one_voicing_fixed(active: &[u8], blocks: Vec<Vec<u8>>, expect: Fit) {
        one_case(active, KeyShape::BlockVoicingFixed { blocks }, expect);
    }

    #[test]
    fn test_block_voicing_fixed() {
        let examples = [
            (
                vec![1, 2, 3, 4],
                vec![vec![1, 2], vec![4, 3]],
                Fit::Complete { reference: 1 },
            ),
            (vec![1, 2, 3], vec![vec![1, 2], vec![4, 3]], Fit::Failed),
            (
                vec![3, 4, 5, 6],
                vec![vec![3], vec![5, 4]],
                Fit::Partial {
                    reference: 3,
                    next: 6,
                },
            ),
            (vec![0, 1, 2], vec![vec![0, 2], vec![1]], Fit::Failed),
            (
                vec![25, 26, 27],
                vec![vec![1, 2], vec![3]],
                Fit::Complete { reference: 25 },
            ),
            (vec![25, 26, 28], vec![vec![1, 2], vec![3]], Fit::Failed),
        ];

        for (active, blocks, expect) in examples {
            one_voicing_fixed(&active, blocks, expect);
        }
    }

    fn one_voicing_relative(active: &[u8], blocks: Vec<Vec<u8>>, expect: Fit) {
        one_case(active, KeyShape::BlockVoicingRelative { blocks }, expect);
    }

    #[test]
    fn test_block_voicing_relative() {
        let examples = [
            (
                vec![4, 5, 6, 7],
                vec![vec![0, 1], vec![3, 2]],
                Fit::Complete { reference: 4 },
            ),
            (
                vec![0, 1, 2, 3],
                vec![vec![0], vec![2, 1]],
                Fit::Partial {
                    reference: 0,
                    next: 3,
                },
            ),
            (vec![1, 2, 3], vec![vec![0, 2], vec![1]], Fit::Failed),
            (
                vec![25 + 1, 25 + 2, 25 + 3],
                vec![vec![0, 1], vec![2]],
                Fit::Complete { reference: 25 + 1 },
            ),
            // the next two come from a real bug
            (
                vec![60, 67, 70, 75],
                vec![vec![0], vec![3, 7, 10]],
                Fit::Complete { reference: 60 },
            ),
            (
                vec![60, 67, 70, 72, 75],
                vec![vec![0], vec![3, 7, 10]],
                Fit::Complete { reference: 60 },
            ),
        ];

        for (active, blocks, expect) in examples {
            one_voicing_relative(&active, blocks, expect);
        }
    }

    fn one_exact_fixed(active: &[u8], keys: Vec<u8>, expect: Fit) {
        one_case(active, KeyShape::ExactFixed { keys }, expect);
    }

    #[test]
    fn test_exact_fixed() {
        let examples = [
            (vec![0], vec![0], Fit::Complete { reference: 0 }),
            (vec![1], vec![0], Fit::Failed),
            (vec![0], vec![1], Fit::Failed),
            (
                vec![0, 1],
                vec![0],
                Fit::Partial {
                    reference: 0,
                    next: 1,
                },
            ),
            (
                vec![0, 2, 3],
                vec![0, 2],
                Fit::Partial {
                    reference: 0,
                    next: 3,
                },
            ),
            (
                vec![10, 32, 45],
                vec![10, 32, 45],
                Fit::Complete { reference: 10 },
            ),
            (vec![10, 32, 45], vec![11, 32, 45], Fit::Failed),
            (vec![10, 32], vec![10, 32, 45], Fit::Failed),
            (
                vec![10, 32, 45],
                vec![10, 32],
                Fit::Partial {
                    reference: 10,
                    next: 33,
                },
            ),
        ];

        for (active, keys, expect) in examples {
            one_exact_fixed(&active, keys, expect);
        }
    }

    fn one_exact_relative(active: &[u8], offsets: Vec<u8>, expect: Fit) {
        one_case(active, KeyShape::ExactRelative { offsets }, expect);
    }

    #[test]
    fn test_exact_relative() {
        let examples = [
            (vec![0], vec![0], Fit::Complete { reference: 0 }),
            (vec![1], vec![0], Fit::Complete { reference: 1 }),
            (
                vec![0, 1],
                vec![0],
                Fit::Partial {
                    reference: 0,
                    next: 1,
                },
            ),
            (
                vec![0, 2, 3],
                vec![0, 2],
                Fit::Partial {
                    reference: 0,
                    next: 3,
                },
            ),
            (
                vec![10, 32, 45],
                vec![0, 22, 35],
                Fit::Complete { reference: 10 },
            ),
            (vec![10, 32, 45], vec![1, 22, 35], Fit::Failed),
            (vec![10, 32], vec![0, 22, 35], Fit::Failed),
            (
                vec![10, 32, 45],
                vec![0, 22],
                Fit::Partial {
                    reference: 10,
                    next: 33,
                },
            ),
            (
                vec![20, 42, 55],
                vec![0, 22],
                Fit::Partial {
                    reference: 20,
                    next: 43,
                },
            ),
        ];

        for (active, offsets, expect) in examples {
            one_exact_relative(&active, offsets, expect);
        }
    }
}
