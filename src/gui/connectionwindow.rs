use std::{sync::mpsc, time::Instant};

use eframe::egui;
use midir::{MidiInputPort, MidiOutputPort};

use crate::{
    gui::r#trait::GuiShow,
    interval::stacktype::r#trait::StackType,
    msg::{FromUi, HandleMsgRef, ToUi},
};

pub struct Input {}
pub struct Output {}

pub trait IO {
    type Port;
    fn direction_string() -> &'static str;
    fn connect_msg<T: StackType>(port: Self::Port, portname: String) -> FromUi<T>;
    fn disconnect_msg<T: StackType>() -> FromUi<T>;
}

impl IO for Input {
    type Port = MidiInputPort;

    fn direction_string() -> &'static str {
        "input"
    }

    fn connect_msg<T: StackType>(port: Self::Port, portname: String) -> FromUi<T> {
        FromUi::ConnectInput {
            port,
            portname,
            time: Instant::now(),
        }
    }

    fn disconnect_msg<T: StackType>() -> FromUi<T> {
        FromUi::DisconnectInput
    }
}

impl IO for Output {
    type Port = MidiOutputPort;

    fn direction_string() -> &'static str {
        "output"
    }

    fn connect_msg<T: StackType>(port: Self::Port, portname: String) -> FromUi<T> {
        FromUi::ConnectOutput {
            port,
            portname,
            time: Instant::now(),
        }
    }

    fn disconnect_msg<T: StackType>() -> FromUi<T> {
        FromUi::DisconnectOutput
    }
}

pub enum ConnectionWindow<X: IO> {
    Connected {
        portname: String,
    },
    Unconnected {
        error: Option<String>,
        available_ports: Vec<(X::Port, String)>,
    },
}

impl<X: IO> ConnectionWindow<X> {
    pub fn new() -> Self {
        Self::Unconnected {
            error: None {},
            available_ports: vec![],
        }
    }
}

pub fn port_selector<X, T: StackType>(
    available_ports: &[(X::Port, String)],
    ui: &mut egui::Ui,
    forward: &mpsc::Sender<FromUi<T>>,
) -> Option<(X::Port, String)>
where
    X: IO,
    <X as IO>::Port: PartialEq + Clone,
{
    let mut selected_port = None {};
    if egui::ComboBox::from_id_salt(format!("select {}", X::direction_string()))
        .selected_text(egui::RichText::new(format!(
            "select {}",
            X::direction_string()
        )))
        .show_ui(ui, |ui| {
            for (port, pname) in available_ports {
                ui.selectable_value(
                    &mut selected_port,
                    Some((port.clone(), pname.clone())),
                    pname,
                );
            }
        })
        .response
        .clicked()
    {
        let _ = forward.send(X::disconnect_msg());
    }

    selected_port
}

pub fn disconnector<X: IO>(portname: &str, ui: &mut egui::Ui) -> bool {
    let mut disconnect = false;
    ui.with_layout(egui::Layout::left_to_right(egui::Align::Min), |ui| {
        ui.label(format!("{} is \"{}\"", X::direction_string(), portname));
        if ui.button("disconnect").clicked() {
            disconnect = true;
        }
    });
    disconnect
}

impl<X, T> GuiShow<T> for ConnectionWindow<X>
where
    T: StackType,
    X: IO,
    <X as IO>::Port: PartialEq + Clone,
{
    fn show(&mut self, ui: &mut egui::Ui, forward: &mpsc::Sender<FromUi<T>>) {
        match self {
            ConnectionWindow::Connected { portname } => {
                if disconnector::<X>(&portname, ui) {
                    let _ = forward.send(X::disconnect_msg());
                }
            }
            ConnectionWindow::Unconnected {
                error,
                available_ports,
            } => {
                if let Some(str) = error {
                    ui.label(
                        egui::RichText::new(format!(
                            "{} connection error:\n{str}",
                            X::direction_string()
                        ))
                        .color(ui.style().visuals.warn_fg_color),
                    );
                }

                if let Some((port, portname)) = port_selector::<X, T>(&available_ports, ui, forward)
                {
                    let _ = forward.send(X::connect_msg(port, portname));
                }
            }
        }
    }
}

impl<T: StackType> HandleMsgRef<ToUi<T>, FromUi<T>> for ConnectionWindow<Input> {
    fn handle_msg_ref(&mut self, msg: &ToUi<T>, _forward: &mpsc::Sender<FromUi<T>>) {
        match msg {
            ToUi::InputConnectionError { reason } => match self {
                ConnectionWindow::Unconnected { error, .. } => *error = Some(reason.clone()),
                ConnectionWindow::Connected { .. } => unreachable!(),
            },
            ToUi::InputConnected { portname } => {
                *self = Self::Connected {
                    portname: portname.clone(),
                }
            }
            ToUi::InputDisconnected { available_ports } => {
                let error = match self {
                    ConnectionWindow::Connected { .. } => None {},
                    ConnectionWindow::Unconnected { error, .. } => error.clone(),
                };
                *self = Self::Unconnected {
                    error,
                    available_ports: available_ports.clone(),
                };
            }
            _ => {}
        }
    }
}

impl<T: StackType> HandleMsgRef<ToUi<T>, FromUi<T>> for ConnectionWindow<Output> {
    fn handle_msg_ref(&mut self, msg: &ToUi<T>, _forward: &mpsc::Sender<FromUi<T>>) {
        match msg {
            ToUi::OutputConnectionError { reason } => match self {
                ConnectionWindow::Unconnected { error, .. } => *error = Some(reason.clone()),
                ConnectionWindow::Connected { .. } => unreachable!(),
            },
            ToUi::OutputConnected { portname } => {
                *self = Self::Connected {
                    portname: portname.clone(),
                }
            }
            ToUi::OutputDisconnected { available_ports } => {
                let error = match self {
                    ConnectionWindow::Connected { .. } => None {},
                    ConnectionWindow::Unconnected { error, .. } => error.clone(),
                };
                *self = Self::Unconnected {
                    error,
                    available_ports: available_ports.clone(),
                };
            }

            _ => {}
        }
    }
}
