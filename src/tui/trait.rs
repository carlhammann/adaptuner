use std::{io::Stdout, sync::mpsc, time::Instant};

use ratatui::prelude::{CrosstermBackend, Terminal};

use crate::{msg, util::dimension::Dimension};

/// A type alias for the terminal type used in this application
pub type Tui = Terminal<CrosstermBackend<Stdout>>;

pub trait UIState<D: Dimension, T: Dimension> {
    fn handle_msg(
        &mut self,
        time: Instant,
        msg: msg::ToUI<D, T>,
        to_process: &mpsc::Sender<(Instant, msg::ToProcess<D, T>)>,
    );
}
