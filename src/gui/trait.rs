use std::sync::mpsc;

use eframe::egui;

use crate::msg::FromUi;

pub trait GuiShow {
    fn show(&mut self, ctx: &egui::Context, ui: &mut egui::Ui, forward: &mpsc::Sender<FromUi>);
}
