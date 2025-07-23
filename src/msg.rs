use std::{
    sync::mpsc,
    time::{Duration, Instant},
};

use midi_msg::Channel;
use midir::{MidiInputPort, MidiOutputPort};

use crate::{
    bindable::Bindable,
    config::ExtendedStrategyConfig,
    interval::{base::Semitones, stack::Stack, stacktype::r#trait::StackType},
    reference::Reference,
    strategy::r#trait::StrategyAction,
    util::list_action::ListAction,
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

pub trait MessageTranslate3<B, C, D> {
    fn translate3(self) -> (Option<B>, Option<C>, Option<D>);
}

pub trait MessageTranslate4<B, C, D, E> {
    fn translate4(self) -> (Option<B>, Option<C>, Option<D>, Option<E>);
}

pub enum ToProcess<T: StackType> {
    Stop,
    Start {
        time: Instant,
    },
    Reset {
        time: Instant,
    },
    IncomingMidi {
        time: Instant,
        bytes: Vec<u8>,
    },
    ToStrategy(ToStrategy<T>),
    NoteOn {
        channel: Channel,
        note: u8,
        velocity: u8,
        time: Instant,
    },
    NoteOff {
        channel: Channel,
        note: u8,
        velocity: u8,
        time: Instant,
    },
    PedalHold {
        channel: Channel,
        value: u8,
        time: Instant,
    },
    BindAction {
        action: Option<StrategyAction>,
        bindable: Bindable,
    },
    StrategyListAction {
        action: ListAction<ExtendedStrategyConfig<T>>,
        time: Instant,
    },
}

pub enum FromProcess<T: StackType> {
    Notify {
        line: String,
    },
    MidiParseErr(String),
    OutgoingMidi {
        bytes: Vec<u8>,
        time: Instant,
    },
    FromStrategy(FromStrategy<T>),
    TunedNoteOn {
        channel: Channel,
        note: u8,
        velocity: u8,
        tuning: Semitones,
        tuning_stack: Stack<T>,
        time: Instant,
    },
    NoteOn {
        channel: Channel,
        note: u8,
        velocity: u8,
        time: Instant,
    },
    NoteOff {
        channel: Channel,
        note: u8,
        velocity: u8,
        time: Instant,
    },
    PedalHold {
        channel: Channel,
        value: u8,
        time: Instant,
    },
    ProgramChange {
        channel: Channel,
        program: u8,
        time: Instant,
    },
    CurrentStrategyIndex(Option<usize>),
}

pub enum ToStrategy<T: StackType> {
    Consider {
        stack: Stack<T>,
        time: Instant,
    },
    ApplyTemperamentToCurrentNeighbourhood {
        temperament: usize,
        time: Instant,
    },
    MakeCurrentNeighbourhoodPure {
        time: Instant,
    },
    NewNeighbourhood {
        name: String,
    },
    DeleteCurrentNeighbourhood {
        time: Instant,
    },
    SetTuningReference {
        reference: Reference<T>,
        time: Instant,
    },
    SetReference {
        reference: Stack<T>,
        time: Instant,
    },
    Action {
        action: StrategyAction,
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
        stack: Stack<T>,
    },
    SetTuningReference {
        reference: Reference<T>,
    },
    Consider {
        stack: Stack<T>,
    },
    CurrentNeighbourhoodName {
        index: usize,
        n_neighbourhoods: usize,
        name: String,
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
    TunedNoteOn {
        channel: Channel,
        note: u8,
        velocity: u8,
        tuning: Semitones,
        time: Instant,
    },
    NoteOn {
        channel: Channel,
        note: u8,
        velocity: u8,
        time: Instant,
    },
    Retune {
        note: u8,
        tuning: Semitones,
        time: Instant,
    },
    NoteOff {
        channel: Channel,
        note: u8,
        velocity: u8,
        time: Instant,
    },
    PedalHold {
        channel: Channel,
        value: u8,
        time: Instant,
    },
    ProgramChange {
        channel: Channel,
        program: u8,
        time: Instant,
    },
    BendRange {
        range: Semitones,
        time: Instant,
    },
    ChannelsToUse {
        channels: [bool; 16],
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
    TunedNoteOn {
        channel: Channel,
        note: u8,
        tuning_stack: Stack<T>,
        time: Instant,
    },
    NoteOn {
        channel: Channel,
        note: u8,
        time: Instant,
    },
    Retune {
        note: u8,
        tuning_stack: Stack<T>,
    },
    NoteOff {
        channel: Channel,
        note: u8,
        time: Instant,
    },
    PedalHold {
        channel: Channel,
        value: u8,
        time: Instant,
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
        stack: Stack<T>,
    },
    SetTuningReference {
        reference: Reference<T>,
    },
    Consider {
        stack: Stack<T>,
    },
    CurrentNeighbourhoodName {
        index: usize,
        n_neighbourhoods: usize,
        name: String,
    },
    DetunedNote {
        note: u8,
        should_be: Semitones,
        actual: Semitones,
        explanation: &'static str,
    },
    CurrentStrategyIndex(Option<usize>),
}

pub enum FromUi<T: StackType> {
    Consider {
        stack: Stack<T>,
        time: Instant,
    },
    DeleteCurrentNeighbourhood {
        time: Instant,
    },
    NewNeighbourhood {
        name: String,
    },
    ApplyTemperamentToCurrentNeighbourhood {
        temperament: usize,
        time: Instant,
    },
    MakeCurrentNeighbourhoodPure {
        time: Instant,
    },
    DisconnectInput,
    ConnectInput {
        port: MidiInputPort,
        portname: String,
        time: Instant,
    },
    DisconnectOutput,
    ConnectOutput {
        port: MidiOutputPort,
        portname: String,
        time: Instant,
    },
    SetTuningReference {
        reference: Reference<T>,
        time: Instant,
    },
    SetReference {
        reference: Stack<T>,
        time: Instant,
    },
    NoteOn {
        channel: Channel,
        note: u8,
        velocity: u8,
        time: Instant,
    },
    NoteOff {
        channel: Channel,
        note: u8,
        velocity: u8,
        time: Instant,
    },
    PedalHold {
        channel: Channel,
        value: u8,
        time: Instant,
    },
    BendRange {
        range: Semitones,
        time: Instant,
    },
    ChannelsToUse {
        channels: [bool; 16],
        time: Instant,
    },
    StrategyListAction {
        action: ListAction<ExtendedStrategyConfig<T>>,
        time: Instant,
    },
    Action {
        action: StrategyAction,
        time: Instant,
    },
    BindAction {
        action: Option<StrategyAction>,
        bindable: Bindable,
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

impl<T: StackType> MessageTranslate3<ToBackend, ToMidiOut, ToUi<T>> for FromProcess<T> {
    fn translate3(self) -> (Option<ToBackend>, Option<ToMidiOut>, Option<ToUi<T>>) {
        match self {
            FromProcess::Notify { line } => (None {}, None {}, Some(ToUi::Notify { line })),
            FromProcess::MidiParseErr(err) => (
                None {},
                None {},
                Some(ToUi::Notify {
                    line: err.to_string(),
                }),
            ),
            FromProcess::OutgoingMidi { bytes, time } => (
                None {},
                Some(ToMidiOut::OutgoingMidi { time, bytes }),
                None {},
            ),
            FromProcess::TunedNoteOn {
                channel,
                note,
                velocity,
                tuning,
                tuning_stack,
                time,
            } => (
                Some(ToBackend::TunedNoteOn {
                    channel,
                    note,
                    velocity,
                    tuning,
                    time,
                }),
                None {},
                Some(ToUi::TunedNoteOn {
                    channel,
                    note,
                    tuning_stack,
                    time,
                }),
            ),
            FromProcess::FromStrategy(msg) => {
                let (to_backend, to_ui) = msg.translate2();
                (to_backend, None {}, to_ui)
            }
            FromProcess::NoteOn {
                channel,
                note,
                velocity,
                time,
            } => (
                Some(ToBackend::NoteOn {
                    channel,
                    note,
                    velocity,
                    time,
                }),
                None {},
                Some(ToUi::NoteOn {
                    channel,
                    time,
                    note,
                }),
            ),
            FromProcess::NoteOff {
                channel,
                note,
                velocity,
                time,
            } => (
                Some(ToBackend::NoteOff {
                    channel,
                    note,
                    velocity,
                    time,
                }),
                None {},
                Some(ToUi::NoteOff {
                    time,
                    channel,
                    note,
                }),
            ),
            FromProcess::PedalHold {
                value,
                time,
                channel,
            } => (
                Some(ToBackend::PedalHold {
                    channel,
                    value,
                    time,
                }),
                None {},
                Some(ToUi::PedalHold {
                    channel,
                    value,
                    time,
                }),
            ),
            FromProcess::ProgramChange {
                channel,
                program,
                time,
            } => (
                Some(ToBackend::ProgramChange {
                    channel,
                    program,
                    time,
                }),
                None {},
                None {},
            ),
            FromProcess::CurrentStrategyIndex(i) => (
                None {},
                None {},
                Some(ToUi::CurrentStrategyIndex(i))
            )
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
            FromStrategy::SetReference { stack } => (None {}, Some(ToUi::SetReference { stack })),
            FromStrategy::Consider { stack } => (None {}, Some(ToUi::Consider { stack })),
            FromStrategy::CurrentNeighbourhoodName {
                index,
                n_neighbourhoods,
                name,
            } => (
                None {},
                Some(ToUi::CurrentNeighbourhoodName {
                    index,
                    n_neighbourhoods,
                    name,
                }),
            ),
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
            FromStrategy::SetTuningReference { reference } => {
                (None {}, Some(ToUi::SetTuningReference { reference }))
            }
        }
    }
}

impl<T: StackType> MessageTranslate4<ToProcess<T>, ToBackend, ToMidiIn, ToMidiOut> for FromUi<T> {
    fn translate4(
        self,
    ) -> (
        Option<ToProcess<T>>,
        Option<ToBackend>,
        Option<ToMidiIn>,
        Option<ToMidiOut>,
    ) {
        match self {
            FromUi::Consider { stack, time } => (
                Some(ToProcess::ToStrategy(ToStrategy::Consider { stack, time })),
                None {},
                None {},
                None {},
            ),
            FromUi::NewNeighbourhood { name } => (
                Some(ToProcess::ToStrategy(ToStrategy::NewNeighbourhood { name })),
                None {},
                None {},
                None {},
            ),
            FromUi::DeleteCurrentNeighbourhood { time } => (
                Some(ToProcess::ToStrategy(
                    ToStrategy::DeleteCurrentNeighbourhood { time },
                )),
                None {},
                None {},
                None {},
            ),
            FromUi::ApplyTemperamentToCurrentNeighbourhood { temperament, time } => (
                Some(ToProcess::ToStrategy(
                    ToStrategy::ApplyTemperamentToCurrentNeighbourhood { temperament, time },
                )),
                None {},
                None {},
                None {},
            ),
            FromUi::DisconnectInput => (None {}, None {}, Some(ToMidiIn::Disconnect), None {}),
            FromUi::ConnectInput {
                port,
                portname,
                time,
            } => (
                Some(ToProcess::Reset { time }),
                Some(ToBackend::Reset { time }),
                Some(ToMidiIn::Connect { port, portname }),
                None {},
            ),
            FromUi::DisconnectOutput => (None {}, None {}, None {}, Some(ToMidiOut::Disconnect)),
            FromUi::ConnectOutput {
                port,
                portname,
                time,
            } => (
                Some(ToProcess::Reset { time }),
                Some(ToBackend::Reset { time }),
                None {},
                Some(ToMidiOut::Connect { port, portname }),
            ),
            FromUi::SetTuningReference { reference, time } => (
                Some(ToProcess::ToStrategy(ToStrategy::SetTuningReference {
                    reference,
                    time,
                })),
                None {},
                None {},
                None {},
            ),
            FromUi::NoteOn {
                channel,
                note,
                velocity,
                time,
            } => (
                Some(ToProcess::NoteOn {
                    channel,
                    note,
                    velocity,
                    time,
                }),
                None {},
                None {},
                None {},
            ),
            FromUi::NoteOff {
                channel,
                note,
                velocity,
                time,
            } => (
                Some(ToProcess::NoteOff {
                    channel,
                    note,
                    velocity,
                    time,
                }),
                None {},
                None {},
                None {},
            ),
            FromUi::PedalHold {
                channel,
                value,
                time,
            } => (
                Some(ToProcess::PedalHold {
                    channel,
                    value,
                    time,
                }),
                None {},
                None {},
                None {},
            ),
            FromUi::SetReference { reference, time } => (
                Some(ToProcess::ToStrategy(ToStrategy::SetReference {
                    reference,
                    time,
                })),
                None {},
                None {},
                None {},
            ),
            FromUi::BendRange { range, time } => (
                None {},
                Some(ToBackend::BendRange { range, time }),
                None {},
                None {},
            ),
            FromUi::ChannelsToUse { channels, time } => (
                None {},
                Some(ToBackend::ChannelsToUse { channels, time }),
                None {},
                None {},
            ),
            FromUi::Action { action, time } => (
                Some(ToProcess::ToStrategy(ToStrategy::Action { action, time })),
                None {},
                None {},
                None {},
            ),
            FromUi::MakeCurrentNeighbourhoodPure { time } => (
                Some(ToProcess::ToStrategy(
                    ToStrategy::MakeCurrentNeighbourhoodPure { time },
                )),
                None {},
                None {},
                None {},
            ),
            FromUi::BindAction { action, bindable } => (
                Some(ToProcess::BindAction { action, bindable }),
                None {},
                None {},
                None {},
            ),
            FromUi::StrategyListAction { action, time } => (
                Some(ToProcess::StrategyListAction { action, time }),
                None {},
                None {},
                None {},
            ),
        }
    }
}

impl<T: StackType> MessageTranslate2<ToProcess<T>, ToUi<T>> for FromMidiIn {
    fn translate2(self) -> (Option<ToProcess<T>>, Option<ToUi<T>>) {
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

impl<T: StackType> HasStop for ToProcess<T> {
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
