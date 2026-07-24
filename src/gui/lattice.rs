use std::time::Instant;

use eframe::egui::{self, pos2, vec2, Popup, PopupCloseBehavior};
use midi_msg::Channel;
use serde_derive::{Deserialize, Serialize};

use crate::{
    custom_serde::common::{deserialize_channel, serialize_channel},
    gui::{
        common::temperament_applier,
        r#trait::{ReceiveToUiRef, GuiShow, UiAdaptor},
    },
    interval::{
        base::Semitones,
        stack::{ScaledAdd, Stack},
        stacktype::r#trait::{StackCoeff, StackType},
    },
    msg::{FromUi, ToUi},
    neighbourhood::{Neighbourhood, Partial},
    notename::{correction::Correction, HasNoteNames},
    process::r#trait::StackWithTuning,
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
pub struct LatticeWindowConfig {
    pub zoom: f32,
    pub interval_heights: Vec<f32>,
    pub background_around_reference: bool,
    pub background_low: Vec<StackCoeff>,
    pub background_high: Vec<StackCoeff>,
    pub project_dimension: usize,
    pub color_period_ct: Semitones,
    #[serde(
        serialize_with = "serialize_channel",
        deserialize_with = "deserialize_channel"
    )]
    pub screen_keyboard_channel: Channel,
    pub screen_keyboard_velocity: u8,
    pub highlight_playable_keys: bool,
}

struct Positions {
    c4_hpos: f32,
    grid_reference_pos: egui::Pos2, // not necessarily the reference of the current neighbourhood
    bottom: f32,
    left: f32,
    /// The next two will not necessarily be equal to the values in [LatticeWindowControls] (and
    /// [LatticeWindowConfig], as they extend to include also the considered or soundign notes.
    background_low: Vec<StackCoeff>,
    background_high: Vec<StackCoeff>,
}

struct OneNodeDrawState<T: StackType> {
    tmp_temperaments: Vec<bool>,
    tmp_correction: Correction<T>,
    tmp_relative_stack: Stack<T>,
}

pub struct LatticeWindow<T: StackType> {
    considered_notes: Partial<T>,

    reset_position: bool,

    grid_reference: Stack<T>,
    positions: Positions,

    draw_state: OneNodeDrawState<T>,
    tmp_stack: Stack<T>,
    other_tmp_stack: Stack<T>,
}

struct PureStacksAround<'a, T: StackType> {
    low: &'a [StackCoeff],
    high: &'a [StackCoeff],
    reference: &'a Stack<T>,
    curr: Stack<T>,
}

impl<'a, T: StackType> PureStacksAround<'a, T> {
    /// entries of low must be less than or equal to 0, entries of high must be nonnegative
    fn new(low: &'a [StackCoeff], high: &'a [StackCoeff], reference: &'a Stack<T>) -> Self {
        let mut curr = reference.clone();

        for i in 0..T::num_intervals() {
            curr.increment_at_index_pure(i, low[i]);
        }

        curr.increment_at_index_pure(T::num_intervals() - 1, -1);

        Self {
            low,
            high,
            reference,
            curr,
        }
    }
}

impl<'a, T: StackType> PureStacksAround<'a, T> {
    fn next(&mut self) -> Option<&Stack<T>> {
        for i in (0..T::num_intervals()).rev() {
            if self.curr.target[i] < self.reference.target[i] + self.high[i] {
                self.curr.increment_at_index_pure(i, 1);
                return Some(&self.curr);
            }
            self.curr
                .increment_at_index_pure(i, self.low[i] - self.high[i]);
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
    ui: &egui::Ui,
    config: &LatticeWindowConfig,
    stack: &Stack<T>,
) -> egui::Color32 {
    let t: f32 = ((stack.semitones() - stack.target_semitones())
        .rem_euclid(config.color_period_ct / 100.0)
        / config.color_period_ct
        * 100.0) as f32;
    let start_color = ecolor::HsvaGamma::from(ui.style().visuals.selection.bg_fill);
    (ecolor::HsvaGamma {
        a: start_color.a,
        h: (start_color.h + t).rem_euclid(1.0),
        s: start_color.s,
        v: start_color.v,
    })
    .into()
}

impl<T: StackType + HasNoteNames> OneNodeDrawState<T> {
    /// returns a rect that may not be as wide as the complete note name, but that is as high as it.
    fn draw_corrected_note_name(
        &self,
        ui: &mut egui::Ui,
        stack: &Stack<T>,
        pos: egui::Pos2,
        style: NoteDrawStyle,
        adaptor: &impl UiAdaptor<T>,
    ) -> egui::Rect {
        let egui::Pos2 { x: hpos, y: vpos } = pos;
        let config = &adaptor.config().lattice;

        let first_line_height = match style {
            NoteDrawStyle::Background | NoteDrawStyle::Considered | NoteDrawStyle::Antenna => {
                config.zoom * FONT_SIZE
            }
            NoteDrawStyle::Playing => config.zoom * 1.5 * FONT_SIZE,
        };
        let spacing = config.zoom * 0.5 * FONT_SIZE;
        let other_lines_height = config.zoom * 0.6 * FONT_SIZE;
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
            stack.notename(&adaptor.config().notenamestyle),
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
            if adaptor.config().use_cent_values {
                write_cents();
            } else {
                if let Some(correction) = Correction::new(stack) {
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
                    format!(
                        "={}",
                        stack.actual_notename(&adaptor.config().notenamestyle)
                    ),
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
        adaptor: &impl UiAdaptor<T>,
    ) {
        let popup_id = ui.id().with(&stack.target);
        let response = ui.interact(rect, egui::Id::new(&stack.target), egui::Sense::click());
        if response.clicked() {
            for b in self.tmp_temperaments.iter_mut() {
                *b = false;
            }
            self.tmp_relative_stack.clone_from(stack);
            self.tmp_relative_stack.scaled_add(-1, reference);

            if !self.tmp_correction.set_with(&self.tmp_relative_stack) {
                self.tmp_correction.reset_to_zero();
            }
        }
        Popup::menu(&response)
            .id(popup_id)
            .close_behavior(PopupCloseBehavior::CloseOnClickOutside)
            .show(|ui| {
                if temperament_applier(
                    Some(&format!(
                        "make pure relative to {}",
                        reference.corrected_notename(
                            &adaptor.config().notenamestyle,
                            adaptor.config().use_cent_values
                        )
                    )),
                    ui,
                    &mut self.tmp_correction,
                    &mut self.tmp_relative_stack,
                ) {
                    let _ = adaptor.send_consider(&self.tmp_relative_stack, Instant::now());
                }
            });
    }

    fn draw_note_and_interaction_zone(
        &mut self,
        ui: &mut egui::Ui,
        stack: &Stack<T>,
        pos: egui::Pos2,
        reference: &Stack<T>,
        style: NoteDrawStyle,
        adaptor: &impl UiAdaptor<T>,
    ) {
        let config = &adaptor.config().lattice;
        let draw_activation_circle = |active: bool| {
            if active {
                ui.painter().circle_filled(
                    pos,
                    config.zoom * FONT_SIZE,
                    activation_color(ui, config, stack),
                );
            } else {
                ui.painter().circle_filled(
                    pos,
                    config.zoom * 0.6 * FONT_SIZE,
                    ui.style().visuals.window_fill,
                );
            }
        };

        draw_activation_circle(style == NoteDrawStyle::Playing);
        let rect = self.draw_corrected_note_name(ui, stack, pos, style, adaptor);

        match style {
            NoteDrawStyle::Playing => {}
            NoteDrawStyle::Antenna => {}
            NoteDrawStyle::Background => {
                if ui
                    .interact(rect, egui::Id::new(stack), egui::Sense::click())
                    .clicked()
                {
                    self.tmp_relative_stack.clone_from(&stack);
                    self.tmp_relative_stack.scaled_add(-1, reference);
                    let _ = adaptor.send_consider(&self.tmp_relative_stack, Instant::now());
                }
            }
            NoteDrawStyle::Considered => {
                self.retemper_popup(ui, rect, stack, reference, adaptor);
            }
        }
    }
}

impl<T: StackType> LatticeWindow<T> {
    pub fn new() -> Self {
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
            grid_reference: Stack::new_zero(),
            positions: Positions {
                left: 0.0,
                bottom: 0.0,
                c4_hpos: 0.0,
                grid_reference_pos: pos2(0.0, 0.0),
                background_low: vec![0; T::num_intervals()],
                background_high: vec![0; T::num_intervals()],
            },
        }
    }

    // pub fn restart_from_config(&mut self) { //, config: LatticeWindowConfig) {
    //     *self = LatticeWindow::new();
    // }
}

impl<T: StackType + HasNoteNames> LatticeWindow<T> {
    fn keyboard_hover_interaction(&self, ui: &mut egui::Ui, adaptor: &impl UiAdaptor<T>) {
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
                                let note = 60 + offset;
                                if note <= 127 && note >= 0 {
                                    if *pressed {
                                        let _ = adaptor.send(FromUi::NoteOn {
                                            channel: adaptor
                                                .config()
                                                .lattice
                                                .screen_keyboard_channel,
                                            note: note as u8,
                                            velocity: adaptor
                                                .config()
                                                .lattice
                                                .screen_keyboard_velocity,
                                            time: Instant::now(),
                                        });
                                    } else {
                                        let _ = adaptor.send(FromUi::NoteOff {
                                            channel: adaptor
                                                .config()
                                                .lattice
                                                .screen_keyboard_channel,
                                            note: note as u8,
                                            velocity: adaptor
                                                .config()
                                                .lattice
                                                .screen_keyboard_velocity,
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
        adaptor: &impl UiAdaptor<T>,
    ) {
        let r = ui.interact(rect, ui.id().with(key_number), egui::Sense::drag());

        if r.drag_started() {
            let _ = adaptor.send(FromUi::NoteOn {
                channel: adaptor.config().lattice.screen_keyboard_channel,
                note: key_number,
                velocity: adaptor.config().lattice.screen_keyboard_velocity,
                time: Instant::now(),
            });
        }

        if r.drag_stopped() {
            let _ = adaptor.send(FromUi::NoteOff {
                channel: adaptor.config().lattice.screen_keyboard_channel,
                note: key_number,
                velocity: adaptor.config().lattice.screen_keyboard_velocity,
                time: Instant::now(),
            });
        }
    }

    fn key_border_color(
        &self,
        ui: &egui::Ui,
        key_number: u8,
        adaptor: &impl UiAdaptor<T>,
    ) -> egui::Color32 {
        if !adaptor.config().lattice.highlight_playable_keys {
            if key_number >= 109 || key_number <= 20
            // the range of the piano
            {
                ui.style().visuals.weak_text_color()
            } else {
                ui.style().visuals.strong_text_color()
            }
        } else {
            let d = key_number as i16 - 60;
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

    fn draw_white_keys(&mut self, ui: &mut egui::Ui, adaptor: &impl UiAdaptor<T>) {
        let bottom = self.positions.bottom;
        let left = self.positions.left;
        let zoom = adaptor.config().lattice.zoom;
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
            let border_color = self.key_border_color(ui, key_number, adaptor);
            if adaptor.key_state(key_number as usize).is_sounding() {
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
            self.key_click_interaction(rect, key_number, ui, adaptor);
            rect = rect.translate(vec2(white_key_width, 0.0));
            key_number += steps[pitch_class];
            pitch_class = (pitch_class + 1) % 7;
        }
    }

    fn draw_black_keys(&mut self, ui: &mut egui::Ui, adaptor: &impl UiAdaptor<T>) {
        let bottom = self.positions.bottom;
        let left = self.positions.left;
        let zoom = adaptor.config().lattice.zoom;
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
            let border_color = self.key_border_color(ui, key_number, adaptor);
            ui.painter().rect(
                rect,
                egui::CornerRadius::default(),
                if adaptor.key_state(key_number as usize).is_sounding() {
                    active_color
                } else {
                    border_color
                },
                egui::Stroke::new(zoom * PIANO_KEY_BORDER_THICKNESS, border_color),
                egui::StrokeKind::Middle,
            );
            self.key_click_interaction(rect, key_number, ui, adaptor);
            rect = rect.translate(vec2(spacing_steps[pitch_class], 0.0));
            key_number += key_number_steps[pitch_class];
            pitch_class = (pitch_class + 1) % 5;
        }
    }

    fn draw_ruler(&self, ui: &mut egui::Ui, adaptor: &impl UiAdaptor<T>) {
        let bottom = self.positions.bottom;
        let zoom = adaptor.config().lattice.zoom;
        let mut x = self.positions.left + adaptor.config().lattice.zoom / 2.0;
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

    fn draw_keyboard(&mut self, ui: &mut egui::Ui, adaptor: &impl UiAdaptor<T>) {
        self.draw_ruler(ui, adaptor);
        self.draw_white_keys(ui, adaptor);
        self.draw_black_keys(ui, adaptor);
    }

    fn c4_offset(&self, adaptor: &impl UiAdaptor<T>) -> f32 {
        adaptor.config().lattice.zoom
            * (0.5 // half a key width on the ruler above the piano
                   + adaptor.tuning_reference().c4_semitones() as f32)
    }

    fn update_positions(&mut self, adaptor: &impl UiAdaptor<T>) {
        if adaptor.config().lattice.background_around_reference {
            self.grid_reference.clone_from(&adaptor.reference());
        } else {
            self.grid_reference.reset_to_zero();
        }

        self.positions
            .background_low
            .copy_from_slice(&adaptor.config().lattice.background_low);
        self.positions
            .background_high
            .copy_from_slice(&adaptor.config().lattice.background_high);

        for (_, relative_stack) in self.considered_notes.iter() {
            for i in 0..T::num_intervals() {
                if i == adaptor.config().lattice.project_dimension {
                    continue;
                }
                let x = relative_stack.target[i] + adaptor.reference().target[i]
                    - self.grid_reference.target[i];
                self.positions.background_low[i] = self.positions.background_low[i].min(x);
                self.positions.background_high[i] = self.positions.background_high[i].max(x);
            }
        }

        for j in 0..128 {
            if adaptor.key_state(j).is_sounding() {
                let StackWithTuning { stack, .. } = &*adaptor.tuning(j);
                for i in 0..T::num_intervals() {
                    if i == adaptor.config().lattice.project_dimension {
                        continue;
                    }
                    let x = stack.target[i] - self.grid_reference.target[i];
                    self.positions.background_low[i] = self.positions.background_low[i].min(x);
                    self.positions.background_high[i] = self.positions.background_high[i].max(x);
                }
            }
        }

        self.positions.c4_hpos = self.positions.left + self.c4_offset(adaptor);

        self.positions.grid_reference_pos.x = self.positions.c4_hpos
            + adaptor.config().lattice.zoom * self.grid_reference.semitones() as f32;

        let mut lowest_background: f32 = 0.0;
        let mut background = PureStacksAround::new(
            &self.positions.background_low,
            &self.positions.background_high,
            &self.grid_reference,
        );
        while let Some(stack) = background.next() {
            lowest_background =
                lowest_background.max(self.vpos_relative_to_grid_reference(stack, adaptor));
        }

        self.positions.grid_reference_pos.y = self.positions.bottom
            - self.keyboard_height(adaptor)
            - adaptor.config().lattice.zoom * FREE_SPACE_ABOVE_KEYBOARD
            - lowest_background;
    }

    fn vpos_relative_to_grid_reference(
        &self,
        stack: &Stack<T>,
        adaptor: &impl UiAdaptor<T>,
    ) -> f32 {
        let mut y = 0.0;
        for i in 0..T::num_intervals() {
            y += (stack.target[i] - self.grid_reference.target[i]) as f32
                * adaptor.config().lattice.interval_heights[i];
        }
        adaptor.config().lattice.zoom * y
    }

    fn vpos(&self, stack: &Stack<T>, adaptor: &impl UiAdaptor<T>) -> f32 {
        self.positions.grid_reference_pos.y + self.vpos_relative_to_grid_reference(stack, adaptor)
    }

    fn hpos(&self, stack: &Stack<T>, adaptor: &impl UiAdaptor<T>) -> f32 {
        self.positions.c4_hpos + adaptor.config().lattice.zoom * stack.semitones() as f32
    }

    fn pos(&self, stack: &Stack<T>, adaptor: &impl UiAdaptor<T>) -> egui::Pos2 {
        pos2(self.hpos(stack, adaptor), self.vpos(stack, adaptor))
    }

    fn has_projection(&self, stack: &Stack<T>, adaptor: &impl UiAdaptor<T>) -> bool {
        stack.target[adaptor.config().lattice.project_dimension]
            != self.grid_reference.target[adaptor.config().lattice.project_dimension]
    }

    fn projected_pos(&self, stack: &Stack<T>, adaptor: &impl UiAdaptor<T>) -> egui::Pos2 {
        self.pos(stack, adaptor)
            - (stack.target[adaptor.config().lattice.project_dimension]
                - self.grid_reference.target[adaptor.config().lattice.project_dimension])
                as f32
                * adaptor.config().lattice.zoom
                * vec2(
                    T::intervals()[adaptor.config().lattice.project_dimension].semitones as f32,
                    adaptor.config().lattice.interval_heights
                        [adaptor.config().lattice.project_dimension],
                )
    }

    fn grid_line_stroke(&self, ui: &egui::Ui, adaptor: &impl UiAdaptor<T>) -> egui::Stroke {
        egui::Stroke::new(
            adaptor.config().lattice.zoom * FAINT_GRID_LINE_THICKNESS,
            grid_line_color(ui),
        )
    }

    fn draw_grid_lines(&mut self, ui: &egui::Ui, adaptor: &impl UiAdaptor<T>) {
        let color = grid_line_color(ui);
        let stroke = self.grid_line_stroke(ui, adaptor);

        let draw_circle = |pos| {
            ui.painter().circle_filled(
                pos,
                adaptor.config().lattice.zoom * GRID_NODE_RADIUS,
                color,
            );
        };

        let draw_limb = |direction: usize, forward: bool, start_pos: egui::Pos2| {
            let end_pos = start_pos
                + adaptor.config().lattice.zoom
                    * if forward { 1.0 } else { -1.0 }
                    * vec2(
                        T::intervals()[direction].semitones as f32,
                        adaptor.config().lattice.interval_heights[direction],
                    );
            ui.painter().line_segment([start_pos, end_pos], stroke);
            end_pos
        };

        let mut background = PureStacksAround::new(
            &self.positions.background_low,
            &self.positions.background_high,
            &self.grid_reference,
        );
        while let Some(stack) = background.next() {
            for i in 0..T::num_intervals() {
                let d = stack.target[i] - self.grid_reference.target[i];
                if d == 0 {
                    continue;
                }
                let p = self.pos(&stack, adaptor);
                // draw_circle(p);
                let _ = draw_limb(i, d < 0, p);
            }
        }

        for i in 0..128 {
            if adaptor.key_state(i).is_sounding() {
                let StackWithTuning { stack, .. } = &*adaptor.tuning(i);
                let mut pos = self.projected_pos(&stack, adaptor);
                let d = stack.target[adaptor.config().lattice.project_dimension]
                    - self.grid_reference.target[adaptor.config().lattice.project_dimension];
                for _ in 0..d.abs() {
                    pos = draw_limb(adaptor.config().lattice.project_dimension, d > 0, pos);
                    draw_circle(pos);
                }
            }
        }
    }

    fn draw_down_lines(&self, ui: &egui::Ui, adaptor: &impl UiAdaptor<T>) {
        let bottom = self.keyboard_top(adaptor);

        for i in 0..128 {
            if adaptor.key_state(i).is_sounding() {
                let StackWithTuning { stack, .. } = &*adaptor.tuning(i);
                let ppos = self.projected_pos(&stack, adaptor);
                ui.painter().vline(
                    ppos.x,
                    egui::Rangef {
                        min: ppos.y,
                        max: bottom,
                    },
                    self.grid_line_stroke(ui, adaptor),
                );

                if self.has_projection(&stack, adaptor) {
                    let pos = self.pos(&stack, adaptor);
                    ui.painter().vline(
                        pos.x,
                        egui::Rangef {
                            min: pos.y,
                            max: bottom,
                        },
                        self.grid_line_stroke(ui, adaptor),
                    );
                }
            }
        }
    }

    fn draw_note_names_and_interaction_zones(
        &mut self,
        ui: &mut egui::Ui,
        adaptor: &impl UiAdaptor<T>,
    ) {
        let write_considered_stack_to_draw = |considered: &Stack<T>, output: &mut Stack<T>| {
            output.clone_from(&adaptor.reference());
            output.scaled_add(1, considered);
            output.increment_at_index_pure(
                adaptor.config().lattice.project_dimension,
                self.grid_reference.target[adaptor.config().lattice.project_dimension]
                    - adaptor.reference().target[adaptor.config().lattice.project_dimension]
                    - considered.target[adaptor.config().lattice.project_dimension],
            );
        };

        let write_sounding_stack_to_draw = |sounding: &Stack<T>, output: &mut Stack<T>| {
            output.clone_from(sounding);
            output.increment_at_index_pure(
                adaptor.config().lattice.project_dimension,
                self.grid_reference.target[adaptor.config().lattice.project_dimension]
                    - sounding.target[adaptor.config().lattice.project_dimension],
            );
        };

        let mut background = PureStacksAround::new(
            &self.positions.background_low,
            &self.positions.background_high,
            &self.grid_reference,
        );
        while let Some(stack) = background.next() {
            let draw_this = self.considered_notes.iter().all(|(_, considered)| {
                write_considered_stack_to_draw(considered, &mut self.tmp_stack);
                self.tmp_stack.target != stack.target
            }) && (0..128).all(|i| {
                if !adaptor.key_state(i).is_sounding() {
                    true
                } else {
                    let StackWithTuning {
                        stack: sounding, ..
                    } = &*adaptor.tuning(i);
                    write_sounding_stack_to_draw(&sounding, &mut self.tmp_stack);
                    self.tmp_stack.target != stack.target
                }
            });
            if draw_this {
                self.draw_state.draw_note_and_interaction_zone(
                    ui,
                    stack,
                    self.pos(stack, adaptor),
                    &*adaptor.reference(),
                    NoteDrawStyle::Background,
                    adaptor,
                );
            }
        }

        for (_, stack) in self.considered_notes.iter() {
            write_considered_stack_to_draw(stack, &mut self.tmp_stack);
            let draw_this = (0..128).all(|i| {
                if !adaptor.key_state(i).is_sounding() {
                    true
                } else {
                    let StackWithTuning {
                        stack: sounding, ..
                    } = &*adaptor.tuning(i);
                    write_sounding_stack_to_draw(&sounding, &mut self.other_tmp_stack);
                    self.tmp_stack.target != self.other_tmp_stack.target
                }
            });
            if draw_this {
                self.draw_state.draw_note_and_interaction_zone(
                    ui,
                    &self.tmp_stack,
                    self.pos(&self.tmp_stack, adaptor),
                    &*adaptor.reference(),
                    NoteDrawStyle::Considered,
                    adaptor,
                );
            }
        }

        for i in 0..128 {
            if adaptor.key_state(i).is_sounding() {
                let StackWithTuning { stack, .. } = &*adaptor.tuning(i);
                write_sounding_stack_to_draw(&stack, &mut self.tmp_stack);
                self.draw_state.draw_note_and_interaction_zone(
                    ui,
                    &self.tmp_stack,
                    self.pos(&self.tmp_stack, adaptor),
                    &*adaptor.reference(),
                    NoteDrawStyle::Playing,
                    adaptor,
                );
                if self.has_projection(&stack, adaptor) {
                    self.draw_state.draw_note_and_interaction_zone(
                        ui,
                        &stack,
                        self.pos(&stack, adaptor),
                        &*adaptor.reference(),
                        NoteDrawStyle::Antenna,
                        adaptor,
                    );
                }
            }
        }
    }

    fn draw_lattice(&mut self, ui: &mut egui::Ui, adaptor: &impl UiAdaptor<T>) {
        self.update_positions(adaptor);
        self.draw_down_lines(ui, adaptor);
        self.draw_grid_lines(ui, adaptor);
        self.draw_note_names_and_interaction_zones(ui, adaptor);
    }

    fn keyboard_height(&self, adaptor: &impl UiAdaptor<T>) -> f32 {
        adaptor.config().lattice.zoom * (WHITE_KEY_LENGTH + MARKER_LENGTH)
    }

    fn keyboard_top(&self, adaptor: &impl UiAdaptor<T>) -> f32 {
        self.positions.bottom - self.keyboard_height(adaptor)
    }
}

impl<T: StackType, A:UiAdaptor<T>> ReceiveToUiRef<T, A> for LatticeWindow<T> {
    fn receive_to_ui_ref(&mut self, msg: &ToUi<T>, _adaptor: &A) {
        match msg {
            ToUi::Consider { stack } => {
                let _ = self.considered_notes.insert(stack);
            }

            // ToUi::PedalHold { channel, value, .. } => {
            //     adaptor.config().lattice.screen_keyboard_pedal_hold =
            //         (*channel == adaptor.config().lattice.screen_keyboard_channel) & (*value != 0);
            // }
            _ => {}
        }
    }
}

impl<T: StackType + HasNoteNames> GuiShow<T> for LatticeWindow<T> {
    fn show(&mut self, ui: &mut egui::Ui, adaptor: &impl UiAdaptor<T>) {
        let r = ui.interact(
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
            self.positions.left = center - self.c4_offset(adaptor);
            self.positions.bottom = bottom;
        }
        self.keyboard_hover_interaction(ui, adaptor);
        self.draw_keyboard(ui, adaptor);
        self.draw_lattice(ui, adaptor);
    }
}
