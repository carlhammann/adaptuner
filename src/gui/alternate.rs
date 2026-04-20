use std::time::Instant;

use eframe::{self, egui};

use crate::{
    gui::{
        lattice::LatticeWindow,
        r#trait::{Gui, GuiShow, UiAdaptor},
    },
    interval::stacktype::r#trait::StackType,
    msg::{FromUi, ReceiveMsg, ToUi},
    notename::HasNoteNames,
};

pub struct TopLevelGui<T: StackType, A: UiAdaptor<T>> {
    adaptor: A,
    lattice: LatticeWindow<T>,
}

impl<T: StackType + HasNoteNames, A: UiAdaptor<T>> eframe::App for TopLevelGui<T, A> {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            self.lattice.show(ui, &self.adaptor);
        });
    }
}

impl<T: StackType, A: UiAdaptor<T>> ReceiveMsg<ToUi<T>> for TopLevelGui<T, A> {
    fn receive_msg(&mut self, msg: ToUi<T>) {
        match msg {
            ToUi::Notify { line } => println!("{line}"),
            ToUi::NoteOn {
                channel,
                note,
                time,
            } => println!("note on {note}"),
            ToUi::Retune { note } => println!("retune {note}"),
            ToUi::NoteOff {
                channel,
                note,
                time,
            } => println!("note off {note}"),
            ToUi::EventLatency { since_input } => println!("latency: {since_input:?}"),
            ToUi::InputConnectionError { reason } => {
                println!("input connection error: {reason}")
            }
            ToUi::InputConnected { portname } => println!("input {portname} connected"),
            ToUi::InputDisconnected { available_ports } => {
                let (port, portname) = &available_ports[0];
                let _ = self.adaptor.send(FromUi::ConnectInput {
                    port: port.clone(),
                    portname: portname.clone(),
                    time: Instant::now(),
                });
            }
            ToUi::OutputConnectionError { reason } => {
                println!("output connection error: {reason}")
            }
            ToUi::OutputConnected { portname } => println!("output {portname} connected"),
            ToUi::OutputDisconnected { available_ports } => {
                let (port, portname) = &available_ports[1];
                let _ = self.adaptor.send(FromUi::ConnectOutput {
                    port: port.clone(),
                    portname: portname.clone(),
                    time: Instant::now(),
                });
            }
            _ => {}
        }
    }
}

impl<T: StackType + HasNoteNames, A: UiAdaptor<T>> Gui<T, A> for TopLevelGui<T, A> {
    fn new(config: crate::config::GuiConfig, adaptor: A) -> Self {
        Self {
            adaptor,
            lattice: LatticeWindow::new(),
        }
    }
}
