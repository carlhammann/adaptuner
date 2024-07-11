use std::{
    sync::{mpsc, Arc},
    time::Instant,
};

use colorous;
use crossterm::event::{KeyCode, KeyEventKind};
use ratatui::{prelude::*, widgets::WidgetRef};

use crate::{
    config::r#trait::Config,
    interval::{Semitones, Stack, StackCoeff, StackType},
    msg,
    neighbourhood::Neighbourhood,
    notename::NoteNameStyle,
    tui::r#trait::{Tui, UIState},
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

pub struct Grid<T: StackType> {
    pub reference_key: i8,
    pub reference_stack: Stack<T>,
    pub active_temperaments: Vec<bool>,

    pub neighbourhood: Neighbourhood,

    /// these are "absolute pitch classes", i.e. not relative to the reference. The number counts
    /// how many keys of that pich class are currently pressed
    pub active_classes: [bool; 12],

    pub active_notes: [bool; 128],

    pub config: GridConfig<T>,
}

impl<T: StackType> Widget for Grid<T> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        self.render_ref(area, buf)
    }
}

impl<T: StackType> WidgetRef for Grid<T> {
    /// rendering of Grids expects there to be 2n rows of characters for an n-row grid, because
    /// Cells are two rows high
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        let mut the_stack = self.reference_stack.clone();

        let origin_fifth = the_stack.coefficients()[1];
        let origin_third = the_stack.coefficients()[2];

        let (mut min_fifth, mut max_fifth) = self.neighbourhood.bounds(1);
        let (mut min_third, mut max_third) = self.neighbourhood.bounds(2);
        min_fifth += origin_fifth;
        max_fifth += origin_fifth;
        min_third += origin_third;
        max_third += origin_third;

        // if max_fifth - min_fifth < self.width {
        //     min_fifth = origin_fifth - self.width / 2;
        //     max_fifth = min_fifth + self.width - 1;
        // }

        let max_lines = area.height as StackCoeff / 2;
        if max_third - min_third > max_lines {
            min_third = origin_third - max_lines / 2;
            max_third = min_third + max_lines - 1;
        }

        let cols = 1 + max_fifth - min_fifth;
        let cellwidth = area.width / cols as u16;

        the_stack.increment_at_index(
            &self.active_temperaments,
            2,
            min_third - self.reference_stack.coefficients()[2],
        );
        the_stack.increment_at_index(
            &self.active_temperaments,
            1,
            min_fifth - self.reference_stack.coefficients()[1],
        );

        for i in min_third..=max_third {
            for j in min_fifth..=max_fifth {
                let mut state = CellState::Off;
                for k in 0..12 {
                    if self.neighbourhood.coefficients[k][1] + origin_fifth == j
                        && self.neighbourhood.coefficients[k][2] + origin_third == i
                    {
                        if self.active_classes[(k as usize + self.reference_key as usize) % 12]
                        {
                            state = CellState::On;
                        } else {
                            state = CellState::Considered;
                        }
                    }
                }
                render_stack(
                    &the_stack,
                    state,
                    &self.config.display_config,
                    Rect {
                        x: area.x + cellwidth * (j - min_fifth) as u16,
                        y: area.y + 2 * (max_third - i) as u16,
                        width: cellwidth,
                        height: 2,
                    },
                    buf,
                );
                the_stack.increment_at_index(&self.active_temperaments, 1, 1);
            }
            the_stack.increment_at_index(&self.active_temperaments, 2, 1);
            the_stack.increment_at_index(&self.active_temperaments, 1, min_fifth - max_fifth - 1);
        }
    }
}

fn render_stack<T: StackType>(
    stack: &Stack<T>,
    state: CellState,
    config: &DisplayConfig,
    area: Rect,
    buf: &mut Buffer,
) {
    // Rendering grid cells expects that we have two rows.
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

impl<T: StackType> UIState<T> for Grid<T> {
    fn handle_msg(
        &mut self,
        time: Instant,
        msg: msg::ToUI<T>,
        to_process: &mpsc::Sender<(Instant, msg::ToProcess)>,
        tui: &mut Tui,
    ) {
        let draw_frame = |t: &mut Tui, g: &Self| {
            t.draw(|frame| frame.render_widget(g, frame.size()))
                .expect("");
            0
        };

        let send_to_process =
            |msg: msg::ToProcess, time: Instant| to_process.send((time, msg)).unwrap_or(());

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
                            KeyCode::Esc => {
                                *self = GridConfig::initialise(&self.config);
                                send_to_process(msg::ToProcess::Reset, time);
                            }
                            KeyCode::Down => {
                                self.neighbourhood.fivelimit_inc_index();
                                send_to_process(msg::ToProcess::SetNeighboughood {neighbourhood: self.neighbourhood.clone()}, time);
                            }
                            KeyCode::Up => {
                                self.neighbourhood.fivelimit_dec_index();
                                send_to_process(msg::ToProcess::SetNeighboughood {neighbourhood: self.neighbourhood.clone()}, time);

                            }
                            KeyCode::Left => {
                                self.neighbourhood.fivelimit_inc_offset();
                                send_to_process(msg::ToProcess::SetNeighboughood {neighbourhood: self.neighbourhood.clone()}, time);

                            }
                            KeyCode::Right => {
                                self.neighbourhood.fivelimit_dec_offset();
                                send_to_process(msg::ToProcess::SetNeighboughood {neighbourhood: self.neighbourhood.clone()}, time);

                            }
                            KeyCode::Char('+') => {
                                self.neighbourhood.fivelimit_inc_width();
                                send_to_process(msg::ToProcess::SetNeighboughood {neighbourhood: self.neighbourhood.clone()}, time);

                            }
                            KeyCode::Char('-') => {
                                self.neighbourhood.fivelimit_dec_width();
                                send_to_process(msg::ToProcess::SetNeighboughood {neighbourhood: self.neighbourhood.clone()}, time);

                            }

                            KeyCode::Char('1') => {
                                let index = 0;
                                self.active_temperaments[index] = !self.active_temperaments[index];
                                send_to_process(msg::ToProcess::ToggleTemperament {index}, time);
                            }
                            KeyCode::Char('2') => {
                                let index = 1;
                                self.active_temperaments[index] = !self.active_temperaments[index];
                                send_to_process(msg::ToProcess::ToggleTemperament {index}, time);
                            }
                            _ => {}
                        }
                    }
                    _ =>{},
                }
                draw_frame(tui, self);
            }
            msg::ToUI::SetReference { key, stack } => {
                self.reference_key = key as i8;
                self.reference_stack.clone_from(&stack);
                draw_frame(tui, self);
            }
            msg::ToUI::TunedNoteOn { note, .. } => { // TODO use the tuning data!
                self.active_classes[(note % 12) as usize] = true;
                self.active_notes[note as usize] = true;
                draw_frame(tui, self);
            }
            msg::ToUI::NoteOff { note } => {
                self.active_notes[note as usize] = false;
                let mut any_on = false;
                for i in ((note%12)..128).step_by(12) {
                    if self.active_notes[i as usize] {
                        any_on = true;
                    }
                }

                self.active_classes[(note % 12) as usize] = any_on; 
                draw_frame(tui, self);
            }
        }
    }
}

pub struct GridConfig<T: StackType> {
    pub display_config: DisplayConfig,
    pub initial_reference_key: i8,
    pub stack_type: Arc<T>,
    pub initial_neighbourhood_width: StackCoeff,
    pub initial_neighbourhood_index: StackCoeff,
    pub initial_neighbourhood_offset: StackCoeff,
}

// derive(Clone) doesn't handle cloning `Arc` correctly
impl<T: StackType> Clone for GridConfig<T> {
    fn clone(&self) -> Self {
        GridConfig {
            display_config: self.display_config.clone(),
            initial_reference_key: self.initial_reference_key.clone(),
            stack_type: self.stack_type.clone(),
            initial_neighbourhood_width: self.initial_neighbourhood_width,
            initial_neighbourhood_index: self.initial_neighbourhood_index,
            initial_neighbourhood_offset: self.initial_neighbourhood_offset,
        }
    }
}

impl<T: StackType> Config<Grid<T>> for GridConfig<T> {
    fn initialise(config: &Self) -> Grid<T> {
        let no_active_temperaments = vec![false; config.stack_type.num_temperaments()];
        Grid {
            reference_key: config.initial_reference_key,
            reference_stack: Stack::new(
                config.stack_type.clone(),
                &no_active_temperaments,
                vec![0; config.stack_type.num_intervals()],
            ),
            active_temperaments: no_active_temperaments,
            neighbourhood: Neighbourhood::fivelimit_new(
                config.initial_neighbourhood_width,
                config.initial_neighbourhood_index,
                config.initial_neighbourhood_offset,
            ),
            active_notes: [false; 128],
            active_classes: [false; 12],
            config: config.clone(),
        }
    }
}
