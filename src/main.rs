use std::io;

use colorous;
use crossterm::event;

use ndarray::{arr1, arr2, Array2};

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

fn init_temperaments() -> [Temperament<StackCoeff>; 3] {
    [
        Temperament::new(
            "1/4-comma meantone".into(),
            arr2(&[[0, 4, 0], [1, 0, 0], [0, 0, 1]]),
            &arr2(&[[2, 0, 1], [1, 0, 0], [0, 0, 1]]),
        )
        .unwrap(),
        Temperament::new(
            "1/3-comma meantone".into(),
            arr2(&[[0, 3, 0], [1, 0, 0], [0, 0, 1]]),
            &arr2(&[[2, -1, 1], [1, 0, 0], [0, 0, 1]]),
        )
        .unwrap(),
        Temperament::new(
            "12edo".into(),
            arr2(&[[0, 12, 0], [0, 0, 3], [1, 0, 0]]),
            &arr2(&[[7, 0, 0], [1, 0, 0], [1, 0, 0]]),
        )
        .unwrap(),
    ]
}

fn init_stacktype() -> StackType {
    StackType::new(Vec::from(init_intervals()), Vec::from(init_temperaments())).unwrap()
}

fn init_grid<'a>(
    stacktype: &'a StackType,
    config: &'a DisplayConfig,
    active_temperings: &[bool],
    minfifth: StackCoeff,
    minthird: StackCoeff,
    cols: usize,
    rows: usize,
) -> Grid<'a> {
    let mut res = Grid {
        cells: Array2::from_shape_fn((rows, cols), |(i, j)| Cell {
            config,
            stack: Stack::new(
                stacktype,
                &arr1(active_temperings),
                arr1(&[
                    0,
                    minfifth + j as StackCoeff,
                    minthird + (rows - i - 1) as StackCoeff,
                ]),
            )
            .unwrap(),
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
