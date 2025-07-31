use serde_derive::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "kebab-case")]
pub enum KeyShape {
    #[serde(rename_all = "kebab-case")]
    ClassesFixed { classes: Vec<u8>, zero: u8 },
    #[serde(rename_all = "kebab-case")]
    ClassesRelative { classes: Vec<u8> },
    #[serde(rename_all = "kebab-case")]
    VoicingFixed { blocks: Vec<Vec<u8>>, zero: u8 },
    #[serde(rename_all = "kebab-case")]
    VoicingRelative { blocks: Vec<Vec<u8>> },
}

#[derive(Debug, PartialEq)]
pub struct Fit {
    pub reference: u8,
    next: usize,
}

impl Fit {
    pub fn new_worst() -> Self {
        Self {
            reference: 0,
            next: 0,
        }
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

impl KeyShape {
    pub fn fit<N: HasActivationStatus>(&self, notes: &[N; 128], start: usize) -> Fit {
        match self {
            Self::ClassesFixed { classes, zero } => fit_classes_fixed(classes, *zero, notes, start),
            Self::ClassesRelative { classes } => fit_classes_relative(classes, notes, start),
            Self::VoicingFixed { blocks, zero } => fit_voicing_fixed(blocks, *zero, notes, start),
            Self::VoicingRelative { blocks } => fit_voicing_relative(blocks, notes, start),
        }
    }
}

fn fit_classes_fixed<N: HasActivationStatus>(
    classes: &[u8],
    zero: u8,
    notes: &[N; 128],
    start: usize,
) -> Fit {
    let period_keys = 12;
    let mut used = vec![false; classes.len()];
    let mut i = start;
    while i < 128 {
        if !notes[i].active() {
            i += 1;
            continue;
        }
        match classes
            .iter()
            .position(|&x| (x + zero) % period_keys as u8 == i as u8 % period_keys)
        {
            Some(j) => {
                i += 1;
                used[j] = true
            }
            None {} => break,
        }
    }
    if used.iter().any(|&u| !u) {
        Fit {
            reference: zero,
            next: start,
        }
    } else {
        Fit {
            reference: zero,
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
    Fit {
        reference: 0,
        next: 0,
    }
}

fn fit_voicing_fixed<N: HasActivationStatus>(
    blocks: &[Vec<u8>],
    zero: u8,
    notes: &[N; 128],
    start: usize,
) -> Fit {
    let mut next = start;
    let mut i = 0;
    while i < blocks.len() {
        match fit_classes_fixed(&blocks[i], zero, notes, next) {
            Fit { next: new_next, .. } => {
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
            reference: zero,
            next,
        }
    } else {
        Fit {
            reference: zero,
            next: start,
        }
    }
}

fn fit_voicing_relative<N: HasActivationStatus>(
    blocks: &[Vec<u8>],
    notes: &[N; 128],
    start: usize,
) -> Fit {
    for zero in 0..12 {
        let res = fit_voicing_fixed(blocks, u8::from(zero), notes, start);
        match res {
            Fit { next, .. } => {
                if next > start {
                    return res;
                }
            }
        }
    }
    Fit {
        reference: 0,
        next: 0,
    }
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
        let actual = pat.fit(&active_notes, 0);
        assert!(actual == expect, "for\npattern: {pat:?}\nactive: {active:?}\n\nexpected: {expect:?}\n     got: {actual:?}");
    }

    fn one_classes_fixed(active: &[u8], classes: Vec<u8>, zero: u8, reference: u8, next: usize) {
        one_case(
            active,
            KeyShape::ClassesFixed { classes, zero },
            Fit { reference, next },
        );
    }

    #[test]
    fn test_classes_fixed() {
        let examples = [
            (vec![0], vec![0], 0, 0, 128),
            (vec![0, 1], vec![0], 0, 0, 1),
            (vec![1], vec![1], 0, 0, 128),
            (vec![1], vec![0], 1, 1, 128),
            (vec![1], vec![0], 0, 0, 0),
            (vec![0], vec![0], 1, 1, 0),
            (vec![0, 5], vec![0, 5], 0, 0, 128),
            (vec![0, 4], vec![0, 5], 0, 0, 0),
            (vec![0, 5], vec![0, 4], 0, 0, 0),
            (vec![1, 5], vec![0, 4], 1, 1, 128),
            (vec![0, 4], vec![1, 5], 11, 11, 128),
            (vec![0, 5, 6], vec![0, 5], 0, 0, 6),
            // the order doesn't matter, as long as the "matching" keys come first:
            (vec![8, 3, 11], vec![0, 5], 3, 3, 11),
            (vec![8, 3, 4], vec![0, 5], 3, 3, 0),
            // permutations (active notes)
            (vec![1, 2, 3], vec![1, 2, 3], 0, 0, 128),
            (vec![1, 3, 2], vec![1, 2, 3], 0, 0, 128),
            (vec![2, 1, 3], vec![1, 2, 3], 0, 0, 128),
            (vec![2, 3, 1], vec![1, 2, 3], 0, 0, 128),
            (vec![3, 1, 2], vec![1, 2, 3], 0, 0, 128),
            (vec![3, 2, 1], vec![1, 2, 3], 0, 0, 128),
            // permutations (pattern)
            (vec![1, 2, 3], vec![1, 2, 3], 0, 0, 128),
            (vec![1, 2, 3], vec![1, 3, 2], 0, 0, 128),
            (vec![1, 2, 3], vec![2, 1, 3], 0, 0, 128),
            (vec![1, 2, 3], vec![2, 3, 1], 0, 0, 128),
            (vec![1, 2, 3], vec![3, 1, 2], 0, 0, 128),
            (vec![1, 2, 3], vec![3, 2, 1], 0, 0, 128),
            // longer than one octave
            (vec![0, 13], vec![0, 1], 0, 0, 128),
            (vec![20, 7], vec![0, 1], 7, 7, 128),
        ];

        for (active, classes, zero, reference, next) in examples {
            one_classes_fixed(&active, classes, u8::from(zero), u8::from(reference), next);
        }
    }

    fn one_classes_relative(active: &[u8], classes: Vec<u8>, reference: u8, next: usize) {
        one_case(
            active,
            KeyShape::ClassesRelative { classes },
            Fit { reference, next },
        );
    }

    #[test]
    fn test_classes_relative() {
        let examples = [
            (vec![0], vec![0], 0, 128),
            (vec![1], vec![0], 1, 128),
            (vec![0], vec![1], 11, 128),
            (vec![1, 5], vec![0, 4], 1, 128),
            (vec![0, 4], vec![1, 5], 11, 128),
            (vec![0, 5, 6], vec![0, 5], 0, 6),
            // the order doesn't matter, as long as the "matching" keys come first:
            (vec![8, 3, 11], vec![0, 5], 3, 11),
            (vec![8, 3, 4], vec![0, 5], 0, 0),
            // big major chord with octave doublings
            (vec![1, 13, 18, 22, 34], vec![0, 4, 7], 6, 128),
        ];

        for (active, classes, reference, next) in examples {
            one_classes_relative(&active, classes, u8::from(reference), next);
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
            KeyShape::VoicingFixed { blocks, zero },
            Fit { reference, next },
        );
    }

    #[test]
    fn test_voicing_fixed() {
        let examples = [
            (vec![1, 2, 3, 4], vec![vec![1, 2], vec![4, 3]], 0, 0, 128),
            (vec![1, 2, 3, 4], vec![vec![1], vec![3, 2]], 0, 0, 4),
            (vec![1, 2, 3], vec![vec![1, 3], vec![2]], 0, 0, 0),
            // [zero]s can be offset by multiples of 12
            (
                vec![25 + 1, 25 + 2, 25 + 3],
                vec![vec![1, 2], vec![3]],
                25,
                25,
                128,
            ),
            (
                vec![25 + 1, 25 + 2, 25 + 3],
                vec![vec![1, 2], vec![3]],
                1,
                1,
                128,
            ),
        ];

        for (active, blocks, zero, reference, next) in examples {
            one_voicing_fixed(&active, blocks, u8::from(zero), u8::from(reference), next);
        }
    }

    fn one_voicing_relative(active: &[u8], blocks: Vec<Vec<u8>>, reference: u8, next: usize) {
        one_case(
            active,
            KeyShape::VoicingRelative { blocks },
            Fit { reference, next },
        );
    }

    #[test]
    fn test_voicing_relative() {
        let examples = [
            (vec![4, 5, 6, 7], vec![vec![1, 2], vec![4, 3]], 3, 128),
            (vec![0, 1, 2, 3], vec![vec![1], vec![3, 2]], 11, 3),
            (vec![1, 2, 3], vec![vec![1, 3], vec![2]], 0, 0),
            // the [zero] in the range 0..12 is chosen:
            (
                vec![25 + 1, 25 + 2, 25 + 3],
                vec![vec![1, 2], vec![3]],
                1,
                128,
            ),
        ];

        for (active, blocks, reference, next) in examples {
            one_voicing_relative(&active, blocks, u8::from(reference), next);
        }
    }
}
