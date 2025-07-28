use std::{sync::mpsc, time::SystemTime};

use eframe::egui::{self};
use egui_file_dialog::{DirectoryEntry, FileDialog, FileDialogConfig};
use serde::{Deserialize, Serialize};

use crate::{
    config::{BackendConfig, Config, GuiConfig, ProcessConfig},
    gui::{diffshow::DiffShow, r#trait::GuiShow},
    interval::stacktype::r#trait::{IntervalBasis, StackType},
    msg::{FromUi, HandleMsg, ToUi},
    util::update_cell::UpdateCell,
};

enum ConfigByParts<T: IntervalBasis> {
    Parts {
        process: Option<ProcessConfig<T>>,
        backend: Option<BackendConfig>,
        gui: Option<GuiConfig>,
    },
    Whole(String),
}

impl<T: StackType + Serialize> ConfigByParts<T> {
    fn empty() -> Self {
        Self::Parts {
            process: None {},
            backend: None {},
            gui: None {},
        }
    }

    fn make_complete(self) -> Self {
        match self {
            ConfigByParts::Parts {
                process: Some(process),
                backend: Some(backend),
                gui: Some(gui),
            } => ConfigByParts::Whole(
                serde_yml::to_string(&Config::join(
                    process,
                    backend,
                    gui,
                    T::temperament_definitions().clone(),
                    T::named_intervals().clone(),
                ))
                .unwrap(),
            ),
            _ => self,
        }
    }

    fn add_process_if_nonexistent(self, process: ProcessConfig<T>) -> Self {
        match self {
            ConfigByParts::Parts {
                process: None {},
                backend,
                gui,
            } => ConfigByParts::Parts {
                process: Some(process),
                backend,
                gui,
            }
            .make_complete(),
            _ => self,
        }
    }

    fn add_backend_if_nonexistent(self, backend: BackendConfig) -> Self {
        match self {
            ConfigByParts::Parts {
                process,
                backend: None {},
                gui,
            } => ConfigByParts::Parts {
                process,
                backend: Some(backend),
                gui,
            }
            .make_complete(),
            _ => self,
        }
    }

    fn add_gui_if_nonexistent(self, gui: GuiConfig) -> Self {
        match self {
            ConfigByParts::Parts {
                process,
                backend,
                gui: None {},
            } => ConfigByParts::Parts {
                process,
                backend,
                gui: Some(gui),
            }
            .make_complete(),
            _ => self,
        }
    }
}

pub struct ConfigSaver<T: IntervalBasis> {
    file_dialog: FileDialog,
    considered_entry: Option<DirectoryEntry>,
    considered_time: SystemTime,
    my_config: UpdateCell<ConfigByParts<T>>,
    diffshow: DiffShow,
}

impl<T: StackType + Serialize> ConfigSaver<T> {
    pub fn new() -> Self {
        Self {
            file_dialog: FileDialog::with_config(
                FileDialogConfig {
                    title: Some("save configuration".into()),
                    show_left_panel: false,
                    show_back_button: false,
                    show_forward_button: false,
                    show_path_edit_button: false,
                    show_working_directory_button: false,
                    default_save_extension: Some("YAML file".into()),
                    ..FileDialogConfig::default()
                }
                .add_save_extension("YAML file", "yaml")
                .add_save_extension("YML file", "yml"),
            ),
            considered_entry: None {},
            considered_time: SystemTime::now(),
            my_config: UpdateCell::new(ConfigByParts::empty()),
            diffshow: DiffShow::new(),
        }
    }

    pub fn open(&mut self, gui_config: GuiConfig, forward: &mpsc::Sender<FromUi<T>>) {
        self.my_config.set(ConfigByParts::empty());
        self.my_config
            .update(|x| x.add_gui_if_nonexistent(gui_config));
        self.considered_entry = None {};
        self.file_dialog.save_file();
        let _ = forward.send(FromUi::GetCurrentProcessConfig);
        let _ = forward.send(FromUi::GetCurrentBackendConfig);
    }
}

impl<T: StackType + Serialize> HandleMsg<ToUi<T>, FromUi<T>> for ConfigSaver<T> {
    fn handle_msg(&mut self, msg: ToUi<T>, _forward: &mpsc::Sender<FromUi<T>>) {
        match msg {
            ToUi::CurrentProcessConfig(process_config) => {
                self.my_config
                    .update(|x| x.add_process_if_nonexistent(process_config));
            }
            ToUi::CurrentBackendConfig(backend_config) => {
                self.my_config
                    .update(|x| x.add_backend_if_nonexistent(backend_config));
            }
            _ => {}
        }
    }
}

impl<T: StackType + Serialize + for<'a> Deserialize<'a>> GuiShow<T> for ConfigSaver<T> {
    fn show(&mut self, ui: &mut egui::Ui, _forward: &mpsc::Sender<FromUi<T>>) {
        self.file_dialog.update_with_right_panel_ui(
            ui.ctx(),
            &mut |ui: &mut egui::Ui, file_dialog| {
                ui.set_min_width(200.0);
                if let Some(entry) = file_dialog.selected_entry() {
                    if let ConfigByParts::Whole(my_config) = &*self.my_config.borrow() {
                        if !entry.is_file() {
                            self.considered_entry = None {};
                            return;
                        }

                        let mut update = false;
                        if let Some(old_entry) = &self.considered_entry {
                            update |= !entry.path_eq(old_entry);
                            if let Ok(metadata) = std::fs::metadata(old_entry.as_path()) {
                                if let Ok(modified) = metadata.modified() {
                                    update |=
                                        self.considered_time.duration_since(modified).is_err();
                                }
                            }
                        } else {
                            update = true;
                        }
                        if update {
                            self.considered_entry = None {};
                            self.considered_time = SystemTime::now();
                            let Ok(file) = std::fs::File::open(entry.as_path()) else {
                                return;
                            };
                            match serde_yml::from_reader::<_, Config<T>>(file) {
                                Err(_) => {}
                                Ok(config_in_file) => {
                                    println!("!!");
                                    self.diffshow.update(
                                        &serde_yml::to_string(&config_in_file).unwrap(),
                                        my_config,
                                        ui,
                                    );
                                    self.considered_entry = Some(entry.clone());
                                }
                            }
                        }

                        if self.considered_entry.is_some() {
                            self.diffshow.show(
                                "This file contains a configuration that is equivalent \
                            to the current one.",
                                ui,
                            );
                        } else {
                            ui.label(
                                "This file contains no valid configuration, so I can't compare \
                            its contents to the current configuration.",
                            );
                        }
                    }
                } else {
                    self.considered_entry = None {};
                }
            },
        );

        if let Some(path) = self.file_dialog.take_picked() {
            let ConfigByParts::Whole(config) = self.my_config.take() else {
                panic!("somehow the config disappeared before saving?")
            };
            let _ = std::fs::write(path, config);
        }
    }
}
