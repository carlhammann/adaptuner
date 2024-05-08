use std::io::Stdout;

use ratatui::prelude::{CrosstermBackend, Terminal};

use crate::msg;

/// A type alias for the terminal type used in this application
pub type Tui = Terminal<CrosstermBackend<Stdout>>;

pub trait UIState {
    fn handle_msg(&mut self, time: u64, msg: msg::ToUI, terminal: &mut Tui);
}
