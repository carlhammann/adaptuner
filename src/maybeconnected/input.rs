use std::{sync::mpsc, time::Instant};

use midir::{MidiInput, MidiInputConnection, MidiInputPort};

use crate::{
    config::{ExtractConfig, MidiInputConfig},
    maybeconnected::common::MaybeConnected,
    msg::{FromMidiIn, HandleMsg, ToMidiIn},
    util::update_cell::UpdateCell,
};

enum MidiInputOrConnectionInternal {
    Unconnected {
        midi_input: MidiInput,
        tx: mpsc::Sender<FromMidiIn>,
    },
    Connected {
        connection: MidiInputConnection<mpsc::Sender<FromMidiIn>>,
        portname: String,
    },
}

impl MidiInputOrConnectionInternal {
    fn new(midi_input: MidiInput, tx: mpsc::Sender<FromMidiIn>) -> Self {
        Self::Unconnected { midi_input, tx }
    }

    fn connect_internal(self, port: MidiInputPort, portname: &str) -> Result<Self, (String, Self)> {
        match self {
            Self::Unconnected { midi_input, tx } => {
                match midi_input.connect(
                    &port,
                    portname,
                    move |_, bytes, tx| {
                        let time = Instant::now();
                        let _ = tx.send(FromMidiIn::IncomingMidi {
                            time,
                            bytes: bytes.to_vec(),
                        });
                    },
                    tx.clone(),
                ) {
                    Ok(connection) => Ok(Self::Connected {
                        connection,
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

impl MaybeConnected<MidiInput> for MidiInputOrConnectionInternal {
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
            Self::Connected { connection, .. } => {
                let (midi_input, tx) = connection.close();
                Self::Unconnected { midi_input, tx }
            }
            Self::Unconnected { .. } => self,
        }
    }
}

pub struct MidiInputOrConnection {
    internal: UpdateCell<MidiInputOrConnectionInternal>,
}

impl MidiInputOrConnection {
    pub fn new(midi_input: MidiInput, tx: mpsc::Sender<FromMidiIn>) -> Self {
        Self {
            internal: UpdateCell::new(MidiInputOrConnectionInternal::new(midi_input, tx)),
        }
    }
}

impl HandleMsg<ToMidiIn, FromMidiIn> for MidiInputOrConnection {
    fn handle_msg(&mut self, msg: ToMidiIn, forward: &mpsc::Sender<FromMidiIn>) {
        match msg {
            ToMidiIn::Connect { port, portname } => {
                self.internal
                    .update(|old| match old.connect(port, &portname) {
                        Ok(new) => {
                            let _ = forward.send(FromMidiIn::Connected { portname });
                            new
                        }
                        Err((reason, new)) => {
                            let _ = forward.send(FromMidiIn::ConnectionError { reason });
                            new
                        }
                    })
            }
            ToMidiIn::Start | ToMidiIn::Disconnect => self.internal.update(|old| {
                let new = old.disconnect();
                let input = new.unconnected().unwrap(); // this is ok, we just disconnected
                let ports = input
                    .ports()
                    .drain(..)
                    .map(|p| {
                        let name = input.port_name(&p).unwrap_or("<no name>".into());
                        (p, name)
                    })
                    .filter(|(_, name)| !name.contains("adaptuner output"))
                    .collect();
                let _ = forward.send(FromMidiIn::Disconnected {
                    available_ports: ports,
                });
                new
            }),
            ToMidiIn::Stop => {}
        }
    }
}

impl ExtractConfig<MidiInputConfig> for MidiInputOrConnection {
    fn extract_config(&self) -> MidiInputConfig {
        MidiInputConfig {}
    }
}
