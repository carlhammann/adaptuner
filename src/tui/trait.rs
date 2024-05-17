use std::io::Stdout;

use ratatui::prelude::{CrosstermBackend, Terminal};

use crate::{msg, util::dimension::Dimension};

/// A type alias for the terminal type used in this application
pub type Tui = Terminal<CrosstermBackend<Stdout>>;

pub trait UIState<D, T>
where
    D: Dimension + PartialEq,
    T: Dimension + PartialEq,
{
    type Config;
    fn initialise(config: &Self::Config) -> Self;
    fn handle_msg(&mut self, msg: msg::ToUI<D, T>);
}
