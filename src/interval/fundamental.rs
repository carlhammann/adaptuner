use crate::{
    interval::{stack::Stack, stacktype::r#trait::StackCoeff},
    neighbourhood::{self, Neighbourhood},
};

pub use adaptuner_core::interval::fundamental::{HasFundamental, HasOvertone};

/// Returns an overtone or fundamental of all notes in the given neighbourhood, whichever is closest
/// (on the keyboard)
/// to the topmost or bottommost note. If both distances are the same, a fundamental is returned.
///
/// `true` means fundamental, `false` means overtone.
pub fn fundamental_or_overtone<T: HasFundamental + HasOvertone>(
    neigh: &neighbourhood::Partial<T>,
) -> (bool, Stack<T>) {
    let fundamental = T::fundamental_many(neigh.iter().map(|x| x.1));
    let overtone = T::overtone_many(neigh.iter().map(|x| x.1));
    let mut lowest_key = StackCoeff::MAX;
    let mut highest_key = StackCoeff::MIN;
    neigh.for_each_stack(|_, stack| {
        let x = stack.key_number();
        lowest_key = lowest_key.min(x);
        highest_key = highest_key.max(x);
    });
    if (overtone.key_number() - highest_key) < (lowest_key - fundamental.key_number()) {
        (false, overtone)
    } else {
        (true, fundamental)
    }
}
