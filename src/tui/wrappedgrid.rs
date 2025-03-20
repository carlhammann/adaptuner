use std::{hash::Hash, marker::PhantomData, sync::mpsc, time::Instant};

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

pub struct WrappedGrid<T: PeriodicStackType + Eq + Hash, N: AlignedPeriodicNeighbourhood<T>, W: UIState<T>> {
    grid: Grid<T, N>,
    latencyreporter: LatencyReporter,
    special: W,
}

impl<
        T: FiveLimitStackType + PeriodicStackType + Eq + Hash,
        N: AlignedPeriodicNeighbourhood<T> + Clone,
        W: UIState<T>,
    > UIState<T> for WrappedGrid<T, N, W>
{
    fn handle_msg(
        &mut self,
        time: Instant,
        msg: &msg::AfterProcess<T>,
        to_process: &mpsc::Sender<(Instant, crate::msg::ToProcess)>,
        frame: &mut Frame,
        area: Rect,
    ) {
        let layout = Layout::vertical([
            Constraint::Percentage(100),
            Constraint::Min(1),
            Constraint::Min(1),
        ]);
        let [grid_area, special_area, latency_area] = layout.areas(area);
        self.grid
            .handle_msg(time, msg, to_process, frame, grid_area);
        self.special
            .handle_msg(time, msg, to_process, frame, special_area);
        self.latencyreporter
            .handle_msg(time, msg, to_process, frame, latency_area);
    }
}

#[derive(Clone)]
pub struct WrappedGridConfig<
    T: PeriodicStackType + Eq + Hash,
    N: AlignedPeriodicNeighbourhood<T>,
    W: UIState<T>,
    WC: Config<W>,
> {
    pub gridconfig: GridConfig<T, N>,
    pub latencyreporterconfig: LatencyReporterConfig,
    pub special_config: WC,
    pub _phantom: PhantomData<(T, W)>,
}

impl<
        T: FiveLimitStackType + PeriodicStackType + Eq + Hash,
        N: AlignedPeriodicNeighbourhood<T> + Clone,
        W: UIState<T>,
        WC: Config<W>,
    > Config<WrappedGrid<T, N, W>> for WrappedGridConfig<T, N, W, WC>
{
    fn initialise(config: &Self) -> WrappedGrid<T, N, W> {
        WrappedGrid {
            grid: <_ as Config<_>>::initialise(&config.gridconfig),
            latencyreporter: <_ as Config<_>>::initialise(&config.latencyreporterconfig),
            special: <_ as Config<_>>::initialise(&config.special_config),
        }
    }
}
