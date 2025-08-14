use std::{sync::mpsc, time::Instant};

use eframe::egui;
use midi_msg::Channel;

use crate::{
    backend::pitchbend12::Pitchbend12Config,
    config::{BackendConfig, ExtractConfig},
    interval::{base::Semitones, stacktype::r#trait::StackType},
    msg::FromUi,
};

use super::r#trait::GuiShow;

pub struct BackendWindow {
    bend_range: Semitones,
    new_bend_range: Semitones,
    use_channels: [bool; 16],
    new_use_channels: [bool; 16],
}

pub type BackendWindowConfig = BackendConfig;

impl BackendWindow {
    pub fn new(config: BackendWindowConfig) -> Self {
        match config {
            BackendConfig::Pitchbend12(config) => {
                let mut use_channels = [false; 16];
                for c in config.channels {
                    use_channels[Into::<Channel>::into(c) as usize] = true;
                }
                Self {
                    bend_range: config.bend_range,
                    new_bend_range: config.bend_range,
                    new_use_channels: use_channels.clone(),
                    use_channels,
                }
            }
        }
    }

    pub fn restart_from_config(&mut self, config: BackendWindowConfig, _time: Instant) {
        match config {
            BackendConfig::Pitchbend12(config) => {
                self.bend_range = config.bend_range;
                self.use_channels.iter_mut().for_each(|b| *b = false);
                for c in config.channels {
                    self.use_channels[Into::<Channel>::into(c) as usize] = true;
                }
                self.new_use_channels.clone_from(&self.use_channels);
            }
        }
    }
}

impl<T: StackType> GuiShow<T> for BackendWindow {
    fn show(&mut self, ui: &mut egui::Ui, forward: &mpsc::Sender<FromUi<T>>) {
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
                            ui.toggle_value(&mut self.new_use_channels[ch], format!("{}", ch + 1));
                        }
                    });
                }
            });

            let mut n_enabled = 0;
            for b in self.new_use_channels {
                if b {
                    n_enabled += 1;
                }
            }

            let bend_range_changed = self.new_bend_range != self.bend_range;
            let use_channels_changed = self.new_use_channels != self.use_channels;

            if ui
                .add_enabled(
                    n_enabled == 12,
                    egui::Button::new("update").selected(bend_range_changed | use_channels_changed),
                )
                .clicked()
            {
                if bend_range_changed {
                    self.bend_range = self.new_bend_range;
                    let _ = forward.send(FromUi::BendRange {
                        range: self.bend_range,
                        time: Instant::now(),
                    });
                }
                if use_channels_changed {
                    self.use_channels.clone_from(&self.new_use_channels);
                    let _ = forward.send(FromUi::ChannelsToUse {
                        channels: self.use_channels.clone(),
                        time: Instant::now(),
                    });
                }
            }
        });
    }
}

impl ExtractConfig<BackendWindowConfig> for BackendWindow {
    fn extract_config(&self) -> BackendWindowConfig {
        BackendWindowConfig::Pitchbend12(Pitchbend12Config {
            bend_range: self.bend_range,
            channels: {
                let mut channels = [Channel::Ch1.into(); 12];
                let mut i = 0;
                for j in 0..16 {
                    if self.use_channels[j] {
                        channels[i] = Channel::from_u8(j as u8).into();
                        i += 1;
                    }
                }
                channels
            },
        })
    }
}
