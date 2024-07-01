use std::{fmt, sync::mpsc, time::Instant};

use crate::{config::r#trait::Config, msg, tui::r#trait::UIState, util::dimension::Dimension};

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
    ) {
        match msg {
            crate::msg::ToUI::Notify { line } => println!("{}", line),
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
