use std::{
    sync::mpsc,
    time::{Duration, Instant},
};

use midi_msg::MidiMsg;
use midir::{MidiInputPort, MidiOutputPort};

use crate::interval::{
    base::Semitones,
    stack::Stack,
    stacktype::r#trait::{StackCoeff, StackType},
};

pub trait HandleMsg<I, O> {
    fn handle_msg(&mut self, msg: I, forward: &mpsc::Sender<O>);
}

pub trait HandleMsgRef<I, O> {
    fn handle_msg_ref(&mut self, msg: &I, forward: &mpsc::Sender<O>);
}

pub trait HasStart {
    fn is_start(&self) -> bool;
    fn mk_start() -> Self;
}

/// Convention: the handler wil handle a 'stop' message, and immediately after that the thread will exit.
pub trait HasStop {
    fn is_stop(&self) -> bool;
    fn mk_stop() -> Self;
}

pub trait MessageTranslate<B> {
    fn translate(self) -> Option<B>;
}

pub trait MessageTranslate2<B, C> {
    fn translate2(self) -> (Option<B>, Option<C>);
}

pub trait MessageTranslate4<B, C, D, E> {
    fn translate4(self) -> (Option<B>, Option<C>, Option<D>, Option<E>);
}

pub enum ToProcess {
    Stop,
    IncomingMidi { time: Instant, bytes: Vec<u8> },
    FromUi(FromUi),
    ToStrategy(ToStrategy),
}

pub enum FromProcess<T: StackType> {
    Notify {
        line: String,
    },
    MidiParseErr(String),
    ForwardMidi {
        msg: MidiMsg,
        time: Instant,
    },
    FromStrategy(FromStrategy<T>),
}

pub enum ToStrategy {
    Consider {
        coefficients: Vec<StackCoeff>,
        time: Instant,
    },
    ToggleTemperament {
        index: usize,
        time: Instant,
    },
}

pub enum FromStrategy<T: StackType> {
    Retune {
        note: u8,
        tuning: Semitones,
        tuning_stack: Stack<T>,
        time: Instant,
    },
    SetReference {
        key: u8,
        stack: Stack<T>,
    },
    Consider {
        stack: Stack<T>,
    },
    NotifyFit {
        pattern_name: String,
        reference_stack: Stack<T>,
    },
    NotifyNoFit,
}

pub enum ToBackend {
    Start {
        time: Instant,
    },
    Reset {
        time: Instant,
    },
    Stop,
    ForwardMidi {
        msg: MidiMsg,
        time: Instant,
    },
    Retune {
        note: u8,
        tuning: Semitones,
        time: Instant,
    },
}

pub enum FromBackend {
    OutgoingMidi {
        time: Instant,
        bytes: Vec<u8>,
    },
    DetunedNote {
        note: u8,
        should_be: Semitones,
        actual: Semitones,
        explanation: &'static str,
    },
}

pub enum ToUi<T: StackType> {
    Stop,
    Notify {
        line: String,
    },
    ForwardMidi {
        time: Instant,
        msg: MidiMsg,
    },
    Retune {
        note: u8,
        tuning_stack: Stack<T>,
    },
    EventLatency {
        since_input: Duration,
    },
    InputConnectionError {
        reason: String,
    },
    InputConnected {
        portname: String,
    },
    InputDisconnected {
        available_ports: Vec<(MidiInputPort, String)>,
    },
    OutputConnectionError {
        reason: String,
    },
    OutputConnected {
        portname: String,
    },
    OutputDisconnected {
        available_ports: Vec<(MidiOutputPort, String)>,
    },
    NotifyFit {
        pattern_name: String,
        reference_stack: Stack<T>,
    },
    NotifyNoFit,
    SetReference {
        key: u8,
        stack: Stack<T>,
    },
    Consider {
        stack: Stack<T>,
    },
    DetunedNote {
        note: u8,
        should_be: Semitones,
        actual: Semitones,
        explanation: &'static str,
    },
}

pub enum FromUi {
    Consider {
        coefficients: Vec<StackCoeff>,
        time: Instant,
    },
    ToggleTemperament {
        index: usize,
        time: Instant,
    },
    DisconnectInput,
    ConnectInput {
        port: MidiInputPort,
        portname: String,
    },
    DisconnectOutput,
    ConnectOutput {
        port: MidiOutputPort,
        portname: String,
    },
}

pub enum ToMidiIn {
    Connect {
        port: MidiInputPort,
        portname: String,
    },
    Disconnect,
    Start,
    Stop,
}

pub enum FromMidiIn {
    IncomingMidi {
        time: Instant,
        bytes: Vec<u8>,
    },
    ConnectionError {
        reason: String,
    },
    Connected {
        portname: String,
    },
    Disconnected {
        available_ports: Vec<(MidiInputPort, String)>,
    },
}

pub enum ToMidiOut {
    OutgoingMidi {
        time: Instant,
        bytes: Vec<u8>,
    },
    Connect {
        port: MidiOutputPort,
        portname: String,
    },
    Disconnect,
    Start,
    Stop,
}

pub enum FromMidiOut {
    EventLatency {
        since_input: Duration,
    },
    ConnectionError {
        reason: String,
    },
    Connected {
        portname: String,
    },
    Disconnected {
        available_ports: Vec<(MidiOutputPort, String)>,
    },
}

impl<T: StackType> MessageTranslate2<ToBackend, ToUi<T>> for FromProcess<T> {
    fn translate2(self) -> (Option<ToBackend>, Option<ToUi<T>>) {
        match self {
            FromProcess::Notify { line } => (None {}, Some(ToUi::Notify { line })),
            FromProcess::MidiParseErr(err) => (
                None {},
                Some(ToUi::Notify {
                    line: err.to_string(),
                }),
            ),
            FromProcess::ForwardMidi { msg, time: original_time } => (
                Some(ToBackend::ForwardMidi {
                    msg: msg.clone(),
                    time: original_time,
                }),
                Some(ToUi::ForwardMidi { time: original_time, msg }),
            ),
            FromProcess::FromStrategy(msg) => msg.translate2(),
        }
    }
}

impl<T: StackType> MessageTranslate2<ToBackend, ToUi<T>> for FromStrategy<T> {
    fn translate2(self) -> (Option<ToBackend>, Option<ToUi<T>>) {
        match self {
            FromStrategy::Retune {
                note,
                tuning,
                tuning_stack,
                time,
            } => (
                Some(ToBackend::Retune { note, tuning, time }),
                Some(ToUi::Retune { note, tuning_stack }),
            ),
            FromStrategy::SetReference { key, stack } => {
                (None {}, Some(ToUi::SetReference { key, stack }))
            }
            FromStrategy::Consider { stack } => (None {}, Some(ToUi::Consider { stack })),
            FromStrategy::NotifyFit {
                pattern_name,
                reference_stack,
            } => (
                None {},
                Some(ToUi::NotifyFit {
                    pattern_name,
                    reference_stack,
                }),
            ),
            FromStrategy::NotifyNoFit => (None {}, Some(ToUi::NotifyNoFit)),
        }
    }
}

impl MessageTranslate4<ToProcess, ToBackend, ToMidiIn, ToMidiOut> for FromUi {
    fn translate4(
        self,
    ) -> (
        Option<ToProcess>,
        Option<ToBackend>,
        Option<ToMidiIn>,
        Option<ToMidiOut>,
    ) {
        match self {
            FromUi::Consider { coefficients, time } => (
                Some(ToProcess::ToStrategy(ToStrategy::Consider {
                    coefficients,
                    time,
                })),
                None {},
                None {},
                None {},
            ),
            FromUi::ToggleTemperament { index, time } => (
                Some(ToProcess::ToStrategy(ToStrategy::ToggleTemperament {
                    index,
                    time,
                })),
                None {},
                None {},
                None {},
            ),
            FromUi::DisconnectInput => (None {}, None {}, Some(ToMidiIn::Disconnect), None {}),
            FromUi::ConnectInput { port, portname } => (
                None {},
                None {},
                Some(ToMidiIn::Connect { port, portname }),
                None {},
            ),
            FromUi::DisconnectOutput => (None {}, None {}, None {}, Some(ToMidiOut::Disconnect)),
            FromUi::ConnectOutput { port, portname } => (
                None {},
                None {},
                None {},
                Some(ToMidiOut::Connect { port, portname }),
            ),
        }
    }
}

impl<T: StackType> MessageTranslate2<ToProcess, ToUi<T>> for FromMidiIn {
    fn translate2(self) -> (Option<ToProcess>, Option<ToUi<T>>) {
        match self {
            FromMidiIn::IncomingMidi { time, bytes } => {
                (Some(ToProcess::IncomingMidi { time, bytes }), None {})
            }
            FromMidiIn::ConnectionError { reason } => {
                (None {}, Some(ToUi::InputConnectionError { reason }))
            }
            FromMidiIn::Connected { portname } => {
                (None {}, Some(ToUi::InputConnected { portname }))
            }
            FromMidiIn::Disconnected { available_ports } => {
                (None {}, Some(ToUi::InputDisconnected { available_ports }))
            }
        }
    }
}

impl<T: StackType> MessageTranslate<ToUi<T>> for FromMidiOut {
    fn translate(self) -> Option<ToUi<T>> {
        match self {
            FromMidiOut::EventLatency { since_input } => Some(ToUi::EventLatency { since_input }),
            FromMidiOut::ConnectionError { reason } => Some(ToUi::OutputConnectionError { reason }),
            FromMidiOut::Connected { portname } => Some(ToUi::OutputConnected { portname }),
            FromMidiOut::Disconnected { available_ports } => {
                Some(ToUi::OutputDisconnected { available_ports })
            }
        }
    }
}

impl<T: StackType> MessageTranslate2<ToMidiOut, ToUi<T>> for FromBackend {
    fn translate2(self) -> (Option<ToMidiOut>, Option<ToUi<T>>) {
        match self {
            FromBackend::OutgoingMidi {
                time: original_time,
                bytes,
            } => (
                Some(ToMidiOut::OutgoingMidi {
                    time: original_time,
                    bytes,
                }),
                None {},
            ),
            FromBackend::DetunedNote {
                note,
                should_be,
                actual,
                explanation,
            } => (
                None {},
                Some(ToUi::DetunedNote {
                    note,
                    should_be,
                    actual,
                    explanation,
                }),
            ),
        }
    }
}

impl HasStop for ToProcess {
    fn is_stop(&self) -> bool {
        match self {
            Self::Stop => true,
            _ => false,
        }
    }
    fn mk_stop() -> Self {
        Self::Stop
    }
}

impl HasStop for ToBackend {
    fn is_stop(&self) -> bool {
        match self {
            Self::Stop => true,
            _ => false,
        }
    }
    fn mk_stop() -> Self {
        Self::Stop
    }
}

impl<T: StackType> HasStop for ToUi<T> {
    fn is_stop(&self) -> bool {
        match self {
            Self::Stop => true,
            _ => false,
        }
    }
    fn mk_stop() -> Self {
        Self::Stop
    }
}

impl HasStop for ToMidiIn {
    fn is_stop(&self) -> bool {
        match self {
            Self::Stop => true,
            _ => false,
        }
    }
    fn mk_stop() -> Self {
        Self::Stop
    }
}

impl HasStop for ToMidiOut {
    fn is_stop(&self) -> bool {
        match self {
            Self::Stop => true,
            _ => false,
        }
    }
    fn mk_stop() -> Self {
        Self::Stop
    }
}
