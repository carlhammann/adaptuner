use std::{marker::PhantomData, time::Instant};

use crate::{
    interval::{stack::Stack, stacktype::r#trait::StackType},
    keystate::KeyState,
    msg,
    neighbourhood::CompleteNeigbourhood,
    strategy::twostep::{AnchorStrategy, IntervalSolution},
};

pub struct AnchorFixed<T: StackType, N: CompleteNeigbourhood<T>> {
    _phantom: PhantomData<T>,
    neighbourhood: N,
}

impl<T: StackType, N: CompleteNeigbourhood<T>> AnchorStrategy<T> for AnchorFixed<T, N> {
    fn solve(
        &mut self,
        keys: &[KeyState; 128],
        tunings: &mut [Stack<T>; 128],
        intervals: IntervalSolution<T>,
        time: Instant,
    ) -> Option<Vec<crate::msg::FromStrategy<T>>> {
        let reference_stack = self.neighbourhood.get_relative_stack(intervals.reference as i8 - 60);
        todo!()
    }

    fn handle_msg(
        &mut self,
        keys: &[KeyState; 128],
        tunings: &mut [Stack<T>; 128],
        msg: msg::ToStrategy,
    ) -> Option<Vec<msg::FromStrategy<T>>> {
        None {}
        //vec![]
        //todo!()
    }
}
