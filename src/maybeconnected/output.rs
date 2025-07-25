use std::{cell::Cell, sync::mpsc, time::Instant};

use midir::*;

use crate::{
    config::{ExtractConfig, MidiOutputConfig}, maybeconnected::common::MaybeConnected, msg::{FromMidiOut, HandleMsg, ToMidiOut}
};

enum MidiOutputOrConnectionInternal {
    Unconnected {
        midi_output: MidiOutput,
    },
    Connected {
        connection: MidiOutputConnection,
        portname: String,
    },
}

impl MidiOutputOrConnectionInternal {
    fn new(midi_output: MidiOutput) -> Self {
        Self::Unconnected { midi_output }
    }

    /// Use this only for data that will never be read!
    fn empty_placeholder() -> Self {
        Self::Unconnected {
            midi_output: midir::MidiOutput::new("adaptuner placeholder output").unwrap(),
        }
    }

    fn connect_internal(
        self,
        port: MidiOutputPort,
        portname: &str,
    ) -> Result<Self, (String, Self)> {
        match self {
            Self::Unconnected { midi_output } => match midi_output.connect(&port, portname) {
                Ok(connection) => Ok(Self::Connected {
                    connection,
                    portname: portname.into(),
                }),
                Err(err) => {
                    let err_string = err.to_string();
                    Err((
                        err_string,
                        Self::Unconnected {
                            midi_output: err.into_inner(),
                        },
                    ))
                }
            },
            Self::Connected { .. } => unreachable!(),
        }
    }
}

impl MaybeConnected<MidiOutput> for MidiOutputOrConnectionInternal {
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
            Self::Connected { connection, .. } => Self::Unconnected {
                midi_output: connection.close(),
            },
            Self::Unconnected { .. } => self,
        }
    }
}

pub struct MidiOutputOrConnection {
    internal: Cell<MidiOutputOrConnectionInternal>,
}

impl MidiOutputOrConnection {
    pub fn new(midi_output: MidiOutput) -> Self {
        Self {
            internal: Cell::new(MidiOutputOrConnectionInternal::new(midi_output)),
        }
    }
}

impl HandleMsg<ToMidiOut, FromMidiOut> for MidiOutputOrConnection {
    fn handle_msg(&mut self, msg: ToMidiOut, forward: &mpsc::Sender<FromMidiOut>) {
        match msg {
            ToMidiOut::OutgoingMidi { time, bytes } => match self.internal.get_mut() {
                MidiOutputOrConnectionInternal::Connected { connection, .. } => {
                    let _ = connection.send(&bytes);
                    let now = Instant::now();
                    let _ = forward.send(FromMidiOut::EventLatency {
                        since_input: now.duration_since(time),
                    });
                }
                MidiOutputOrConnectionInternal::Unconnected { .. } => {}
            },
            ToMidiOut::Connect { port, portname } => {
                let old = self
                    .internal
                    .replace(MidiOutputOrConnectionInternal::empty_placeholder());
                match old.connect(port, &portname) {
                    Ok(new) => {
                        let _ = forward.send(FromMidiOut::Connected { portname });
                        self.internal.set(new);
                    }
                    Err((reason, new)) => {
                        let _ = forward.send(FromMidiOut::ConnectionError { reason });
                        self.internal.set(new);
                    }
                }
            }
            ToMidiOut::Start | ToMidiOut::Disconnect => {
                let old = self
                    .internal
                    .replace(MidiOutputOrConnectionInternal::empty_placeholder());
                let new = old.disconnect();
                let input = new.unconnected().unwrap(); // this is ok, we just disconnected
                let ports = input
                    .ports()
                    .drain(..)
                    .map(|p| {
                        let name = input.port_name(&p).unwrap_or("<no name>".into());
                        (p, name)
                    })
                    .collect();
                let _ = forward.send(FromMidiOut::Disconnected {
                    available_ports: ports,
                });

                self.internal.set(new);
            }
            ToMidiOut::Stop => {}
        }
    }
}

impl ExtractConfig<MidiOutputConfig> for MidiOutputOrConnection {
    fn extract_config(&self) -> MidiOutputConfig {
        MidiOutputConfig {}
    }
}
