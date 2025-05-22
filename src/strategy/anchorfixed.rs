use std::{marker::PhantomData, sync::mpsc, time::Instant};

use crate::{
    interval::{stack::Stack, stacktype::r#trait::StackType},
    keystate::KeyState,
    msg::{FromProcess, ToProcess, ToStrategy},
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
        forward: &mpsc::Sender<FromProcess<T>>,
    ) -> bool {
        todo!()
    }

    fn handle_msg(
        &mut self,
        keys: &[KeyState; 128],
        tunings: &mut [Stack<T>; 128],
        msg: ToStrategy<T>,
        forward: &mpsc::Sender<FromProcess<T>>,
    ) -> bool {
        todo!()
    }
}
