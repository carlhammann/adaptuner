use std::sync::mpsc;

use eframe::egui;

use crate::{interval::stacktype::r#trait::StackType, msg::FromUi};

pub trait GuiShow<T:StackType> {
    fn show(&mut self, ctx: &egui::Context, ui: &mut egui::Ui, forward: &mpsc::Sender<FromUi<T>>);
}
