use serde_derive::{Deserialize, Serialize};
use serde_with::serde_as;
use std::marker::PhantomData;

use crate::interval::{Interval, StackCoeff, Temperament};

#[serde_as]
#[derive(Serialize, Deserialize)]
pub struct Config<const D: usize, const T: usize> {
    #[serde_as(as = "[_; D]")]
    intervals: [Interval; D],
    #[serde_as(as = "[_; T]")]
    temperaments: [TemperamentConfig<D>; T],
    // tui: TuiConfig,
}

#[serde_as]
#[derive(Serialize, Deserialize)]
pub struct TemperamentConfig<const D: usize> {
    name: String,
    #[serde_as(as = "[[_; D]; D]")]
    pure_stacks: [[StackCoeff; D]; D],
    #[serde_as(as = "[[_; D]; D]")]
    tempered_stacks: [[StackCoeff; D]; D],
}

// use typenum::{consts::*, Unsigned};
//
// pub struct StackType<D: Unsigned, T: Unsigned> {
//     _x: PhantomData<(D, T)>,
// }
//
// fn foo(d:usize, n:usize) -> StackType<dyn Unsigned, dyn Unsigned> {
//     StackType::<U0, U2> { _x: PhantomData }
// }

// pub trait Size {
//     fn check(n: usize) -> bool
//     where
//         Self: Sized;
// }
//
// pub struct Foo<D: Size + ?Sized> {
//     x: PhantomData<D>,
// }
//
// impl<D: Size + ?Sized> Foo<D> {
//     pub fn new(x: PhantomData<D>) -> Self {
//         Self { x }
//     }
// }
//
// pub struct Size0 {}
// pub struct Size1 {}
//
// impl Size for Size0 {
//     fn check(n: usize) -> bool {
//         0 == n
//     }
// }
//
// impl Size for Size1 {
//     fn check(n: usize) -> bool {
//         1 == n
//     }
// }
//
// pub fn foo_from(_: usize) -> Box<Foo<dyn Size>> {
//     let f = Foo::<Size0>::new(PhantomData);
//     if 3 * 4 == 13 {
//         Box::new(f)
//     } else {
//     }
//     //   Box::new(Foo::<Size0> { x: PhantomData })
// }
//
// // pub fn temperament_from_config<
// // const D: usize>(
// //     conf: TemperamentConfig<D>,
// // ) -> Temperament<D, StackCoeff> {
// //     Temperament::new(Box::from(conf.name), conf.pure_stacks, conf.tempered_stacks).unwrap()
// // }
//
// //
// // #[derive(Serialize, Deserialize)]
// // pub struct TuiConfig {
// //     grid: GridConfig,
// // }
// //
// // #[derive(Serialize, Deserialize)]
// // pub struct GridConfig {
// //     left_right: usize,
// //     up_down: usize,
// // }
