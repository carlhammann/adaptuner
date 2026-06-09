use std::{ops::Deref, time::Instant};

use eframe::egui;
use midi_msg::Channel;

use crate::{
    backend::pitchbend12::Pitchbend12Config,
    gui::{common::toggle_bit, r#trait::UiAdaptor},
    interval::{base::Semitones, stacktype::r#trait::StackType},
    msg::FromUi,
};

use super::r#trait::GuiShow;

pub struct BackendWindow {
    new_bend_range: Semitones,
    new_used_channels_map: u16,
}

impl BackendWindow {
    pub fn new(config: impl Deref<Target = Pitchbend12Config>) -> Self {
        let mut used_channels_map = 0;
        for c in config.channels {
            used_channels_map |= 1 << Channel::from(c) as u8;
        }
        Self {
            new_bend_range: config.bend_range,
            new_used_channels_map: used_channels_map,
        }
    }
}

impl<T: StackType> GuiShow<T> for BackendWindow {
    fn show(&mut self, ui: &mut egui::Ui, adaptor: &impl UiAdaptor<T>) {
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                ui.label("pitch bend range:");
                ui.add(egui::DragValue::new(&mut self.new_bend_range).range(0.2..=12.0));
                ui.label("semitones");
            });

            ui.label("output channels (must be exactly 12):");
            ui.horizontal(|ui| {
                for i in 0..4 {
                    ui.vertical(|ui| {
                        for j in 0..4 {
                            let ch = 4 * i + j;
                            toggle_bit(
                                ui,
                                &mut self.new_used_channels_map,
                                ch,
                                &format!("{}", ch + 1),
                            );
                        }
                    });
                }
            });

            let bend_range_changed = self.new_bend_range != adaptor.backend_config().bend_range;
            let use_channels_changed = !adaptor
                .backend_config()
                .uses_channels(self.new_used_channels_map);

            let mut n_enabled = 0;
            for i in 0..16 {
                if 0 != self.new_used_channels_map & 1 << i {
                    n_enabled += 1;
                }
            }

            if ui
                .add_enabled(
                    n_enabled == 12,
                    egui::Button::new("update").selected(bend_range_changed | use_channels_changed),
                )
                .clicked()
            {
                if bend_range_changed {
                    let _ = adaptor.send(FromUi::BendRange {
                        range: self.new_bend_range,
                        time: Instant::now(),
                    });
                }
                if use_channels_changed {
                    let _ = adaptor.send(FromUi::ChannelsToUse {
                        channels: self.new_used_channels_map,
                        time: Instant::now(),
                    });
                }
            }
        });
    }
}
