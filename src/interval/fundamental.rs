use crate::interval::{stack::Stack, stacktype::r#trait::StackType};

pub trait HasFundamental: StackType {
    /// Like [HasFundamental::fundamental], but mutating the second argument to become the
    /// fundamental.
    fn fundamental_inplace(a: &Stack<Self>, b: &mut Stack<Self>);

    /// Determines the fundamental of `a` and `b`, i.e. the [Stack] describing the hightest note
    /// that has both `a` and `b` as overtones.
    ///
    /// The fundamental is a "logical" property of the [Stack::target]s of the arguments: It only
    /// really makes sense to talk of the fundamental when both arguments are "in tune" in the
    /// sense of [Stack::is_target]. Otherwise, the aurally perceptible fundamental will be out of
    /// tune in relation to the two notes anyway.
    ///
    /// The only exception to this out-of-tune-ness of the fundamental is when both `a` and `b` are
    /// out of tune by the same amount. In this case, the returned [Stack] should obviously be the
    /// out of tune be that amount as well.
    ///
    /// In every other case, the the result should have at least `b` as an overtone.
    fn fundamental(a: &Stack<Self>, b: &Stack<Self>) -> Stack<Self> {
        let mut res = b.clone();
        Self::fundamental_inplace(a, &mut res);
        res
    }

    /// Like [HasFundamental::fundamental_many], but with an output argument that is mutated. The
    /// `res` argument will also be one of the overtones.
    fn fundamental_many_inplace<'a, I>(notes: I, res: &mut Stack<Self>)
    where
        I: Iterator<Item = &'a Stack<Self>>,
        Self: 'a,
    {
        for note in notes {
            Self::fundamental_inplace(note, res);
        }
    }

    /// Compute the fundamental of many notes. 
    ///
    /// Will panic if `notes` doesn't contain at least one element.
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
