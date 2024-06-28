use std::{fmt, sync::mpsc};

use crate::{config::r#trait::Config, msg, tui::r#trait::UIState, util::dimension::Dimension};

pub struct OnlyNotify {}

impl<D, T> UIState<D, T> for OnlyNotify
where
    D: Dimension + fmt::Debug,
    T: Dimension + fmt::Debug,
{
    fn handle_msg(
        &mut self,
        _time: u64,
        msg: msg::ToUI<D, T>,
        _to_process: &mpsc::Sender<(u64, msg::ToProcess<D, T>)>,
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
