use std::{fmt, io::stdout, sync::mpsc, time::Instant};

use crossterm::{execute, terminal::*};

use crate::{
    config::r#trait::Config,
    interval::stacktype::r#trait::StackType,
    msg,
    tui::r#trait::{Tui, UIState},
};

pub struct OnlyNotify {}

impl<T: StackType + fmt::Debug> UIState<T> for OnlyNotify {
    fn handle_msg(
        &mut self,
        _time: Instant,
        msg: msg::ToUI<T>,
        _to_process: &mpsc::Sender<(Instant, msg::ToProcess)>,
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
