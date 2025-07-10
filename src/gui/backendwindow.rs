use std::{sync::mpsc, time::Instant};

use eframe::egui;

use crate::{
    backend::pitchbend12::Pitchbend12Config,
    interval::{base::Semitones, stacktype::r#trait::StackType},
    msg::{FromUi, HandleMsgRef, ToUi},
};

use super::r#trait::GuiShow;

pub struct BackendWindow {
    bend_range: Semitones,
    new_bend_range: Semitones,
    use_channels: [bool; 16],
    new_use_channels: [bool; 16],
}

pub type BackendWindowConfig = Pitchbend12Config;

impl BackendWindow {
    pub fn new(config: BackendWindowConfig) -> Self {
        let mut use_channels = [false; 16];
        for c in config.channels {
            use_channels[c as usize] = true;
        }
        Self {
            bend_range: config.bend_range,
            new_bend_range: config.bend_range,
            new_use_channels: use_channels.clone(),
            use_channels,
        }
    }
}

impl<T: StackType> HandleMsgRef<ToUi<T>, FromUi<T>> for BackendWindow {
    fn handle_msg_ref(&mut self, _msg: &ToUi<T>, _forward: &mpsc::Sender<FromUi<T>>) {}
}

impl<T: StackType> GuiShow<T> for BackendWindow {
    fn show(&mut self, _ctx: &egui::Context, ui: &mut egui::Ui, forward: &mpsc::Sender<FromUi<T>>) {
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
