use std::{fmt, io::stdout, sync::mpsc, time::Instant};

use crossterm::{
    event::DisableMouseCapture,
    execute,
    terminal::{disable_raw_mode, LeaveAlternateScreen},
};
use ratatui::prelude::{Frame, Rect};

use midi_msg::{
    ChannelVoiceMsg::{NoteOff, NoteOn},
    MidiMsg,
};

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
            msg::AfterProcess::ForwardMidi { msg } => match msg {
                MidiMsg::ChannelVoice {
                    channel,
                    msg: NoteOn { note, .. },
                } => {
                    println!("noteon ch {} note {}", channel, note)
                }
                MidiMsg::ChannelVoice {
                    channel,
                    msg: NoteOff { note, .. },
                } => {
                    println!("noteoff ch {} note {}", channel, note)
                }
                _ => {}
            },
            msg::AfterProcess::FromStrategy(msg) => match msg {
                msg::FromStrategy::Retune {
                    note,
                    tuning: _,
                    tuning_stack,
                } => println!(
                    "retune {} {} {} {} {}",
                    note,
                    tuning_stack.notename(&NoteNameStyle::JohnstonFiveLimitFull),
                    tuning_stack.target,
                    tuning_stack.actual,
                    tuning_stack.absolute_semitones() - tuning_stack.target_absolute_semitones(),
                ),
                _ => {}
            },
            msg::AfterProcess::BackendLatency { since_input } => {
                println!("latency {since_input:?}")
            }
            msg::AfterProcess::DetunedNote {
                note,
                should_be,
                actual,
                explanation,
            } => println!(
                "detuned {note} should be {should_be} but is {actual}. Reason: {explanation}"
            ),
            _ => {}
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
