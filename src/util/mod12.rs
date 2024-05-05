use std::{
    cmp::PartialOrd,
    ops::{Add, Rem, Sub},
};

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
