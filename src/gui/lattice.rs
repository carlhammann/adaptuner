use std::{cell::RefCell, hash::Hash, rc::Rc, sync::mpsc, time::Instant};

use eframe::egui::{self, pos2, vec2};
use midi_msg::Channel;
use serde_derive::{Deserialize, Serialize};

use crate::{
    config::ExtractConfig,
    custom_serde::common::{deserialize_channel, serialize_channel},
    interval::{
        base::Semitones,
        stack::{ScaledAdd, Stack},
        stacktype::r#trait::{StackCoeff, StackType},
    },
    msg::{FromUi, HandleMsgRef, ToUi},
    neighbourhood::{Neighbourhood, Partial},
    notename::{correction::Correction, HasNoteNames, NoteNameStyle},
};

use super::{
    common::{temperament_applier, CorrectionSystemChooser},
    latticecontrol::AsBigControls,
    toplevel::KeysAndTunings,
};

// The following measurements are all in units of [LatticeWindow::zoom], which is the width of one
// equally tempered semitone.

const OCTAVE_WIDTH: f32 = 12.0;
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

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "kebab-case")]
pub struct Hsv {
    pub h: f32,
    pub s: f32,
    pub v: f32,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "kebab-case")]
pub struct LatticeWindowConfig {
    pub zoom: f32,
    pub interval_heights: Vec<f32>,
    pub background_stack_distances: Vec<StackCoeff>,
    pub project_dimension: usize,
    #[serde(
        serialize_with = "serialize_channel",
        deserialize_with = "deserialize_channel"
    )]
    pub screen_keyboard_channel: Channel,
    pub screen_keyboard_velocity: u8,
    pub notenamestyle: NoteNameStyle,
    pub highlight_playable_keys: bool,
    pub in_tune_note_color: Hsv,
    pub out_of_tune_note_color: Hsv,
    pub color_period: Semitones,
}

impl LatticeWindowConfig {
    pub fn to_controls<T: StackType>(
        self,
        correction_system_chooser: Rc<RefCell<CorrectionSystemChooser<T>>>,
    ) -> LatticeWindowControls<T> {
        let LatticeWindowConfig {
            zoom,
            interval_heights,
            background_stack_distances,
            project_dimension,
            screen_keyboard_channel,
            screen_keyboard_velocity,
            notenamestyle,
            highlight_playable_keys,
            in_tune_note_color,
            out_of_tune_note_color,
            color_period,
        } = self;
        LatticeWindowControls {
            zoom,
            interval_heights,
            background_stack_distances,
            project_dimension,
            screen_keyboard_channel,
            screen_keyboard_velocity,
            screen_keyboard_pedal_hold: false,
            screen_keyboard_center: 60,
            notenamestyle,
            correction_system_chooser,
            highlight_playable_keys,
            in_tune_note_color: ecolor::Hsva {
                h: in_tune_note_color.h,
                s: in_tune_note_color.s,
                v: in_tune_note_color.v,
                a: 1.0,
            },
            out_of_tune_note_color: ecolor::Hsva {
                h: out_of_tune_note_color.h,
                s: out_of_tune_note_color.s,
                v: out_of_tune_note_color.v,
                a: 1.0,
            },
            color_period_ct: color_period,
            tmp_correction: Correction::new_zero(),
        }
    }
}

pub struct LatticeWindowControls<T: StackType> {
    pub zoom: f32,
    pub interval_heights: Vec<f32>,
    pub background_stack_distances: Vec<StackCoeff>,
    pub project_dimension: usize,
    pub screen_keyboard_channel: Channel,
    pub screen_keyboard_velocity: u8,
    pub screen_keyboard_pedal_hold: bool,
    pub screen_keyboard_center: u8,
    pub notenamestyle: NoteNameStyle,
    pub correction_system_chooser: Rc<RefCell<CorrectionSystemChooser<T>>>,
    pub highlight_playable_keys: bool,
    pub in_tune_note_color: ecolor::Hsva,
    pub out_of_tune_note_color: ecolor::Hsva,
    pub color_period_ct: Semitones,
    pub tmp_correction: Correction<T>,
}

struct Positions {
    c4_hpos: f32,
    reference_pos: egui::Pos2,
    bottom: f32,
    left: f32,
}

struct OneNodeDrawState<T: StackType> {
    tmp_temperaments: Vec<bool>,
    tmp_correction: Correction<T>,
    tmp_relative_stack: Stack<T>,
}

pub struct LatticeWindow<T: StackType> {
    pub controls: LatticeWindowControls<T>,

    considered_notes: Partial<T>,

    reset_position: bool,
    show_controls: bool,

    positions: Positions,

    draw_state: OneNodeDrawState<T>,
    tmp_stack: Stack<T>,
    other_tmp_stack: Stack<T>,
}

struct PureStacksAround<'a, T: StackType> {
    dists: &'a [StackCoeff],
    reference: &'a Stack<T>,
    curr: Stack<T>,
}

impl<'a, T: StackType> PureStacksAround<'a, T> {
    /// entries of dists must be nonnegative
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

impl<'a, T: StackType> PureStacksAround<'a, T> {
    fn next(&mut self) -> Option<&Stack<T>> {
        for i in (0..T::num_intervals()).rev() {
            if self.curr.target[i] < self.reference.target[i] + self.dists[i] {
                self.curr.increment_at_index_pure(i, 1);
                return Some(&self.curr);
            }
            self.curr.increment_at_index_pure(i, -2 * self.dists[i]);
        }
        return None {};
    }
}

#[derive(PartialEq, Clone, Copy)]
enum NoteDrawStyle {
    Background,
    Considered,
    Playing,
    Antenna,
}

fn background_notename_color(ui: &egui::Ui) -> egui::Color32 {
    ui.style().visuals.weak_text_color()
}

fn foreground_notename_color(ui: &egui::Ui) -> egui::Color32 {
    ui.style().visuals.strong_text_color()
}

fn grid_line_color(ui: &egui::Ui) -> egui::Color32 {
    ui.style().visuals.weak_text_color()
}

fn activation_color<T: StackType>(
    controls: &LatticeWindowControls<T>,
    stack: &Stack<T>,
) -> egui::Color32 {
    let t: f32 = ((stack.semitones() - stack.target_semitones())
        .rem_euclid(controls.color_period_ct / 100.0)
        / controls.color_period_ct
        * 100.0) as f32;
    let start: ecolor::HsvaGamma = controls.in_tune_note_color.into();
    let end: ecolor::HsvaGamma = controls.out_of_tune_note_color.into();
    let x = ecolor::HsvaGamma {
        a: (1.0 - t) * start.a + t * end.a,
        h: {
            let d = end.h - start.h;

            let delta = if d < -0.5 {
                d + 1.0
            } else if d > 0.5 {
                d - 1.0
            } else {
                d
            };

            start.h + t * delta
        },
        s: (1.0 - t) * start.s + t * end.s,
        v: (1.0 - t) * start.v + t * end.v,
    };

    x.into()
}

impl<T: StackType + HasNoteNames> OneNodeDrawState<T> {
    /// returns a rect that may not be as wide as the complete note name, but that is as high as it.
    fn draw_corrected_note_name(
        &self,
        ui: &mut egui::Ui,
        stack: &Stack<T>,
        pos: egui::Pos2,
        controls: &LatticeWindowControls<T>,
        style: NoteDrawStyle,
    ) -> egui::Rect {
        let egui::Pos2 { x: hpos, y: vpos } = pos;

        let first_line_height = match style {
            NoteDrawStyle::Background | NoteDrawStyle::Considered | NoteDrawStyle::Antenna => {
                controls.zoom * FONT_SIZE
            }
            NoteDrawStyle::Playing => controls.zoom * 1.5 * FONT_SIZE,
        };
        let spacing = controls.zoom * 0.5 * FONT_SIZE;
        let other_lines_height = controls.zoom * 0.6 * FONT_SIZE;
        let second_line_vpos = vpos + 0.5 * first_line_height + spacing;
        let third_line_vpos = second_line_vpos + 0.5 * other_lines_height + spacing;
        let text_color = match style {
            NoteDrawStyle::Background | NoteDrawStyle::Antenna => background_notename_color(ui),
            NoteDrawStyle::Considered | NoteDrawStyle::Playing => foreground_notename_color(ui),
        };

        let mut bottom = vpos;

        ui.painter().text(
            pos2(hpos, vpos),
            egui::Align2::CENTER_CENTER,
            stack.notename(&controls.notenamestyle),
            egui::FontId::proportional(first_line_height),
            text_color,
        );
        bottom += first_line_height * 0.5;

        if !stack.is_target() {
            let write_cents = || {
                let d = stack.semitones() - stack.target_semitones();
                ui.painter().text(
                    pos2(hpos, second_line_vpos),
                    egui::Align2::CENTER_CENTER,
                    format!("{}{:.02}ct", if d > 0.0 { "+" } else { "" }, d * 100.0),
                    egui::FontId::proportional(other_lines_height),
                    text_color,
                );
            };
            if controls.correction_system_chooser.borrow().use_cent_values {
                write_cents();
            } else {
                if let Some(correction) = Correction::new(
                    stack,
                    controls
                        .correction_system_chooser
                        .borrow()
                        .preference_order(),
                ) {
                    ui.painter().text(
                        pos2(hpos, second_line_vpos),
                        egui::Align2::CENTER_CENTER,
                        correction.str(),
                        egui::FontId::proportional(other_lines_height),
                        text_color,
                    );
                } else {
                    write_cents();
                }
            }
            bottom += spacing + other_lines_height;
            if stack.is_pure() {
                ui.painter().text(
                    pos2(hpos, third_line_vpos),
                    egui::Align2::CENTER_CENTER,
                    format!("={}", stack.actual_notename(&controls.notenamestyle)),
                    egui::FontId::proportional(other_lines_height),
                    text_color,
                );
                bottom += spacing + other_lines_height;
            }
        }

        let dx = 0.5 * ui.style().spacing.interact_size.x;
        let dy = 0.5 * ui.style().spacing.interact_size.y;
        egui::Rect::from_min_max(pos2(hpos - dx, vpos - dy), pos2(hpos + dx, bottom))
    }

    fn retemper_popup(
        &mut self,
        ui: &mut egui::Ui,
        rect: egui::Rect,
        stack: &Stack<T>,
        reference: &Stack<T>,
        controls: &LatticeWindowControls<T>,
        forward: &mpsc::Sender<FromUi<T>>,
    ) {
        let popup_id = ui.id().with(&stack.target);
        let response = ui.interact(rect, egui::Id::new(&stack.target), egui::Sense::click());
        if response.clicked() {
            for b in self.tmp_temperaments.iter_mut() {
                *b = false;
            }
            ui.memory_mut(|mem| mem.toggle_popup(popup_id));
            self.tmp_relative_stack.clone_from(stack);
            self.tmp_relative_stack.scaled_add(-1, reference);

            if !self.tmp_correction.set_with(
                &self.tmp_relative_stack,
                controls
                    .correction_system_chooser
                    .borrow()
                    .preference_order(),
            ) {
                self.tmp_correction.reset_to_zero();
            }
        }
        egui::popup::popup_below_widget(
            ui,
            popup_id,
            &response,
            egui::popup::PopupCloseBehavior::CloseOnClickOutside,
            |ui| {
                if temperament_applier(
                    Some(&format!(
                        "make pure relative to {}",
                        reference.corrected_notename(
                            &controls.notenamestyle,
                            controls
                                .correction_system_chooser
                                .borrow()
                                .preference_order(),
                            controls.correction_system_chooser.borrow().use_cent_values,
                        )
                    )),
                    ui,
                    &mut self.tmp_correction,
                    &mut self.tmp_relative_stack,
                    controls
                        .correction_system_chooser
                        .borrow()
                        .preference_order(),
                ) {
                    let _ = forward.send(FromUi::Consider {
                        stack: self.tmp_relative_stack.clone(),
                        time: Instant::now(),
                    });
                }
            },
        );
    }

    fn draw_note_and_interaction_zone(
        &mut self,
        ui: &mut egui::Ui,
        stack: &Stack<T>,
        pos: egui::Pos2,
        reference: &Stack<T>,
        controls: &LatticeWindowControls<T>,
        style: NoteDrawStyle,
        forward: &mpsc::Sender<FromUi<T>>,
    ) where
        T: Hash,
    {
        let draw_activation_circle = |active: bool| {
            if active {
                ui.painter().circle_filled(
                    pos,
                    controls.zoom * FONT_SIZE,
                    activation_color(controls, stack),
                );
            } else {
                ui.painter().circle_filled(
                    pos,
                    controls.zoom * 0.6 * FONT_SIZE,
                    ui.style().visuals.window_fill,
                );
            }
        };

        draw_activation_circle(style == NoteDrawStyle::Playing);
        let rect = self.draw_corrected_note_name(ui, stack, pos, controls, style);

        match style {
            NoteDrawStyle::Playing => {}
            NoteDrawStyle::Antenna => {}
            NoteDrawStyle::Background => {
                if ui
                    .interact(rect, egui::Id::new(stack), egui::Sense::click())
                    .clicked()
                {
                    let _ = forward.send(FromUi::Consider {
                        stack: {
                            let mut x = stack.clone();
                            x.scaled_add(-1, reference);
                            x
                        },
                        time: Instant::now(),
                    });
                }
            }
            NoteDrawStyle::Considered => {
                self.retemper_popup(ui, rect, stack, reference, controls, forward);
            }
        }
    }
}

impl<T: StackType> LatticeWindow<T> {
    pub fn new(
        config: LatticeWindowConfig,
        correction_system_chooser: Rc<RefCell<CorrectionSystemChooser<T>>>,
    ) -> Self {
        Self {
            considered_notes: Partial::new(),
            draw_state: OneNodeDrawState {
                tmp_relative_stack: Stack::new_zero(),
                tmp_temperaments: vec![false; T::num_temperaments()],
                tmp_correction: Correction::new_zero(),
            },
            tmp_stack: Stack::new_zero(),
            other_tmp_stack: Stack::new_zero(),
            reset_position: true,
            show_controls: false,
            positions: Positions {
                left: 0.0,
                bottom: 0.0,
                c4_hpos: 0.0,
                reference_pos: pos2(0.0, 0.0),
            },
            controls: config.to_controls(correction_system_chooser),
        }
    }

    pub fn restart_from_config(
        &mut self,
        config: LatticeWindowConfig,
        correction_system_chooser: Rc<RefCell<CorrectionSystemChooser<T>>>,
    ) {
        *self = LatticeWindow::new(config, correction_system_chooser);
    }
}

impl<T: StackType + HasNoteNames> LatticeWindow<T> {
    fn keyboard_hover_interaction(&self, ui: &mut egui::Ui, forward: &mpsc::Sender<FromUi<T>>) {
        if ui.ui_contains_pointer() {
            ui.input(|i| {
                for e in &i.events {
                    match e {
                        egui::Event::Key {
                            key,
                            physical_key,
                            pressed,
                            repeat,
                            ..
                        } => {
                            if *repeat {
                                return;
                            }
                            let the_key = physical_key.unwrap_or(*key);
                            let offset: Option<i16> = match the_key {
                                egui::Key::Q => Some(0), // C
                                egui::Key::Num2 => Some(1),
                                egui::Key::W => Some(2),
                                egui::Key::Num3 => Some(3),
                                egui::Key::E => Some(4),
                                egui::Key::R => Some(5),
                                egui::Key::Num5 => Some(6),
                                egui::Key::T => Some(7),
                                egui::Key::Num6 => Some(8),
                                egui::Key::Y => Some(9),
                                egui::Key::Num7 => Some(10),
                                egui::Key::U => Some(11),
                                egui::Key::I => Some(12), // C above
                                egui::Key::Num9 => Some(13),
                                egui::Key::O => Some(14),
                                egui::Key::Num0 => Some(15),
                                egui::Key::P => Some(16),
                                egui::Key::OpenBracket => Some(17),
                                egui::Key::Equals => Some(18),
                                egui::Key::CloseBracket => Some(19), // G above
                                egui::Key::Slash => Some(-1),
                                egui::Key::Semicolon => Some(-2),
                                egui::Key::Period => Some(-3),
                                egui::Key::L => Some(-4),
                                egui::Key::Comma => Some(-5),
                                egui::Key::K => Some(-6),
                                egui::Key::M => Some(-7),
                                egui::Key::N => Some(-8),
                                egui::Key::H => Some(-9),
                                egui::Key::B => Some(-10),
                                egui::Key::G => Some(-11),
                                egui::Key::V => Some(-12), // C below
                                egui::Key::C => Some(-13),
                                egui::Key::D => Some(-14),
                                egui::Key::X => Some(-15),
                                egui::Key::S => Some(-16),
                                egui::Key::Z => Some(-17), // G below
                                egui::Key::A => Some(-18),
                                _ => None {},
                            };
                            if let Some(offset) = offset {
                                let note = self.controls.screen_keyboard_center as i16 + offset;
                                if note <= 127 && note >= 0 {
                                    if *pressed {
                                        let _ = forward.send(FromUi::NoteOn {
                                            channel: self.controls.screen_keyboard_channel,
                                            note: note as u8,
                                            velocity: self.controls.screen_keyboard_velocity,
                                            time: Instant::now(),
                                        });
                                    } else {
                                        let _ = forward.send(FromUi::NoteOff {
                                            channel: self.controls.screen_keyboard_channel,
                                            note: note as u8,
                                            velocity: self.controls.screen_keyboard_velocity,
                                            time: Instant::now(),
                                        });
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }
            });
        }
    }

    fn key_click_interaction(
        &mut self,
        rect: egui::Rect,
        key_number: u8,
        ui: &mut egui::Ui,
        forward: &mpsc::Sender<FromUi<T>>,
    ) {
        let r = ui.interact(rect, ui.id().with(key_number), egui::Sense::drag());

        if r.drag_started() {
            let _ = forward.send(FromUi::NoteOn {
                channel: self.controls.screen_keyboard_channel,
                note: key_number,
                velocity: self.controls.screen_keyboard_velocity,
                time: Instant::now(),
            });
        }

        if r.drag_stopped() {
            let _ = forward.send(FromUi::NoteOff {
                channel: self.controls.screen_keyboard_channel,
                note: key_number,
                velocity: self.controls.screen_keyboard_velocity,
                time: Instant::now(),
            });
        }
    }

    fn key_border_color(&self, ui: &egui::Ui, key_number: u8) -> egui::Color32 {
        if !self.controls.highlight_playable_keys {
            if key_number >= 109 || key_number <= 20
            // the range of the piano
            {
                ui.style().visuals.weak_text_color()
            } else {
                ui.style().visuals.strong_text_color()
            }
        } else {
            let d = key_number as i16 - self.controls.screen_keyboard_center as i16;
            if d <= 19 && d >= -18
            // the range playable in [Self.key_interaction]
            {
                ui.style().visuals.strong_text_color()
            } else if key_number >= 109 || key_number <= 20 {
                ui.style().visuals.weak_text_color()
            } else {
                ui.style().visuals.text_color()
            }
        }
    }

    fn draw_white_keys(
        &mut self,
        ui: &mut egui::Ui,
        state: &KeysAndTunings<T>,
        forward: &mpsc::Sender<FromUi<T>>,
    ) {
        let bottom = self.positions.bottom;
        let left = self.positions.left;
        let zoom = self.controls.zoom;
        let white_key_width = zoom * OCTAVE_WIDTH / 7.0;
        let mut rect = egui::Rect::from_min_max(
            pos2(left, bottom - zoom * WHITE_KEY_LENGTH),
            pos2(left + white_key_width, bottom),
        );

        let active_color = ui.style().visuals.selection.bg_fill;

        let steps = [2, 2, 1, 2, 2, 2, 1];
        let mut key_number: u8 = 0;
        let mut pitch_class = 0;
        while key_number <= 127 {
            let border_color = self.key_border_color(ui, key_number);
            if state.active_notes[key_number as usize].is_sounding() {
                ui.painter().rect(
                    rect,
                    egui::CornerRadius::default(),
                    active_color,
                    egui::Stroke::new(zoom * PIANO_KEY_BORDER_THICKNESS, border_color),
                    egui::StrokeKind::Middle,
                );
            } else {
                ui.painter().rect_stroke(
                    rect,
                    egui::CornerRadius::default(),
                    egui::Stroke::new(zoom * PIANO_KEY_BORDER_THICKNESS, border_color),
                    egui::StrokeKind::Middle,
                );
            }
            self.key_click_interaction(rect, key_number, ui, forward);
            rect = rect.translate(vec2(white_key_width, 0.0));
            key_number += steps[pitch_class];
            pitch_class = (pitch_class + 1) % 7;
        }
    }

    fn draw_black_keys(
        &mut self,
        ui: &mut egui::Ui,
        state: &KeysAndTunings<T>,
        forward: &mpsc::Sender<FromUi<T>>,
    ) {
        let bottom = self.positions.bottom;
        let left = self.positions.left;
        let zoom = self.controls.zoom;
        let key_number_steps = [2, 3, 2, 2, 3];
        let w = zoom * OCTAVE_WIDTH / 7.0; // bottom width of white key.
        let b = zoom * BLACK_KEY_WIDTH; // width of a black key;
        let w1 = b; // top width of a white key that is between two black keys (D, G, A)
        let w2 = (3.0 * w - 2.0 * b - w1) / 2.0; // top width of C and E keys
        let w3 = (4.0 * w - 3.0 * b - 2.0 * w1) / 2.0; // top width of F and B keys

        let spacing_steps = [b + w1, b + w2 + w3, b + w1, b + w1, b + w3 + w2];

        let mut rect = egui::Rect::from_min_max(
            pos2(left + w2, bottom - zoom * WHITE_KEY_LENGTH),
            pos2(
                left + w2 + b,
                bottom - zoom * (WHITE_KEY_LENGTH - BLACK_KEY_LENGTH),
            ),
        );

        let active_color = ui.style().visuals.selection.bg_fill;

        let mut key_number: u8 = 1;
        let mut pitch_class = 0;
        while key_number <= 127 {
            let border_color = self.key_border_color(ui, key_number);
            ui.painter().rect(
                rect,
                egui::CornerRadius::default(),
                if state.active_notes[key_number as usize].is_sounding() {
                    active_color
                } else {
                    border_color
                },
                egui::Stroke::new(zoom * PIANO_KEY_BORDER_THICKNESS, border_color),
                egui::StrokeKind::Middle,
            );
            self.key_click_interaction(rect, key_number, ui, forward);
            rect = rect.translate(vec2(spacing_steps[pitch_class], 0.0));
            key_number += key_number_steps[pitch_class];
            pitch_class = (pitch_class + 1) % 5;
        }
    }

    fn draw_ruler(&self, ui: &mut egui::Ui) {
        let bottom = self.positions.bottom;
        let zoom = self.controls.zoom;
        let mut x = self.positions.left + self.controls.zoom / 2.0;
        let y = egui::Rangef {
            min: bottom - zoom * (WHITE_KEY_LENGTH + MARKER_LENGTH),
            max: bottom - zoom * WHITE_KEY_LENGTH,
        };
        for _ in 0..128 {
            ui.painter().vline(
                x,
                y,
                egui::Stroke::new(zoom * MARKER_THICKNESS, ui.style().visuals.text_color()),
            );
            x += zoom * OCTAVE_WIDTH / 12.0;
        }
    }

    fn draw_keyboard(
        &mut self,
        ui: &mut egui::Ui,
        state: &KeysAndTunings<T>,
        forward: &mpsc::Sender<FromUi<T>>,
    ) {
        self.draw_ruler(ui);
        self.draw_white_keys(ui, state, forward);
        self.draw_black_keys(ui, state, forward);
    }

    fn c4_offset(&self, state: &KeysAndTunings<T>) -> f32 {
        self.controls.zoom
            * (0.5 // half a key width on the ruler above the piano
                   + state.tuning_reference.c4_semitones() as f32)
    }

    fn compute_reference_positions(&mut self, state: &KeysAndTunings<T>) {
        self.positions.c4_hpos = self.positions.left + self.c4_offset(state);

        self.positions.reference_pos.x =
            self.positions.c4_hpos + self.controls.zoom * state.reference.semitones() as f32;

        let lowest_background = self
            .controls
            .background_stack_distances
            .iter()
            .enumerate()
            .fold(0.0, |acc, (i, c)| {
                acc + *c as f32 * self.controls.zoom * self.controls.interval_heights[i].abs()
            });

        let lowest_considered = self
            .considered_notes
            .iter()
            .fold(0.0, |acc: f32, (_i, stack)| {
                let d = stack.target.iter().enumerate().fold(0.0, |acc, (i, c)| {
                    acc + *c as f32 * self.controls.zoom * self.controls.interval_heights[i]
                });
                acc.max(d)
            });

        self.positions.reference_pos.y = self.positions.bottom
            - self.keyboard_height()
            - self.controls.zoom * FREE_SPACE_ABOVE_KEYBOARD
            - lowest_considered.max(lowest_background);
    }

    fn vpos_relative_to_reference(&self, state: &KeysAndTunings<T>, stack: &Stack<T>) -> f32 {
        self.controls.zoom * {
            let mut y = 0.0;
            for i in 0..T::num_intervals() {
                y += (stack.target[i] - state.reference.target[i]) as f32
                    * self.controls.interval_heights[i];
            }
            y
        }
    }

    fn vpos(&self, state: &KeysAndTunings<T>, stack: &Stack<T>) -> f32 {
        self.positions.reference_pos.y + self.vpos_relative_to_reference(state, stack)
    }

    fn hpos(&self, stack: &Stack<T>) -> f32 {
        self.positions.c4_hpos + self.controls.zoom * stack.semitones() as f32
    }

    fn pos(&self, state: &KeysAndTunings<T>, stack: &Stack<T>) -> egui::Pos2 {
        pos2(self.hpos(stack), self.vpos(state, stack))
    }

    fn has_projection(&self, state: &KeysAndTunings<T>, stack: &Stack<T>) -> bool {
        stack.target[self.controls.project_dimension]
            != state.reference.target[self.controls.project_dimension]
    }

    fn projected_pos(&self, state: &KeysAndTunings<T>, stack: &Stack<T>) -> egui::Pos2 {
        self.pos(state, stack)
            - (stack.target[self.controls.project_dimension]
                - state.reference.target[self.controls.project_dimension]) as f32
                * self.controls.zoom
                * vec2(
                    T::intervals()[self.controls.project_dimension].semitones as f32,
                    self.controls.interval_heights[self.controls.project_dimension],
                )
    }

    fn grid_line_stroke(&self, ui: &egui::Ui) -> egui::Stroke {
        egui::Stroke::new(
            self.controls.zoom * FAINT_GRID_LINE_THICKNESS,
            grid_line_color(ui),
        )
    }

    fn draw_grid_lines(&mut self, ui: &egui::Ui, state: &KeysAndTunings<T>) {
        let color = grid_line_color(ui);
        let stroke = self.grid_line_stroke(ui);

        let draw_circle = |pos| {
            ui.painter()
                .circle_filled(pos, self.controls.zoom * GRID_NODE_RADIUS, color);
        };

        let draw_limb = |direction: usize, forward: bool, start_pos: egui::Pos2| {
            let end_pos = start_pos
                + self.controls.zoom
                    * if forward { 1.0 } else { -1.0 }
                    * vec2(
                        T::intervals()[direction].semitones as f32,
                        self.controls.interval_heights[direction],
                    );
            ui.painter().line_segment([start_pos, end_pos], stroke);
            end_pos
        };

        let mut background =
            PureStacksAround::new(&self.controls.background_stack_distances, &state.reference);
        while let Some(stack) = background.next() {
            for i in 0..T::num_intervals() {
                let d = stack.target[i] - state.reference.target[i];
                if d == 0 {
                    continue;
                }
                let p = self.pos(state, stack);
                // draw_circle(p);
                let _ = draw_limb(i, d < 0, p);
            }
        }

        let draw_path_without_projection = |stack: &Stack<T>| {
            let mut in_bounds = true;
            for i in 0..T::num_intervals() {
                if i == self.controls.project_dimension {
                    continue;
                }
                if (stack.target[i] - state.reference.target[i]).abs()
                    > self.controls.background_stack_distances[i]
                {
                    in_bounds = false;
                    break;
                }
            }

            if !in_bounds {
                let mut pos = self.positions.reference_pos;
                for i in 0..T::num_intervals() {
                    if i == self.controls.project_dimension {
                        continue;
                    }
                    let d = stack.target[i] - state.reference.target[i];
                    for _ in 0..d.abs() {
                        pos = draw_limb(i, d > 0, pos);
                        draw_circle(pos);
                    }
                }
            }
        };

        self.considered_notes.for_each_stack(|_, stk| {
            self.tmp_stack.clone_from(&state.reference);
            self.tmp_stack.scaled_add(1, stk);
            draw_path_without_projection(&self.tmp_stack);
        });

        for (i, stack) in state.tunings.iter().enumerate() {
            if state.active_notes[i].is_sounding() {
                draw_path_without_projection(stack);
                let mut pos = self.projected_pos(state, stack);
                let d = stack.target[self.controls.project_dimension]
                    - state.reference.target[self.controls.project_dimension];
                for _ in 0..d.abs() {
                    pos = draw_limb(self.controls.project_dimension, d > 0, pos);
                    draw_circle(pos);
                }
            }
        }
    }

    fn draw_down_lines(&self, ui: &egui::Ui, state: &KeysAndTunings<T>) {
        let bottom = self.keyboard_top();
        for (i, stack) in state.tunings.iter().enumerate() {
            if state.active_notes[i].is_sounding() {
                let ppos = self.projected_pos(state, stack);
                ui.painter().vline(
                    ppos.x,
                    egui::Rangef {
                        min: ppos.y,
                        max: bottom,
                    },
                    self.grid_line_stroke(ui),
                );

                if self.has_projection(state, stack) {
                    let pos = self.pos(state, stack);
                    ui.painter().vline(
                        pos.x,
                        egui::Rangef {
                            min: pos.y,
                            max: bottom,
                        },
                        self.grid_line_stroke(ui),
                    );
                }
            }
        }
    }

    fn draw_note_names_and_interaction_zones(
        &mut self,
        ui: &mut egui::Ui,
        state: &KeysAndTunings<T>,
        forward: &mpsc::Sender<FromUi<T>>,
    ) where
        T: Hash,
    {
        let write_considered_stack_to_draw = |considered: &Stack<T>, output: &mut Stack<T>| {
            output.clone_from(considered);
            output.increment_at_index_pure(
                self.controls.project_dimension,
                -considered.target[self.controls.project_dimension],
            );
            output.scaled_add(1, &state.reference);
        };

        let write_sounding_stack_to_draw = |sounding: &Stack<T>, output: &mut Stack<T>| {
            output.clone_from(sounding);
            output.increment_at_index_pure(
                self.controls.project_dimension,
                state.reference.target[self.controls.project_dimension]
                    - sounding.target[self.controls.project_dimension],
            );
        };

        let mut background =
            PureStacksAround::new(&self.controls.background_stack_distances, &state.reference);
        while let Some(stack) = background.next() {
            let draw_this = self.considered_notes.iter().all(|(_, considered)| {
                write_considered_stack_to_draw(considered, &mut self.tmp_stack);
                self.tmp_stack.target != stack.target
            }) && state.tunings.iter().enumerate().all(|(i, sounding)| {
                if !state.active_notes[i].is_sounding() {
                    return true;
                }
                write_sounding_stack_to_draw(sounding, &mut self.tmp_stack);
                self.tmp_stack.target != stack.target
            });
            if draw_this {
                self.draw_state.draw_note_and_interaction_zone(
                    ui,
                    stack,
                    self.pos(state, stack),
                    &state.reference,
                    &self.controls,
                    NoteDrawStyle::Background,
                    forward,
                );
            }
        }

        for (_, stack) in self.considered_notes.iter() {
            write_considered_stack_to_draw(stack, &mut self.tmp_stack);
            let draw_this = state.tunings.iter().enumerate().all(|(i, sounding)| {
                if !state.active_notes[i].is_sounding() {
                    return true;
                }
                write_sounding_stack_to_draw(sounding, &mut self.other_tmp_stack);
                self.tmp_stack.target != self.other_tmp_stack.target
            });
            if draw_this {
                self.draw_state.draw_note_and_interaction_zone(
                    ui,
                    &self.tmp_stack,
                    self.pos(state, &self.tmp_stack),
                    &state.reference,
                    &self.controls,
                    NoteDrawStyle::Considered,
                    forward,
                );
            }
        }

        for (i, stack) in state.tunings.iter().enumerate() {
            if state.active_notes[i].is_sounding() {
                write_sounding_stack_to_draw(stack, &mut self.tmp_stack);
                self.draw_state.draw_note_and_interaction_zone(
                    ui,
                    &self.tmp_stack,
                    self.pos(state, &self.tmp_stack),
                    &state.reference,
                    &self.controls,
                    NoteDrawStyle::Playing,
                    forward,
                );
                if self.has_projection(state, stack) {
                    self.draw_state.draw_note_and_interaction_zone(
                        ui,
                        stack,
                        self.pos(state, stack),
                        &state.reference,
                        &self.controls,
                        NoteDrawStyle::Antenna,
                        forward,
                    );
                }
            }
        }
    }

    fn draw_lattice(
        &mut self,
        ui: &mut egui::Ui,
        state: &KeysAndTunings<T>,
        forward: &mpsc::Sender<FromUi<T>>,
    ) where
        T: Hash,
    {
        self.compute_reference_positions(state);
        self.draw_down_lines(ui, state);
        self.draw_grid_lines(ui, state);
        self.draw_note_names_and_interaction_zones(ui, state, forward);
    }

    fn keyboard_height(&self) -> f32 {
        self.controls.zoom * (WHITE_KEY_LENGTH + MARKER_LENGTH)
    }

    fn keyboard_top(&self) -> f32 {
        self.positions.bottom - self.keyboard_height()
    }
}

impl<T: StackType + HasNoteNames + Hash> LatticeWindow<T> {
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        state: &KeysAndTunings<T>,
        forward: &mpsc::Sender<FromUi<T>>,
    ) {
        let mut r = ui.interact(
            ui.max_rect(),
            egui::Id::new("global_grid_interaction"),
            egui::Sense::click_and_drag(),
        );

        if r.dragged() {
            let egui::Vec2 { x, y } = r.drag_delta();
            self.positions.left += x;
            self.positions.bottom = (self.positions.bottom + y).max(ui.max_rect().bottom());
            self.reset_position = false;
        }
        if r.double_clicked() {
            self.reset_position = true;
        }
        if self.reset_position {
            let egui::Pos2 {
                x: center,
                y: bottom,
            } = ui.max_rect().center_bottom();
            let left = ui.max_rect().left();
            self.positions.left = left - (self.c4_offset(state) - center);
            self.positions.bottom = bottom;
        }

        self.keyboard_hover_interaction(ui, forward);

        // egui::Frame::new().show(ui, |ui| {
        self.draw_keyboard(ui, state, forward);
        self.draw_lattice(ui, state, forward);
        // });

        ui.horizontal(|ui| {
            r = ui.button("☰");
            if r.clicked() {
                self.show_controls = !self.show_controls;
            }

            if ui.button("🔍+").clicked() {
                self.controls.zoom *= 1.2;
            }
            if ui.button("🔍-").clicked() {
                self.controls.zoom /= 1.2;
            }
        });

        if self.show_controls {
            egui::Window::new("lattice_control_window")
                .resizable(false)
                .title_bar(false)
                .fixed_pos(r.rect.left_bottom() + vec2(0.0, ui.style().spacing.item_spacing.y))
                .pivot(egui::Align2::LEFT_TOP)
                .show(ui.ctx(), |ui| {
                    let reference_name = state.reference.corrected_notename(
                        &self.controls.notenamestyle,
                        self.controls
                            .correction_system_chooser
                            .borrow()
                            .preference_order(),
                        self.controls
                            .correction_system_chooser
                            .borrow()
                            .use_cent_values,
                    );
                    AsBigControls(self).show(&reference_name, ui);
                });
        }
    }
}

impl<T: StackType> HandleMsgRef<ToUi<T>, FromUi<T>> for LatticeWindow<T> {
    fn handle_msg_ref(&mut self, msg: &ToUi<T>, _forward: &mpsc::Sender<FromUi<T>>) {
        match msg {
            ToUi::Consider { stack } => {
                let _ = self.considered_notes.insert(stack);
            }

            ToUi::PedalHold { channel, value, .. } => {
                self.controls.screen_keyboard_pedal_hold =
                    (*channel == self.controls.screen_keyboard_channel) & (*value != 0);
            }

            ToUi::DetunedNote { .. } => todo!(),

            _ => {}
        }
    }
}

impl<T: StackType> ExtractConfig<LatticeWindowConfig> for LatticeWindow<T> {
    fn extract_config(&self) -> LatticeWindowConfig {
        let LatticeWindowControls {
            zoom,
            interval_heights,
            background_stack_distances,
            project_dimension,
            screen_keyboard_channel,
            screen_keyboard_velocity,
            notenamestyle,
            highlight_playable_keys,
            in_tune_note_color,
            out_of_tune_note_color,
            color_period_ct: color_period,
            ..
        } = &self.controls;
        LatticeWindowConfig {
            zoom: *zoom,
            interval_heights: interval_heights.clone(),
            background_stack_distances: background_stack_distances.clone(),
            project_dimension: *project_dimension,
            screen_keyboard_channel: *screen_keyboard_channel,
            screen_keyboard_velocity: *screen_keyboard_velocity,
            notenamestyle: *notenamestyle,
            highlight_playable_keys: *highlight_playable_keys,
            in_tune_note_color: Hsv {
                h: in_tune_note_color.h,
                s: in_tune_note_color.s,
                v: in_tune_note_color.v,
            },
            out_of_tune_note_color: Hsv {
                h: out_of_tune_note_color.h,
                s: out_of_tune_note_color.s,
                v: out_of_tune_note_color.v,
            },
            color_period: *color_period,
        }
    }
}
