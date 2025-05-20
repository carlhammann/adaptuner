use std::{sync::mpsc, time::Duration};

use crate::{
    gui::r#trait::GuiShow,
    interval::stacktype::r#trait::StackType,
    msg::{self, FromUi, HandleMsgRef, ToUi},
};
use eframe::{self, egui};

pub struct LatencyWindow {
    values: Vec<Duration>,
    next_to_update: usize,
    mean: Duration,
}

impl LatencyWindow {
    pub fn new(window_length: usize) -> Self {
        Self {
            values: vec![Duration::ZERO; window_length],
            next_to_update: 0,
            mean: Duration::ZERO,
        }
    }
}

impl<T: StackType> HandleMsgRef<ToUi<T>, FromUi> for LatencyWindow {
    fn handle_msg_ref(&mut self, msg: &ToUi<T>, _forward: &mpsc::Sender<FromUi>) {
        match msg {
            msg::ToUi::EventLatency { since_input } => {
                let n = self.values.len();
                self.values[self.next_to_update] = *since_input;
                self.next_to_update = (self.next_to_update + 1) % n;
                self.mean = self.values.iter().sum::<Duration>() / n.try_into().unwrap();
            }
            _ => {}
        }
    }
}

impl GuiShow for LatencyWindow {
    fn show(&mut self, _ctx: &egui::Context, ui: &mut egui::Ui, _forward: &mpsc::Sender<FromUi>) {
        ui.label(format!(
            "mean latency (last {} events): {:?}",
            self.values.len(),
            self.mean
        ));
    }
}
