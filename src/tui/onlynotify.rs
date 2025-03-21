use std::{fmt, hash::Hash, io::stdout, sync::mpsc, time::Instant};

use crossterm::{
    event::DisableMouseCapture,
    execute,
    terminal::{disable_raw_mode, LeaveAlternateScreen},
};
use ratatui::prelude::{Frame, Rect};

use crate::{
    config::r#trait::Config, interval::stacktype::r#trait::StackType, msg, tui::r#trait::UIState,
};

pub struct OnlyNotify {}

impl<T: StackType + fmt::Debug + Eq + Hash> UIState<T> for OnlyNotify {
    fn handle_msg(
        &mut self,
        _time: Instant,
        msg: &msg::AfterProcess<T>,
        _to_process: &mpsc::Sender<(Instant, msg::ToProcess)>,
        _tui: &mut Frame,
        _area: Rect,
    ) {
        match msg {
            msg::AfterProcess::Start => {
                execute!(stdout(), LeaveAlternateScreen).expect("Could not leave alternate screen");
                execute!(stdout(), DisableMouseCapture).expect("Could not disable mouse capture");
                disable_raw_mode().expect("Could not disable raw mode");
            }
            msg::AfterProcess::Notify { line } => println!("{}", line),
            msg::AfterProcess::BackendLatency { .. } => {}
            _ => println!("raw message received by UI: {:?}", msg),
        }
    }
}

#[derive(Clone)]
pub struct OnlyNotifyConfig {}

impl Config<OnlyNotify> for OnlyNotifyConfig {
    fn initialise(_: &Self) -> OnlyNotify {
        OnlyNotify {}
    }
}
