use std::io;

use colorous;
use crossterm::event;

use ndarray::Array2;

use adaptuner::{
    interval::{Interval, Semitones, Stack, StackCoeff, StackType, Temperament},
    neighbourhood::fivelimit_neighbours,
    notename::NoteNameStyle,
    tui::{
        self,
        grid::{Cell, CellState, DisplayConfig, Grid},
    },
    util::{fixed_sizes::*, matrix, vector, Dimension},
};

fn init_displayconfig() -> DisplayConfig {
    DisplayConfig {
        notenamestyle: NoteNameStyle::JohnstonFiveLimitClass,
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
                vector(&[0, minfifth + j as StackCoeff, minthird + i as StackCoeff]).unwrap(),
            ),
            state: CellState::Off,
        }),
    };

    highlight(&mut res, 4, 0, 0);

    res
}

pub fn highlight<'a, T: Dimension>(
    grid: &mut Grid<'a, T>,
    width: StackCoeff,
    index: StackCoeff,
    offset: StackCoeff,
) {
    let rows = grid.cells.raw_dim()[0];
    let cols = grid.cells.raw_dim()[1];
    let mut chosen = [(0, 0); 12];
    for cell in &mut grid.cells {
        cell.state = CellState::Off;
    }
    fivelimit_neighbours(&mut chosen, width, index, offset);

    for k in 0..12 {
        let (i, j) = chosen[k];
        if ((i + 3) as usize) < rows && ((j + 6) as usize) < cols {
            grid.cells[((i + 3) as usize, (j + 6) as usize)].state = CellState::Considered;
        }
    }
    let (i, j) = chosen[0];
    if ((i + 3) as usize) < rows && ((j + 6) as usize) < cols {
        grid.cells[((i + 3) as usize, (j + 6) as usize)].state = CellState::On;
    }
}

pub fn main() -> io::Result<()> {
    let st = init_stacktype();
    let dc = init_displayconfig();

    let mut width = 4; //1,2,3...12 //fifths thirds
    let mut index = 4; // 0,1,2,3...,11 //sharps/flats
    let mut offset = 1; // 0,1,...,width-1 //pluses/minuses
    let mut notes = init_grid(&st, &dc, &[false, false], -6, -3, 12, 7);

    let mut terminal = tui::init()?;
    loop {
        highlight(&mut notes, width, index, offset);
        terminal.draw(|frame| frame.render_widget(&notes, frame.size()))?;
        if let event::Event::Key(k) = event::read()? {
            if k.kind == event::KeyEventKind::Press {
                match k.code {
                    event::KeyCode::Char('z') => {
                        width = (width - 1).max(1);
                        offset = offset.min(width - 1);
                    }
                    event::KeyCode::Char('u') => width = (width + 1).min(12),
                    event::KeyCode::Char('h') => index = (index - 1).max(0),
                    event::KeyCode::Char('j') => index = (index + 1).min(11),
                    event::KeyCode::Char('m') => offset = (offset - 1).max(0),
                    event::KeyCode::Char('n') => offset = (offset + 1).min(width - 1),
                    event::KeyCode::Char('q') => break,
                    _ => {}
                }
            }
        } else {
        }
    }
    tui::restore()?;
    Ok(())
}
