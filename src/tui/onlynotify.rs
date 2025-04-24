use std::{fmt, io::stdout, sync::mpsc, time::Instant};

use crossterm::{
    event::DisableMouseCapture,
    execute,
    terminal::{disable_raw_mode, LeaveAlternateScreen},
};
use ratatui::prelude::{Frame, Rect};

use crate::{
    config::r#trait::Config,
    interval::stacktype::r#trait::{FiveLimitStackType, StackType},
    msg,
    notename::NoteNameStyle,
    tui::r#trait::UIState,
};

pub struct OnlyNotify {}

impl<T: StackType + fmt::Debug + FiveLimitStackType> UIState<T> for OnlyNotify {
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
            msg::AfterProcess::NoteOn { channel, note, .. } => {
                println!("noteon ch {} note {}", channel, note)
            }
            msg::AfterProcess::NoteOff { channel, note, .. } => {
                println!("noteoff ch {} note {}", channel, note)
            }
            msg::AfterProcess::Retune {
                note, tuning_stack, ..
            } => println!(
                "retune {} {} {} {} {}",
                note,
                tuning_stack.notename(&NoteNameStyle::JohnstonFiveLimitFull),
                tuning_stack.target,
                tuning_stack.actual,
                tuning_stack.absolute_semitones() - tuning_stack.target_absolute_semitones(),
            ),
            _ => {
                //println!("raw message received by UI: {:?}", msg)
            }
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
