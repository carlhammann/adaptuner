use std::{sync::mpsc, time::Instant};

use eframe::egui::{self, pos2, vec2};

use crate::{
    interval::{stack::Stack, stacktype::r#trait::StackType},
    keystate::KeyState,
    msg::{FromUi, HandleMsgRef, ToUi},
    neighbourhood::Neighbourhood,
};

use super::r#trait::GuiShow;

// these are all in units of [LatticeWindow::zoom]
const OCTAVE_WIDTH: f32 = 12.0;
const WHITE_KEY_WIDTH: f32 = OCTAVE_WIDTH / 7.0;
const BLACK_KEY_WIDTH: f32 = WHITE_KEY_WIDTH / 2.0;
const WHITE_KEY_LENGTH: f32 = OCTAVE_WIDTH / 2.5;
const BLACK_KEY_LENGTH: f32 = 3.0 * WHITE_KEY_LENGTH / 5.0;
const PIANO_KEY_BORDER_THICKNESS: f32 = 0.1;
const MARKER_HEIGHT: f32 = BLACK_KEY_WIDTH / 2.0;
const MARKER_THICKNESS: f32 = PIANO_KEY_BORDER_THICKNESS;

pub struct LatticeWindow<T: StackType, N: Neighbourhood<T>> {
    active_notes: [KeyState; 128],
    tunings: [Stack<T>; 128],

    reference: Stack<T>,
    considered_notes: N,

    zoom: f32,
}

impl<T: StackType, N: Neighbourhood<T>> LatticeWindow<T, N> {
    pub fn new(reference: Stack<T>, considered_notes: N, zoom: f32) -> Self {
        let now = Instant::now();
        Self {
            active_notes: core::array::from_fn(|_| KeyState::new(now)),
            tunings: core::array::from_fn(|_| Stack::new_zero()),
            reference,
            considered_notes,
            zoom,
        }
    }

    fn draw_keyboard(&mut self, ui: &mut egui::Ui) {
        let rect = ui.clip_rect();
        let middle_c_center = rect.left() + rect.width() / 2.0;
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
            ui.painter().with_clip_rect(rect).rect(
                egui::Rect::from_center_size(
                    pos2(horizontal_center, black_key_vertical_center),
                    self.zoom * vec2(BLACK_KEY_WIDTH, BLACK_KEY_LENGTH),
                ),
                egui::CornerRadius::default(),
                if on {
                    ui.style().visuals.selection.bg_fill
                } else {
                    border_color
                },
                egui::Stroke::new(self.zoom * PIANO_KEY_BORDER_THICKNESS, border_color),
                egui::StrokeKind::Middle,
            );
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
            // let response = ui.interact(key_rect, egui::Id::new(format!("white key {}", key_number)), egui::Sense::click());
            // if response.clicked() {
            //     keystate.note_on(Channel::Ch1, Instant::now());
            // }
        };

        let draw_octave = |octave: i16| {
            let c_left = middle_c_center
                + self.zoom * ((octave as f32 - 4.0) * OCTAVE_WIDTH - OCTAVE_WIDTH / 24.0);
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

        let mut x = middle_c_center - self.zoom * 5.0 * OCTAVE_WIDTH;
        for _ in 0..128 {
            ui.painter().with_clip_rect(rect).vline(
                x,
                egui::Rangef {
                    min: rect.bottom() - self.zoom * (WHITE_KEY_LENGTH + MARKER_HEIGHT),
                    max: rect.bottom() - self.zoom * WHITE_KEY_LENGTH,
                },
                egui::Stroke::new(
                    self.zoom * MARKER_THICKNESS,
                    ui.style().visuals.strong_text_color(),
                ),
            );
            x += self.zoom * OCTAVE_WIDTH / 12.0;
        }
    }
}

impl<T: StackType, N: Neighbourhood<T>> GuiShow<T> for LatticeWindow<T, N> {
    fn show(&mut self, ctx: &egui::Context, ui: &mut egui::Ui, forward: &mpsc::Sender<FromUi<T>>) {
        egui::TopBottomPanel::bottom("lattice window bottom panel").show_inside(ui, |ui| {
            ui.add(
                egui::widgets::Slider::new(&mut self.zoom, 5.0..=100.0)
                    .smart_aim(false)
                    .logarithmic(true)
                    .show_value(false)
                    .text("zoom"),
            );
        });
        egui::CentralPanel::default().show_inside(ui, |ui| {
            self.draw_keyboard(ui);
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
                self.active_notes[*note as usize].note_off(*channel, false, *time);
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
