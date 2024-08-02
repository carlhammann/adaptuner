use std::{marker::PhantomData, sync::mpsc, time::Instant};

use colorous;
use crossterm::event::{KeyCode, KeyEventKind, MouseButton, MouseEvent, MouseEventKind};
use ratatui::{
    prelude::{Buffer, Color, Frame, Rect, Style, Widget},
    widgets::WidgetRef,
};

use crate::{
    config::r#trait::Config,
    interval::{
        interval::Semitones,
        stack::Stack,
        stacktype::r#trait::{FiveLimitStackType, PeriodicStackType, StackCoeff},
    },
    msg,
    neighbourhood::AlignedPeriodicNeighbourhood,
    notename::NoteNameStyle,
    tui::r#trait::UIState,
};

#[derive(Clone)]
pub struct DisplayConfig {
    pub notenamestyle: NoteNameStyle,
    pub color_range: Semitones,
    pub gradient: colorous::Gradient,
}

enum CellState {
    Off,
    Considered,
    On,
}

fn foreground_for_background(r: u8, g: u8, b: u8) -> u8 {
    // Counting the perceptive luminance - human eye favors green color...
    let luminance = (0.299 * r as f32 + 0.587 * g as f32 + 0.114 * b as f32) / 255.0;

    if luminance > 0.5 {
        return 0; // bright colors - black font
    } else {
        return 255; // dark colors - white font
    }
}

#[derive(Copy, Clone, PartialEq)]
enum NoteOnState {
    Pressed,
    Sustained,
}

pub struct Grid<T: PeriodicStackType, N: AlignedPeriodicNeighbourhood<T>> {
    horizontal_index: usize,
    vertical_index: usize,
    column_width: u16,
    min_horizontal: StackCoeff,
    max_horizontal: StackCoeff,
    min_vertical: StackCoeff,
    max_vertical: StackCoeff,

    active_temperaments: Vec<bool>,

    // the stacks are relative to the initial_reference_key
    active_notes: Vec<(u8, Stack<T>, NoteOnState)>, // TODO: this sometimes does weird things, because we don't track which channel(s) the notes are on
    considered_notes: N,

    reference_key: i8,
    reference_stack: Stack<T>,

    horizontal_margin: StackCoeff,
    vertical_margin: StackCoeff,

    config: GridConfig<T, N>,
}

impl<T: FiveLimitStackType + PeriodicStackType, N: AlignedPeriodicNeighbourhood<T>> Widget
    for Grid<T, N>
{
    fn render(self, area: Rect, buf: &mut Buffer) {
        self.render_ref(area, buf)
    }
}

impl<T: PeriodicStackType, N: AlignedPeriodicNeighbourhood<T>> Grid<T, N> {
    fn recalculate_dimensions(&mut self, area: &Rect) {
        let horizontal_index = self.horizontal_index;
        let vertical_index = self.vertical_index;

        let origin_horizontal = self.reference_stack.coefficients()[horizontal_index];
        let origin_vertical = self.reference_stack.coefficients()[vertical_index];

        let (mut min_horizontal, mut max_horizontal) = (origin_horizontal, origin_horizontal);
        let (mut min_vertical, mut max_vertical) = (origin_vertical, origin_vertical);

        for (_, stack, _) in &self.active_notes {
            let hor = stack.coefficients()[horizontal_index];
            if hor < min_horizontal {
                min_horizontal = hor;
            }
            if hor > max_horizontal {
                max_horizontal = hor;
            }
            let ver = stack.coefficients()[vertical_index];
            if ver < min_vertical {
                min_vertical = ver;
            }
            if ver > max_vertical {
                max_vertical = ver;
            }
        }
        self.considered_notes.for_each_stack(|_, stack| {
            let hor = stack.coefficients()[horizontal_index] + origin_horizontal;
            if hor < min_horizontal {
                min_horizontal = hor;
            }
            if hor > max_horizontal {
                max_horizontal = hor;
            }
            let ver = stack.coefficients()[vertical_index] + origin_vertical;
            if ver < min_vertical {
                min_vertical = ver;
            }
            if ver > max_vertical {
                max_vertical = ver;
            }
        });

        max_vertical += self.vertical_margin;
        min_vertical -= self.vertical_margin;
        max_horizontal += self.horizontal_margin;
        min_horizontal -= self.horizontal_margin;

        // each cell must be at exactly two characters tall:
        let max_rows = area.height as StackCoeff / 2;
        if max_vertical - min_vertical + 1 > max_rows {
            min_vertical = origin_vertical - max_rows / 2;
            max_vertical = min_vertical + max_rows - 1;
        }

        // each cell must be at least four characters wide:
        let max_cols = area.width as StackCoeff / 4;
        if max_horizontal - min_horizontal + 1 > max_cols {
            min_horizontal = origin_horizontal - max_cols / 2;
            max_horizontal = min_horizontal + max_cols - 1;
        }

        let cols = 1 + max_horizontal - min_horizontal;

        self.column_width = area.width / cols as u16;
        self.min_horizontal = min_horizontal;
        self.max_horizontal = max_horizontal;
        self.min_vertical = min_vertical;
        self.max_vertical = max_vertical;
    }
}

impl<T: FiveLimitStackType + PeriodicStackType, N: AlignedPeriodicNeighbourhood<T>> WidgetRef
    for Grid<T, N>
{
    /// This expects [recalculate_dimensions][Grid::recalculate_dimensions] to be called first.
    /// Otherwise, expect bad things to happen!    
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        let mut the_stack = self.reference_stack.clone();
        the_stack.increment_at_index(
            &self.active_temperaments,
            self.vertical_index,
            self.min_vertical - self.reference_stack.coefficients()[self.vertical_index],
        );
        the_stack.increment_at_index(
            &self.active_temperaments,
            self.horizontal_index,
            self.min_horizontal - self.reference_stack.coefficients()[self.horizontal_index],
        );

        for i in self.min_vertical..=self.max_vertical {
            for j in self.min_horizontal..=self.max_horizontal {
                render_stack(
                    &the_stack,
                    CellState::Off,
                    &self.config.display_config,
                    Rect {
                        x: area.x + self.column_width * (j - self.min_horizontal) as u16,
                        y: area.y + 2 * (self.max_vertical - i) as u16,
                        width: self.column_width,
                        height: 2,
                    },
                    buf,
                );
                the_stack.increment_at_index(&self.active_temperaments, self.horizontal_index, 1);
            }
            the_stack.increment_at_index(&self.active_temperaments, self.vertical_index, 1);
            the_stack.increment_at_index(
                &self.active_temperaments,
                self.horizontal_index,
                self.min_horizontal - self.max_horizontal - 1,
            );
        }
        self.considered_notes.for_each_stack(|_, relative_stack| {
            the_stack.clone_from(relative_stack);
            the_stack.add_mul(1, &self.reference_stack);
            let i = the_stack.coefficients()[self.vertical_index];
            let j = the_stack.coefficients()[self.horizontal_index];
            if !(i < self.min_vertical
                || i > self.max_vertical
                || j < self.min_horizontal
                || j > self.max_horizontal)
            {
                render_stack(
                    &the_stack,
                    CellState::Considered,
                    &self.config.display_config,
                    Rect {
                        x: area.x + self.column_width * (j - self.min_horizontal) as u16,
                        y: area.y + 2 * (self.max_vertical - i) as u16,
                        width: self.column_width,
                        height: 2,
                    },
                    buf,
                );
            }
        });
        for (_, the_stack, _) in &self.active_notes {
            let i = the_stack.coefficients()[self.vertical_index];
            let j = the_stack.coefficients()[self.horizontal_index];
            if i < self.min_vertical
                || i > self.max_vertical
                || j < self.min_horizontal
                || j > self.max_horizontal
            {
                break;
            }
            render_stack(
                &the_stack,
                CellState::On,
                &self.config.display_config,
                Rect {
                    x: area.x + self.column_width * (j - self.min_horizontal) as u16,
                    y: area.y + 2 * (self.max_vertical - i) as u16,
                    width: self.column_width,
                    height: 2,
                },
                buf,
            );
        }
    }
}

fn render_stack<T: FiveLimitStackType>(
    stack: &Stack<T>,
    state: CellState,
    config: &DisplayConfig,
    area: Rect,
    buf: &mut Buffer,
) {
    // Rendering grid cells expects that we have two rows.

    // reset all cells in the area.
    for pos in area.positions() {
        buf.get_mut(pos.x, pos.y).reset()
    }

    buf.set_string(
        area.x,
        area.y,
        stack.notename(&config.notenamestyle),
        Style::default(),
    );
    let deviation = stack.impure_semitones();
    if !stack.is_pure() {
        buf.set_string(
            area.x,
            area.y + 1,
            format!("{d:+.0}", d = deviation * 100.0),
            Style::default(),
        );
    }
    let t = ((deviation / config.color_range).max(-1.0).min(1.0) + 1.0) / 2.0; // in range 0..1
    let colorous::Color { r, g, b } = config.gradient.eval_continuous(t);
    let style = match state {
        CellState::Off => {
            let f = foreground_for_background(r / 4, g / 4, b / 4) / 2;
            Style::from((Color::Rgb(f, f, f), Color::Rgb(r / 4, g / 4, b / 4)))
        }
        CellState::Considered => {
            let f = foreground_for_background(r / 2, g / 2, b / 2);
            Style::from((Color::Rgb(f, f, f), Color::Rgb(r / 2, g / 2, b / 2)))
        }
        CellState::On => {
            let f = foreground_for_background(r, g, b);
            Style::from((Color::Rgb(f, f, f), Color::Rgb(r, g, b)))
        }
    };
    buf.set_style(area, style);
}

impl<T: FiveLimitStackType + PeriodicStackType, N: AlignedPeriodicNeighbourhood<T> + Clone>
    UIState<T> for Grid<T, N>
{
    fn handle_msg(
        &mut self,
        time: Instant,
        msg: &msg::AfterProcess<T>,
        to_process: &mpsc::Sender<(Instant, msg::ToProcess)>,
        frame: &mut Frame,
        area: Rect,
    ) {
        // let draw_frame = |t: &mut Tui, g: &mut Self| {
        //     let mut frame = t.get_frame();
        //     g.recalculate_dimensions(&area);
        //     frame.render_widget(&*g, area);
        // };

        // let other_draw_fra reference_stack.coefficients()[self.vertical_index];e = || {
        //     tui.draw(|frame| frame.render_widget(self, frame.size()))
        // };

        let send_to_process =
            |msg: msg::ToProcess, time: Instant| to_process.send((time, msg)).unwrap_or(());

        match msg {
            msg::AfterProcess::Start => {}
            msg::AfterProcess::Stop => {
                send_to_process(msg::ToProcess::Stop, time);
            }
            msg::AfterProcess::Notify { .. } => {}
            msg::AfterProcess::MidiParseErr(_) => {}
            msg::AfterProcess::DetunedNote { .. } => {}
            msg::AfterProcess::CrosstermEvent(e) => {
                match e {
                    crossterm::event::Event::Key(k) => {
                        if k.kind == KeyEventKind::Press {
                            match k.code {
                                KeyCode::Char('q') => send_to_process(msg::ToProcess::Stop, time),
                                KeyCode::Esc => {
                                    *self = GridConfig::initialise(&self.config);
                                    send_to_process(msg::ToProcess::Reset, time);
                                }

                                // KeyCode::Down => {
                                //     self.neighbourhood.fivelimit_inc_index();
                                //     send_to_process(msg::ToProcess::SetNeighboughood {neighbourhood: self.neighbourhood.clone()}, time);
                                // }
                                // KeyCode::Up => {
                                //     self.neighbourhood.fivelimit_dec_index();
                                //     send_to_process(msg::ToProcess::SetNeighboughood {neighbourhood: self.neighbourhood.clone()}, time);
                                //
                                // }
                                // KeyCode::Left => {
                                //     self.neighbourhood.fivelimit_inc_offset();
                                //     send_to_process(msg::ToProcess::SetNeighboughood {neighbourhood: self.neighbourhood.clone()}, time);
                                //
                                // }
                                // KeyCode::Right => {
                                //     self.neighbourhood.fivelimit_dec_offset();
                                //     send_to_process(msg::ToProcess::SetNeighboughood {neighbourhood: self.neighbourhood.clone()}, time);
                                //
                                // }
                                KeyCode::Char('+') => {
                                    self.vertical_margin += 1;
                                    self.horizontal_margin += 1;
                                }
                                KeyCode::Char('-') => {
                                    if self.vertical_margin >= 1 {
                                        self.vertical_margin -= 1;
                                    }
                                    if self.horizontal_margin >= 1 {
                                        self.horizontal_margin -= 1;
                                    }
                                }

                                KeyCode::Char(' ') => {
                                    send_to_process(msg::ToProcess::Special { code: 2 }, time);
                                }

                                KeyCode::Char('t') => {
                                    send_to_process(msg::ToProcess::Special { code: 1 }, time);
                                }

                                KeyCode::Char('1') => {
                                    let index = 0;
                                    self.active_temperaments[index] =
                                        !self.active_temperaments[index];
                                    send_to_process(
                                        msg::ToProcess::ToggleTemperament { index },
                                        time,
                                    );
                                }
                                KeyCode::Char('2') => {
                                    let index = 1;
                                    self.active_temperaments[index] =
                                        !self.active_temperaments[index];
                                    send_to_process(
                                        msg::ToProcess::ToggleTemperament { index },
                                        time,
                                    );
                                }
                                _ => {}
                            }
                        }
                    }
                    crossterm::event::Event::Mouse(MouseEvent {
                        kind: MouseEventKind::Down(MouseButton::Left),
                        column,
                        row,
                        modifiers: _,
                    }) => {
                        let horizontal_offset = self.min_horizontal
                            + *column as StackCoeff / self.column_width as StackCoeff
                            - self.reference_stack.coefficients()[self.horizontal_index];
                        let vertical_offset = self.max_vertical
                            - *row as StackCoeff / 2
                            - self.reference_stack.coefficients()[self.vertical_index];

                        let mut coefficients = vec![0; T::num_intervals()];
                        coefficients[self.vertical_index] = vertical_offset;
                        coefficients[self.horizontal_index] = horizontal_offset;

                        send_to_process(msg::ToProcess::Consider { coefficients }, time);
                    }
                    _ => {}
                }
            }
            msg::AfterProcess::SetReference { key, stack } => {
                self.reference_key = *key as i8;
                self.reference_stack.clone_from(&stack);
            }

            msg::AfterProcess::TunedNoteOn {
                note, tuning_stack, ..
            } => {
                let mut inserted = false;
                for (key, stack, _) in &mut self.active_notes {
                    if *key == *note {
                        inserted = true;
                        stack.clone_from(&tuning_stack);
                        break;
                    }
                }
                if !inserted {
                    self.active_notes
                        .push((*note, tuning_stack.clone(), NoteOnState::Pressed));
                }
            }
            msg::AfterProcess::Retune {
                note, tuning_stack, ..
            } => {
                for (key, stack, _) in &mut self.active_notes {
                    if *key == *note {
                        stack.clone_from(&tuning_stack);
                        break;
                    }
                }
            }
            msg::AfterProcess::NoteOff {
                held_by_sustain,
                note,
                ..
            } => {
                if let Some(index) = self
                    .active_notes
                    .iter()
                    .position(|(key, _, _)| *key == *note)
                {
                    if *held_by_sustain {
                        self.active_notes[index].2 = NoteOnState::Sustained;
                    } else {
                        self.active_notes.swap_remove(index);
                    }
                }
            }

            msg::AfterProcess::Consider { stack } => {
                self.considered_notes.insert(stack);
            }
            msg::AfterProcess::Reset => {}
            msg::AfterProcess::Sustain { value, .. } => {
                if *value == 0 {
                    self.active_notes
                        .retain(|(_, _, s)| *s != NoteOnState::Sustained);
                }
            }
            msg::AfterProcess::ProgramChange { .. } => {}
            msg::AfterProcess::ForwardMidi { .. } => {}
            msg::AfterProcess::BackendLatency { .. } => {}
            msg::AfterProcess::NotifyFit { .. } => {}
            msg::AfterProcess::NotifyNoFit => {}
        }
        self.recalculate_dimensions(&area);
        frame.render_widget(&*self, area);
    }
}

#[derive(Clone)]
pub struct GridConfig<T: PeriodicStackType, N: AlignedPeriodicNeighbourhood<T>> {
    pub horizontal_index: usize,
    pub vertical_index: usize,
    pub fifth_index: usize,
    pub third_index: usize,

    pub display_config: DisplayConfig,

    pub initial_reference_key: i8,
    pub initial_neighbourhood: N,
    pub _phantom: PhantomData<T>,
}

impl<T: FiveLimitStackType + PeriodicStackType, N: AlignedPeriodicNeighbourhood<T> + Clone>
    Config<Grid<T, N>> for GridConfig<T, N>
{
    fn initialise(config: &Self) -> Grid<T, N> {
        let no_active_temperaments = vec![false; T::num_temperaments()];
        Grid {
            horizontal_index: config.horizontal_index,
            vertical_index: config.vertical_index,

            reference_key: config.initial_reference_key,
            reference_stack: Stack::new(&no_active_temperaments, vec![0; T::num_intervals()]),
            active_temperaments: no_active_temperaments,

            considered_notes: config.initial_neighbourhood.clone(),

            active_notes: Vec::with_capacity(12),
            config: config.clone(),

            horizontal_margin: 1,
            vertical_margin: 1,
            min_horizontal: -1,
            max_horizontal: 1,
            min_vertical: -1,
            max_vertical: 1,
            column_width: 0, // this will be changed. I initialise to zero to make division panic
                             // if not
        }
    }
}
