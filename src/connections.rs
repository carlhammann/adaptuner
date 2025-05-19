use std::{sync::mpsc, thread, time::Instant};

use midir::{
    MidiIO, MidiInput, MidiInputConnection, MidiInputPort, MidiOutput, MidiOutputConnection,
    MidiOutputPort,
};

use crate::msg;

pub trait MaybeConnected<IO: MidiIO> {
    fn unconnected(&self) -> Option<&IO>;
    fn connected_port_name(&self) -> Option<&str>;
    fn connect(self, port: IO::Port, portname: &str) -> Result<Self, (String, Self)>
    where
        Self: Sized;
    fn disconnect(self) -> Self;
}

pub enum MidiInputOrConnection {
    Unconnected {
        midi_input: MidiInput,
        tx: mpsc::Sender<(Instant, msg::ToProcess)>,
    },
    Connected {
        connection: MidiInputConnection<()>,
        tx: mpsc::Sender<(Instant, msg::ToProcess)>,
        portname: String,
    },
}

impl MidiInputOrConnection {
    pub fn new(midi_input: MidiInput) -> (Self, mpsc::Receiver<(Instant, msg::ToProcess)>) {
        let (tx, rx) = mpsc::channel();
        (Self::Unconnected { midi_input, tx }, rx)
    }

    /// Use this only for data that will never be read!
    pub fn empty_placeholder() -> Self {
        let (tx, _rx) = mpsc::channel();
        Self::Unconnected {
            midi_input: midir::MidiInput::new("adaptuner placeholder input").unwrap(),
            tx,
        }
    }

    pub fn get_sender(&self) -> mpsc::Sender<(Instant, msg::ToProcess)> {
        match self {
            Self::Unconnected { tx, .. } => tx.clone(),
            Self::Connected { tx, .. } => tx.clone(),
        }
    }

    fn connect_internal(self, port: MidiInputPort, portname: &str) -> Result<Self, (String, Self)> {
        match self {
            Self::Unconnected { midi_input, tx } => {
                let txclone = tx.clone();
                match midi_input.connect(
                    &port,
                    portname,
                    move |_, bytes, _| {
                        let time = Instant::now();
                        txclone
                            .send((
                                time,
                                msg::ToProcess::IncomingMidi {
                                    bytes: bytes.to_vec(),
                                },
                            ))
                            .unwrap_or(());
                    },
                    (),
                ) {
                    Ok(connection) => Ok(Self::Connected {
                        connection,
                        tx,
                        portname: portname.to_string(),
                    }),
                    Err(err) => {
                        let err_string = err.to_string();
                        Err((
                            err_string,
                            Self::Unconnected {
                                midi_input: err.into_inner(),
                                tx,
                            },
                        ))
                    }
                }
            }
            Self::Connected { .. } => unreachable!(),
        }
    }
}

impl MaybeConnected<MidiInput> for MidiInputOrConnection {
    fn unconnected(&self) -> Option<&MidiInput> {
        match self {
            Self::Unconnected { midi_input, .. } => Some(midi_input),
            Self::Connected { .. } => None {},
        }
    }

    fn connected_port_name(&self) -> Option<&str> {
        match self {
            Self::Unconnected { .. } => None {},
            Self::Connected { portname, .. } => Some(portname),
        }
    }

    fn connect(self, port: MidiInputPort, portname: &str) -> Result<Self, (String, Self)> {
        match self {
            Self::Unconnected { .. } => self.connect_internal(port, portname),
            Self::Connected { .. } => {
                let disconnected = self.disconnect();
                disconnected.connect_internal(port, portname)
            }
        }
    }

    fn disconnect(self) -> Self {
        match self {
            Self::Connected { connection, tx, .. } => Self::Unconnected {
                midi_input: connection.close().0,
                tx,
            },
            Self::Unconnected { .. } => self,
        }
    }
}

pub enum MidiOutputOrConnection {
    Unconnected {
        midi_output: MidiOutput,
        rx: mpsc::Receiver<(Instant, msg::ToMidiOut)>,
        tx: mpsc::Sender<(Instant, msg::ToMidiOut)>,
        latency_tx: mpsc::Sender<msg::FromMidiOut>,
    },
    Connected {
        joinhandle: thread::JoinHandle<(
            MidiOutputConnection,
            mpsc::Receiver<(Instant, msg::ToMidiOut)>,
            mpsc::Sender<msg::FromMidiOut>,
        )>,
        tx: mpsc::Sender<(Instant, msg::ToMidiOut)>,
        portname: String,
    },
}

impl MidiOutputOrConnection {
    pub fn new(midi_output: MidiOutput) -> (Self, mpsc::Receiver<msg::FromMidiOut>) {
        let (tx, rx) = mpsc::channel();
        let (latency_tx, latency_rx) = mpsc::channel();
        (
            Self::Unconnected {
                midi_output,
                rx,
                tx,
                latency_tx,
            },
            latency_rx,
        )
    }

    /// Use this only for data that will never be read!
    pub fn empty_placeholder() -> Self {
        let (tx, rx) = mpsc::channel();
        let (latency_tx, _latency_rx) = mpsc::channel();
        Self::Unconnected {
            midi_output: midir::MidiOutput::new("adaptuner placeholder output").unwrap(),
            rx,
            tx,
            latency_tx,
        }
    }

    pub fn get_sender(&self) -> mpsc::Sender<(Instant, msg::ToMidiOut)> {
        match self {
            Self::Unconnected { tx, .. } => tx.clone(),
            Self::Connected { tx, .. } => tx.clone(),
        }
    }

    fn connect_internal(
        self,
        port: MidiOutputPort,
        portname: &str,
    ) -> Result<Self, (String, Self)> {
        match self {
            Self::Unconnected {
                midi_output,
                rx,
                tx,
                latency_tx,
            } => match midi_output.connect(&port, portname) {
                Ok(mut connection) => {
                    let joinhandle = thread::spawn(move || {
                        loop {
                            match rx.recv() {
                                Ok((_, msg::ToMidiOut::Stop)) => break,
                                Ok((original_time, msg::ToMidiOut::OutgoingMidi { bytes })) => {
                                    connection.send(&bytes).unwrap_or(());
                                    let time = Instant::now();
                                    latency_tx
                                        .send(msg::FromMidiOut::EventLatency {
                                            since_input: time.duration_since(original_time),
                                        })
                                        .unwrap_or(());
                                }
                                Err(_) => break,
                            }
                        }
                        (connection, rx, latency_tx)
                    });
                    Ok(Self::Connected {
                        joinhandle,
                        tx,
                        portname: portname.to_string(),
                    })
                }
                Err(err) => {
                    let err_string = err.to_string();
                    Err((
                        err_string,
                        Self::Unconnected {
                            midi_output: err.into_inner(),
                            rx,
                            tx,
                            latency_tx,
                        },
                    ))
                }
            },
            Self::Connected { .. } => unreachable!(),
        }
    }
}

impl MaybeConnected<MidiOutput> for MidiOutputOrConnection {
    fn connected_port_name(&self) -> Option<&str> {
        match self {
            Self::Unconnected { .. } => None {},
            Self::Connected { portname, .. } => Some(portname),
        }
    }

    fn unconnected(&self) -> Option<&MidiOutput> {
        match self {
            Self::Unconnected { midi_output, .. } => Some(midi_output),
            Self::Connected { .. } => None {},
        }
    }

    fn connect(self, port: MidiOutputPort, portname: &str) -> Result<Self, (String, Self)> {
        match self {
            Self::Unconnected { .. } => self.connect_internal(port, portname),
            Self::Connected { .. } => {
                let disconnected = self.disconnect();
                disconnected.connect_internal(port, portname)
            }
        }
    }

    fn disconnect(self) -> Self {
        match self {
            Self::Connected { joinhandle, tx, .. } => {
                tx.send((Instant::now(), msg::ToMidiOut::Stop))
                    .unwrap_or(());
                match joinhandle.join() {
                    Ok((connection, rx, latency_tx)) => Self::Unconnected {
                        midi_output: connection.close(),
                        rx,
                        tx,
                        latency_tx,
                    },
                    Err(err) => panic!("{:?}", err),
                }
            }
            Self::Unconnected { .. } => self,
        }
    }
}
