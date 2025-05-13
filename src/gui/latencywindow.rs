use std::{
    sync::mpsc,
    time::{Duration, Instant},
};

use crate::{gui::r#trait::GuiState, interval::stacktype::r#trait::StackType, msg};
use eframe::{self, egui};

use super::r#trait::WindowGuiState;

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

impl<T: StackType> GuiState<T> for LatencyWindow {
    fn handle_msg(
        &mut self,
        _time: Instant,
        msg: &msg::AfterProcess<T>,
        _to_process: &mpsc::Sender<(Instant, msg::ToProcess)>,
        _ctx: &egui::Context,
    ) {
        match msg {
            msg::AfterProcess::BackendLatency { since_input } => {
                let n = self.values.len();
                self.values[self.next_to_update] = *since_input;
                self.next_to_update = (self.next_to_update + 1) % n;
                self.mean = self.values.iter().sum::<Duration>() / n.try_into().unwrap();
            }
            _ => {}
        }
    }
}

impl<T: StackType> WindowGuiState<T> for LatencyWindow {
    fn show(&mut self, _ctx: &egui::Context, ui: &mut egui::Ui) {
        ui.label(format!(
            "mean latency (last {} events): {:?}",
            self.values.len(),
            self.mean
        ));
    }
}
