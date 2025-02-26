use std::{marker::PhantomData, ops};

use ndarray::Array1;
use num_integer::{gcd, lcm};
use serde_derive::{Deserialize, Serialize};

// TODO: rename interval::interval to interval::base
use crate::interval::stacktype::r#trait::{StackCoeff, StackType};

use super::interval::Semitones;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Stack<T: StackType> {
    _phantom: PhantomData<T>,
    target: Array1<StackCoeff>,

    /// must always be strictly positive
    denominator: StackCoeff,
    numerators: Array1<StackCoeff>,
}

//impl<T: StackType, P: ops::Deref<Target = Stack<T>>> ops::AddAssign<P> for Stack<T> {
//    fn add_assign(&mut self, other: P) {
//        self.scaled_add(1, other)
//    }
//}
//
//impl<T: StackType + Clone, P: ops::Deref<Target = Stack<T>>> ops::Add<P> for Stack<T> {
//    type Output = Self;
//    fn add(self, other: P) -> Self {
//        let mut res = self.clone();
//        res += other;
//        res
//    }
//}
//
//impl<T: StackType, P: ops::Deref<Target = Stack<T>>> ops::SubAssign<P> for Stack<T> {
//    fn sub_assign(&mut self, other: P) {
//        self.scaled_add(-1, other)
//    }
//}
//
//impl<T: StackType + Clone, P: ops::Deref<Target = Stack<T>>> ops::Sub<P> for Stack<T> {
//    type Output = Self;
//    fn sub(self, other: P) -> Self {
//        let mut res = self;
//        res -= other;
//        res
//    }
//}

impl<T: StackType> Stack<T> {
    pub fn new_zero() -> Self {
        Stack {
            _phantom: PhantomData,
            target: Array1::zeros(T::num_intervals()),
            denominator: 1,
            numerators: Array1::zeros(T::num_intervals()),
        }
    }

    pub fn from_pure_interval(interval_index: usize) -> Self {
        let mut target = Array1::zeros(T::num_intervals());
        target[interval_index] = 1;
        Stack {
            _phantom: PhantomData,
            target: target.clone(),
            denominator: 1,
            numerators: target,
        }
    }

    pub fn target_coefficients(&self) -> &[StackCoeff] {
        //self.target.as_slice().unwrap()
        todo!() // is the line above safe?
    }

    /// Ensures that
    /// - there is no non-trivial factor of self.denominator and all entries in self.numerators
    /// - self.denominator >= 1
    fn normalise(&mut self) {
        let mut g = self.denominator;
        for &c in &self.numerators {
            g = gcd(g, c);
        }
        if self.denominator < 0 {
            g = -g;
        }
        self.denominator /= g;
        self.numerators /= g;
    }

    pub fn scaled_add<P: ops::Deref<Target = Stack<T>>>(&mut self, scalar: StackCoeff, other: P) {
        self.target.scaled_add(scalar, &other.target);

        self.numerators *= other.denominator;
        self.numerators
            .scaled_add(scalar * self.denominator, &other.numerators);

        self.normalise();
    }

    pub fn is_target(&self) -> bool {
        (self.denominator == 1) & (self.numerators == self.target)
    }

    /// - `s.is_target()´ implies ´s.is_pure()´, but not vice versa.
    pub fn is_pure(&self) -> bool {
        self.denominator == 1
    }

    pub fn semitones(&self) -> Semitones {
        let mut res = 0.0;
        for (i, &c) in self.numerators.iter().enumerate() {
            res += T::intervals()[i].semitones * c as Semitones / self.denominator as Semitones;
        }
        res
    }

    /// If the zero stack corresponds to middle C, return the "fractional MIDI note number"
    /// described by this stack.
    pub fn absolute_semitones(&self) -> Semitones {
        self.semitones() + 60.0
    }

    /// How many fractional semitones higher than the target note is the actual note described by
    /// this stack?
    pub fn semitones_above_target(&self) -> Semitones {
        let mut res = 0.0;
        for (i, &c) in self.target.iter().enumerate() {
            res += T::intervals()[i].semitones * c as Semitones;
        }
        self.semitones() - res
    }

    pub fn key_distance(&self) -> StackCoeff {
        let mut res = 0;
        for (i, &c) in self.target.iter().enumerate() {
            res += T::intervals()[i].key_distance as StackCoeff * c;
        }
        res
    }

    /// If the zero stack corresponds to middle C, return the MIDI note number of the key that this
    /// stack describes. This uses the [Self::key_distance], so it returns the "enharmonically
    /// correct" key, not the one whose (equally tempered) MIDI note is closest to the actually
    /// sounding note.
    pub fn key_number(&self) -> StackCoeff {
        self.key_distance() + 60
    }

    pub fn reset_to_zero(&mut self) {
        self.target.fill(0);
        self.denominator = 1;
        self.numerators.fill(0);
    }

    pub fn retemper(&mut self, active_temperaments: &[bool]) {
        self.denominator = 1;
        self.numerators.clone_from(&self.target);

        for (t, &active) in active_temperaments.iter().enumerate() {
            if active {
               //let g = lcm(self.denominator, T::temperaments()[t])
            }
        }
    }
}
