use std::{sync::mpsc, thread, time::Instant};

use eframe::{self, egui};

use midir::{
    MidiIO, MidiInput, MidiInputConnection, MidiInputPort, MidiOutput, MidiOutputConnection,
    MidiOutputPort,
};

use crate::{gui::r#trait::GuiShowUpdating, msg};

/// invariant: each exactly one of (`input`, `input_connection`) and (`output`,
/// `output_connection`) are `Some`.
pub struct MidiConnections {
    input: Option<MidiInput>,
    input_connection: Option<(MidiInputConnection<()>, String)>,
    midi_input_tx: mpsc::Sender<(Instant, msg::ToProcess)>,
    output: Option<(MidiOutput, mpsc::Receiver<(Instant, msg::ToMidiOut)>)>,
    output_connection: Option<(
        thread::JoinHandle<(
            MidiOutputConnection,
            mpsc::Receiver<(Instant, msg::ToMidiOut)>,
        )>,
        String,
    )>,
    midi_output_tx: mpsc::Sender<(Instant, msg::ToMidiOut)>,
    // midi_output_rx: mpsc::Receiver<(Instant, msg::ToMidiOut)>,
    latency_tx: mpsc::Sender<msg::FromMidiOut>,
}

impl MidiConnections {
    pub fn new(
        input: MidiInput,
        output: MidiOutput,
    ) -> (
        Self,
        mpsc::Receiver<(Instant, msg::ToProcess)>,
        mpsc::Sender<(Instant, msg::ToMidiOut)>,
        mpsc::Receiver<msg::FromMidiOut>,
    ) {
        let (midi_input_tx, midi_input_rx) = mpsc::channel();
        let (midi_output_tx, midi_output_rx) = mpsc::channel();
        let (latency_tx, latency_rx) = mpsc::channel();
        (
            Self {
                input: Some(input),
                input_connection: None {},
                midi_input_tx,
                output: Some((output, midi_output_rx)),
                output_connection: None {},
                midi_output_tx: midi_output_tx.clone(),
                latency_tx,
            },
            midi_input_rx,
            midi_output_tx,
            latency_rx,
        )
    }

    pub fn new_sender_to_process(&self) -> mpsc::Sender<(Instant, msg::ToProcess)> {
        self.midi_input_tx.clone()
    }

    /// Use this only for placeholders that will never be read! The return value violates the
    /// invariant of [MidiConnections].
    pub fn empty_placeholder() -> Self {
        let (midi_input_tx, _) = mpsc::channel();
        let (midi_output_tx, midi_output_rx) = mpsc::channel();
        let (latency_tx, _) = mpsc::channel();
        Self {
            input: None {},
            input_connection: None {},
            midi_input_tx,
            output: None {},
            output_connection: None {},
            midi_output_tx,
            latency_tx,
        }
    }
}

pub struct ConnectionWindow {
    input_connection_error: Option<String>,
    output_connection_error: Option<String>,
}

impl ConnectionWindow {
    pub fn new() -> Self {
        Self {
            input_connection_error: None {},
            output_connection_error: None {},
        }
    }
}

fn port_selector<IO>(io: &IO, direction: &str, ui: &mut egui::Ui) -> Option<(IO::Port, String)>
where
    IO: MidiIO,
    <IO as MidiIO>::Port: PartialEq,
{
    let mut selected_port = None {};
    egui::ComboBox::from_id_salt(format!("select {}", direction))
        .selected_text(
            egui::RichText::new(format!("select {}", direction)), //.color(ui.style().visuals.warn_fg_color),
        )
        .show_ui(ui, |ui| {
            for port in io.ports().iter() {
                let pname = io
                    .port_name(&port)
                    .unwrap_or("<name cannot be shown>".into());
                //if ui
                ui.selectable_value(
                    &mut selected_port,
                    Some((port.clone(), pname.clone())),
                    pname,
                );
            }
        });

    selected_port
}

fn disconnector(direction: &str, portname: &str, ui: &mut egui::Ui) -> bool {
    let mut disconnect = false;
    ui.with_layout(egui::Layout::left_to_right(egui::Align::Min), |ui| {
        ui.label(format!("{} is \"{}\"", direction, portname));
        if ui.button("disconnect").clicked() {
            disconnect = true;
        }
    });
    disconnect
}
impl GuiShowUpdating<MidiConnections> for ConnectionWindow {
    fn show_updating(
        &mut self,
        data: MidiConnections,
        ctx: &egui::Context,
        ui: &mut egui::Ui,
    ) -> MidiConnections {
        let (input, input_connection) = match (data.input, data.input_connection) {
            (Some(input), None {}) => {
                match &self.input_connection_error {
                    Some(str) => {
                        ui.label(
                            egui::RichText::new(format!("input connection error:\n{str}"))
                                .color(ui.style().visuals.warn_fg_color),
                        );
                    }
                    None {} => {}
                }

                match port_selector(&input, "input", ui) {
                    Some((port, portname)) => {
                        let tx = data.midi_input_tx.clone();
                        match input.connect(
                            &port,
                            "adaptuner input",
                            move |_time, bytes, _| {
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
                            Ok(connection) => {
                                self.input_connection_error = None {};
                                (None {}, Some((connection, portname)))
                            }
                            Err(err) => {
                                self.input_connection_error = Some(err.to_string());
                                (Some(err.into_inner()), None {})
                            }
                        }
                    }
                    None {} => (Some(input), None {}),
                }
            }

            (None {}, Some((connection, portname))) => {
                if disconnector("input", &portname, ui) {
                    (Some(connection.close().0), None {})
                } else {
                    (None {}, Some((connection, portname)))
                }
            }

            _ => unreachable!(),
        };

        let (output, output_connection) = match (data.output, data.output_connection) {
            (Some((output, midi_output_rx)), None {}) => {
                match &self.output_connection_error {
                    Some(str) => {
                        ui.label(
                            egui::RichText::new(format!("output connection error:\n{str}"))
                                .color(ui.style().visuals.warn_fg_color),
                        );
                    }
                    None {} => {}
                }

                match port_selector(&output, "output", ui) {
                    Some((port, portname)) => match output.connect(&port, "adaptuner output") {
                        Ok(mut connection) => {
                            let latency_tx = data.latency_tx.clone();
                            let joinhandle = thread::spawn(move || {
                                loop {
                                    match midi_output_rx.recv() {
                                        Ok((_, msg::ToMidiOut::Stop)) => break,
                                        Ok((
                                            original_time,
                                            msg::ToMidiOut::OutgoingMidi { bytes },
                                        )) => {
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
                                (connection, midi_output_rx)
                            });
                            (None {}, Some((joinhandle, portname)))
                        }
                        Err(err) => {
                            self.output_connection_error = Some(err.to_string());
                            (Some((err.into_inner(), midi_output_rx)), None {})
                        }
                    },
                    None {} => (Some((output, midi_output_rx)), None {}),
                }
            }

            (None {}, Some((joinhandle, portname))) => {
                if disconnector("output", &portname, ui) {
                    data.midi_output_tx
                        .send((Instant::now(), msg::ToMidiOut::Stop))
                        .unwrap_or(());
                    match joinhandle.join() {
                        Ok((connection, midi_output_rx)) => {
                            (Some((connection.close(), midi_output_rx)), None {})
                        }
                        Err(err) => panic!("{:?}", err),
                    }
                } else {
                    (None {}, Some((joinhandle, portname)))
                }
            }

            _ => unreachable!(),
        };

        MidiConnections {
            input,
            input_connection,
            midi_input_tx: data.midi_input_tx,
            output,
            output_connection,
            midi_output_tx: data.midi_output_tx,
            // midi_output_rx: data.midi_output_rx,
            latency_tx: data.latency_tx,
        }
    }
}
