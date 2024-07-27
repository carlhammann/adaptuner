use std::{
    sync::mpsc,
    time::{Duration, Instant},
};

use ratatui::prelude::Line;

use crate::{
    config::r#trait::Config, interval::stacktype::r#trait::StackType, tui::r#trait::UIState,
};

pub struct LatencyReporter<const N: usize> {
    values: [Duration; N],
    next_to_update: usize,
}

impl<const N: usize, T: StackType> UIState<T> for LatencyReporter<N> {
    fn handle_msg(
        &mut self,
        _time: Instant,
        msg: crate::msg::AfterProcess<T>,
        _to_process: &mpsc::Sender<(Instant, crate::msg::ToProcess)>,
        tui: &mut super::r#trait::Tui,
    ) {
        match msg {
            crate::msg::AfterProcess::BackendLatency { since_input } => {
                self.values[self.next_to_update] = since_input;
                self.next_to_update = (self.next_to_update + 1) % N;
                let mean = self.values.iter().map(|x| x.as_micros()).sum::<u128>() / N as u128;
                match tui.draw(|frame| {
                    frame.render_widget(
                        Line::from(format!("mean latency to backend: {mean} usec")),
                        frame.size(),
                    )
                }) {
                    _ => (),
                };
            }
            _ => {}
        }
    }
}

#[derive(Clone)]
pub struct LatencyReporterConfig {}

impl<const N: usize> Config<LatencyReporter<N>> for LatencyReporterConfig {
    fn initialise(_: &Self) -> LatencyReporter<N> {
        LatencyReporter {
            values: [Duration::new(0, 0); N],
            next_to_update: 0,
        }
    }
}
