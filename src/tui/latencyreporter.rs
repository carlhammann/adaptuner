use std::{
    sync::mpsc,
    time::{Duration, Instant},
};

use ratatui::prelude::{Frame, Line, Rect};

use crate::{
    config::r#trait::Config, interval::stacktype::r#trait::StackType, msg, tui::r#trait::UIState,
};

pub struct LatencyReporter {
    values: Vec<Duration>,
    next_to_update: usize,
    mean: u128,
}

impl<T: StackType> UIState<T> for LatencyReporter {
    fn handle_msg(
        &mut self,
        _time: Instant,
        msg: &msg::AfterProcess<T>,
        _to_process: &mpsc::Sender<(Instant, crate::msg::ToProcess)>,
        frame: &mut Frame,
        area: Rect,
    ) {
        match msg {
            crate::msg::AfterProcess::BackendLatency { since_input } => {
                let n = self.values.len();
                self.values[self.next_to_update] = *since_input;
                self.next_to_update = (self.next_to_update + 1) % n;
                self.mean = self.values.iter().map(|x| x.as_micros()).sum::<u128>() / n as u128;
            }
            _ => {}
        }
        let n = self.values.len();
        frame.render_widget(
            Line::from(format!(
                "mean MIDI latency (last {n} events): {} microseconds",
                self.mean
            )),
            area,
        );
    }
}

#[derive(Clone)]
pub struct LatencyReporterConfig {
    pub nsamples: usize,
}

impl Config<LatencyReporter> for LatencyReporterConfig {
    fn initialise(config: &Self) -> LatencyReporter {
        LatencyReporter {
            values: vec![Duration::new(0, 0); config.nsamples],
            next_to_update: 0,
            mean: 0,
        }
    }
}
