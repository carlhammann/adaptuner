use std::time::Duration;

use crate::{
    gui::r#trait::{ReceiveToUiRef, UiAdaptor},
    interval::stacktype::r#trait::StackType,
    msg::ToUi,
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

impl<T: StackType, A: UiAdaptor<T>> ReceiveToUiRef<T, A> for LatencyWindow {
    fn receive_to_ui_ref(&mut self, msg: &ToUi<T>, _adaptor: &A) {
        match msg {
            ToUi::EventLatency { since_input } => {
                let n = self.values.len();
                self.values[self.next_to_update] = *since_input;
                self.next_to_update = (self.next_to_update + 1) % n;
                self.mean = self.values.iter().sum::<Duration>() / n.try_into().unwrap();
            }
            _ => {}
        }
    }
}

impl LatencyWindow {
    pub fn show(&self, ui: &mut egui::Ui) {
        ui.label(format!(
            "mean latency (last {} events): {:?}",
            self.values.len(),
            self.mean
        ));
    }
}
