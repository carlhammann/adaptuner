use std::{collections::HashMap, hash::Hash, sync::mpsc, time::Instant};

use eframe::egui::{self, epaint, pos2, vec2};
use midi_msg::Channel;

use crate::{
    interval::{
        stack::{ScaledAdd, Stack},
        stacktype::r#trait::{FiveLimitStackType, StackCoeff, StackType},
    },
    keystate::KeyState,
    msg::{FromUi, HandleMsgRef, ToUi},
    neighbourhood::Neighbourhood,
    notename::NoteNameStyle,
    reference::Reference,
};

use super::r#trait::GuiShow;

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

pub struct LatticeWindow<T: StackType, N: Neighbourhood<T>> {
    tuning_reference: Reference<T>,
    active_temperaments: Vec<bool>,

    active_notes: [KeyState; 128],
    pedal_hold: [bool; 16],
    tunings: [Stack<T>; 128],

    reference: Stack<T>,
    considered_notes: N,

    zoom: f32,

    keyboard_channel: Channel,
    keyboard_velocity: u8,

    notenamestyle: NoteNameStyle,

    /// The vertical sizes of intervals in the grid. (Horozontal sizes are determined by the size
    /// in equally tempered semitones.)
    interval_heights: Vec<f32>,
    interval_colours: Vec<egui::Color32>,

    /// the distances around the reference note, by "base interval" up to which "background" stacks
    /// should be drawn.
    background_stack_distances: Vec<StackCoeff>,

    // lattice_elements: LatticeElements,
    stacks_to_draw: HashMap<Stack<T>, DrawStyle>,
}

#[derive(PartialEq)]
enum DrawStyle {
    Background,
    Considered,
    Playing,
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

fn tipped_line(
    start: egui::Pos2,
    end: egui::Pos2,
    width: f32,
    color: egui::Color32,
) -> egui::Shape {
    let v = (end - start).normalized() * width.min(end.distance(start) / 2.0);
    let w = v.clone().rot90();
    egui::Shape::convex_polygon(
        vec![
            start,
            start + v + w,
            end - v + w,
            end,
            end - v - w,
            start + v - w,
        ],
        color,
        epaint::PathStroke::NONE,
    )
}

impl<T: FiveLimitStackType + Hash + Eq, N: Neighbourhood<T>> LatticeWindow<T, N> {
    pub fn new(
        tuning_reference: Reference<T>,
        active_temperaments: Vec<bool>,
        reference: Stack<T>,
        considered_notes: N,
        zoom: f32,
        notenamestyle: NoteNameStyle,
        interval_heights: Vec<f32>,
        interval_colours: Vec<egui::Color32>,
        background_stack_distances: Vec<StackCoeff>,
    ) -> Self {
        let now = Instant::now();
        Self {
            tuning_reference,
            active_temperaments,
            active_notes: core::array::from_fn(|_| KeyState::new(now)),
            pedal_hold: [false; 16],
            tunings: core::array::from_fn(|_| Stack::new_zero()),
            reference,
            considered_notes,
            zoom,
            keyboard_channel: Channel::Ch1,
            keyboard_velocity: 127,
            notenamestyle,
            interval_heights,
            interval_colours,
            background_stack_distances,
            stacks_to_draw: HashMap::new(),
        }
    }

    fn draw_stacks(&self, ui: &mut egui::Ui, forward: &mpsc::Sender<FromUi<T>>) {
        let rect = ui.max_rect();

        let c4_hpos = rect.left()
            + self.zoom * ET_SEMITONE_WIDTH * (0.5 + self.tuning_reference.c4_semitones() as f32);

        // compute the reference position
        let mut max_y_offset = f32::MIN;
        for (stack, _style) in self.stacks_to_draw.iter() {
            let y_offset = self.zoom * ET_SEMITONE_WIDTH * {
                let mut y = 0.0;
                for (i, &c) in stack.target.iter().enumerate() {
                    y += (c - self.reference.target[i]) as f32 * self.interval_heights[i];
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
            c4_hpos + self.zoom * ET_SEMITONE_WIDTH * self.reference.target_semitones() as f32;

        let grid_line_color = ui
            .style()
            .visuals
            .gray_out(ui.style().visuals.weak_text_color());

        // first, draw the vertical lines
        for (stack, style) in self.stacks_to_draw.iter() {
            match style {
                DrawStyle::Playing => {}
                _ => continue,
            }
            let hpos = c4_hpos + self.zoom * ET_SEMITONE_WIDTH * stack.semitones() as f32;
            let vpos = reference_vpos
                + self.zoom * ET_SEMITONE_WIDTH * {
                    let mut y = 0.0;
                    for (i, &c) in stack.target.iter().enumerate() {
                        y += (c - self.reference.target[i]) as f32 * self.interval_heights[i];
                    }
                    y
                };
            ui.painter().with_clip_rect(rect).vline(
                hpos,
                egui::Rangef {
                    min: vpos,
                    max: rect.bottom() - self.keyboard_height(),
                },
                egui::Stroke::new(self.zoom * MARKER_THICKNESS, grid_line_color),
            );
        }

        // then, draw the grid lines
        for (stack, style) in self.stacks_to_draw.iter() {
            let mut in_bounds = true;
            for i in 0..T::num_intervals() {
                if (stack.target[i] - self.reference.target[i]).abs()
                    > self.background_stack_distances[i]
                {
                    in_bounds = false;
                    break;
                }
            }
            if (*style == DrawStyle::Background) | in_bounds {
                let start_hpos =
                    c4_hpos + self.zoom * ET_SEMITONE_WIDTH * stack.target_semitones() as f32;
                let start_vpos = reference_vpos
                    + self.zoom * ET_SEMITONE_WIDTH * {
                        let mut y = 0.0;
                        for (i, &c) in stack.target.iter().enumerate() {
                            y += (c - self.reference.target[i]) as f32 * self.interval_heights[i];
                        }
                        y
                    };

                for i in 0..T::num_intervals() {
                    if stack.target[i] == self.reference.target[i] {
                        continue;
                    }

                    let inc = if stack.target[i] > self.reference.target[i] {
                        -1.0
                    } else {
                        1.0
                    };

                    let end_hpos =
                        start_hpos + inc * self.zoom * T::intervals()[i].semitones as f32;
                    let end_vpos = start_vpos + inc * self.zoom * self.interval_heights[i];

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
                    let mut c = self.reference.target[i];

                    let inc = if stack.target[i] > c { 1 } else { -1 };

                    while c != stack.target[i] {
                        end_hpos += self.zoom
                            * ET_SEMITONE_WIDTH
                            * inc as f32
                            * T::intervals()[i].semitones as f32;
                        end_vpos +=
                            self.zoom * ET_SEMITONE_WIDTH * inc as f32 * self.interval_heights[i];

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
        for (stack, style) in self.stacks_to_draw.iter() {
            let hpos = c4_hpos + self.zoom * ET_SEMITONE_WIDTH * stack.semitones() as f32;
            let vpos = reference_vpos
                + self.zoom * ET_SEMITONE_WIDTH * {
                    let mut y = 0.0;
                    for (i, &c) in stack.target.iter().enumerate() {
                        y += (c - self.reference.target[i]) as f32 * self.interval_heights[i];
                    }
                    y
                };

            let name = stack.notename(&self.notenamestyle);

            ui.painter().with_clip_rect(rect).text(
                pos2(hpos, vpos),
                egui::Align2::CENTER_CENTER,
                &name,
                match style {
                    DrawStyle::Background | DrawStyle::Considered => {
                        egui::FontId::proportional(self.zoom * FONT_SIZE)
                    }
                    DrawStyle::Playing => egui::FontId::proportional(self.zoom * 1.5 * FONT_SIZE),
                },
                match style {
                    DrawStyle::Background => ui.style().visuals.weak_text_color(),
                    DrawStyle::Considered | DrawStyle::Playing => {
                        ui.style().visuals.strong_text_color()
                    }
                },
            );

            if ui
                .interact(
                    egui::Rect::from_center_size(
                        pos2(hpos, vpos),
                        self.zoom * vec2(FONT_SIZE, FONT_SIZE),
                    ),
                    egui::Id::new(name),
                    egui::Sense::click(),
                )
                .clicked()
            {
                let _ = forward.send(FromUi::Consider {
                    coefficients: {
                        let mut v = stack.target.to_vec();
                        for i in 0..T::num_intervals() {
                            v[i] -= self.reference.target[i];
                        }
                        v
                    },
                    time: Instant::now(),
                });
            }
        }
    }

    fn draw_lattice(&mut self, ui: &mut egui::Ui, forward: &mpsc::Sender<FromUi<T>>) {
        self.stacks_to_draw.clear();

        for stack in PureStacksAround::new(&self.background_stack_distances, &self.reference) {
            self.stacks_to_draw.insert(stack, DrawStyle::Background);
        }

        self.considered_notes.for_each_stack(|_, relative_stack| {
            match self.considered_notes.try_period_index() {
                Some(period_index) => {
                    let mut stack = relative_stack.clone();
                    stack.increment_at_index_pure(period_index, -stack.target[period_index]);
                    stack.scaled_add(1, &self.reference);
                    self.stacks_to_draw.insert(stack, DrawStyle::Considered);
                }
                None {} => todo!(),
            }
        });

        for (i, state) in self.active_notes.iter().enumerate() {
            if state.is_sounding() {
                self.stacks_to_draw
                    .insert(self.tunings[i].clone(), DrawStyle::Playing);
            }
        }

        self.draw_stacks(ui, forward);
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

        let mut key_center = rect.left() + self.zoom * OCTAVE_WIDTH / 24.0;
        for _ in 0..128 {
            ui.painter().with_clip_rect(rect).vline(
                key_center,
                egui::Rangef {
                    min: rect.bottom() - self.zoom * (WHITE_KEY_LENGTH + MARKER_LENGTH),
                    max: rect.bottom() - self.zoom * WHITE_KEY_LENGTH,
                },
                egui::Stroke::new(
                    self.zoom * MARKER_THICKNESS,
                    ui.style().visuals.strong_text_color(),
                ),
            );
            key_center += self.zoom * OCTAVE_WIDTH / 12.0;
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
            for (stack, _style) in self.stacks_to_draw.iter() {
                let y_offset = ET_SEMITONE_WIDTH * {
                    let mut y = 0.0;
                    for (i, &c) in stack.target.iter().enumerate() {
                        y += c as f32 * self.interval_heights[i];
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

impl<T: FiveLimitStackType + Hash + Eq, N: Neighbourhood<T>> GuiShow<T> for LatticeWindow<T, N> {
    fn show(&mut self, ctx: &egui::Context, ui: &mut egui::Ui, forward: &mpsc::Sender<FromUi<T>>) {
        egui::TopBottomPanel::bottom("lattice window zoom bottom panel").show_inside(ui, |ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add(
                    egui::widgets::Slider::new(&mut self.zoom, 5.0..=100.0)
                        .smart_aim(false)
                        .logarithmic(true)
                        .show_value(false)
                        .text("zoom"),
                );
            });
        });

        egui::TopBottomPanel::bottom("lattice window bottom panel").show_inside(ui, |ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                ui.with_layout(egui::Layout::bottom_up(egui::Align::RIGHT), |ui| {
                    for i in 0..T::num_intervals() {
                        ui.add(
                            egui::widgets::Slider::new(
                                &mut self.background_stack_distances[i],
                                0..=6,
                            )
                            .smart_aim(false)
                            .text(format!("{}s", T::intervals()[i].name)),
                        );
                    }
                    ui.label("show notes around the reference:");
                });

                ui.separator();

                // egui::Grid::new("neighbourhood button grid").show(ui, |ui| {
                //     ui.button("1");
                //     // ui.button("2");
                //     // ui.button("3");
                //     // ui.button("4");
                //     // ui.button("5");
                //     // ui.end_row();
                //     // ui.button("6");
                //     // ui.button("7");
                //     // ui.button("8");
                //     // ui.button("9");
                //     // ui.button("10");
                // });
            });
        });

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

impl<T: StackType, N: Neighbourhood<T>> HandleMsgRef<ToUi<T>, FromUi<T>> for LatticeWindow<T, N> {
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
            ToUi::SetReference { stack } => self.reference.clone_from(stack),
            ToUi::SetTuningReference { reference } => self.tuning_reference.clone_from(reference),
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
