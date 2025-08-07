use std::{sync::mpsc, time::Instant};

use eframe::egui::{self};
use midi_msg::Channel;

use crate::{interval::stacktype::r#trait::StackType, msg::FromUi, notename::HasNoteNames};

use super::{common::rational_drag_value, lattice::LatticeWindow, r#trait::GuiShow};

pub struct AsKeyboardControls<'a, T: StackType>(pub &'a mut LatticeWindow<T>);

impl<'a, T: StackType> GuiShow<T> for AsKeyboardControls<'a, T> {
    fn show(&mut self, ui: &mut egui::Ui, forward: &mpsc::Sender<FromUi<T>>) {
        let AsKeyboardControls(LatticeWindow { controls, .. }) = self;
        ui.horizontal(|ui| {
            if ui
                .add(egui::Button::new("sustain").selected(controls.screen_keyboard_pedal_hold))
                .clicked()
            {
                controls.screen_keyboard_pedal_hold = !controls.screen_keyboard_pedal_hold;
                let _ = forward.send(FromUi::PedalHold {
                    time: Instant::now(),
                    value: if controls.screen_keyboard_pedal_hold {
                        127
                    } else {
                        0
                    },
                    channel: controls.screen_keyboard_channel,
                });
            }

            ui.label("velocity:");
            ui.add(egui::DragValue::new(&mut controls.screen_keyboard_velocity).range(0..=127));

            ui.label("MIDI channel:");

            egui::ComboBox::from_id_salt("keyboard MIDI channel")
                .width(ui.style().spacing.interact_size.y)
                .selected_text(format!("{}", controls.screen_keyboard_channel))
                .show_ui(ui, |ui| {
                    for i in 0..16 {
                        let ch = Channel::from_u8(i);
                        ui.selectable_value(
                            &mut controls.screen_keyboard_channel,
                            ch,
                            format!("{ch}"),
                        );
                    }
                });
        });
    }
}

pub struct AsBigControls<'a, T: StackType>(pub &'a mut LatticeWindow<T>);

impl<'a, T: StackType + HasNoteNames> AsBigControls<'a, T> {
    pub fn show(&mut self, reference_name: &str, ui: &mut egui::Ui) {
        let AsBigControls(lw) = self;
        let controls = &mut lw.controls;

        controls.correction_system_chooser.borrow_mut().show(ui);
        ui.separator();

        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                ui.label("grid range around the reference ( currently ");
                ui.strong(reference_name);
                ui.label(" )");
            });

            for i in (0..T::num_intervals()).rev() {
                ui.add(
                    egui::Slider::new(&mut controls.background_stack_distances[i], 0..=6)
                        .smart_aim(false)
                        .text(&T::intervals()[i].name),
                );
            }
        });

        ui.separator();

        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                ui.label("repeat background colours after");
                if ui
                    .add(egui::DragValue::new(&mut controls.color_period_ct))
                    .changed()
                {
                    if controls.color_period_ct <= 0.0 {
                        controls.color_period_ct = 100.0;
                    }
                    controls.tmp_correction.reset_to_zero();
                }
                ui.label("ct");
            });

            if T::num_named_intervals() > 0 {
                ui.label("or");
                let mut correction_changed = false;
                for (i, x) in controls.tmp_correction.coeffs.indexed_iter_mut() {
                    ui.horizontal(|ui| {
                        let name = &T::named_intervals()[i].name;
                        if rational_drag_value(ui, ui.id().with(name), x) {
                            correction_changed = true;
                        }
                        ui.label(name);
                    });
                }
                if correction_changed && controls.tmp_correction.is_nonzero() {
                    controls.color_period_ct = controls.tmp_correction.semitones() * 100.0;
                }
            }
        });
    }
}
