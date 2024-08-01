use std::{sync::mpsc, time::Instant};

use ratatui::prelude::{Constraint, Frame, Layout, Rect};

use crate::{
    config::r#trait::Config,
    interval::stacktype::r#trait::{FiveLimitStackType, PeriodicStackType},
    msg,
    neighbourhood::AlignedPeriodicNeighbourhood,
    tui::grid::{Grid, GridConfig},
    tui::latencyreporter::{LatencyReporter, LatencyReporterConfig},
    tui::r#trait::UIState,
};

pub struct WrappedGrid<T: PeriodicStackType, N: AlignedPeriodicNeighbourhood<T>> {
    grid: Grid<T, N>,
    latencyreporter: LatencyReporter,
}

impl<T: FiveLimitStackType + PeriodicStackType, N: AlignedPeriodicNeighbourhood<T> + Clone>
    UIState<T> for WrappedGrid<T, N>
{
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

#[derive(Clone)]
pub struct WrappedGridConfig<T: PeriodicStackType, N: AlignedPeriodicNeighbourhood<T>> {
    pub gridconfig: GridConfig<T, N>,
    pub latencyreporterconfig: LatencyReporterConfig,
}

impl<T: FiveLimitStackType + PeriodicStackType, N: AlignedPeriodicNeighbourhood<T> + Clone>
    Config<WrappedGrid<T, N>> for WrappedGridConfig<T, N>
{
    fn initialise(config: &Self) -> WrappedGrid<T, N> {
        WrappedGrid {
            grid: <_ as Config<_>>::initialise(&config.gridconfig),
            latencyreporter: <_ as Config<_>>::initialise(&config.latencyreporterconfig),
        }
    }
}
