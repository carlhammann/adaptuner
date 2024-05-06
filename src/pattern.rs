use crate::util::mod12::{PitchClass, PitchClass::*};

#[derive(Debug)]
pub enum Pattern<'a> {
    ClassesFixed {
        classes: &'a [PitchClass],
        zero: PitchClass,
    },
    ClassesRelative {
        classes: &'a [PitchClass],
    },
    VoicingFixed {
        blocks: &'a [Vec<PitchClass>],
        zero: PitchClass,
    },
    VoicingRelative {
        blocks: &'a [Vec<PitchClass>],
    },
}

#[derive(Debug, PartialEq)]
pub struct Fit {
    pub reference: PitchClass,
    pub next: usize,
}

impl<'a> Pattern<'a> {
    /// assumption: `active_notes` contains at least one `true` entry
    ///
    /// TODO: write a `normalise` function for [Pattern]s.
    pub fn fit(&self, active_notes: &[bool; 128], start: usize) -> Fit {
        match self {
            Self::ClassesFixed { classes, zero } => {
                let mut used = vec![false; classes.len()];
                let mut i = start;
                while i < 128 {
                    if !active_notes[i] {
                        i += 1;
                        continue;
                    }
                    match classes
                        .iter()
                        .position(|&x| (x + *zero) as u8 == i as u8 % 12)
                    {
                        Some(j) => {
                            i += 1;
                            used[j] = true
                        }
                        None => break,
                    }
                }
                if used.iter().any(|&u| !u) {
                    Fit {
                        reference: *zero,
                        next: start,
                    }
                } else {
                    Fit {
                        reference: *zero,
                        next: i,
                    }
                }
            }
            Self::ClassesRelative { classes } => {
                for zero in 0..12 {
                    let res = (Self::ClassesFixed {
                        classes,
                        zero: PitchClass::from(zero),
                    })
                    .fit(active_notes, start);
                    match res {
                        Fit { next, .. } => {
                            if next > start {
                                return res;
                            }
                        }
                    }
                }
                Fit {
                    reference: PC0,
                    next: 0,
                }
            }
            Self::VoicingFixed { blocks, zero } => {
                let mut next = start;
                let mut i = 0;
                while i < blocks.len() {
                    // println!("i={i}, next={next}, classes={:?}", blocks[i]);
                    match (Self::ClassesFixed {
                        classes: &blocks[i],
                        zero: *zero,
                    })
                    .fit(active_notes, next)
                    {
                        Fit { next: new_next, .. } => {
                            // println!("new_next={new_next}, next={next}");
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
                        reference: *zero,
                        next,
                    }
                } else {
                    Fit {
                        reference: *zero,
                        next: start,
                    }
                }
            }
            Self::VoicingRelative { blocks } => {
                for zero in 0..12 {
                    let res = (Self::VoicingFixed {
                        blocks,
                        zero: PitchClass::from(zero),
                    })
                    .fit(active_notes, start);
                    match res {
                        Fit { next, .. } => {
                            if next > start {
                                return res;
                            }
                        }
                    }
                }
                Fit {
                    reference: PC0,
                    next: 0,
                }
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn one_case(active: &[u8], pat: Pattern, expect: Fit) {
        let mut active_notes = [false; 128];
        for i in active {
            active_notes[*i as usize] = true;
        }
        let actual = pat.fit(&active_notes, 0);
        assert!(actual == expect, "for\npattern: {pat:?}\nactive: {active:?}\n\nexpected: {expect:?}\n     got: {actual:?}");
    }

    fn one_classes_fixed(
        active: &[u8],
        classes: &[PitchClass],
        zero: PitchClass,
        reference: PitchClass,
        next: usize,
    ) {
        one_case(
            active,
            Pattern::ClassesFixed { classes, zero },
            Fit { reference, next },
        );
    }

    #[test]
    fn test_classes_fixed() {
        let examples = [
            (vec![0], vec![PC0], 0, 0, 128),
            (vec![0, 1], vec![PC0], 0, 0, 1),
            (vec![1], vec![PC1], 0, 0, 128),
            (vec![1], vec![PC0], 1, 1, 128),
            (vec![1], vec![PC0], 0, 0, 0),
            (vec![0], vec![PC0], 1, 1, 0),
            (vec![0, 5], vec![PC0, PC5], 0, 0, 128),
            (vec![0, 4], vec![PC0, PC5], 0, 0, 0),
            (vec![0, 5], vec![PC0, PC4], 0, 0, 0),
            (vec![1, 5], vec![PC0, PC4], 1, 1, 128),
            (vec![0, 4], vec![PC1, PC5], 11, 11, 128),
            (vec![0, 5, 6], vec![PC0, PC5], 0, 0, 6),
            // the order doesn't matter, as long as the "matching" keys come first:
            (vec![8, 3, 11], vec![PC0, PC5], 3, 3, 11),
            (vec![8, 3, 4], vec![PC0, PC5], 3, 3, 0),
            // permutations (active notes)
            (vec![1, 2, 3], vec![PC1, PC2, PC3], 0, 0, 128),
            (vec![1, 3, 2], vec![PC1, PC2, PC3], 0, 0, 128),
            (vec![2, 1, 3], vec![PC1, PC2, PC3], 0, 0, 128),
            (vec![2, 3, 1], vec![PC1, PC2, PC3], 0, 0, 128),
            (vec![3, 1, 2], vec![PC1, PC2, PC3], 0, 0, 128),
            (vec![3, 2, 1], vec![PC1, PC2, PC3], 0, 0, 128),
            // permutations (pattern)
            (vec![1, 2, 3], vec![PC1, PC2, PC3], 0, 0, 128),
            (vec![1, 2, 3], vec![PC1, PC3, PC2], 0, 0, 128),
            (vec![1, 2, 3], vec![PC2, PC1, PC3], 0, 0, 128),
            (vec![1, 2, 3], vec![PC2, PC3, PC1], 0, 0, 128),
            (vec![1, 2, 3], vec![PC3, PC1, PC2], 0, 0, 128),
            (vec![1, 2, 3], vec![PC3, PC2, PC1], 0, 0, 128),
            // longer than one octave
            (vec![0, 13], vec![PC0, PC1], 0, 0, 128),
            (vec![20, 7], vec![PC0, PC1], 7, 7, 128),
        ];

        for (active, classes, zero, reference, next) in examples {
            one_classes_fixed(
                &active,
                &classes,
                PitchClass::from(zero),
                PitchClass::from(reference),
                next,
            );
        }
    }

    fn one_classes_relative(
        active: &[u8],
        classes: &[PitchClass],
        reference: PitchClass,
        next: usize,
    ) {
        one_case(
            active,
            Pattern::ClassesRelative { classes },
            Fit { reference, next },
        );
    }

    #[test]
    fn test_classes_relative() {
        let examples = [
            (vec![0], vec![PC0], 0, 128),
            (vec![1], vec![PC0], 1, 128),
            (vec![0], vec![PC1], 11, 128),
            (vec![1, 5], vec![PC0, PC4], 1, 128),
            (vec![0, 4], vec![PC1, PC5], 11, 128),
            (vec![0, 5, 6], vec![PC0, PC5], 0, 6),
            // the order doesn't matter, as long as the "matching" keys come first:
            (vec![8, 3, 11], vec![PC0, PC5], 3, 11),
            (vec![8, 3, 4], vec![PC0, PC5], 0, 0),
            // big major chord with octave doublings
            (vec![1, 13, 18, 22, 34], vec![PC0, PC4, PC7], 6, 128),
        ];

        for (active, classes, reference, next) in examples {
            one_classes_relative(&active, &classes, PitchClass::from(reference), next);
        }
    }

    fn one_voicing_fixed(
        active: &[u8],
        blocks: &[Vec<PitchClass>],
        zero: PitchClass,
        reference: PitchClass,
        next: usize,
    ) {
        one_case(
            active,
            Pattern::VoicingFixed { blocks, zero },
            Fit { reference, next },
        );
    }

    #[test]
    fn test_voicing_fixed() {
        let examples = [
            (
                vec![1, 2, 3, 4],
                vec![vec![PC1, PC2], vec![PC4, PC3]],
                0,
                0,
                128,
            ),
            (vec![1, 2, 3, 4], vec![vec![PC1], vec![PC3, PC2]], 0, 0, 4),
            (vec![1, 2, 3], vec![vec![PC1, PC3], vec![PC2]], 0, 0, 0),
            // [zero]s can be offset by multiples of 12
            (
                vec![25 + 1, 25 + 2, 25 + 3],
                vec![vec![PC1, PC2], vec![PC3]],
                25,
                25,
                128,
            ),
            (
                vec![25 + 1, 25 + 2, 25 + 3],
                vec![vec![PC1, PC2], vec![PC3]],
                1,
                1,
                128,
            ),
        ];

        for (active, blocks, zero, reference, next) in examples {
            one_voicing_fixed(
                &active,
                &blocks,
                PitchClass::from(zero),
                PitchClass::from(reference),
                next,
            );
        }
    }

    fn one_voicing_relative(
        active: &[u8],
        blocks: &[Vec<PitchClass>],
        reference: PitchClass,
        next: usize,
    ) {
        one_case(
            active,
            Pattern::VoicingRelative { blocks },
            Fit { reference, next },
        );
    }

    #[test]
    fn test_voicing_relative() {
        let examples = [
            (
                vec![4, 5, 6, 7],
                vec![vec![PC1, PC2], vec![PC4, PC3]],
                3,
                128,
            ),
            (vec![0, 1, 2, 3], vec![vec![PC1], vec![PC3, PC2]], 11, 3),
            (vec![1, 2, 3], vec![vec![PC1, PC3], vec![PC2]], 0, 0),
            // the [zero] in the range 0..12 is chosen:
            (
                vec![25 + 1, 25 + 2, 25 + 3],
                vec![vec![PC1, PC2], vec![PC3]],
                1,
                128,
            ),
        ];

        for (active, blocks, reference, next) in examples {
            one_voicing_relative(&active, &blocks, PitchClass::from(reference), next);
        }
    }
}
