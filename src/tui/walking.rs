use std::{sync::mpsc, time::Instant};

use ratatui::prelude::*;

use crate::{
    config::r#trait::Config,
    interval::{
        stack::Stack,
        stacktype::r#trait::{FiveLimitStackType, StackType},
    },
    msg,
    notename::NoteNameStyle,
    tui::r#trait::UIState,
};

pub struct Walking<T: FiveLimitStackType> {
    config: WalkingConfig<T>,
    current_fit: Option<(String, Stack<T>)>,
    notenamestyle: NoteNameStyle,
    key_center: Stack<T>,
}

impl<T: FiveLimitStackType> UIState<T> for Walking<T> {
    fn handle_msg(
        &mut self,
        _time: Instant,
        msg: &msg::AfterProcess<T>,
        _to_process: &mpsc::Sender<(Instant, crate::msg::ToProcess)>,
        frame: &mut Frame,
        area: Rect,
    ) {
        match msg {
            msg::AfterProcess::Reset => {
                self.key_center.clone_from(&self.config.initial_key_center);
                self.current_fit = None;
            }
            msg::AfterProcess::NotifyFit {
                pattern_name,
                reference_stack,
            } => {
                self.current_fit = Some((pattern_name.clone(), reference_stack.clone()));
            }
            msg::AfterProcess::NotifyNoFit => self.current_fit = None,
            msg::AfterProcess::SetReference { stack, .. } => self.key_center.clone_from(stack),
            _ => {}
        }
        frame.render_widget(
            Line::from(match &self.current_fit {
                None => format!(
                    "key center: {}, no current fit",
                    self.key_center.notename(&self.notenamestyle)
                ),
                Some((name, stack)) => {
                    format!(
                        "key center: {}, current fit: {}, reference: {}",
                        self.key_center.notename(&self.notenamestyle),
                        name,
                        stack.notename(&self.notenamestyle)
                    )
                }
            }),
            area,
        );
    }
}

#[derive(Clone)]
pub struct WalkingConfig<T: FiveLimitStackType> {
    pub notenamestyle: NoteNameStyle,
    pub initial_key_center: Stack<T>,
}

impl<T: FiveLimitStackType> Config<Walking<T>> for WalkingConfig<T> {
    fn initialise(config: &Self) -> Walking<T> {
        Walking {
            config: config.clone(),
            current_fit: None,
            notenamestyle: config.notenamestyle,
            key_center: config.initial_key_center.clone(),
        }
    }
}
