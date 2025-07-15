use std::{
    cell::{RefCell, RefMut},
    rc::Rc,
    sync::mpsc,
    time::Instant,
};

use eframe::egui::{self};
use midi_msg::Channel;

use crate::{
    interval::stacktype::r#trait::StackType,
    msg::{FromUi, HandleMsgRef, ToUi},
};

use super::{common::correction_system_chooser, lattice::LatticeWindowControls, r#trait::GuiShow};

pub fn lattice_control<T: StackType>(
    ui: &mut egui::Ui,
    values: &Rc<RefCell<LatticeWindowControls>>,
) {
    let _ = RefMut::map(values.borrow_mut(), |x| {
        ui.horizontal(|ui| {
            correction_system_chooser::<T>(ui, &mut x.correction_system_index);

            ui.separator();

            ui.vertical(|ui| {
                ui.label("show notes around the reference");
                for i in (0..T::num_intervals()).rev() {
                    ui.add(
                        egui::Slider::new(&mut x.background_stack_distances[i], 0..=6)
                            .smart_aim(false)
                            .text(&T::intervals()[i].name),
                    );
                }
            });
        });

        x // whatever
    });
}

pub struct LatticeWindowSmallControls(pub Rc<RefCell<LatticeWindowControls>>);

impl<T: StackType> HandleMsgRef<ToUi<T>, FromUi<T>> for LatticeWindowSmallControls {
    fn handle_msg_ref(&mut self, msg: &ToUi<T>, _forward: &mpsc::Sender<FromUi<T>>) {
        match msg {
            ToUi::PedalHold { channel, value, .. } => {
                let LatticeWindowSmallControls(control_values) = self;
                let my_channel = control_values.borrow().keyboard_channel;
                if *channel == my_channel {
                    let _ = RefMut::map(control_values.borrow_mut(), |x| {
                        x.pedal_hold = *value > 0;

                        x // whatever
                    });
                }
            }
            _ => {}
        }
    }
}

impl<T: StackType> GuiShow<T> for LatticeWindowSmallControls {
    fn show(&mut self, ui: &mut egui::Ui, forward: &mpsc::Sender<FromUi<T>>) {
        let LatticeWindowSmallControls(control_values) = self;
        let _ = RefMut::map(control_values.borrow_mut(), |x| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add(
                    egui::widgets::Slider::new(&mut x.zoom, 5.0..=100.0)
                        .smart_aim(false)
                        .show_value(false)
                        .logarithmic(true)
                        .text("zoom"),
                );
                ui.separator();

                if ui.toggle_value(&mut x.pedal_hold, "sustain").changed() {
                    let _ = forward.send(FromUi::PedalHold {
                        time: Instant::now(),
                        value: if x.pedal_hold { 127 } else { 0 },
                        channel: x.keyboard_channel,
                    });
                }

                ui.add(
                    // egui::widgets::Slider::new(&mut x.keyboard_velocity, 0..=127).text("velocity"),
                    egui::DragValue::new(&mut x.keyboard_velocity).range(0..=127),
                );
                ui.label("velocity");

                egui::ComboBox::from_id_salt("keyboard MIDI channel")
                    .width(ui.style().spacing.interact_size.y)
                    .selected_text(format!("{}", x.keyboard_channel))
                    .show_ui(ui, |ui| {
                        for i in 0..16 {
                            let ch = Channel::from_u8(i);
                            ui.selectable_value(&mut x.keyboard_channel, ch, format!("{ch}"));
                        }
                    });

                ui.label("screen keyboard MIDI:");
            });

            x // whatever
        });
    }
}
