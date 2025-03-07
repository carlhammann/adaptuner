//use std::{sync::mpsc, time::Instant};
//
//use midi_msg::{ChannelVoiceMsg, ControlChange, MidiMsg};
//
//use crate::{
//    interval::{stack::Stack, stacktype::r#trait::StackType},
//    msg,
//    neighbourhood::CompleteNeigbourhood,
//    process::{r#trait::ProcessState, springs::solver::*},
//};
//
//pub struct State<T: StackType, N: CompleteNeigbourhood<T>> {
//    active_temperaments: Vec<bool>,
//    neighbourhood: N,
//    key_center: Stack<T>,
//    reference: Stack<T>,
//}
//
//impl<T: StackType, N: CompleteNeigbourhood<T>> ProcessState<T> for State<T, N> {
//    fn handle_msg(
//        &mut self,
//        time: Instant,
//        msg: msg::ToProcess,
//        to_backend: &mpsc::Sender<(Instant, msg::AfterProcess<T>)>,
//    ) {
//        match msg {
//            msg::ToProcess::Start => {}
//            msg::ToProcess::Stop => {}
//            msg::ToProcess::Reset => {}
//            msg::ToProcess::IncomingMidi { bytes } => match MidiMsg::from_midi(&bytes) {
//                Err(err) => send_to_backend(msg::AfterProcess::MidiParseErr(err.to_string()), time),
//                Ok((msg, _nbtyes)) => match msg {},
//            },
//
//            msg::ToProcess::Consider { coefficients: _ } => {}
//            msg::ToProcess::ToggleTemperament { index: _ } => {}
//            msg::ToProcess::Special { code: _ } => {}
//        }
//    }
//}
