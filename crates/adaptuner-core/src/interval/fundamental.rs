use crate::interval::{
    stack::Stack,
    stacktype::r#trait::{FiveLimitIntervalBasis, IntervalBasis},
};

/// Computes the highest common fundamental of two stacks.
pub trait HasFundamental: IntervalBasis {
    /// Like [`Self::fundamental`], but mutates the second argument
    /// to become the fundamental.
    fn fundamental_inplace(a: &Stack<Self>, b: &mut Stack<Self>);

    /// Determines the fundamental of `a` and `b`.
    ///
    /// The result is a clone of `b` modified to become the fundamental.
    fn fundamental(a: &Stack<Self>, b: &Stack<Self>) -> Stack<Self> {
        let mut res = b.clone();
        Self::fundamental_inplace(a, &mut res);
        res
    }

    /// Computes the fundamental of multiple notes in place.
    fn fundamental_many_inplace<'a, I>(notes: I, res: &mut Stack<Self>)
    where
        I: Iterator<Item = &'a Stack<Self>>,
        Self: 'a,
    {
        for note in notes {
            Self::fundamental_inplace(note, res);
        }
    }

    /// Computes the fundamental of multiple notes.
    ///
    /// # Panics
    ///
    /// Panics if `notes` is empty.
    fn fundamental_many<'a, I>(mut notes: I) -> Stack<Self>
    where
        I: Iterator<Item = &'a Stack<Self>>,
        Self: 'a,
    {
        let mut res = notes.next().expect("fundamental_many: no notes").clone();
        Self::fundamental_many_inplace(notes, &mut res);
        res
    }
}

/// Computes the lowest common overtone of multiple stacks.
pub trait HasOvertone: IntervalBasis {
    fn overtone_inplace(a: &Stack<Self>, b: &mut Stack<Self>);

    fn overtone(a: &Stack<Self>, b: &Stack<Self>) -> Stack<Self> {
        let mut res = b.clone();
        Self::overtone_inplace(a, &mut res);
        res
    }

    fn overtone_many_inplace<'a, I>(notes: I, res: &mut Stack<Self>)
    where
        I: Iterator<Item = &'a Stack<Self>>,
        Self: 'a,
    {
        for note in notes {
            Self::overtone_inplace(note, res);
        }
    }

    /// Computes the overtone of multiple notes.
    ///
    /// # Panics
    ///
    /// Panics if `notes` is empty.
    fn overtone_many<'a, I>(mut notes: I) -> Stack<Self>
    where
        I: Iterator<Item = &'a Stack<Self>>,
        Self: 'a,
    {
        let mut res = notes.next().expect("overtone_many: no notes").clone();
        Self::overtone_many_inplace(notes, &mut res);
        res
    }
}

impl<T: FiveLimitIntervalBasis> HasFundamental for T {
    fn fundamental_inplace(a: &Stack<Self>, b: &mut Stack<Self>) {
        let twos = (b.target[Self::octave_index()]
            - b.target[Self::fifth_index()]
            - 2 * b.target[Self::third_index()])
        .min(
            a.target[Self::octave_index()]
                - a.target[Self::fifth_index()]
                - 2 * a.target[Self::third_index()],
        );
        let threes = b.target[Self::fifth_index()].min(a.target[Self::fifth_index()]);
        let fives = b.target[Self::third_index()].min(a.target[Self::third_index()]);

        b.increment_at_index_pure(
            Self::octave_index(),
            (twos + threes + 2 * fives) - b.target[Self::octave_index()],
        );
        b.increment_at_index_pure(Self::fifth_index(), threes - b.target[Self::fifth_index()]);
        b.increment_at_index_pure(Self::third_index(), fives - b.target[Self::third_index()]);
    }
}

impl<T: FiveLimitIntervalBasis> HasOvertone for T {
    fn overtone_inplace(a: &Stack<Self>, b: &mut Stack<Self>) {
        let twos = (b.target[Self::octave_index()]
            - b.target[Self::fifth_index()]
            - 2 * b.target[Self::third_index()])
        .max(
            a.target[Self::octave_index()]
                - a.target[Self::fifth_index()]
                - 2 * a.target[Self::third_index()],
        );
        let threes = b.target[Self::fifth_index()].max(a.target[Self::fifth_index()]);
        let fives = b.target[Self::third_index()].max(a.target[Self::third_index()]);

        b.increment_at_index_pure(
            Self::octave_index(),
            (twos + threes + 2 * fives) - b.target[Self::octave_index()],
        );
        b.increment_at_index_pure(Self::fifth_index(), threes - b.target[Self::fifth_index()]);
        b.increment_at_index_pure(Self::third_index(), fives - b.target[Self::third_index()]);
    }
}
