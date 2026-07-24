use std::time::{Duration, Instant};

use midi_msg::Channel;
use midir::{MidiInputPort, MidiOutputPort};

use crate::{
    bindable::BindableStrategyAction,
    interval::{base::Semitones, stack::Stack, stacktype::r#trait::StackType},
    util::list_action::ListAction,
};

pub trait ReceiveMsg<I> {
    fn receive_msg(&mut self, msg: I);
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
    Stop {
        time: Instant,
    },
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
    StrategyListAction {
        action: ListAction,
        time: Instant,
    },
    RestartFromConfig {
        time: Instant,
    },
}

pub enum FromProcess<T: StackType> {
    MidiParseErr(String),
    OutgoingMidi {
        bytes: Vec<u8>,
        time: Instant,
    },
    FromStrategy(FromStrategy<T>),
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

pub enum ToChordList {
    UpdateChordList { time: Instant },
    ToggleEnable { time: Instant },
}

pub enum ToStaticNeighbourhoods<T: StackType> {
    SelectScale {
        index: usize,
        time: Instant,
    },
    UpdateScales {
        only_this_scale: Option<usize>,
        time: Instant,
    },

    SetReference {
        reference: Stack<T>,
        time: Instant,
    },

    Consider {
        stack: Stack<T>,
        time: Instant,
    },
}

pub enum ToStaticNeighbourhoodsAsMelody<T: StackType> {
    SelectScale {
        index: usize,
        time: Instant,
    },
    UpdateScales {
        only_this_scale: Option<usize>,
        time: Instant,
    },

    SetReference {
        reference: Stack<T>,
        time: Instant,
    },

    Consider {
        stack: Stack<T>,
        time: Instant,
    },

    ToggleReanchor {
        time: Instant,
    },
    SetGroupMs {
        group_ms: u64,
    },
}

pub enum ToTwoStep<T: StackType> {
    ToHarmonyStrategy(ToHarmony),
    ToMelodyStrategy(ToMelody<T>),
}

pub enum ToMelody<T: StackType> {
    StaticNeighbourhoods(ToStaticNeighbourhoodsAsMelody<T>),
}

pub enum ToHarmony {
    ChordList(ToChordList),
}

pub enum ToStrategy<T: StackType> {
    Start {
        time: Instant,
    },
    Stop {
        time: Instant,
    },
    NoteOn {
        note: u8,
        time: Instant,
    },
    NoteOff {
        note: u8,
        time: Instant,
    },
    UpdateTuningReference {
        time: Instant,
    },

    TwoStep(ToTwoStep<T>),
    StaticNeighbourhoods(ToStaticNeighbourhoods<T>),

    BoundAction {
        action: BindableStrategyAction,
        time: Instant,
    },
}

pub enum FromStrategy<T: StackType> {
    Retune {
        note: u8,
        time: Instant,
    },
    UpdateReference {},
    SelectScale {
        index: usize,
    },
    CurrentHarmony {
        pattern_index: Option<usize>,
        reference: Option<Stack<T>>,
    },
    EnableChordList {
        enable: bool,
    },
    ReanchorOnMatch {
        reanchor: bool,
    },
    Consider {
        stack: Stack<T>,
    },
}

pub enum ToBackend {
    Start {
        time: Instant,
    },
    Reset {
        time: Instant,
    },
    Stop {
        time: Instant,
    },
    RestartFromConfig {
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
        /// i-th bit is set if the i-th channel should be used
        channels: u16,
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
    Notify {
        line: String,
    },
    NoteOn {
        channel: Channel,
        note: u8,
        time: Instant,
    },
    Retune {
        note: u8,
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
    UpdateReference {},
    SelectScale {
        index: usize,
    },
    DetunedNote {
        note: u8,
        should_be: Semitones,
        actual: Semitones,
        explanation: &'static str,
    },
    CurrentStrategyIndex(Option<usize>),
    CurrentHarmony {
        pattern_index: Option<usize>,
        reference: Option<Stack<T>>,
    },
    EnableChordList {
        enable: bool,
    },
    ReanchorOnMatch {
        reanchor: bool,
    },
    Consider {
        stack: Stack<T>,
    },
}

pub enum FromUi<T: StackType> {
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
    UpdateTuningReference {
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
        /// i-th bit is set if the i-th channel should be used.
        channels: u16,
        time: Instant,
    },
    Stop {
        time: Instant,
    },
    RestartFromConfig {
        time: Instant,
    },
    StrategyListAction {
        action: ListAction,
        time: Instant,
    },
    ToStrategy(ToStrategy<T>), // this should somehow be merged with/include the following messages
    UpdateChordList {
        time: Instant,
    },
    ToggleChordList {
        time: Instant,
    },
    ToggleReanchorOnMatch {
        time: Instant,
    },
    SetGroupMs {
        group_ms: u64,
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
            FromProcess::CurrentStrategyIndex(i) => {
                (None {}, None {}, Some(ToUi::CurrentStrategyIndex(i)))
            }
        }
    }
}

impl<T: StackType> MessageTranslate2<ToBackend, ToUi<T>> for FromStrategy<T> {
    fn translate2(self) -> (Option<ToBackend>, Option<ToUi<T>>) {
        match self {
            FromStrategy::Retune { note, time } => (
                Some(ToBackend::Retune { note, time }),
                Some(ToUi::Retune { note }),
            ),
            FromStrategy::UpdateReference {} => (None {}, Some(ToUi::UpdateReference {})),
            FromStrategy::SelectScale { index } => (None {}, Some(ToUi::SelectScale { index })),
            FromStrategy::CurrentHarmony {
                pattern_index,
                reference,
            } => (
                None {},
                Some(ToUi::CurrentHarmony {
                    pattern_index,
                    reference,
                }),
            ),
            FromStrategy::EnableChordList { enable } => {
                (None {}, Some(ToUi::EnableChordList { enable }))
            }
            FromStrategy::ReanchorOnMatch { reanchor } => {
                (None {}, Some(ToUi::ReanchorOnMatch { reanchor }))
            }
            FromStrategy::Consider { stack } => (None {}, Some(ToUi::Consider { stack })),
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
            FromUi::UpdateTuningReference { time } => (
                Some(ToProcess::ToStrategy(ToStrategy::UpdateTuningReference {
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
                Some(ToProcess::ToStrategy(ToStrategy::StaticNeighbourhoods(
                    ToStaticNeighbourhoods::SetReference { reference, time },
                ))),
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
            FromUi::StrategyListAction { action, time } => (
                Some(ToProcess::StrategyListAction { action, time }),
                None {},
                None {},
                None {},
            ),
            FromUi::ToStrategy(action) => (
                Some(ToProcess::ToStrategy(action)),
                None {},
                None {},
                None {},
            ),
            FromUi::Stop { time } => (
                Some(ToProcess::Stop { time }),
                Some(ToBackend::Stop { time }),
                None {},
                None {},
            ),
            FromUi::RestartFromConfig { time } => (
                Some(ToProcess::RestartFromConfig { time }),
                Some(ToBackend::RestartFromConfig { time }),
                None {},
                None {},
            ),
            FromUi::UpdateChordList { time } => (
                Some(ToProcess::ToStrategy(ToStrategy::TwoStep(
                    ToTwoStep::ToHarmonyStrategy(ToHarmony::ChordList(
                        ToChordList::UpdateChordList { time },
                    )),
                ))),
                None {},
                None {},
                None {},
            ),
            FromUi::ToggleChordList { time } => (
                Some(ToProcess::ToStrategy(ToStrategy::TwoStep(
                    ToTwoStep::ToHarmonyStrategy(ToHarmony::ChordList(ToChordList::ToggleEnable {
                        time,
                    })),
                ))),
                None {},
                None {},
                None {},
            ),
            FromUi::ToggleReanchorOnMatch { time } => (
                Some(ToProcess::ToStrategy(ToStrategy::TwoStep(
                    ToTwoStep::ToMelodyStrategy(ToMelody::StaticNeighbourhoods(
                        ToStaticNeighbourhoodsAsMelody::ToggleReanchor { time },
                    )),
                ))),
                None {},
                None {},
                None {},
            ),
            FromUi::SetGroupMs { group_ms } => (
                Some(ToProcess::ToStrategy(ToStrategy::TwoStep(
                    ToTwoStep::ToMelodyStrategy(ToMelody::StaticNeighbourhoods(
                        ToStaticNeighbourhoodsAsMelody::SetGroupMs { group_ms },
                    )),
                ))),
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
