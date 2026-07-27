use std::time::Instant;

use eframe::egui;
use midi_msg::Channel;

use crate::{
    gui::{common::toggle_bit, r#trait::UiAdaptor},
    interval::stacktype::r#trait::StackType,
    msg::FromUi,
    util::ordered_locks::{OrderedLocks, Zero},
};

use super::r#trait::GuiShow;

pub struct BackendWindow {}

impl BackendWindow {
    pub fn new() -> Self {
        Self {}
    }
}

impl<T: StackType> GuiShow<T> for BackendWindow {
    fn show<A: UiAdaptor<StackType = T>>(
        &mut self,
        ui: &mut egui::Ui,
        mut adaptor: OrderedLocks<A, Zero>,
    ) -> OrderedLocks<A, Zero> {
        (_, adaptor) = adaptor.backend_config_mut(|backend_config, adaptor| {
            ui.vertical(|ui| {
                let mut bend_range_changed = false;
                let mut use_channels_changed = false;

                ui.horizontal(|ui| {
                    ui.label("pitch bend range:");
                    if ui
                        .add(egui::DragValue::new(&mut backend_config.bend_range).range(0.2..=12.0))
                        .changed()
                    {
                        bend_range_changed = true;
                    }
                    ui.label("semitones");
                });

                let mut used_channels_map: u16 = 0;
                for c in backend_config.channels {
                    used_channels_map |= 1 << Channel::from(c) as u8;
                }
                ui.label("output channels (must be exactly 12):");
                ui.horizontal(|ui| {
                    for i in 0..4 {
                        ui.vertical(|ui| {
                            for j in 0..4 {
                                let ch = 4 * i + j;

                                if toggle_bit(
                                    ui,
                                    &mut used_channels_map,
                                    ch,
                                    &format!("{}", ch + 1),
                                )
                                .changed()
                                {
                                    use_channels_changed = true;
                                }
                            }
                        });
                    }
                });

                if ui
                    .add_enabled(
                        used_channels_map.count_ones() == 12,
                        egui::Button::new("update")
                            .selected(bend_range_changed | use_channels_changed),
                    )
                    .clicked()
                {
                    if bend_range_changed {
                        let _ = adaptor.send(FromUi::UpdateBendRange {
                            time: Instant::now(),
                        });
                    }
                    if use_channels_changed {
                        let _ = adaptor.send(FromUi::UpdateChannelsToUse {
                            time: Instant::now(),
                        });
                    }
                }
            });
        });
        adaptor
    }
}
