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
};

fn init_displayconfig() -> DisplayConfig {
    DisplayConfig {
        notenamestyle: NoteNameStyle::JohnstonFiveLimitClass,
        color_range: 0.2,
        gradient: colorous::SPECTRAL,
    }
}

fn init_intervals() -> [Interval; 3] {
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

fn init_temperaments() -> [Temperament<3, StackCoeff>; 3] {
    [
        Temperament::new(
            "1/4-comma meantone".into(),
            [[0, 4, 0], [1, 0, 0], [0, 0, 1]],
            [[2, 0, 1], [1, 0, 0], [0, 0, 1]],
        )
        .unwrap(),
        Temperament::new(
            "1/3-comma meantone".into(),
            [[0, 3, 0], [1, 0, 0], [0, 0, 1]],
            [[2, -1, 1], [1, 0, 0], [0, 0, 1]],
        )
        .unwrap(),
        Temperament::new(
            "12edo".into(),
            [[0, 12, 0], [0, 0, 3], [1, 0, 0]],
            [[7, 0, 0], [1, 0, 0], [1, 0, 0]],
        )
        .unwrap(),
    ]
}

fn init_stacktype() -> StackType<3, 3> {
    StackType::new(init_intervals(), init_temperaments())
}

fn init_grid<'a, const T: usize>(
    stacktype: &'a StackType<3, T>,
    config: &'a DisplayConfig,
    active_temperings: &[bool; T],
    minfifth: StackCoeff,
    minthird: StackCoeff,
    cols: usize,
    rows: usize,
) -> Grid<'a, T> {
    let mut res = Grid {
        cells: Array2::from_shape_fn((rows, cols), |(i, j)| Cell {
            config,
            stack: Stack::new(
                stacktype,
                active_temperings,
                [
                    0,
                    minfifth + j as StackCoeff,
                    minthird + (rows - i - 1) as StackCoeff,
                ],
            ),
            state: CellState::Considered,
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

    let notes = init_grid(&st, &dc, &[true, false, false], -6, -3, 12, 7);

    let mut terminal = tui::init()?;
    terminal.draw(|frame| frame.render_widget(notes, frame.size()))?;
    match event::read()? {
        _ => {}
    }
    tui::restore()?;
    Ok(())
}
