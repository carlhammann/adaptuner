use std::{collections::HashMap, hash::Hash, sync::mpsc, time::Instant};

use eframe::egui::{self, pos2, vec2};
use midi_msg::Channel;
use ndarray::Array1;
use num_rational::Ratio;

use crate::{
    interval::{
        stack::{semitones_from_actual, semitones_from_target, ScaledAdd, Stack},
        stacktype::r#trait::{FiveLimitStackType, StackCoeff, StackType},
    },
    keystate::KeyState,
    msg::{FromUi, HandleMsgRef, ToUi},
    neighbourhood::{Neighbourhood, PartialNeighbourhood},
    notename::{correction::Correction, NoteNameStyle},
    reference::Reference,
};

use super::{
    common::{correction_system_chooser, temperament_applier},
    r#trait::GuiShow,
};

// The following measurements are all in units of [LatticeWindow::zoom]

const OCTAVE_WIDTH: f32 = 12.0;
const ET_SEMITONE_WIDTH: f32 = OCTAVE_WIDTH / 12.0;
const WHITE_KEY_WIDTH: f32 = OCTAVE_WIDTH / 7.0;
const BLACK_KEY_WIDTH: f32 = OCTAVE_WIDTH / 12.0;
const WHITE_KEY_LENGTH: f32 = OCTAVE_WIDTH / 2.5;
const BLACK_KEY_LENGTH: f32 = 3.0 * WHITE_KEY_LENGTH / 5.0;
const PIANO_KEY_BORDER_THICKNESS: f32 = 0.1;

const MARKER_LENGTH: f32 = BLACK_KEY_WIDTH / 2.0;
const MARKER_THICKNESS: f32 = PIANO_KEY_BORDER_THICKNESS;

const FREE_SPACE_ABOVE_KEYBOARD: f32 = 2.0;
const FONT_SIZE: f32 = 2.0;
const FAINT_GRID_LINE_THICKNESS: f32 = MARKER_THICKNESS;
const GRID_NODE_RADIUS: f32 = 4.0 * FAINT_GRID_LINE_THICKNESS;

pub struct LatticeWindow<T: StackType> {
    active_notes: [KeyState; 128],
    pedal_hold: [bool; 16],
    tunings: [Stack<T>; 128],

    reference: Option<Stack<T>>,
    tuning_reference: Option<Reference<T>>,

    considered_notes: PartialNeighbourhood<T>,
    curr_neighbourhood_name_and_index: Option<(String, usize)>,
    new_neighbourhood_name: String,

    zoom: f32,
    flatten: f32,

    notenamestyle: NoteNameStyle,

    keyboard_channel: Channel,
    keyboard_velocity: u8,

    /// The vertical sizes of intervals in the grid. (Horozontal sizes are determined by the size
    /// in equally tempered semitones.)
    interval_heights: Vec<f32>,

    /// the distances around the reference note, by "base interval" up to which "background" stacks
    /// should be drawn.
    background_stack_distances: Vec<StackCoeff>,

    stacks_to_draw: HashMap<Array1<StackCoeff>, (Array1<Ratio<StackCoeff>>, DrawStyle)>,

    correction_system_index: usize,
    tmp_temperaments: Vec<bool>,
    tmp_correction: Correction<T>,
    tmp_stack: Stack<T>,
    tmp_relative_stack: Stack<T>,
    show_controls: bool,
}

#[derive(PartialEq)]
enum DrawStyle {
    Background,
    Considered,
    Playing,
    Antenna,
}

struct PureStacksAround<'a, T: StackType> {
    dists: &'a [StackCoeff],
    reference: &'a Stack<T>,
    curr: Stack<T>,
}

impl<'a, T: StackType> PureStacksAround<'a, T> {
    /// dist must be nonnegative
    fn new(dists: &'a [StackCoeff], reference: &'a Stack<T>) -> Self {
        let mut curr = reference.clone();

        for i in 0..T::num_intervals() {
            curr.increment_at_index_pure(i, -dists[i]);
        }

        curr.increment_at_index_pure(T::num_intervals() - 1, -1);

        Self {
            dists,
            reference,
            curr,
        }
    }
}

impl<'a, T: StackType> Iterator for PureStacksAround<'a, T> {
    type Item = Stack<T>;
    fn next(&mut self) -> Option<Self::Item> {
        for i in (0..T::num_intervals()).rev() {
            if self.curr.target[i] < self.reference.target[i] + self.dists[i] {
                self.curr.increment_at_index_pure(i, 1);
                return Some(self.curr.clone());
            }
            self.curr.increment_at_index_pure(i, -2 * self.dists[i]);
        }

        return None {};
    }
}

pub struct LatticeWindowConfig {
    pub zoom: f32,
    pub flatten: f32,
    pub notenamestyle: NoteNameStyle,
    pub correction_system_index: usize,
    pub interval_heights: Vec<f32>,
    pub background_stack_distances: Vec<StackCoeff>,
}

impl<T: FiveLimitStackType + Hash + Eq> LatticeWindow<T> {
    pub fn new(config: LatticeWindowConfig) -> Self {
        let now = Instant::now();
        Self {
            active_notes: core::array::from_fn(|_| KeyState::new(now)),
            pedal_hold: [false; 16],
            tunings: core::array::from_fn(|_| Stack::new_zero()),

            tuning_reference: None {},
            reference: None {},

            considered_notes: PartialNeighbourhood::new("lattice window neighbourhood".into()),
            curr_neighbourhood_name_and_index: None {},
            new_neighbourhood_name: String::with_capacity(64),

            zoom: config.zoom,
            flatten: config.flatten,
            notenamestyle: config.notenamestyle,
            correction_system_index: config.correction_system_index,
            keyboard_channel: Channel::Ch1,
            keyboard_velocity: 64,
            interval_heights: config.interval_heights,
            background_stack_distances: config.background_stack_distances,
            stacks_to_draw: HashMap::new(),
            tmp_temperaments: vec![false; T::num_temperaments()],
            tmp_correction: Correction::new_zero(config.correction_system_index),
            tmp_stack: Stack::new_zero(),
            tmp_relative_stack: Stack::new_zero(),
            show_controls: false,
        }
    }

    pub fn toggle_controls(&mut self) {
        self.show_controls = !self.show_controls;
    }

    fn draw_stacks(&mut self, ui: &mut egui::Ui, forward: &mpsc::Sender<FromUi<T>>) {
        if self.reference.is_none() | self.tuning_reference.is_none() {
            return;
        }
        let reference: &Stack<T> = self.reference.as_ref().unwrap();
        let tuning_reference: &Reference<T> = self.tuning_reference.as_ref().unwrap();

        let rect = ui.max_rect();

        let c4_hpos = rect.left()
            + self.zoom * ET_SEMITONE_WIDTH * (0.5 + tuning_reference.c4_semitones() as f32);

        // compute the reference position
        let mut max_y_offset = f32::MIN;
        for (target, (_actual, _style)) in self.stacks_to_draw.iter() {
            let y_offset = self.zoom * ET_SEMITONE_WIDTH * {
                let mut y = 0.0;
                for (i, &c) in target.iter().enumerate() {
                    y += (c - reference.target[i]) as f32 * self.flatten * self.interval_heights[i];
                }
                y
            };
            max_y_offset = max_y_offset.max(y_offset);
        }
        let reference_vpos = rect.bottom()
            - self.keyboard_height()
            - self.zoom * FREE_SPACE_ABOVE_KEYBOARD
            - max_y_offset;
        let reference_hpos =
            c4_hpos + self.zoom * ET_SEMITONE_WIDTH * reference.target_semitones() as f32;

        let grid_line_color = ui
            .style()
            .visuals
            .gray_out(ui.style().visuals.weak_text_color());

        // first, draw the vertical lines and highlight circles
        for (target, (actual, style)) in self.stacks_to_draw.iter() {
            match style {
                DrawStyle::Playing | DrawStyle::Antenna => {}
                _ => continue,
            }

            let hpos = c4_hpos
                + self.zoom * ET_SEMITONE_WIDTH * semitones_from_actual::<T>(actual.into()) as f32;
            let vpos = reference_vpos
                + self.zoom * ET_SEMITONE_WIDTH * {
                    let mut y = 0.0;
                    for (i, &c) in target.iter().enumerate() {
                        y += (c - reference.target[i]) as f32
                            * self.flatten
                            * self.interval_heights[i];
                    }
                    y
                };

            if *style == DrawStyle::Playing {
                ui.painter().with_clip_rect(rect).circle_filled(
                    pos2(hpos, vpos),
                    self.zoom * FONT_SIZE,
                    ui.style().visuals.selection.bg_fill,
                );
            }

            ui.painter().with_clip_rect(rect).vline(
                hpos,
                egui::Rangef {
                    min: vpos,
                    max: rect.bottom() - self.keyboard_height(),
                },
                egui::Stroke::new(self.zoom * MARKER_THICKNESS, grid_line_color),
            );
        }

        // then, draw the grid lines. They won't be affected by temperaments
        for (target, (_actual, style)) in self.stacks_to_draw.iter() {
            let mut in_bounds = true;
            for i in 0..T::num_intervals() {
                if (target[i] - reference.target[i]).abs() > self.background_stack_distances[i] {
                    in_bounds = false;
                    break;
                }
            }
            if (*style == DrawStyle::Background) | in_bounds {
                let start_hpos = c4_hpos
                    + self.zoom
                        * ET_SEMITONE_WIDTH
                        * semitones_from_target::<T>(target.into()) as f32;
                let start_vpos = reference_vpos
                    + self.zoom * ET_SEMITONE_WIDTH * {
                        let mut y = 0.0;
                        for (i, &c) in target.iter().enumerate() {
                            y += (c - reference.target[i]) as f32
                                * self.flatten
                                * self.interval_heights[i];
                        }
                        y
                    };

                for i in 0..T::num_intervals() {
                    if target[i] == reference.target[i] {
                        continue;
                    }

                    let inc = if target[i] > reference.target[i] {
                        -1.0
                    } else {
                        1.0
                    };

                    let end_hpos =
                        start_hpos + inc * self.zoom * T::intervals()[i].semitones as f32;
                    let end_vpos =
                        start_vpos + inc * self.zoom * self.flatten * self.interval_heights[i];

                    ui.painter().with_clip_rect(rect).line_segment(
                        [pos2(start_hpos, start_vpos), pos2(end_hpos, end_vpos)],
                        egui::Stroke::new(self.zoom * FAINT_GRID_LINE_THICKNESS, grid_line_color),
                    );
                    ui.painter().with_clip_rect(rect).circle_filled(
                        pos2(start_hpos, start_vpos),
                        self.zoom * GRID_NODE_RADIUS,
                        grid_line_color,
                    );
                    ui.painter().with_clip_rect(rect).circle_filled(
                        pos2(end_hpos, end_vpos),
                        self.zoom * GRID_NODE_RADIUS,
                        grid_line_color,
                    );
                }
            } else {
                let mut start_hpos = reference_hpos;
                let mut start_vpos = reference_vpos;

                let mut end_hpos = reference_hpos;
                let mut end_vpos = reference_vpos;

                for i in (0..T::num_intervals()).rev() {
                    let mut c = reference.target[i];

                    let inc = if target[i] > c { 1 } else { -1 };

                    while c != target[i] {
                        end_hpos += self.zoom
                            * ET_SEMITONE_WIDTH
                            * inc as f32
                            * T::intervals()[i].semitones as f32;
                        end_vpos += self.zoom
                            * ET_SEMITONE_WIDTH
                            * inc as f32
                            * self.flatten
                            * self.interval_heights[i];

                        ui.painter().with_clip_rect(rect).line_segment(
                            [pos2(start_hpos, start_vpos), pos2(end_hpos, end_vpos)],
                            egui::Stroke::new(
                                self.zoom * FAINT_GRID_LINE_THICKNESS,
                                grid_line_color,
                            ),
                        );
                        ui.painter().with_clip_rect(rect).circle_filled(
                            pos2(start_hpos, start_vpos),
                            self.zoom * GRID_NODE_RADIUS,
                            grid_line_color,
                        );
                        ui.painter().with_clip_rect(rect).circle_filled(
                            pos2(end_hpos, end_vpos),
                            self.zoom * GRID_NODE_RADIUS,
                            grid_line_color,
                        );

                        start_hpos = end_hpos;
                        start_vpos = end_vpos;
                        c += inc;
                    }
                }
            }
        }

        // last, draw the note names, add the interaction zones
        for (target, (actual, style)) in self.stacks_to_draw.iter() {
            let hpos = c4_hpos
                + self.zoom * ET_SEMITONE_WIDTH * semitones_from_actual::<T>(actual.into()) as f32;
            let vpos = reference_vpos
                + self.zoom * ET_SEMITONE_WIDTH * {
                    let mut y = 0.0;
                    for (i, &c) in target.iter().enumerate() {
                        y += (c - reference.target[i]) as f32
                            * self.flatten
                            * self.interval_heights[i];
                    }
                    y
                };

            self.tmp_stack.target.assign(target);
            self.tmp_stack.actual.assign(actual);

            let note_name_height_below_vpos =
                self.draw_corrected_note_name(ui, hpos, vpos, &self.tmp_stack, style);
            let interaction_rect = egui::Rect::from_min_max(
                pos2(hpos - self.zoom * FONT_SIZE, vpos - self.zoom * FONT_SIZE),
                pos2(
                    hpos + self.zoom * FONT_SIZE,
                    vpos + note_name_height_below_vpos,
                ),
            );

            // add the interaction zones.
            match style {
                DrawStyle::Antenna => {}
                DrawStyle::Background => {
                    if ui
                        .interact(
                            interaction_rect,
                            egui::Id::new(target),
                            egui::Sense::click(),
                        )
                        .clicked()
                    {
                        self.tmp_relative_stack.clone_from(&self.tmp_stack);
                        self.tmp_relative_stack.scaled_add(-1, reference);
                        let _ = forward.send(FromUi::Consider {
                            stack: self.tmp_relative_stack.clone(),
                            time: Instant::now(),
                        });
                    }
                }
                DrawStyle::Considered | DrawStyle::Playing => {
                    let popup_id = ui.id().with(target);
                    let response = ui.interact(
                        interaction_rect,
                        egui::Id::new(target),
                        egui::Sense::click(),
                    );
                    if response.clicked() {
                        for b in self.tmp_temperaments.iter_mut() {
                            *b = false;
                        }
                        ui.memory_mut(|mem| mem.toggle_popup(popup_id));
                    }
                    egui::popup::popup_below_widget(
                        ui,
                        popup_id,
                        &response,
                        egui::popup::PopupCloseBehavior::CloseOnClickOutside,
                        |ui| {
                            self.tmp_relative_stack.clone_from(&self.tmp_stack);
                            self.tmp_relative_stack.scaled_add(-1, reference);
                            if temperament_applier(
                                true,
                                ui,
                                &mut self.tmp_temperaments,
                                &mut self.tmp_correction,
                                self.correction_system_index,
                                &mut self.tmp_relative_stack,
                            ) {
                                let _ = forward.send(FromUi::Consider {
                                    stack: self.tmp_relative_stack.clone(),
                                    time: Instant::now(),
                                });
                            }
                        },
                    );
                }
            }
        }
    }

    /// returns the height below vpos of the last text's base (which varies depending on the
    /// `style` and on whether the stack is target/pure)
    fn draw_corrected_note_name(
        &self,
        ui: &mut egui::Ui,
        hpos: f32,
        vpos: f32,
        stack: &Stack<T>,
        style: &DrawStyle,
    ) -> f32 {
        let rect = ui.max_rect();
        let first_line_height = match style {
            DrawStyle::Background | DrawStyle::Considered | DrawStyle::Antenna => {
                self.zoom * FONT_SIZE
            }
            DrawStyle::Playing => self.zoom * 1.5 * FONT_SIZE,
        };
        let spacing = self.zoom * 0.5 * FONT_SIZE;
        let other_lines_height = self.zoom * 0.6 * FONT_SIZE;
        let second_line_vpos = vpos + 0.5 * first_line_height + spacing;
        let third_line_vpos = second_line_vpos + 0.5 * other_lines_height + spacing;
        let text_color = match style {
            DrawStyle::Background | DrawStyle::Antenna => ui.style().visuals.weak_text_color(),
            DrawStyle::Considered | DrawStyle::Playing => ui.style().visuals.strong_text_color(),
        };

        let mut height_below_vpos = 0.0;

        ui.painter().with_clip_rect(rect).text(
            pos2(hpos, vpos),
            egui::Align2::CENTER_CENTER,
            stack.notename(&self.notenamestyle),
            egui::FontId::proportional(first_line_height),
            text_color,
        );
        height_below_vpos += first_line_height * 0.5;

        if !stack.is_target() {
            let correction = Correction::new(stack, self.correction_system_index);
            ui.painter().with_clip_rect(rect).text(
                pos2(hpos, second_line_vpos),
                egui::Align2::CENTER_CENTER,
                correction.str(),
                egui::FontId::proportional(other_lines_height),
                text_color,
            );
            height_below_vpos += spacing + other_lines_height;
            if stack.is_pure() {
                ui.painter().with_clip_rect(rect).text(
                    pos2(hpos, third_line_vpos),
                    egui::Align2::CENTER_CENTER,
                    format!("={}", stack.actual_notename(&self.notenamestyle)),
                    egui::FontId::proportional(other_lines_height),
                    text_color,
                );
                height_below_vpos += spacing + other_lines_height;
            }
        }

        height_below_vpos
    }

    fn draw_lattice(&mut self, ui: &mut egui::Ui, forward: &mpsc::Sender<FromUi<T>>) {
        if let Some(reference) = &self.reference {
            self.stacks_to_draw.clear();

            for stack in PureStacksAround::new(&self.background_stack_distances, reference) {
                self.stacks_to_draw
                    .insert(stack.target, (stack.actual, DrawStyle::Background));
            }

            match T::try_period_index() {
                Some(period_index) => {
                    self.considered_notes.for_each_stack(|_, relative_stack| {
                        let mut stack = relative_stack.clone();
                        stack.increment_at_index_pure(period_index, -stack.target[period_index]);
                        stack.scaled_add(1, reference);
                        self.stacks_to_draw
                            .insert(stack.target, (stack.actual, DrawStyle::Considered));
                    });

                    for (i, state) in self.active_notes.iter().enumerate() {
                        if state.is_sounding() {
                            let mut stack = self.tunings[i].clone();

                            if stack.target.iter().enumerate().any(|(i, c)| {
                                (reference.target[i] - c).abs() > self.background_stack_distances[i]
                            }) {
                                self.stacks_to_draw.insert(
                                    stack.target.clone(),
                                    (stack.actual.clone(), DrawStyle::Antenna),
                                );
                            }

                            stack.increment_at_index_pure(
                                period_index,
                                reference.target[period_index] - stack.target[period_index],
                            );
                            self.stacks_to_draw
                                .insert(stack.target, (stack.actual, DrawStyle::Playing));
                        }
                    }
                }
                None {} => todo!(),
            }
            self.draw_stacks(ui, forward);
        }
    }

    fn draw_keyboard(&self, ui: &mut egui::Ui, forward: &mpsc::Sender<FromUi<T>>) {
        let rect = ui.max_rect();
        let white_key_vertical_center = rect.bottom() - self.zoom * WHITE_KEY_LENGTH / 2.0;
        let black_key_vertical_center =
            rect.bottom() - self.zoom * (WHITE_KEY_LENGTH - BLACK_KEY_LENGTH / 2.0);

        let draw_black_key = |horizontal_center, key_number| {
            let keystate: &KeyState = &self.active_notes[key_number];
            let on = keystate.is_sounding();
            let border_color = if (key_number < 109) & (key_number > 20) {
                ui.style().visuals.strong_text_color()
            } else {
                ui.style().visuals.weak_text_color()
            };
            let key_rect = egui::Rect::from_center_size(
                pos2(horizontal_center, black_key_vertical_center),
                self.zoom * vec2(BLACK_KEY_WIDTH, BLACK_KEY_LENGTH),
            );
            ui.painter().with_clip_rect(rect).rect(
                key_rect,
                egui::CornerRadius::default(),
                if on {
                    ui.style().visuals.selection.bg_fill
                } else {
                    border_color
                },
                egui::Stroke::new(self.zoom * PIANO_KEY_BORDER_THICKNESS, border_color),
                egui::StrokeKind::Middle,
            );

            let response = ui.interact(
                key_rect,
                egui::Id::new(format!("black key {}", key_number)),
                egui::Sense::drag(),
            );
            if response.drag_started() {
                let _ = forward.send(FromUi::NoteOn {
                    note: key_number as u8,
                    channel: self.keyboard_channel,
                    velocity: self.keyboard_velocity,
                    time: Instant::now(),
                });
            }
            if response.drag_stopped() {
                let _ = forward.send(FromUi::NoteOff {
                    note: key_number as u8,
                    channel: self.keyboard_channel,
                    velocity: self.keyboard_velocity,
                    time: Instant::now(),
                });
            }
        };

        let draw_white_key = |horizontal_center, key_number| {
            let keystate: &KeyState = &self.active_notes[key_number];
            let on = keystate.is_sounding();
            let key_rect = egui::Rect::from_center_size(
                pos2(horizontal_center, white_key_vertical_center),
                self.zoom * vec2(WHITE_KEY_WIDTH, WHITE_KEY_LENGTH),
            );
            let border_color = if (key_number < 109) & (key_number > 20) {
                ui.style().visuals.strong_text_color()
            } else {
                ui.style().visuals.weak_text_color()
            };
            if on {
                ui.painter().with_clip_rect(rect).rect(
                    key_rect,
                    egui::CornerRadius::default(),
                    ui.style().visuals.selection.bg_fill,
                    egui::Stroke::new(self.zoom * PIANO_KEY_BORDER_THICKNESS, border_color),
                    egui::StrokeKind::Middle,
                );
            } else {
                ui.painter().with_clip_rect(rect).rect_stroke(
                    key_rect,
                    egui::CornerRadius::default(),
                    egui::Stroke::new(self.zoom * PIANO_KEY_BORDER_THICKNESS, border_color),
                    egui::StrokeKind::Middle,
                );
            }
            let response = ui.interact(
                key_rect,
                egui::Id::new(format!("white key {}", key_number)),
                egui::Sense::drag(),
            );
            if response.drag_started() {
                let _ = forward.send(FromUi::NoteOn {
                    note: key_number as u8,
                    channel: self.keyboard_channel,
                    velocity: self.keyboard_velocity,
                    time: Instant::now(),
                });
            }
            if response.drag_stopped() {
                let _ = forward.send(FromUi::NoteOff {
                    note: key_number as u8,
                    channel: self.keyboard_channel,
                    velocity: self.keyboard_velocity,
                    time: Instant::now(),
                });
            }
        };

        let draw_octave = |octave: i16| {
            let c_left = rect.left() + self.zoom * ((octave as f32 + 1.0) * OCTAVE_WIDTH);
            let white_key_numbers = [0, 2, 4, 5, 7, 9, 11];
            for i in 0..7 {
                let x = c_left + self.zoom * (i as f32 * WHITE_KEY_WIDTH + WHITE_KEY_WIDTH / 2.0);
                let key_number = (60 + 12 * (octave - 4) + white_key_numbers[i]) as usize;
                if key_number > 127 {
                    break;
                }
                draw_white_key(x, key_number);
            }

            // widths of the white keys at the top. See
            //
            // https://www.mathpages.com/home/kmath043.htm
            //
            // for an explanation.
            let cde_width = self.zoom * (WHITE_KEY_WIDTH - 2.0 * BLACK_KEY_WIDTH / 3.0);
            let fgab_width = self.zoom * (WHITE_KEY_WIDTH - 3.0 * BLACK_KEY_WIDTH / 4.0);

            let mut offset = c_left + cde_width + self.zoom * BLACK_KEY_WIDTH / 2.0;
            let mut key_number = (60 + 12 * (octave - 4) + 1) as usize;
            if key_number > 127 {
                return;
            }
            draw_black_key(offset, key_number);
            offset += cde_width + self.zoom * BLACK_KEY_WIDTH;
            key_number += 2;
            if key_number > 127 {
                return;
            }
            draw_black_key(offset, key_number);
            offset += fgab_width + cde_width + self.zoom * BLACK_KEY_WIDTH;
            key_number += 3;
            if key_number > 127 {
                return;
            }
            draw_black_key(offset, key_number);
            offset += fgab_width + self.zoom * BLACK_KEY_WIDTH;
            key_number += 2;
            if key_number > 127 {
                return;
            }
            draw_black_key(offset, key_number);
            offset += fgab_width + self.zoom * BLACK_KEY_WIDTH;
            key_number += 2;
            if key_number > 127 {
                return;
            }
            draw_black_key(offset, key_number);
        };

        for i in (-1)..10 {
            draw_octave(i);
        }

        let mut reference = rect.left() + self.zoom * OCTAVE_WIDTH / 24.0;
        for _ in 0..128 {
            ui.painter().with_clip_rect(rect).vline(
                reference,
                egui::Rangef {
                    min: rect.bottom() - self.zoom * (WHITE_KEY_LENGTH + MARKER_LENGTH),
                    max: rect.bottom() - self.zoom * WHITE_KEY_LENGTH,
                },
                egui::Stroke::new(
                    self.zoom * MARKER_THICKNESS,
                    ui.style().visuals.strong_text_color(),
                ),
            );
            reference += self.zoom * OCTAVE_WIDTH / 12.0;
        }
    }

    fn keyboard_width(&self) -> f32 {
        self.zoom * WHITE_KEY_WIDTH * 75.0 // 75 is the number of white keys in the MIDI range
    }

    fn drawing_width(&self) -> f32 {
        self.keyboard_width()
    }

    fn keyboard_height(&self) -> f32 {
        self.zoom * (WHITE_KEY_LENGTH + MARKER_LENGTH)
    }

    fn grid_height(&self) -> f32 {
        self.zoom * {
            let mut min_y_offset = f32::MAX;
            let mut max_y_offset = f32::MIN;
            for (target, (_actual, _style)) in self.stacks_to_draw.iter() {
                let y_offset = ET_SEMITONE_WIDTH * {
                    let mut y = 0.0;
                    for (i, &c) in target.iter().enumerate() {
                        y += c as f32 * self.flatten * self.interval_heights[i];
                    }
                    y
                };

                min_y_offset = min_y_offset.min(y_offset);
                max_y_offset = max_y_offset.max(y_offset);
            }
            max_y_offset - min_y_offset + FONT_SIZE
        }
    }

    fn drawing_height(&self) -> f32 {
        self.keyboard_height() + self.grid_height() + self.zoom * FREE_SPACE_ABOVE_KEYBOARD
    }
}

impl<T: FiveLimitStackType + Hash + Eq> GuiShow<T> for LatticeWindow<T> {
    fn show(&mut self, _ctx: &egui::Context, ui: &mut egui::Ui, forward: &mpsc::Sender<FromUi<T>>) {
        egui::TopBottomPanel::bottom("lattice window zoom panel").show_inside(ui, |ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add(
                    egui::widgets::Slider::new(&mut self.zoom, 5.0..=100.0)
                        .smart_aim(false)
                        .show_value(false)
                        .logarithmic(true)
                        .text("zoom"),
                );
                // ui.add(
                //     egui::widgets::Slider::new(&mut self.flatten, 0.0..=1.0)
                //         .smart_aim(false)
                //         .show_value(false)
                //         .text("height"),
                // );
            });
        });

        egui::TopBottomPanel::bottom("lattice window bottom panel").show_animated_inside(
            ui,
            self.show_controls,
            |ui| {
                ui.horizontal(|ui| {
                    correction_system_chooser::<T>(ui, &mut self.correction_system_index);

                    ui.separator();

                    ui.vertical(|ui| {
                        ui.label("show notes around the reference:");
                        for i in (0..T::num_intervals()).rev() {
                            ui.add(
                                egui::widgets::Slider::new(
                                    &mut self.background_stack_distances[i],
                                    0..=6,
                                )
                                .smart_aim(false)
                                .text(format!("{}s", T::intervals()[i].name)),
                            );
                        }
                    });

                    ui.separator();
                    if let Some((curr_neighbourhood_name, curr_neighbourhood_index)) =
                        &self.curr_neighbourhood_name_and_index
                    {
                        ui.vertical(|ui| {
                            ui.label(format!(
                                "neighbourhood {}: \"{}\"",
                                curr_neighbourhood_index, curr_neighbourhood_name,
                            ));

                            if ui.button("switch to next neighbourhood").clicked() {
                                let _ = forward.send(FromUi::NextNeighbourhood {
                                    time: Instant::now(),
                                });
                            }

                            if ui.button("delete current neighbourhood").clicked() {
                                let _ = forward.send(FromUi::DeleteCurrentNeighbourhood {
                                    time: Instant::now(),
                                });
                            }

                            ui.horizontal(|ui| {
                                if ui
                                    .add_enabled(
                                        !self.new_neighbourhood_name.is_empty(),
                                        egui::Button::new("add new neighbourhood"),
                                    )
                                    .clicked()
                                {
                                    let _ = forward.send(FromUi::NewNeighbourhood {
                                        name: self.new_neighbourhood_name.clone(),
                                    });
                                    self.new_neighbourhood_name.clear();
                                }
                                ui.text_edit_singleline(&mut self.new_neighbourhood_name);
                            });
                        });
                    }
                });
            },
        );

        egui::CentralPanel::default().show_inside(ui, |ui| {
            ui.style_mut().spacing.scroll = egui::style::ScrollStyle::solid();
            egui::ScrollArea::both()
                .stick_to_bottom(true)
                .scroll_bar_rect(ui.clip_rect())
                .show(ui, |ui| {
                    ui.allocate_space(vec2(
                        ui.max_rect().width().max(self.drawing_width()),
                        ui.max_rect().height().max(self.drawing_height()),
                    ));
                    self.draw_lattice(ui, forward);
                    self.draw_keyboard(ui, forward);
                });
        });
    }
}

impl<T: StackType> HandleMsgRef<ToUi<T>, FromUi<T>> for LatticeWindow<T> {
    fn handle_msg_ref(&mut self, msg: &ToUi<T>, _forward: &mpsc::Sender<FromUi<T>>) {
        match msg {
            ToUi::NoteOn {
                time,
                channel,
                note,
            } => {
                self.active_notes[*note as usize].note_on(*channel, *time);
            }

            ToUi::NoteOff {
                time,
                channel,
                note,
            } => {
                self.active_notes[*note as usize].note_off(
                    *channel,
                    self.pedal_hold[*channel as usize],
                    *time,
                );
            }

            ToUi::PedalHold {
                channel,
                value,
                time,
            } => {
                self.pedal_hold[*channel as usize] = *value != 0;
                if *value == 0 {
                    for n in self.active_notes.iter_mut() {
                        n.pedal_off(*channel, *time);
                    }
                }
            }

            ToUi::Retune { note, tuning_stack } => {
                self.tunings[*note as usize].clone_from(tuning_stack);
            }

            ToUi::TunedNoteOn {
                time,
                channel,
                note,
                tuning_stack,
            } => {
                self.active_notes[*note as usize].note_on(*channel, *time);
                self.tunings[*note as usize].clone_from(tuning_stack);
            }

            ToUi::Consider { stack } => {
                let _ = self.considered_notes.insert(stack);
            }
            ToUi::CurrentNeighbourhoodName { index, name } => {
                if let Some((old_name, old_index)) = &mut self.curr_neighbourhood_name_and_index {
                    if index != old_index {
                        *old_index = *index;
                        old_name.clone_from(name);
                    }
                } else {
                    self.curr_neighbourhood_name_and_index = Some((name.clone(), *index));
                }
            }
            ToUi::SetReference { stack } => self.reference = Some(stack.clone()),
            ToUi::SetTuningReference { reference } => {
                self.tuning_reference = Some(reference.clone())
            }

            ToUi::Stop => {}
            ToUi::Notify { .. } => {}
            ToUi::EventLatency { .. } => {}
            ToUi::InputConnectionError { .. } => {}
            ToUi::InputConnected { .. } => {}
            ToUi::InputDisconnected { .. } => {}
            ToUi::OutputConnectionError { .. } => {}
            ToUi::OutputConnected { .. } => {}
            ToUi::OutputDisconnected { .. } => {}
            ToUi::NotifyFit { .. } => {}
            ToUi::NotifyNoFit => {}
            ToUi::DetunedNote { .. } => {}
        }
    }
}
