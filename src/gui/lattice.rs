use std::time::Instant;

use eframe::egui::{self, pos2, vec2, Popup, PopupCloseBehavior};
use midi_msg::Channel;
use serde_derive::{Deserialize, Serialize};

use crate::{
    adaptors::lock_levels::{KeyStateLevel, ReferenceLevel, TuningReferenceLevel},
    config::GuiConfig,
    custom_serde::common::{deserialize_channel, serialize_channel},
    gui::{
        common::temperament_applier,
        r#trait::{GuiShow, ReceiveToUiRef, UiAdaptor},
    },
    interval::{
        base::Semitones,
        stack::{ScaledAdd, Stack},
        stacktype::r#trait::{StackCoeff, StackType},
    },
    msg::{FromUi, ToStrategy, ToUi},
    neighbourhood::{Neighbourhood, Partial},
    notename::{correction::Correction, HasNoteNames},
    process::r#trait::StackWithTuning,
    util::ordered_locks::{AtMost, Nat, Zero},
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
    pub background_dimensions: (usize, usize),
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
    grid_reference_pos: egui::Pos2, // not necessarily the reference of the current scale neighbourhood, may also be middle c
    left: f32,

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
        gui_config: &GuiConfig,
    ) -> egui::Rect {
        let egui::Pos2 { x: hpos, y: vpos } = pos;
        let lattice_config = &gui_config.lattice;

        let first_line_height = match style {
            NoteDrawStyle::Background | NoteDrawStyle::Considered | NoteDrawStyle::Antenna => {
                lattice_config.zoom * FONT_SIZE
            }
            NoteDrawStyle::Playing => lattice_config.zoom * 1.5 * FONT_SIZE,
        };
        let spacing = lattice_config.zoom * 0.5 * FONT_SIZE;
        let other_lines_height = lattice_config.zoom * 0.6 * FONT_SIZE;
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
            stack.notename(&gui_config.notenamestyle),
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
            if gui_config.use_cent_values {
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
                    format!("={}", stack.actual_notename(&gui_config.notenamestyle)),
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

    /// returns a note to consider
    fn retemper_popup(
        &mut self,
        ui: &mut egui::Ui,
        rect: egui::Rect,
        stack: &Stack<T>,
        reference: &Stack<T>,
        gui_config: &GuiConfig,
    ) -> Option<(Stack<T>, Instant)> {
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
        let mut res = None {};
        Popup::menu(&response)
            .id(popup_id)
            .close_behavior(PopupCloseBehavior::CloseOnClickOutside)
            .show(|ui| {
                if temperament_applier(
                    Some(&format!(
                        "make pure relative to {}",
                        reference.corrected_notename(
                            &gui_config.notenamestyle,
                            gui_config.use_cent_values
                        )
                    )),
                    ui,
                    &mut self.tmp_correction,
                    &mut self.tmp_relative_stack,
                ) {
                    res = Some((self.tmp_relative_stack.clone(), Instant::now()));
                }
            });
        res
    }

    /// returns a note to condsider.
    fn draw_note_and_interaction_zone(
        &mut self,
        ui: &mut egui::Ui,
        stack: &Stack<T>,
        pos: egui::Pos2,
        reference: &Stack<T>,
        style: NoteDrawStyle,
        gui_config: &GuiConfig,
    ) -> Option<(Stack<T>, Instant)> {
        let lattice_config = &gui_config.lattice;
        let draw_activation_circle = |active: bool| {
            if active {
                ui.painter().circle_filled(
                    pos,
                    lattice_config.zoom * FONT_SIZE,
                    activation_color(ui, lattice_config, stack),
                );
            } else {
                ui.painter().circle_filled(
                    pos,
                    lattice_config.zoom * 0.6 * FONT_SIZE,
                    ui.style().visuals.window_fill,
                );
            }
        };

        draw_activation_circle(style == NoteDrawStyle::Playing);
        let rect = self.draw_corrected_note_name(ui, stack, pos, style, gui_config);

        match style {
            NoteDrawStyle::Playing => None {},
            NoteDrawStyle::Antenna => None {},
            NoteDrawStyle::Background => {
                if ui
                    .interact(rect, egui::Id::new(stack), egui::Sense::click())
                    .clicked()
                {
                    self.tmp_relative_stack.clone_from(&stack);
                    self.tmp_relative_stack.scaled_add(-1, reference);
                    Some((self.tmp_relative_stack.clone(), Instant::now()))
                } else {
                    None {}
                }
            }
            NoteDrawStyle::Considered => {
                self.retemper_popup(ui, rect, stack, reference, gui_config)
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
                // dummy initialisations
                left: 0.0,
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
    fn keyboard_hover_interaction(
        &self,
        ui: &mut egui::Ui,
        config: &LatticeWindowConfig,
        send: impl Fn(FromUi<T>),
    ) {
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
                                        send(FromUi::NoteOn {
                                            channel: config.screen_keyboard_channel,
                                            note: note as u8,
                                            velocity: config.screen_keyboard_velocity,
                                            time: Instant::now(),
                                        });
                                    } else {
                                        send(FromUi::NoteOff {
                                            channel: config.screen_keyboard_channel,
                                            note: note as u8,
                                            velocity: config.screen_keyboard_velocity,
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
        config: &LatticeWindowConfig,
        send: impl Fn(FromUi<T>),
    ) {
        let r = ui.interact(rect, ui.id().with(key_number), egui::Sense::drag());

        if r.drag_started() {
            send(FromUi::NoteOn {
                channel: config.screen_keyboard_channel,
                note: key_number,
                velocity: config.screen_keyboard_velocity,
                time: Instant::now(),
            });
        }

        if r.drag_stopped() {
            send(FromUi::NoteOff {
                channel: config.screen_keyboard_channel,
                note: key_number,
                velocity: config.screen_keyboard_velocity,
                time: Instant::now(),
            });
        }
    }

    fn key_border_color(
        &self,
        ui: &egui::Ui,
        key_number: u8,
        config: &LatticeWindowConfig,
    ) -> egui::Color32 {
        if !config.highlight_playable_keys {
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

    fn draw_white_keys<L>(
        &mut self,
        ui: &mut egui::Ui,
        bottom: f32,
        mut adaptor: UiAdaptor<T, L>,
    ) -> UiAdaptor<T, L>
    where
        L: AtMost<KeyStateLevel>,
    {
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
            let border_color = self.key_border_color(ui, key_number, &adaptor.config().lattice);
            let sounding;
            (sounding, adaptor) = adaptor.key_state(key_number as usize, |k, _| k.is_sounding());
            if sounding {
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
            self.key_click_interaction(rect, key_number, ui, &adaptor.config().lattice, |m| {
                adaptor.send(m)
            });
            rect = rect.translate(vec2(white_key_width, 0.0));
            key_number += steps[pitch_class];
            pitch_class = (pitch_class + 1) % 7;
        }

        adaptor
    }

    fn draw_black_keys<L>(
        &mut self,
        ui: &mut egui::Ui,
        bottom: f32,
        mut adaptor: UiAdaptor<T, L>,
    ) -> UiAdaptor<T, L>
    where
        L: AtMost<KeyStateLevel>,
    {
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
            let border_color = self.key_border_color(ui, key_number, &adaptor.config().lattice);
            let sounding;
            (sounding, adaptor) = adaptor.key_state(key_number as usize, |k, _| k.is_sounding());
            ui.painter().rect(
                rect,
                egui::CornerRadius::default(),
                if sounding { active_color } else { border_color },
                egui::Stroke::new(zoom * PIANO_KEY_BORDER_THICKNESS, border_color),
                egui::StrokeKind::Middle,
            );
            self.key_click_interaction(rect, key_number, ui, &adaptor.config().lattice, |m| {
                adaptor.send(m)
            });
            rect = rect.translate(vec2(spacing_steps[pitch_class], 0.0));
            key_number += key_number_steps[pitch_class];
            pitch_class = (pitch_class + 1) % 5;
        }

        adaptor
    }

    fn draw_ruler(&self, ui: &mut egui::Ui, bottom: f32, config: &LatticeWindowConfig) {
        let zoom = config.zoom;
        let mut x = self.positions.left + zoom / 2.0;
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

    fn draw_keyboard<L>(
        &mut self,
        ui: &mut egui::Ui,
        bottom: f32,
        mut adaptor: UiAdaptor<T, L>,
    ) -> UiAdaptor<T, L>
    where
        L: AtMost<KeyStateLevel>,
    {
        // this rectangle covers the grid that lied behind the keyboard
        ui.painter().rect_filled(
            egui::Rect {
                min: pos2(
                    self.positions.left,
                    bottom - self.keyboard_height(&adaptor.config().lattice),
                ),
                max: pos2(
                    self.positions.left + self.keyboard_width(&adaptor.config().lattice),
                    bottom,
                ),
            },
            egui::CornerRadius::default(),
            ui.style().visuals.window_fill, //.to_opaque(),
        );

        self.draw_ruler(ui, bottom, &adaptor.config().lattice);
        adaptor = self.draw_white_keys(ui, bottom, adaptor);
        self.draw_black_keys(ui, bottom, adaptor)
    }

    fn update_positions<L>(
        &mut self,
        max_rect: egui::Rect,
        mut adaptor: UiAdaptor<T, L>,
    ) -> UiAdaptor<T, L>
    where
        L: AtMost<KeyStateLevel> + AtMost<ReferenceLevel> + AtMost<TuningReferenceLevel>,
    {
        let zoom = adaptor.config().lattice.zoom;
        if adaptor.config().lattice.background_around_reference {
            (_, adaptor) = adaptor.reference(|r, _| self.grid_reference.clone_from(r));
        } else {
            self.grid_reference.reset_to_zero();
        }

        adaptor = adaptor.c4_offset(|offset| self.positions.c4_hpos = self.positions.left + offset);

        self.positions.grid_reference_pos.x =
            self.positions.c4_hpos + zoom * self.grid_reference.semitones() as f32;

        // Now comes the calculation of how many and which background nodes to show: There are four
        // corners of the area on whcih we want to paint the background grid. Additionally, there is
        // the grid_reference_pos , which should be the origin of the grid, i.e. the point (0,0).
        // For the four corners `lt`, `lb`, `rt`, `rb`, we can find linear their coordinates in the
        // basis (v1,v2), e.g. lt - grid_reference_pos = v1 * lt1 + v2 * lt2. The minimum and
        // maximum of these coordinates will be the background_low and background_high.

        let (d1, d2) = adaptor.config().lattice.background_dimensions;
        let v1 = vec2(
            T::intervals()[d1].semitones as f32 * zoom,
            adaptor.config().lattice.interval_heights[d1] * zoom,
        );
        let v2 = vec2(
            T::intervals()[d2].semitones as f32 * zoom,
            adaptor.config().lattice.interval_heights[d2] * zoom,
        );

        // If v1 and v2 are not linearly independent, we can't do anything and return.
        let det = v1.x * v2.y - v1.y * v2.x;
        if det == 0.0 {
            return adaptor;
        }

        // Calculates
        //              (a)
        // (v1,v2)^{-1} (b)
        let inv_v1v2 = |egui::Vec2 { x: a, y: b }: egui::Vec2| {
            ((v2.y * a - v2.x * b) / det, (v1.x * b - v1.y * a) / det)
        };

        let grp = self.positions.grid_reference_pos;
        let (lt1, lt2) = inv_v1v2(max_rect.left_top() - grp);
        let (lb1, lb2) = inv_v1v2(max_rect.left_bottom() - grp);
        let (rt1, rt2) = inv_v1v2(max_rect.right_top() - grp);
        let (rb1, rb2) = inv_v1v2(max_rect.right_bottom() - grp);

        self.positions.background_high[d1] = lt1.max(lb1).max(rt1).max(rb1) as StackCoeff;
        self.positions.background_low[d1] = lt1.min(lb1).min(rt1).min(rb1) as StackCoeff;

        self.positions.background_high[d2] = lt2.max(lb2).max(rt2).max(rb2) as StackCoeff;
        self.positions.background_low[d2] = lt2.min(lb2).min(rt2).min(rb2) as StackCoeff;

        adaptor
    }

    fn vpos_relative_to_grid_reference(
        &self,
        stack: &Stack<T>,
        config: &LatticeWindowConfig,
    ) -> f32 {
        let mut y = 0.0;
        for i in 0..T::num_intervals() {
            y += (stack.target[i] - self.grid_reference.target[i]) as f32
                * config.interval_heights[i];
        }
        config.zoom * y
    }

    fn vpos(&self, stack: &Stack<T>, config: &LatticeWindowConfig) -> f32 {
        self.positions.grid_reference_pos.y + self.vpos_relative_to_grid_reference(stack, config)
    }

    fn hpos(&self, stack: &Stack<T>, config: &LatticeWindowConfig) -> f32 {
        self.positions.c4_hpos + config.zoom * stack.semitones() as f32
    }

    fn pos(&self, stack: &Stack<T>, config: &LatticeWindowConfig) -> egui::Pos2 {
        pos2(self.hpos(stack, config), self.vpos(stack, config))
    }

    fn has_projection(&self, stack: &Stack<T>, config: &LatticeWindowConfig) -> bool {
        stack.target[config.project_dimension]
            != self.grid_reference.target[config.project_dimension]
    }

    fn projected_pos(&self, stack: &Stack<T>, config: &LatticeWindowConfig) -> egui::Pos2 {
        self.pos(stack, config)
            - (stack.target[config.project_dimension]
                - self.grid_reference.target[config.project_dimension]) as f32
                * config.zoom
                * vec2(
                    T::intervals()[config.project_dimension].semitones as f32,
                    config.interval_heights[config.project_dimension],
                )
    }

    fn grid_line_stroke(&self, ui: &egui::Ui, config: &LatticeWindowConfig) -> egui::Stroke {
        egui::Stroke::new(config.zoom * FAINT_GRID_LINE_THICKNESS, grid_line_color(ui))
    }

    fn draw_grid_lines<L>(&mut self, ui: &egui::Ui, adaptor: UiAdaptor<T, L>) -> UiAdaptor<T, L>
    where
        L: AtMost<KeyStateLevel>,
    {
        let color = grid_line_color(ui);
        let stroke = self.grid_line_stroke(ui, &adaptor.config().lattice);

        let draw_circle = |pos, config: &LatticeWindowConfig| {
            ui.painter()
                .circle_filled(pos, config.zoom * GRID_NODE_RADIUS, color);
        };

        let draw_limb = |direction: usize,
                         forward: bool,
                         start_pos: egui::Pos2,
                         config: &LatticeWindowConfig| {
            let end_pos = start_pos
                + config.zoom
                    * if forward { 1.0 } else { -1.0 }
                    * vec2(
                        T::intervals()[direction].semitones as f32,
                        config.interval_heights[direction],
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
                let p = self.pos(&stack, &adaptor.config().lattice);
                // draw_circle(p);
                let _ = draw_limb(i, d < 0, p, &adaptor.config().lattice);
            }
        }

        adaptor.for_all_sounding_tunings(|_, StackWithTuning { stack, .. }, adaptor| {
            let mut pos = self.projected_pos(&stack, &adaptor.config().lattice);
            let d = stack.target[adaptor.config().lattice.project_dimension]
                - self.grid_reference.target[adaptor.config().lattice.project_dimension];
            for _ in 0..d.abs() {
                pos = draw_limb(
                    adaptor.config().lattice.project_dimension,
                    d > 0,
                    pos,
                    &adaptor.config().lattice,
                );
                draw_circle(pos, &adaptor.config().lattice);
            }
        })
    }

    fn draw_down_lines<L>(&self, ui: &egui::Ui, adaptor: UiAdaptor<T, L>) -> UiAdaptor<T, L>
    where
        L: AtMost<KeyStateLevel>,
    {
        let bottom = ui.max_rect().bottom() - self.keyboard_height(&adaptor.config().lattice);

        adaptor.for_all_sounding_tunings(|_, StackWithTuning { stack, .. }, adaptor| {
            let ppos = self.projected_pos(&stack, &adaptor.config().lattice);
            ui.painter().vline(
                ppos.x,
                egui::Rangef {
                    min: ppos.y,
                    max: bottom,
                },
                self.grid_line_stroke(ui, &adaptor.config().lattice),
            );

            if self.has_projection(&stack, &adaptor.config().lattice) {
                let pos = self.pos(&stack, &adaptor.config().lattice);
                ui.painter().vline(
                    pos.x,
                    egui::Rangef {
                        min: pos.y,
                        max: bottom,
                    },
                    self.grid_line_stroke(ui, &adaptor.config().lattice),
                );
            }
        })
    }

    fn draw_note_names_and_interaction_zones<L>(
        &mut self,
        ui: &mut egui::Ui,
        mut adaptor: UiAdaptor<T, L>,
    ) -> UiAdaptor<T, L>
    where
        L: AtMost<KeyStateLevel> + AtMost<ReferenceLevel>,
    {
        let write_considered_stack_to_draw =
            |considered: &Stack<T>,
             output: &mut Stack<T>,
             reference: &Stack<T>,
             config: &LatticeWindowConfig,
             grid_reference: &Stack<T>| {
                output.clone_from(reference);
                output.scaled_add(1, considered);
                output.increment_at_index_pure(
                    config.project_dimension,
                    grid_reference.target[config.project_dimension]
                        - reference.target[config.project_dimension]
                        - considered.target[config.project_dimension],
                );
            };

        let write_sounding_stack_to_draw =
            |sounding: &Stack<T>,
             output: &mut Stack<T>,
             config: &LatticeWindowConfig,
             grid_reference: &Stack<T>| {
                output.clone_from(sounding);
                output.increment_at_index_pure(
                    config.project_dimension,
                    grid_reference.target[config.project_dimension]
                        - sounding.target[config.project_dimension],
                );
            };

        let mut background = PureStacksAround::new(
            &self.positions.background_low,
            &self.positions.background_high,
            &self.grid_reference,
        );
        while let Some(stack) = background.next() {
            let mut draw_this;
            (draw_this, adaptor) = adaptor.reference(|reference, adaptor| {
                self.considered_notes.iter().all(|(_, considered)| {
                    write_considered_stack_to_draw(
                        considered,
                        &mut self.tmp_stack,
                        reference,
                        &adaptor.config().lattice,
                        &self.grid_reference,
                    );
                    self.tmp_stack.target != stack.target
                })
            });

            if !draw_this {
                (draw_this, adaptor) =
                    adaptor.check_all_keys_and_tunings(|_, k, sounding, adaptor| {
                        if k.is_sounding() {
                            true
                        } else {
                            write_sounding_stack_to_draw(
                                &sounding.stack,
                                &mut self.tmp_stack,
                                &adaptor.config().lattice,
                                &self.grid_reference,
                            );
                            self.tmp_stack.target != stack.target
                        }
                    });
            }

            if draw_this {
                let pos = self.pos(stack, &adaptor.config().lattice);
                (_, adaptor) = adaptor.reference(|reference, adaptor| {
                    let x = self.draw_state.draw_note_and_interaction_zone(
                        ui,
                        stack,
                        pos,
                        reference,
                        NoteDrawStyle::Background,
                        &adaptor.config(),
                    );
                    if let Some((stack, time)) = x {
                        adaptor.send(FromUi::ToStrategy(ToStrategy::Consider { stack, time }));
                    }
                });
            }
        }

        for (_, stack) in self.considered_notes.iter() {
            (_, adaptor) = adaptor.reference(|reference, adaptor| {
                write_considered_stack_to_draw(
                    stack,
                    &mut self.tmp_stack,
                    reference,
                    &adaptor.config().lattice,
                    &self.grid_reference,
                )
            });

            let draw_this;
            (draw_this, adaptor) = adaptor.check_all_keys_and_tunings(|_, k, sounding, adaptor| {
                if !k.is_sounding() {
                    true
                } else {
                    write_sounding_stack_to_draw(
                        &sounding.stack,
                        &mut self.other_tmp_stack,
                        &adaptor.config().lattice,
                        &self.grid_reference,
                    );
                    self.tmp_stack.target != self.other_tmp_stack.target
                }
            });

            if draw_this {
                let pos = self.pos(&self.tmp_stack, &adaptor.config().lattice);
                (_, adaptor) = adaptor.reference(|reference, adaptor| {
                    let x = self.draw_state.draw_note_and_interaction_zone(
                        ui,
                        &self.tmp_stack,
                        pos,
                        reference,
                        NoteDrawStyle::Considered,
                        &adaptor.config(),
                    );
                    if let Some((stack, time)) = x {
                        adaptor.send(FromUi::ToStrategy(ToStrategy::Consider { stack, time }));
                    }
                });
            }
        }

        adaptor.for_all_sounding_tunings(|_, StackWithTuning { stack, .. }, adaptor| {
            write_sounding_stack_to_draw(
                &stack,
                &mut self.tmp_stack,
                &adaptor.config().lattice,
                &self.grid_reference,
            );
            adaptor.reference(|reference, adaptor| {
                let x = self.draw_state.draw_note_and_interaction_zone(
                    ui,
                    &self.tmp_stack,
                    self.pos(&self.tmp_stack, &adaptor.config().lattice),
                    reference,
                    NoteDrawStyle::Playing,
                    &adaptor.config(),
                );
                if let Some((stack, time)) = x {
                    adaptor.send(FromUi::ToStrategy(ToStrategy::Consider { stack, time }));
                }
                if self.has_projection(&stack, &adaptor.config().lattice) {
                    let x = self.draw_state.draw_note_and_interaction_zone(
                        ui,
                        &stack,
                        self.pos(&stack, &adaptor.config().lattice),
                        reference,
                        NoteDrawStyle::Antenna,
                        &adaptor.config(),
                    );
                    if let Some((stack, time)) = x {
                        adaptor.send(FromUi::ToStrategy(ToStrategy::Consider { stack, time }));
                    }
                }
            });
        })
    }

    fn draw_lattice<L>(
        &mut self,
        ui: &mut egui::Ui,
        mut adaptor: UiAdaptor<T, L>,
    ) -> UiAdaptor<T, L>
    where
        L: AtMost<KeyStateLevel> + AtMost<ReferenceLevel> + AtMost<TuningReferenceLevel>,
    {
        adaptor = self.draw_down_lines(ui, adaptor);
        adaptor = self.draw_grid_lines(ui, adaptor);
        adaptor = self.draw_note_names_and_interaction_zones(ui, adaptor);
        adaptor
    }

    fn keyboard_height(&self, config: &LatticeWindowConfig) -> f32 {
        config.zoom * (WHITE_KEY_LENGTH + MARKER_LENGTH)
    }

    fn keyboard_width(&self, config: &LatticeWindowConfig) -> f32 {
        // 128 keys plus a half on each end.
        config.zoom * 129.0
    }
}

impl<T: StackType, L: Nat> UiAdaptor<T, L> {
    fn c4_offset(self, mut f: impl FnMut(f32)) -> Self
    where
        L: AtMost<TuningReferenceLevel>,
    {
        self.tuning_reference(|r, adaptor| {
            f(adaptor.config().lattice.zoom
                * (0.5 // half a key width on the ruler above the piano
                   + r.c4_semitones() as f32))
        })
        .1
    }
}

impl<T: StackType> ReceiveToUiRef<T> for LatticeWindow<T> {
    fn receive_to_ui_ref<'a>(
        &mut self,
        msg: &ToUi<T>,
        adaptor: UiAdaptor<T, Zero>,
    ) -> UiAdaptor<T, Zero> {
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
        adaptor
    }
}

impl<T: StackType + HasNoteNames> GuiShow<T> for LatticeWindow<T> {
    fn show(&mut self, ui: &mut egui::Ui, mut adaptor: UiAdaptor<T, Zero>) -> UiAdaptor<T, Zero> {
        let r = ui.interact(
            ui.max_rect(),
            egui::Id::new("global_grid_interaction"),
            egui::Sense::click_and_drag(),
        );

        if r.dragged() {
            let egui::Vec2 { x, y } = r.drag_delta();
            self.positions.left += x;
            self.positions.grid_reference_pos.y += y;
            self.reset_position = false;
        }
        if r.double_clicked() {
            self.reset_position = true;
        }

        if self.reset_position {
            let egui::Pos2 {
                x: center_x,
                y: center_y,
            } = ui.max_rect().center();
            adaptor = adaptor.c4_offset(|offset| self.positions.left = center_x - offset);
            self.positions.grid_reference_pos.y = center_y;
        }
        self.keyboard_hover_interaction(ui, &adaptor.config().lattice, |m| adaptor.send(m));
        adaptor = self.update_positions(ui.max_rect(), adaptor);
        adaptor = self.draw_lattice(ui, adaptor);
        self.draw_keyboard(ui, ui.max_rect().bottom(), adaptor)
    }
}
