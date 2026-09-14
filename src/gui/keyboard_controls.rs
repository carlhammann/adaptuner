use std::time::Instant;

use eframe::egui;
use midi_msg::Channel;

use crate::{
    gui::r#trait::{GuiShow, UiAdaptor},
    interval::stacktype::r#trait::StackType,
    msg::FromUi,
    util::ordered_locks::Zero,
};

pub struct KeyboardControls {}

impl KeyboardControls {
    pub fn new() -> Self {
        Self {}
    }
}

impl<T: StackType> GuiShow<T> for KeyboardControls {
    fn show(&mut self, ui: &mut egui::Ui, mut adaptor: UiAdaptor<T, Zero>) -> UiAdaptor<T, Zero> {
        let screen_keyboard_channel = adaptor.config().lattice.screen_keyboard_channel;
        let pedal_was_held;
        (pedal_was_held, adaptor) = adaptor.pedal_hold(|bs, _| bs[screen_keyboard_channel as usize]);

        ui.horizontal(|ui| {
            if ui
                .add(egui::Button::new("sustain").selected(pedal_was_held))
                .clicked()
            {
                adaptor.send(FromUi::PedalHold {
                    time: Instant::now(),
                    value: if pedal_was_held { 0 } else { 127 },
                    channel: screen_keyboard_channel,
                });
            }

            ui.label("velocity:");
            ui.add(
                egui::DragValue::new(&mut adaptor.config_mut().lattice.screen_keyboard_velocity)
                    .range(0..=127),
            );

            ui.label("MIDI channel:");

            egui::ComboBox::from_id_salt("keyboard MIDI channel")
                .width(ui.style().spacing.interact_size.y)
                .selected_text(format!("{}", screen_keyboard_channel))
                .show_ui(ui, |ui| {
                    for i in 0..16 {
                        let ch = Channel::from_u8(i);
                        ui.selectable_value(
                            &mut adaptor.config_mut().lattice.screen_keyboard_channel,
                            ch,
                            format!("{ch}"),
                        );
                    }
                });
        });

        adaptor
    }
}
