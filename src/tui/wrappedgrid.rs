use std::{sync::mpsc, time::Instant};

use ratatui::prelude::{Constraint, Frame, Layout, Rect};

use crate::{
    config::r#trait::Config,
    interval::stacktype::r#trait::{FiveLimitStackType, StackType},
    msg,
    tui::grid::{Grid, GridConfig},
    tui::latencyreporter::{LatencyReporter, LatencyReporterConfig},
    tui::r#trait::UIState,
};

pub struct WrappedGrid<const NEIGHBOURS: usize, T: StackType> {
    grid: Grid<NEIGHBOURS, T>,
    latencyreporter: LatencyReporter,
}

impl<const N: usize, T: FiveLimitStackType> UIState<T> for WrappedGrid<N, T> {
    fn handle_msg(
        &mut self,
        time: Instant,
        msg: &msg::AfterProcess<T>,
        to_process: &mpsc::Sender<(Instant, crate::msg::ToProcess)>,
        frame: &mut Frame,
        area: Rect,
    ) {
        let layout = Layout::vertical([Constraint::Percentage(100), Constraint::Min(1)]);
        let [grid_area, latency_area] = layout.areas(area);
        self.grid
            .handle_msg(time, msg, to_process, frame, grid_area);
        self.latencyreporter
            .handle_msg(time, msg, to_process, frame, latency_area);
    }
}

pub struct WrappedGridConfig<const NEIGHBOURS: usize, T: StackType> {
    pub gridconfig: GridConfig<NEIGHBOURS, T>,
    pub latencyreporterconfig: LatencyReporterConfig,
}

impl<const N: usize, T: StackType> Clone for WrappedGridConfig<N, T> {
    fn clone(&self) -> Self {
        WrappedGridConfig {
            gridconfig: self.gridconfig.clone(),
            latencyreporterconfig: self.latencyreporterconfig.clone(),
        }
    }
}

impl<const N: usize, T: FiveLimitStackType> Config<WrappedGrid<N, T>> for WrappedGridConfig<N, T> {
    fn initialise(config: &Self) -> WrappedGrid<N, T> {
        WrappedGrid {
            grid: <_ as Config<_>>::initialise(&config.gridconfig),
            latencyreporter: <_ as Config<_>>::initialise(&config.latencyreporterconfig),
        }
    }
}
