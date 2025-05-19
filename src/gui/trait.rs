use std::{sync::mpsc, time::Instant};

use eframe::{self, egui};

use crate::{interval::stacktype::r#trait::StackType, msg};

pub trait GuiState<T: StackType> {
    fn handle_msg(
        &mut self,
        time: Instant,
        msg: &msg::AfterProcess<T>,
        to_process: &mpsc::Sender<(Instant, msg::ToProcess)>,
        ctx: &egui::Context,
    );
}

pub trait GuiShow {
    fn show(&mut self, ctx: &egui::Context, ui: &mut egui::Ui);
}

pub trait GuiShowUpdating<D> {
    fn show_updating(&mut self, data: D, ctx: &egui::Context, ui: &mut egui::Ui) -> D;
}
