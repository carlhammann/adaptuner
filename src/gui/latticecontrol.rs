use std::{sync::mpsc, time::Instant};

use eframe::egui::{self};
use midi_msg::Channel;

use crate::{
    interval::stacktype::r#trait::{FiveLimitStackType, StackType},
    msg::FromUi,
};

use super::{common::correction_system_chooser, lattice::LatticeWindow, r#trait::GuiShow};

pub struct AsSmallControls<'a, T: StackType>(pub &'a mut LatticeWindow<T>);

impl<'a, T: StackType> GuiShow<T> for AsSmallControls<'a, T> {
    fn show(&mut self, ui: &mut egui::Ui, forward: &mpsc::Sender<FromUi<T>>) {
        let AsSmallControls(LatticeWindow { controls, .. }) = self;
        ui.horizontal(|ui| {
            ui.add(
                egui::widgets::Slider::new(&mut controls.zoom, 5.0..=100.0)
                    .smart_aim(false)
                    .show_value(false)
                    .logarithmic(true)
                    .text("zoom"),
            );

            ui.separator();

            ui.label("screen keyboard MIDI channel:");

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

            ui.label("velocity:");
            ui.add(egui::DragValue::new(&mut controls.screen_keyboard_velocity).range(0..=127));

            if ui
                .toggle_value(&mut controls.screen_keyboard_pedal_hold, "sustain")
                .changed()
            {
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
        });
    }
}

pub struct AsBigControls<'a, T: StackType>(pub &'a mut LatticeWindow<T>);

impl<'a, T: FiveLimitStackType> GuiShow<T> for AsBigControls<'a, T> {
    fn show(&mut self, ui: &mut egui::Ui, _forward: &mpsc::Sender<FromUi<T>>) {
        let AsBigControls(lw) = self;
        let reference_name = lw.reference_corrected_note_name();
        let controls = &mut lw.controls;

        ui.horizontal(|ui| {
            correction_system_chooser::<T>(ui, &mut controls.correction_system_index);

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
        });
    }
}
