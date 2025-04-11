use std::{sync::mpsc, time::Instant};

use crossterm::event::{Event, KeyCode, KeyEventKind};
use ratatui::prelude::*;

use crate::{
    config::r#trait::Config,
    interval::{stack::Stack, stacktype::r#trait::FiveLimitStackType},
    msg,
    notename::NoteNameStyle,
    process::walking::{
        PATTERNS_DISABLED, PATTERNS_ENABLED, TOGGLE_PATTERNS, TOGGLE_TEMPER_PATTERN_NEIGHBOURHOODS,
        UPDATE_KEY_CENTER,
    },
    tui::r#trait::UIState,
};

pub struct Walking<T: FiveLimitStackType> {
    config: WalkingConfig<T>,
    current_fit: Option<(String, Stack<T>)>,
    notenamestyle: NoteNameStyle,
    key_center: Stack<T>,
    use_patterns: bool,
}

impl<T: FiveLimitStackType> Walking<T> {
    fn reset(&mut self) {
        self.key_center.clone_from(&self.config.initial_key_center);
        self.current_fit = None;
    }
}

impl<T: FiveLimitStackType> UIState<T> for Walking<T> {
    fn handle_msg(
        &mut self,
        time: Instant,
        msg: &msg::AfterProcess<T>,
        to_process: &mpsc::Sender<(Instant, crate::msg::ToProcess)>,
        frame: &mut Frame,
        area: Rect,
    ) {
        let send_to_process =
            |msg: msg::ToProcess, time: Instant| to_process.send((time, msg)).unwrap_or(());

        match msg {
            msg::AfterProcess::Reset => self.reset(),
            msg::AfterProcess::NotifyFit {
                pattern_name,
                reference_stack,
            } => {
                self.current_fit = Some((pattern_name.clone(), reference_stack.clone()));
            }
            msg::AfterProcess::NotifyNoFit => self.current_fit = None,
            msg::AfterProcess::SetReference { stack, .. } => self.key_center.clone_from(stack),

            msg::AfterProcess::CrosstermEvent(e) => match e {
                Event::Key(k) => {
                    if k.kind == KeyEventKind::Press {
                        match k.code {
                            KeyCode::Char('q') => send_to_process(msg::ToProcess::Stop, time),
                            KeyCode::Esc => {
                                self.reset();
                                send_to_process(msg::ToProcess::Reset, time);
                            }

                            KeyCode::Char('p') => {
                                send_to_process(
                                    msg::ToProcess::Special {
                                        code: TOGGLE_PATTERNS,
                                    },
                                    time,
                                );
                            }

                            KeyCode::Char(' ') => {
                                send_to_process(
                                    msg::ToProcess::Special {
                                        code: UPDATE_KEY_CENTER,
                                    },
                                    time,
                                );
                            }

                            KeyCode::Char('t') => {
                                send_to_process(
                                    msg::ToProcess::Special {
                                        code: TOGGLE_TEMPER_PATTERN_NEIGHBOURHOODS,
                                    },
                                    time,
                                );
                            }

                            _ => {}
                        }
                    }
                }
                _ => {}
            },

            msg::AfterProcess::Special { code } => {
                if *code == PATTERNS_ENABLED {
                    self.use_patterns = true;
                }
                if *code == PATTERNS_DISABLED {
                    self.use_patterns = false;
                }
            }

            _ => {}
        }
        frame.render_widget(
            Line::from({
                let mut str = match &self.current_fit {
                    None => format!(
                        "key center: {}, no current fit",
                        self.key_center.notename(&self.notenamestyle)
                    ),

                    Some((name, stack)) => format!(
                        "key center: {}, current fit: {}, reference: {}",
                        self.key_center.notename(&self.notenamestyle),
                        name,
                        stack.notename(&self.notenamestyle)
                    ),
                };
                if !self.use_patterns {
                    str.push_str(&", patterns disabled");
                }
                str
            }),
            area,
        );
    }
}

#[derive(Clone)]
pub struct WalkingConfig<T: FiveLimitStackType> {
    pub notenamestyle: NoteNameStyle,
    pub initial_key_center: Stack<T>,
    pub use_patterns: bool,
}

impl<T: FiveLimitStackType> Config<Walking<T>> for WalkingConfig<T> {
    fn initialise(config: &Self) -> Walking<T> {
        Walking {
            config: config.clone(),
            current_fit: None,
            notenamestyle: config.notenamestyle,
            key_center: config.initial_key_center.clone(),
            use_patterns: config.use_patterns,
        }
    }
}
