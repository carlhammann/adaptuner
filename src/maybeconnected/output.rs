use std::{sync::mpsc, time::Instant};

use midir::*;

use crate::{
    maybeconnected::common::MaybeConnected,
    msg::{FromMidiOut, ReceiveMsg, ToMidiOut},
    util::update_cell::UpdateCell,
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
    internal: UpdateCell<MidiOutputOrConnectionInternal>,
    forward: mpsc::Sender<FromMidiOut>,
}

impl MidiOutputOrConnection {
    pub fn new(midi_output: MidiOutput, forward: mpsc::Sender<FromMidiOut>) -> Self {
        Self {
            internal: UpdateCell::new(MidiOutputOrConnectionInternal::new(midi_output)),
            forward,
        }
    }
}

impl MidiOutputOrConnection {
    fn send_msg(&self, msg: FromMidiOut) -> bool {
        self.forward.send(msg).is_ok()
    }
}

impl ReceiveMsg<ToMidiOut> for MidiOutputOrConnection {
    fn receive_msg(&mut self, msg: ToMidiOut) {
        match msg {
            ToMidiOut::OutgoingMidi { time, bytes } => match &mut *self.internal.borrow_mut() {
                MidiOutputOrConnectionInternal::Connected { connection, .. } => {
                    let _ = connection.send(&bytes);
                    let now = Instant::now();
                    let _ = self.send_msg(FromMidiOut::EventLatency {
                        since_input: now.duration_since(time),
                    });
                }
                MidiOutputOrConnectionInternal::Unconnected { .. } => {}
            },
            ToMidiOut::Connect { port, portname } => {
                self.internal
                    .update(|old| match old.connect(port, &portname) {
                        Ok(new) => {
                            let _ = self.send_msg(FromMidiOut::Connected { portname });
                            new
                        }
                        Err((reason, new)) => {
                            let _ = self.send_msg(FromMidiOut::ConnectionError { reason });
                            new
                        }
                    });
            }
            ToMidiOut::Start | ToMidiOut::Disconnect => {
                self.internal.update(|old| {
                    let new = old.disconnect();
                    let input = new.unconnected().unwrap(); // this is ok, we just disconnected
                    let ports = input
                        .ports()
                        .drain(..)
                        .map(|p| {
                            let name = input.port_name(&p).unwrap_or("<no name>".into());
                            (p, name)
                        })
                        .filter(|(_, name)| !name.contains("adaptuner input"))
                        .collect();
                    let _ = self.send_msg(FromMidiOut::Disconnected {
                        available_ports: ports,
                    });

                    new
                });
            }
            ToMidiOut::Stop => {}
        }
    }
}
