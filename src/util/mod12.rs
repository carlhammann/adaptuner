use std::{
    cmp::PartialOrd,
    ops::{Add, Rem, Sub},
};

use serde_repr::{Deserialize_repr, Serialize_repr};

#[repr(u8)]
#[derive(Serialize_repr, Deserialize_repr, Copy, Clone, Debug, PartialEq)]
pub enum PitchClass {
    PC0 = 0,
    PC1 = 1,
    PC2 = 2,
    PC3 = 3,
    PC4 = 4,
    PC5 = 5,
    PC6 = 6,
    PC7 = 7,
    PC8 = 8,
    PC9 = 9,
    PC10 = 10,
    PC11 = 11,
}

impl From<u8> for PitchClass {
    fn from(i: u8) -> Self {
        unsafe { std::mem::transmute(i % 12) }
    }
}

impl Add for PitchClass {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        let res: u8 = add_mod12(self as u8, rhs as u8);
        unsafe { std::mem::transmute(res) }
    }
}

pub fn add_mod12<T>(l: T, r: T) -> T
where
    T: Rem<Output = T> + Add<Output = T> + Sub<Output = T> + PartialOrd + From<u8>,
{
    let x = (l % T::from(12)) + (r % T::from(12));
    if x >= T::from(12) {
        x - T::from(12)
    } else {
        x
    }
}

pub fn sub_mod12<T>(l: T, r: T) -> T
where
    T: Rem<Output = T> + Add<Output = T> + Sub<Output = T> + PartialOrd + From<u8>,
{
    let a = l % T::from(12);
    let b = r % T::from(12);
    if a >= b {
        a - b
    } else {
        T::from(12) - b + a
    }
}
