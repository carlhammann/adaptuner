use crate::{
    interval::{Semitones, Stack},
    notename::*,
    util::dimension::{fixed_sizes::Size3, Dimension},
};
use colorous;
use ndarray::Array2;
use ratatui::{prelude::*, widgets::WidgetRef};

pub struct DisplayConfig {
    pub notenamestyle: NoteNameStyle,
    pub color_range: Semitones,
    pub gradient: colorous::Gradient,
}

pub enum CellState {
    Off,
    Considered,
    On,
}

pub struct Cell<'a, T: Dimension> {
    pub config: &'a DisplayConfig,
    pub stack: Stack<'a, Size3, T>,
    pub state: CellState,
}

// fn intermediate_color(weight: Semitones, rgb1: (u8, u8, u8), rgb2: (u8, u8, u8)) -> (u8, u8, u8) {
//     let (r1, g1, b1) = rgb1;
//     let (r2, g2, b2) = rgb2;
//     let t = (weight.min(1.0)).max(0.0);
//     let conv = |t, x, y| (t * y as Semitones + (1.0 - t) * x as Semitones) as u8;
//     (conv(t, r1, r2), conv(t, g1, g2), conv(t, b1, b2))
// }
//
fn foreground_for_background(r: u8, g: u8, b: u8) -> u8 {
    // Counting the perceptive luminance - human eye favors green color...
    let luminance = (0.299 * r as f32 + 0.587 * g as f32 + 0.114 * b as f32) / 255.0;

    if luminance > 0.5 {
        return 0; // bright colors - black font
    } else {
        return 255; // dark colors - white font
    }
}

impl<'a, T: Dimension + Copy> WidgetRef for Cell<'a, T> {
    /// Rendering grid cells expects that we have two rows.
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        buf.set_string(
            area.x,
            area.y,
            self.stack.notename(&self.config.notenamestyle),
            Style::default(),
        );
        let deviation = self.stack.correction_semitones();
        if !self.stack.is_pure() {
            buf.set_string(
                area.x,
                area.y + 1,
                format!("{d:+.0}", d = deviation * 100.0),
                Style::default(),
            );
        }
        // let upwards = deviation > 0.0;
        let (f, r, g, b) = {
            let t = ((deviation / self.config.color_range).max(-1.0).min(1.0) + 1.0) / 2.0; // in range 0..1
            let colorous::Color { r, g, b } = self.config.gradient.eval_continuous(t);

            match self.state {
                CellState::On => {
                    let f = foreground_for_background(r, g, b);
                    (f, r, g, b)
                }
                CellState::Considered => {
                    let f = foreground_for_background(r / 2, g / 2, b / 2);

                    (f, r / 2, g / 2, b / 2)
                }
                CellState::Off => {
                    let f = foreground_for_background(r / 4, g / 4, b / 4);

                    (f / 2, r / 4, g / 4, b / 4)
                }
            }
        };

        buf.set_style(
            area,
            Style::from((Color::Rgb(f, f, f), Color::Rgb(r, g, b))),
        );
    }
}

pub struct Grid<'a, T: Dimension> {
    pub cells: Array2<Cell<'a, T>>,
}

impl<'a, T: Dimension + Copy> Widget for Grid<'a, T> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        self.render_ref(area, buf)
    }
}

impl<'a, T: Dimension + Copy> WidgetRef for Grid<'a, T> {
    /// rendering of Grids expects there to be 2n rows of characters for an n-row grid, because
    /// Cells are two rows high
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        let rows = self.cells.raw_dim()[0];
        let cols = self.cells.raw_dim()[1];
        let cellwidth = area.width / cols as u16;

        for i in 0..rows {
            for j in 0..cols {
                self.cells[[rows-1-i, j]].render_ref(
                    Rect {
                        x: area.x + cellwidth * j as u16,
                        y: area.y + 2 * i as u16,
                        width: cellwidth,
                        height: 2,
                    },
                    buf,
                );
            }
        }
    }
}
