use std::io;

use colorous;
use crossterm::event;

use ndarray::Array2;

use adaptuner::{
    interval::{Interval, Semitones, Stack, StackCoeff, StackType, Temperament},
    notename::NoteNameStyle,
    tui::{
        self,
        grid::{Cell, CellState, DisplayConfig, Grid},
    },
    util::{fixed_sizes::*, matrix, vector},
};

fn init_displayconfig() -> DisplayConfig {
    DisplayConfig {
        notenamestyle: NoteNameStyle::JohnstonClass,
        color_range: 0.5,
        gradient: colorous::SPECTRAL,
    }
}

/// some base intervals: octaves, fifths, thirds.
pub fn init_intervals() -> [Interval; 3] {
    [
        Interval {
            name: "octave".into(),
            semitones: 12.0,
        },
        Interval {
            name: "fifth".into(),
            semitones: 12.0 * (3.0 / 2.0 as Semitones).log2(),
        },
        Interval {
            name: "third".into(),
            semitones: 12.0 * (5.0 / 4.0 as Semitones).log2(),
        },
    ]
}

/// some example temperaments: quarter-comma meantone, and 12-EDO
pub fn init_temperaments() -> [Temperament<Size3, StackCoeff>; 2] {
    [
        Temperament::new(
            "1/4-comma meantone".into(),
            matrix(&[[0, 4, 0], [1, 0, 0], [0, 0, 1]]).unwrap(),
            &matrix(&[[2, 0, 1], [1, 0, 0], [0, 0, 1]]).unwrap(),
        )
        .unwrap(),
        Temperament::new(
            "12edo".into(),
            matrix(&[[0, 12, 0], [0, 0, 3], [1, 0, 0]]).unwrap(),
            &matrix(&[[7, 0, 0], [1, 0, 0], [1, 0, 0]]).unwrap(),
        )
        .unwrap(),
    ]
}

/// an example [StackType].
pub fn init_stacktype() -> StackType<Size3, Size2> {
    StackType::new(
        vector(&init_intervals()).unwrap(),
        vector(&init_temperaments()).unwrap(),
    )
    .unwrap()
}

fn init_grid<'a>(
    stacktype: &'a StackType<Size3, Size2>,
    config: &'a DisplayConfig,
    active_temperings: &[bool; T],
    minfifth: StackCoeff,
    minthird: StackCoeff,
    cols: usize,
    rows: usize,
) -> Grid<'a, Size2> {
    let mut res = Grid {
        cells: Array2::from_shape_fn((rows, cols), |(i, j)| Cell {
            config,
            stack: Stack::new(
                stacktype,
                &vector(active_temperings).unwrap(),
                vector(&[
                    0,
                    minfifth + j as StackCoeff,
                    minthird + (rows - i - 1) as StackCoeff,
                ])
                .unwrap(),
            )
            .unwrap(),
            state: CellState::Off,
        }),
    };

    let on = [(0, 0), (2, 0), (3, 0), (3, -1), (0, 1), (0, -1)];
    let consider = [
        (2, 1),
        (-1, 0),
        (-1, 1),
        (0, 0),
        (0, 1),
        (1, 0),
        (1, 1),
        (2, 0),
    ];

    for (i, j) in consider.iter() {
        res.cells[(
            (rows as StackCoeff + minthird - 1 - j) as usize,
            (-minfifth + i) as usize,
        )]
            .state = CellState::Considered;
    }

    for (i, j) in on.iter() {
        res.cells[(
            (rows as StackCoeff + minthird - 1 - j) as usize,
            (-minfifth + i) as usize,
        )]
            .state = CellState::On;
    }

    res
}

pub fn main() -> io::Result<()> {
    let st = init_stacktype();
    let dc = init_displayconfig();

    let notes = init_grid(&st, &dc, &[true, true], -6, -3, 12, 7);

    let mut terminal = tui::init()?;
    terminal.draw(|frame| frame.render_widget(notes, frame.size()))?;
    match event::read()? {
        _ => {}
    }
    tui::restore()?;
    Ok(())
}
