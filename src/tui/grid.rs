use std::{fmt, sync::mpsc, time::Instant};

use colorous;
use crossterm::event::{KeyCode, KeyEventKind};
use ratatui::{prelude::*, widgets::WidgetRef};

use crate::{
    config::r#trait::Config,
    interval::{Semitones, Stack, StackCoeff},
    msg,
    neighbourhood::Neighbourhood,
    notename::NoteNameStyle,
    tui::r#trait::{Tui, UIState},
    util::dimension::{vector_from_elem, AtLeast, Bounded, Dimension, Vector},
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
        return 50; // 0; // bright colors - black font
    } else {
        return 200; // 255; // dark colors - white font
    }
}

pub struct Grid<D: Dimension + AtLeast<3>, T: Dimension> {
    pub width: StackCoeff,
    pub height: StackCoeff,
    pub reference: Stack<D, T>,
    pub active_temperaments: Vector<T, bool>,

    pub neighbourhood: Neighbourhood<D>,
    pub active_classes: [bool; 12],

    pub config: DisplayConfig,
}

impl<D, T> Widget for Grid<D, T>
where
    D: Dimension + AtLeast<3> + Clone + Copy + fmt::Debug,
    T: Dimension + Copy,
{
    fn render(self, area: Rect, buf: &mut Buffer) {
        self.render_ref(area, buf)
    }
}

impl<D, T> WidgetRef for Grid<D, T>
where
    D: Dimension + AtLeast<3> + Clone + Copy + fmt::Debug,
    T: Dimension + Copy,
{
    /// rendering of Grids expects there to be 2n rows of characters for an n-row grid, because
    /// Cells are two rows high
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        let mut the_stack = self.reference.clone();

        let origin_fifth = the_stack.coefficients()[Bounded::new(1).unwrap()];
        let origin_third = the_stack.coefficients()[Bounded::new(2).unwrap()];

        let (mut min_fifth, mut max_fifth) = self.neighbourhood.bounds(Bounded::new(1).unwrap());
        let (mut min_third, mut max_third) = self.neighbourhood.bounds(Bounded::new(2).unwrap());

        if max_fifth - min_fifth != self.width {
            min_fifth = origin_fifth - self.width / 2;
            max_fifth = min_fifth + self.width - 1;
        }
        
        if max_third - min_third != self.height {
            min_third = origin_third - self.height / 2;
            max_third = min_third + self.height - 1;
        }

        let cols = 1 + max_fifth - min_fifth;
        let cellwidth = area.width / cols as u16;

        the_stack.increment_at_index(
            &self.active_temperaments,
            Bounded::new(2).unwrap(),
            min_third - self.reference.coefficients()[Bounded::new(2).unwrap()],
        );
        the_stack.increment_at_index(
            &self.active_temperaments,
            Bounded::new(1).unwrap(),
            min_fifth - self.reference.coefficients()[Bounded::new(1).unwrap()],
        );

        for i in min_third..=max_third {
            for j in min_fifth..=max_fifth {
                let mut state = CellState::Off;
                for k in 0..12 {
                    if self.neighbourhood.coefficients[k][Bounded::new(1).unwrap()] + origin_fifth == j
                        && self.neighbourhood.coefficients[k][Bounded::new(2).unwrap()] + origin_third == i
                    {
                        if self.active_classes[k] {
                            state = CellState::On;
                        } else {
                            state = CellState::Considered;
                        }
                    }
                }
                render_stack(
                    &the_stack,
                    state,
                    &self.config,
                    Rect {
                        x: area.x + cellwidth * (j - min_fifth) as u16,
                        y: area.y + 2 * (max_third - i) as u16,
                        width: cellwidth,
                        height: 2,
                    },
                    buf,
                );
                the_stack.increment_at_index(
                    &self.active_temperaments,
                    Bounded::new(1).unwrap(),
                    1,
                );
            }
            the_stack.increment_at_index(&self.active_temperaments, Bounded::new(2).unwrap(), 1);
            the_stack.increment_at_index(
                &self.active_temperaments,
                Bounded::new(1).unwrap(),
                min_fifth - max_fifth - 1,
            );
        }
    }
}

fn render_stack<D, T>(
    stack: &Stack<D, T>,
    state: CellState,
    config: &DisplayConfig,
    area: Rect,
    buf: &mut Buffer,
) where
    D: Dimension + AtLeast<3> + fmt::Debug + Copy,
    T: Dimension + Copy,
{
    // Rendering grid cells expects that we have two rows.
    buf.set_string(
        area.x,
        area.y,
        stack.notename(&config.notenamestyle),
        Style::default(),
    );
    let deviation = stack.correction_semitones();
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

impl<D, T> UIState<D, T> for Grid<D, T>
where
    D: Dimension + AtLeast<3> + PartialEq + fmt::Debug + Copy,
    T: Dimension + PartialEq + Copy,
{
    fn handle_msg(
        &mut self,
        time: Instant,
        msg: msg::ToUI<D, T>,
        to_process: &mpsc::Sender<(Instant, msg::ToProcess<D, T>)>,
        tui: &mut Tui,
    ) {
        let draw_frame = |t: &mut Tui, g: &Self| {
            t.draw(|frame| frame.render_widget(g, frame.size()))
                .expect("");
            0
        };

        let send_to_process =
            |msg: msg::ToProcess<D, T>, time: Instant| to_process.send((time, msg)).unwrap_or(());

        match msg {
            msg::ToUI::Start => {
                draw_frame(tui, self);
            }
            msg::ToUI::Stop => {
                send_to_process(msg::ToProcess::Stop, time);
            }
            msg::ToUI::Notify { .. } => {}
            msg::ToUI::MidiParseErr(_) => {}
            msg::ToUI::DetunedNote { ..
                // note,
                // should_be,
                // actual,
                // explanation,
            } => {}
            msg::ToUI::CrosstermEvent(e) => {
                match e {
                    crossterm::event::Event::Key(k) => if k.kind == KeyEventKind::Press {
                        match k.code {
                            KeyCode::Char('q') => send_to_process(msg::ToProcess::Stop, time),
                            _ => {}
                        }
}
                    _ =>{},
                }
                draw_frame(tui, self);
            }
            msg::ToUI::SetNeighboughood { neighbourhood } => {
                self.neighbourhood = neighbourhood;
                draw_frame(tui, self);
            }
            msg::ToUI::ToggleTemperament { index } => {
                self.active_temperaments[index] = !self.active_temperaments[index];
                draw_frame(tui, self);
            }
            msg::ToUI::SetReference { key: _, stack } => {
                self.reference = stack;
                draw_frame(tui, self);
            }
            msg::ToUI::NoteOn { note } => {
                self.active_classes[(note % 12) as usize] = true;
                draw_frame(tui, self);
            }
            msg::ToUI::NoteOff { note } => {
                self.active_classes[(note % 12) as usize] = false;
                draw_frame(tui, self);
            }
        }
    }
}

#[derive(Clone)]
pub struct GridConfig<D: Dimension + AtLeast<3>, T: Dimension> {
    pub display_config: DisplayConfig,
    pub width: StackCoeff,
    pub height: StackCoeff,
    pub reference: Stack<D, T>,
    pub neighbourhood: Neighbourhood<D>,
}

impl<D, T> Config<Grid<D, T>> for GridConfig<D, T>
where
    D: Dimension + AtLeast<3> + Copy,
    T: Dimension + Copy,
{
    fn initialise(config: &Self) -> Grid<D, T> {
        Grid {
            width: config.width,
            height: config.height,
            reference: config.reference.clone(),
            active_temperaments: vector_from_elem(false),
            neighbourhood: config.neighbourhood.clone(),
            active_classes: [false; 12],
            config: config.display_config.clone(),
        }
    }
}
