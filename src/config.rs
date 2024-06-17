pub trait Config<A> {
    fn initialise(config: &Self) -> A;
}

// use serde_derive::{Deserialize, Serialize};
//
// use crate::{
//     interval::{Interval, Semitones, StackCoeff, Temperament},
//     pattern::Pattern,
//     util::dimension::{
//         fixed_sizes::Size3, initialise_runtime_dimension, vector, Dimension, Matrix,
//         RuntimeDimension, Vector,
//     },
// };
//
// #[derive(Serialize, Deserialize, Debug)]
// pub struct RawConfig {
//     //pub intervals: Vec<Interval>,
//     pub temperaments: Vec<TemperamentConfig>,
//     pub patterns: Vec<Pattern>,
// }
//
// #[derive(Serialize, Deserialize, Debug)]
// pub struct TemperamentConfig {
//     pub name: String,
//     pub equations: [([StackCoeff; 3], [StackCoeff; 3]); 3],
// }
//
// #[derive(Debug)]
// pub struct Config<D: Dimension, T: 'static> {
//     pub intervals: Vector<D, Interval>,
//     pub temperaments: Vector<RuntimeDimension<T>, Temperament<D, StackCoeff>>,
//     pub patterns: Vec<Pattern>,
// }
//
// pub fn validate<T: 'static>(raw: RawConfig) -> Config<Size3, T> {
//     let intervals = vector(&[
//         Interval {
//             name: "octave".to_string(),
//             semitones: 12.0,
//             key_distance: 12,
//         },
//         Interval {
//             name: "fifth".to_string(),
//             semitones: 12.0 * (3.0 / 2.0 as Semitones).log2(),
//             key_distance: 7,
//         },
//         Interval {
//             name: "third".to_string(),
//             semitones: 12.0 * (5.0 / 4.0 as Semitones).log2(),
//             key_distance: 4,
//         },
//     ])
//     .unwrap();
//
//     initialise_runtime_dimension::<T>(raw.temperaments.len());
//     let temperaments = Vector::from_fn(|row| {
//         let pure = Matrix::from_fn(|(i, j)| {
//             raw.temperaments[row.get() as usize].equations[i.get()].0[j.get()]
//         });
//         let tempered = Matrix::from_fn(|(i, j)| {
//             raw.temperaments[row.get() as usize].equations[i.get()].1[j.get()]
//         });
//         let name = &raw.temperaments[row.get() as usize].name;
//         Temperament::new(name.to_string(), pure, &tempered).unwrap()
//     });
//
//     Config {
//         intervals,
//         patterns: raw.patterns,
//         temperaments,
//     }
// }
