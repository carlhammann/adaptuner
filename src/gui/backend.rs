use std::time::Instant;

use eframe::egui;
use midi_msg::Channel;

use crate::{
    gui::{common::toggle_bit, r#trait::UiAdaptor},
    interval::stacktype::r#trait::StackType,
    msg::FromUi,
    util::ordered_locks::Zero,
};

use super::r#trait::GuiShow;

pub struct BackendWindow {
    used_channels_map: u16,
    update_used_channels_map_from_config: bool,
    bend_range_changed: bool,
    use_channels_changed: bool,
}

impl BackendWindow {
    pub fn new() -> Self {
        Self {
            used_channels_map: 0,
            update_used_channels_map_from_config: true,
            bend_range_changed: false,
            use_channels_changed: false,
        }
    }
}

impl<T: StackType> GuiShow<T> for BackendWindow {
    fn show(&mut self, ui: &mut egui::Ui, mut adaptor: UiAdaptor<T, Zero>) -> UiAdaptor<T, Zero> {
        (_, adaptor) = adaptor.backend_config_mut(|backend_config, adaptor| {
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    ui.label("pitch bend range:");
                    if ui
                        .add(egui::DragValue::new(&mut backend_config.bend_range).range(0.2..=12.0))
                        .changed()
                    {
                        self.bend_range_changed = true;
                    }
                    ui.label("semitones");
                });

                if self.update_used_channels_map_from_config {
                    self.used_channels_map = 0;
                    for c in backend_config.channels {
                        self.used_channels_map |= 1 << Channel::from(c) as u8;
                    }
                    self.update_used_channels_map_from_config = false;
                }
                ui.label("output channels (must be exactly 12):");
                ui.horizontal(|ui| {
                    for i in 0..4 {
                        ui.vertical(|ui| {
                            for j in 0..4 {
                                let ch = 4 * i + j;

                                if toggle_bit(
                                    ui,
                                    &mut self.used_channels_map,
                                    ch,
                                    &format!("{}", ch + 1),
                                )
                                .changed()
                                {
                                    self.use_channels_changed = true;
                                }
                            }
                        });
                    }
                });

                if ui
                    .add_enabled(
                        self.used_channels_map.count_ones() == 12,
                        egui::Button::new("update")
                            .selected(self.bend_range_changed || self.use_channels_changed),
                    )
                    .clicked()
                {
                    if self.bend_range_changed {
                        let _ = adaptor.send(FromUi::UpdateBendRange {
                            time: Instant::now(),
                        });
                        self.bend_range_changed = false;
                    }
                    if self.use_channels_changed {
                        let mut ch = 0;
                        for pos in 0..12 {
                            while self.used_channels_map % 2 == 0 {
                                ch += 1;
                                self.used_channels_map >>= 1;
                            }
                            backend_config.channels[pos] = Channel::from_u8(ch);
                            ch += 1;
                            self.used_channels_map >>= 1;
                        }
                        self.update_used_channels_map_from_config = true;
                        let _ = adaptor.send(FromUi::UpdateChannelsToUse {
                            time: Instant::now(),
                        });
                        self.use_channels_changed = false;
                    }
                }
            });
        });
        adaptor
    }
}
