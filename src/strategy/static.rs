use std::{marker::PhantomData, time::Instant};

use crate::{
    config::r#trait::Config,
    interval::{
        stack::Stack,
        stacktype::r#trait::{FiveLimitStackType, OctavePeriodicStackType, StackCoeff, StackType},
    },
    keystate::KeyState,
    msg,
    neighbourhood::{new_fivelimit_neighbourhood, CompleteNeigbourhood, PeriodicCompleteAligned},
    strategy::r#trait::Strategy,
};

pub struct StaticTuning<T: StackType, N: CompleteNeigbourhood<T>> {
    _phantom: PhantomData<T>,
    neighbourhood: N,
}

impl<T: StackType, N: CompleteNeigbourhood<T>> Strategy<T> for StaticTuning<T, N> {
    fn note_on<'a>(
        &mut self,
        _keys: &[KeyState; 128],
        tunings: &'a mut [Stack<T>; 128],
        note: u8,
        _time: Instant,
    ) -> Vec<msg::FromStrategy<T>> {
        self.neighbourhood.write_relative_stack(
            tunings
                .get_mut(note as usize)
                .expect("static strategy: note not in range 0..=127"),
            note as i8 - 60,
        );

        vec![msg::FromStrategy::Retune {
            note,
            tuning: tunings[note as usize].absolute_semitones(),
            tuning_stack: tunings[note as usize].clone(),
        }]
    }

    fn note_off<'a>(
        &mut self,
        _keys: &[KeyState; 128],
        _tunings: &'a mut [Stack<T>; 128],
        _note: &[u8],
        _time: Instant,
    ) -> Vec<msg::FromStrategy<T>> {
        vec![]
    }
}

pub struct StaticTuningConfig<T: FiveLimitStackType + OctavePeriodicStackType> {
    pub _phantom: PhantomData<T>,
    pub active_temperaments: Vec<bool>,
    pub width: StackCoeff,
    pub index: StackCoeff,
    pub offset: StackCoeff,
}

impl<T: FiveLimitStackType + OctavePeriodicStackType>
    Config<StaticTuning<T, PeriodicCompleteAligned<T>>> for StaticTuningConfig<T>
{
    fn initialise(config: &Self) -> StaticTuning<T, PeriodicCompleteAligned<T>> {
        StaticTuning {
            _phantom: PhantomData,
            neighbourhood: new_fivelimit_neighbourhood(
                &config.active_temperaments,
                config.width,
                config.index,
                config.offset,
            ),
        }
    }
}
