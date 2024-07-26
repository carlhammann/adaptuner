use std::{io::Stdout, sync::mpsc, time::Instant};

use ratatui::prelude::{CrosstermBackend, Terminal};

use crate::{interval::stacktype::r#trait::StackType, msg};

/// A type alias for the terminal type used in this application
pub type Tui = Terminal<CrosstermBackend<Stdout>>;

pub trait UIState<T: StackType> {
    fn handle_msg(
        &mut self,
        time: Instant,
        msg: msg::AfterProcess<T>,
        to_process: &mpsc::Sender<(Instant, msg::ToProcess)>,
        tui: &mut Tui,
    );
}
