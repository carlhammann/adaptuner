use std::{sync::mpsc, fmt};

use colorous;
use ratatui::{
    prelude::{Buffer, Color, Rect, Style, Widget},
    widgets::WidgetRef,
};

use crate::{
    interval::{Semitones, Stack, StackCoeff},
    msg,
    neighbourhood::Neighbourhood,
    notename::NoteNameStyle,
    tui::r#trait::UIState,
    util::dimension::{AtLeast, Bounded, Dimension, Vector},
};

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
    pub min_fifth: StackCoeff,
    pub min_third: StackCoeff,
    pub max_fifth: StackCoeff,
    pub max_third: StackCoeff,
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
        let cols = 1 + self.max_fifth - self.min_fifth;
        let cellwidth = area.width / cols as u16;

        let mut the_stack = self.reference.clone();
        the_stack.increment_at_index(
            &self.active_temperaments,
            Bounded::new(2).unwrap(),
            self.min_third - self.reference.coefficients()[Bounded::new(2).unwrap()],
        );
        the_stack.increment_at_index(
            &self.active_temperaments,
            Bounded::new(1).unwrap(),
            self.min_third - self.reference.coefficients()[Bounded::new(1).unwrap()],
        );

        for i in self.min_third..=self.max_third {
            for j in self.min_fifth..=self.max_fifth {
                let mut state = CellState::Off;
                for k in 0..12 {
                    if self.neighbourhood.coefficients[k][Bounded::new(1).unwrap()] == j
                        && self.neighbourhood.coefficients[k][Bounded::new(2).unwrap()] == i
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
                        x: area.x + cellwidth * (j - self.min_fifth) as u16,
                        y: area.y + 2 * (self.max_third - i) as u16,
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
                self.min_fifth - self.max_fifth - 1,
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

impl<'a, D: Dimension + AtLeast<3> + PartialEq, T: Dimension + PartialEq + Copy> UIState<D, T>
    for Grid<D, T>
{
    fn handle_msg(&mut self, time: u64, msg: msg::ToUI<D, T>, _: &mpsc::Sender<(u64, msg::ToProcess<D, T>)>) {
        match msg {
            msg::ToUI::Notify { .. } => {}
            msg::ToUI::MidiParseErr(_) => {}
            msg::ToUI::DetunedNote {
                note,
                should_be,
                actual,
                explanation,
            } => {}
            msg::ToUI::CrosstermEvent(_) => {}
            msg::ToUI::SetNeighboughood { neighbourhood } => self.neighbourhood = neighbourhood,
            msg::ToUI::ToggleTemperament { index } => {
                self.active_temperaments[index] = !self.active_temperaments[index]
            }
            msg::ToUI::SetReference { key: _, stack } => self.reference = stack,
            msg::ToUI::NoteOn { note } => self.active_classes[(note % 12) as usize] = true,
            msg::ToUI::NoteOff { note } => self.active_classes[(note % 12) as usize] = false,
        }
    }
}
