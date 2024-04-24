// pub enum Pattern {
//     Any,
//     Absolute(Vec<u8>),
//     Relative(Vec<Vec<u8>>),
// }
//
//
use std::ops;

// #[repr(transparent)]
// #[derive(Clone, Copy, PartialEq)]
// pub struct Mod<const M: u8> {
//     inner: u8,
// }

// impl<const M: u8> From<u8> for Mod<M> {
//     fn from(n: u8) -> Self {
//         Mod { inner: n % 12 }
//     }
// }

fn add_mod12(l: u8, r: u8) -> u8 {
    let x = (l % 12) + (r % 12);
    if x >= 12 {
        x - 12
    } else {
        x
    }
}

// impl<const M: u8> ops::Sub<Mod<M>> for Mod<M> {
//     type Output = Self;
//     fn sub(self, rhs: Self) -> Self {
//         let a = self.inner % M;
//         let b = rhs.inner % M;
//         if a >= b {
//             Mod { inner: a - b }
//         } else {
//             Mod { inner: M - b + a }
//         }
//     }
// }

pub trait Pattern {
    fn fit(&self, active: &[u8], index: usize) -> (usize, u8);
}

pub trait ReferencePattern {
    fn fit_reference(&self, reference: u8, active: &[u8], index: usize) -> usize;
}

// This impl is correct, but not really efficient:

// impl<T: ReferencePattern> Pattern for T {
//     fn fit(&self, active: &[u8], index: usize) -> (usize, u8) {
//         let mut progress = 0;
//         let mut reference = 0;
//
//         for r in 0..12 {
//             let p = self.fit_reference(r, active, index);
//             if p > progress {
//                 progress = p;
//                 reference = r;
//             }
//         }
//
//         (progress, reference)
//     }
// }

impl<T: ReferencePattern> ReferencePattern for Vec<T> {
    fn fit_reference(&self, reference: u8, active: &[u8], index: usize) -> usize {
        let mut i = 0;
        let mut progress = 0;
        let mut ix = index;
        while i < self.len() {
            let delta = self[i].fit_reference(reference, active, ix);
            if delta == 0 {
                return 0;
            } else {
                progress += delta;
                ix += delta;
                i += 1;
            }
        }
        if i == self.len() {
            progress
        } else {
            0
        }
    }
}

pub struct Group(Vec<u8>);

impl Group {
    pub fn new(x: Vec<u8>) -> Self {
        let mut x = x.clone();
        if x.is_empty() {
            panic!("cannot greate empty `Group` pf pitch classes");
        }
        x.iter_mut().for_each(|a| *a = *a % 12);

        x.sort();
        Group(x)
    }
}

impl ReferencePattern for Group {
    fn fit_reference(&self, reference: u8, active: &[u8], index: usize) -> usize {
        let mut unused = self.0.clone();
        let mut used: Vec<u8> = Vec::with_capacity(unused.len());
        let mut i = index;
        while i < active.len() {
            match unused
                .iter()
                .position(|&x| add_mod12(x, reference) == active[i] % 12)
            {
                None => {
                    if used
                        .iter()
                        .any(|&x| add_mod12(x, reference) == active[i] % 12)
                    {
                        i += 1;
                    } else {
                        break;
                    }
                }
                Some(j) => {
                    used.push(unused[j]);
                    unused.remove(j);
                }
            }
        }
        if unused.is_empty() {
            i - index
        } else {
            0
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_cell_reference() {
        assert_eq!(Group::new(vec!(0)).fit_reference(0, &[0], 0), 1);
        assert_eq!(Group::new(vec!(1)).fit_reference(0, &[1], 0), 1);
        assert_eq!(Group::new(vec!(1)).fit_reference(11, &[0], 0), 1);
        assert_eq!(Group::new(vec!(0)).fit_reference(1, &[1], 0), 1);
        assert_eq!(Group::new(vec!(0)).fit_reference(1, &[2], 0), 0);
        assert_eq!(Group::new(vec!(0)).fit_reference(1, &[0], 0), 0);
    }

    #[test]
    fn test_cell_two() {
        assert_eq!(Group::new(vec!(0, 5)).fit_reference(0, &[0, 4], 0), 0);
        assert_eq!(Group::new(vec!(0, 5)).fit_reference(0, &[0], 0), 0);
        assert_eq!(Group::new(vec!(0, 5)).fit_reference(0, &[0, 5], 0), 2);
        assert_eq!(Group::new(vec!(0, 5)).fit_reference(0, &[0, 5, 3], 0), 2);

        assert_eq!(Group::new(vec!(0)).fit_reference(5, &[0, 5], 1), 1);

        assert_eq!(Group::new(vec!(0, 5)).fit_reference(0, &[5, 0], 0), 2);
        assert_eq!(Group::new(vec!(5, 0)).fit_reference(0, &[0, 5], 0), 2);
        assert_eq!(Group::new(vec!(0, 5)).fit_reference(3, &[8, 3, 9], 0), 2);
    }

    #[test]
    fn test_cell_permutations() {
        assert_eq!(Group::new(vec!(1, 2, 3)).fit_reference(0, &[1, 2, 3], 0), 3);
        assert_eq!(Group::new(vec!(1, 3, 2)).fit_reference(0, &[1, 2, 3], 0), 3);
        assert_eq!(Group::new(vec!(2, 1, 3)).fit_reference(0, &[1, 2, 3], 0), 3);
        assert_eq!(Group::new(vec!(2, 3, 1)).fit_reference(0, &[1, 2, 3], 0), 3);
        assert_eq!(Group::new(vec!(3, 1, 2)).fit_reference(0, &[1, 2, 3], 0), 3);
        assert_eq!(Group::new(vec!(3, 2, 1)).fit_reference(0, &[1, 2, 3], 0), 3);

        assert_eq!(Group::new(vec!(1, 2, 3)).fit_reference(0, &[1, 2, 3], 0), 3);
        assert_eq!(Group::new(vec!(1, 2, 3)).fit_reference(0, &[1, 3, 2], 0), 3);
        assert_eq!(Group::new(vec!(1, 2, 3)).fit_reference(0, &[2, 1, 3], 0), 3);
        assert_eq!(Group::new(vec!(1, 2, 3)).fit_reference(0, &[2, 3, 1], 0), 3);
        assert_eq!(Group::new(vec!(1, 2, 3)).fit_reference(0, &[3, 1, 2], 0), 3);
        assert_eq!(Group::new(vec!(1, 2, 3)).fit_reference(0, &[3, 2, 1], 0), 3);
    }

    #[test]
    fn test_cells() {
        assert_eq!(
            vec!(vec!(1, 2), vec!(3, 4)).fit_reference(0, &vec!(1, 2, 3, 4), 0),
            4
        );
        assert_eq!(
            vec!(vec!(1, 2), vec!(3, 4)).fit_reference(0, &vec!(1, 2, 3), 0),
            0
        );
        assert_eq!(
            vec!(vec!(1, 2), vec!(3, 4)).fit_reference(0, &vec!(2, 1, 4, 3), 0),
            4
        );
        assert_eq!(
            vec!(vec!(1, 2), vec!(3, 4)).fit_reference(0, &vec!(1, 3, 2, 4), 0),
            0
        );
        assert_eq!(
            vec!(vec!(1, 2), vec!(3, 4)).fit_reference(0, &vec!(1, 1, 2, 3, 4, 3, 5), 0),
            6
        );
        assert_eq!(
            vec!(vec!(1, 2), vec!(3, 4)).fit_reference(0, &vec!(1, 1, 2, 5, 3, 4, 3, 5), 0),
            0
        );
    }

    #[test]
    fn test_free_cells() {
        assert_eq!(
            vec!(vec!(1, 2), vec!(3, 4)).fit(&vec!(1, 2, 3, 4), 0),
            (4, 0)
        );
        assert_eq!(
            vec!(vec!(2, 3), vec!(4, 5)).fit(&vec!(1, 2, 3, 4), 0),
            (4, 11)
        );
        assert_eq!(
            vec!(vec!(2, 3), vec!(4, 5)).fit(&vec!(2, 1, 3, 4), 0),
            (4, 11)
        );
    }
}
