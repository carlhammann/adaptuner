use std::marker::PhantomData;

use eframe::egui;
use midir::{MidiIO, MidiInput, MidiOutput};

use crate::{connections::MaybeConnected, gui::r#trait::GuiShowUpdating};

pub struct ConnectionWindow<IO: MidiIO> {
    _phantom: PhantomData<IO>,
    error: Option<String>,
    direction: String,
}

pub fn new_input_connection_window() -> ConnectionWindow<MidiInput> {
    ConnectionWindow {
        _phantom: PhantomData,
        error: None {},
        direction: "input".into(),
    }
}

pub fn new_output_connection_window() -> ConnectionWindow<MidiOutput> {
    ConnectionWindow {
        _phantom: PhantomData,
        error: None {},
        direction: "output".into(),
    }
}

pub fn port_selector<IO>(io: &IO, direction: &str, ui: &mut egui::Ui) -> Option<(IO::Port, String)>
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

pub fn disconnector(direction: &str, portname: &str, ui: &mut egui::Ui) -> bool {
    let mut disconnect = false;
    ui.with_layout(egui::Layout::left_to_right(egui::Align::Min), |ui| {
        ui.label(format!("{} is \"{}\"", direction, portname));
        if ui.button("disconnect").clicked() {
            disconnect = true;
        }
    });
    disconnect
}

impl<IO, C> GuiShowUpdating<C> for ConnectionWindow<IO>
where
    IO: MidiIO,
    <IO as MidiIO>::Port: PartialEq,
    C: MaybeConnected<IO>,
{
    fn show_updating(&mut self, data: C, _ctx: &egui::Context, ui: &mut egui::Ui) -> C {
        match &self.error {
            Some(str) => {
                ui.label(
                    egui::RichText::new(format!("{} connection error:\n{str}", self.direction))
                        .color(ui.style().visuals.warn_fg_color),
                );
            }
            None {} => {}
        }

        match data.connected_port_name() {
            None {} => match port_selector(data.unconnected().unwrap(), &self.direction, ui) {
                None {} => data,
                Some((port, portname)) => match data.connect(port, &portname) {
                    Ok(new_data) => {
                        self.error = None {};
                        new_data
                    }
                    Err((err, unchanged_data)) => {
                        self.error = Some(err);
                        unchanged_data
                    }
                },
            },
            Some(portname) => {
                if disconnector(&self.direction, portname, ui) {
                    data.disconnect()
                } else {
                    data
                }
            }
        }
    }
}
