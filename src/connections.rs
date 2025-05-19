use std::{sync::mpsc, thread, time::Instant};

use midir::{
    MidiIO, MidiInput, MidiInputConnection, MidiInputPort, MidiOutput, MidiOutputConnection,
    MidiOutputPort,
};

use crate::msg;

pub enum Either<A, B> {
    Left(A),
    Right(B),
}

use Either::*;

pub trait MaybeConnected<IO: MidiIO> {
    fn unconnected(&self) -> Option<&IO>;
    fn connected_port_name(&self) -> Option<&str>;
    fn connect(self, port: IO::Port, portname: &str) -> Result<Self, (String, Self)>
    where
        Self: Sized;
    fn disconnect(self) -> Self;
}

pub type MidiInputOrConnection = Either<
    (MidiInput, mpsc::Sender<(Instant, msg::ToProcess)>),
    (
        MidiInputConnection<()>,
        mpsc::Sender<(Instant, msg::ToProcess)>,
        String,
    ),
>;

impl MidiInputOrConnection {
    pub fn new(midi_input: MidiInput) -> (Self, mpsc::Receiver<(Instant, msg::ToProcess)>) {
        let (tx, rx) = mpsc::channel();
        (Left((midi_input, tx)), rx)
    }

    /// Use this only for data that will never be read!
    pub fn empty_placeholder() -> Self {
        let (tx, _rx) = mpsc::channel();
        Left((
            midir::MidiInput::new("adaptuner placeholder input").unwrap(),
            tx,
        ))
    }

    pub fn get_sender(&self) -> mpsc::Sender<(Instant, msg::ToProcess)> {
        match self {
            Left((_, tx)) => tx.clone(),
            Right((_, tx, _)) => tx.clone(),
        }
    }

    fn connect_internal(self, port: MidiInputPort, portname: &str) -> Result<Self, (String, Self)> {
        match self {
            Left((input, sender)) => {
                let tx = sender.clone();
                match input.connect(
                    &port,
                    portname,
                    move |_, bytes, _| {
                        let time = Instant::now();
                        tx.send((
                            time,
                            msg::ToProcess::IncomingMidi {
                                bytes: bytes.to_vec(),
                            },
                        ))
                        .unwrap_or(());
                    },
                    (),
                ) {
                    Ok(connection) => Ok(Right((connection, sender, portname.to_string()))),
                    Err(err) => {
                        let err_string = err.to_string();
                        Err((err_string, Left((err.into_inner(), sender))))
                    }
                }
            }
            Right(_) => unreachable!(),
        }
    }
}

impl MaybeConnected<MidiInput> for MidiInputOrConnection {
    fn unconnected(&self) -> Option<&MidiInput> {
        match self {
            Left((input, _)) => Some(input),
            Right(_) => None {},
        }
    }

    fn connected_port_name(&self) -> Option<&str> {
        match self {
            Left(_) => None {},
            Right((_, _, pname)) => Some(pname),
        }
    }

    fn connect(self, port: MidiInputPort, portname: &str) -> Result<Self, (String, Self)> {
        match self {
            Left(_) => self.connect_internal(port, portname),
            Right(_) => {
                let disconnected = self.disconnect();
                disconnected.connect_internal(port, portname)
            }
        }
    }

    fn disconnect(self) -> Self {
        match self {
            Right((connection, sender, _portname)) => Left((connection.close().0, sender)),
            Left(_) => self,
        }
    }
}

pub type MidiOutputOrConnection = Either<
    (
        MidiOutput,
        mpsc::Receiver<(Instant, msg::ToMidiOut)>,
        mpsc::Sender<(Instant, msg::ToMidiOut)>,
        mpsc::Sender<msg::FromMidiOut>,
    ),
    (
        thread::JoinHandle<(
            MidiOutputConnection,
            mpsc::Receiver<(Instant, msg::ToMidiOut)>,
            mpsc::Sender<msg::FromMidiOut>,
        )>,
        mpsc::Sender<(Instant, msg::ToMidiOut)>,
        String,
    ),
>;

impl MidiOutputOrConnection {
    pub fn new(midi_output: MidiOutput) -> (Self, mpsc::Receiver<msg::FromMidiOut>) {
        let (tx, rx) = mpsc::channel();
        let (latency_tx, latency_rx) = mpsc::channel();
        (Left((midi_output, rx, tx, latency_tx)), latency_rx)
    }

    /// Use this only for data that will never be read!
    pub fn empty_placeholder() -> Self {
        let (tx, rx) = mpsc::channel();
        let (latency_tx, _latency_rx) = mpsc::channel();
        Left((
            midir::MidiOutput::new("adaptuner placeholder output").unwrap(),
            rx,
            tx,
            latency_tx,
        ))
    }

    pub fn get_sender(&self) -> mpsc::Sender<(Instant, msg::ToMidiOut)> {
        match self {
            Left((_, _, tx, _)) => tx.clone(),
            Right((_, tx, _)) => tx.clone(),
        }
    }

    fn connect_internal(
        self,
        port: MidiOutputPort,
        portname: &str,
    ) -> Result<Self, (String, Self)> {
        match self {
            Left((output, rx, tx, latency_tx)) => match output.connect(&port, portname) {
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
                    Ok(Right((joinhandle, tx, portname.to_string())))
                }
                Err(err) => {
                    let err_string = err.to_string();
                    Err((err_string, Left((err.into_inner(), rx, tx, latency_tx))))
                }
            },
            Right(_) => unreachable!(),
        }
    }
}

impl MaybeConnected<MidiOutput> for MidiOutputOrConnection {
    fn connected_port_name(&self) -> Option<&str> {
        match self {
            Left(_) => None {},
            Right((_, _, pname)) => Some(pname),
        }
    }

    fn unconnected(&self) -> Option<&MidiOutput> {
        match self {
            Left((output, _, _, _)) => Some(output),
            Right(_) => None {},
        }
    }

    fn connect(self, port: MidiOutputPort, portname: &str) -> Result<Self, (String, Self)> {
        match self {
            Left(_) => self.connect_internal(port, portname),
            Right(_) => {
                let disconnected = self.disconnect();
                disconnected.connect_internal(port, portname)
            }
        }
    }

    fn disconnect(self) -> Self {
        match self {
            Right((joinhandle, tx, _portname)) => {
                tx.send((Instant::now(), msg::ToMidiOut::Stop))
                    .unwrap_or(());
                match joinhandle.join() {
                    Ok((connection, rx, latency_tx)) => {
                        Left((connection.close(), rx, tx, latency_tx))
                    }
                    Err(err) => panic!("{:?}", err),
                }
            }
            Left(_) => self,
        }
    }
}
