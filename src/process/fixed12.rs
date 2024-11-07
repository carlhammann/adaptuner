use std::{ops::DerefMut, time::Instant};

use midi_msg::MidiMsg;

use crate::{
    interval::{stack::Stack, stacktype::r#trait::StackType},
    msg,
    notestore::TunedNoteStore,
    process::r#trait::ProcessState,
};

struct State<T: StackType> {
    /// Tunings for one octave, starting at middle C, i.e. C4. Stacks are relative to middle C.
    base_tunings: [Stack<T>; 12],
    octave: Stack<T>,
}

impl<T: StackType> ProcessState<T> for State<T> {
    fn handle_msg<TS>(&mut self, time: Instant, msg: msg::ToProcess, tuned_store: TS)
    where
        TS: DerefMut<Target = TunedNoteStore<T>>,
    {
        match msg {
            msg::ToProcess::Start => {}
            msg::ToProcess::Stop => {}
            msg::ToProcess::Reset => {}
            msg::ToProcess::IncomingMidi { bytes } => match MidiMsg::from_midi(&bytes) {
                Err(_) => {}
                Ok((m, _)) => match m {
                    MidiMsg::ChannelVoice { channel, msg } => {}
                    _ => {}
                },
            },
            msg::ToProcess::Consider { .. } => {}
            msg::ToProcess::ToggleTemperament { .. } => {}
            msg::ToProcess::Special { .. } => {}
        }
    }
}
