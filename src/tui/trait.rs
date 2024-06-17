use std::{sync::mpsc, io::Stdout};

use ratatui::prelude::{CrosstermBackend, Terminal};

use crate::{msg, util::dimension::Dimension};

/// A type alias for the terminal type used in this application
pub type Tui = Terminal<CrosstermBackend<Stdout>>;

pub trait UIState<D: Dimension, T: Dimension> {
    fn handle_msg(&mut self, time: u64, msg: msg::ToUI<D, T>, to_process: &mpsc::Sender<(u64, msg::ToProcess<D,T>)>);
}
