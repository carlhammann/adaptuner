use std::{fmt, io::stdout, sync::mpsc, time::Instant};

use crossterm::{execute, terminal::*};

use crate::{
    config::r#trait::Config,
    msg,
    tui::r#trait::{Tui, UIState},
    util::dimension::Dimension,
};

pub struct OnlyNotify {}

impl<D, T> UIState<D, T> for OnlyNotify
where
    D: Dimension + fmt::Debug,
    T: Dimension + fmt::Debug,
{
    fn handle_msg(
        &mut self,
        _time: Instant,
        msg: msg::ToUI<D, T>,
        _to_process: &mpsc::Sender<(Instant, msg::ToProcess<D, T>)>,
        _tui: &mut Tui,
    ) {
        match msg {
            msg::ToUI::Start => {
                execute!(stdout(), LeaveAlternateScreen).expect("Could not leave alternate screen");
                disable_raw_mode().expect("Could not disable raw mode");
            }
            msg::ToUI::Notify { line } => println!("{}", line),
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
