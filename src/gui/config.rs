use std::{sync::mpsc, time::SystemTime};

use eframe::egui::{self};
use egui_file_dialog::{DirectoryEntry, FileDialog, FileDialogConfig};
use serde::{Deserialize, Serialize};

use crate::{
    config::{BackendConfig, Config, GuiConfig, ProcessConfig},
    gui::diffshow::DiffShow,
    interval::stacktype::r#trait::{IntervalBasis, StackType},
    msg::{FromUi, HandleMsg, ToUi},
    util::update_cell::UpdateCell,
};

enum ConfigByParts<T: IntervalBasis> {
    Parts {
        process: Option<ProcessConfig<T>>,
        backend: Option<BackendConfig>,
        gui: Option<GuiConfig<T>>,
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

    fn add_gui_if_nonexistent(self, gui: GuiConfig<T>) -> Self {
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

pub struct ConfigFileDialog<T: IntervalBasis> {
    as_load: bool,
    file_dialog: FileDialog,
    considered: Option<(DirectoryEntry, Result<Config<T>, serde_yml::Error>)>,
    considered_time: SystemTime,
    current_config: UpdateCell<ConfigByParts<T>>,
    diffshow: DiffShow,
    save_error: Option<std::io::Error>,
}

impl<T: StackType + Serialize> ConfigFileDialog<T> {
    pub fn new() -> Self {
        Self {
            as_load: false,
            file_dialog: FileDialog::with_config(
                FileDialogConfig {
                    show_left_panel: false,
                    show_back_button: false,
                    show_forward_button: false,
                    show_path_edit_button: false,
                    show_working_directory_button: false,
                    default_save_extension: Some("YAML file".into()),
                    default_file_filter: Some("YAML files".into()),
                    ..FileDialogConfig::default()
                }
                .add_save_extension("YAML file", "yaml")
                .add_save_extension("YML file", "yml")
                .add_file_filter_extensions("YAML files", vec!["yaml", "yml"]),
            ),
            considered: None {},
            considered_time: SystemTime::now(),
            current_config: UpdateCell::new(ConfigByParts::empty()),
            diffshow: DiffShow::new(),
            save_error: None {},
        }
    }

    pub fn as_load(&mut self) -> &mut Self {
        self.as_load = true;
        self
    }

    pub fn as_save(&mut self) -> &mut Self {
        self.as_load = false;
        self
    }

    pub fn open(&mut self, gui_config: GuiConfig<T>, forward: &mpsc::Sender<FromUi<T>>) {
        self.current_config.set(ConfigByParts::empty());
        self.current_config
            .update(|x| x.add_gui_if_nonexistent(gui_config));
        self.considered = None {};
        if self.as_load {
            self.file_dialog.pick_file();
        } else {
            self.file_dialog.save_file();
        }
        let _ = forward.send(FromUi::GetCurrentProcessConfig);
        let _ = forward.send(FromUi::GetCurrentBackendConfig);
    }
}

impl<T: StackType + Serialize> HandleMsg<ToUi<T>, FromUi<T>> for ConfigFileDialog<T> {
    fn handle_msg(&mut self, msg: ToUi<T>, _forward: &mpsc::Sender<FromUi<T>>) {
        match msg {
            ToUi::CurrentProcessConfig(process_config) => {
                self.current_config
                    .update(|x| x.add_process_if_nonexistent(process_config));
            }
            ToUi::CurrentBackendConfig(backend_config) => {
                self.current_config
                    .update(|x| x.add_backend_if_nonexistent(backend_config));
            }
            _ => {}
        }
    }
}

impl<T: StackType + Serialize + for<'a> Deserialize<'a>> ConfigFileDialog<T> {
    pub fn show(&mut self, ui: &mut egui::Ui) -> Option<Config<T>> {
        self.file_dialog.update_with_right_panel_ui(
            ui.ctx(),
            &mut |ui: &mut egui::Ui, file_dialog| {
                ui.set_min_width(200.0);
                if let Some(selected_entry) = file_dialog.selected_entry() {
                    if !selected_entry.is_file() {
                        self.considered = None {};
                        return;
                    }
                    if let ConfigByParts::Whole(current_config) = &*self.current_config.borrow() {
                        let mut update = false;
                        if let Some((old_entry, _)) = &self.considered {
                            update |= !selected_entry.path_eq(old_entry);
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
                            self.considered = None {};
                            self.considered_time = SystemTime::now();
                            if let Ok(file) = std::fs::File::open(selected_entry.as_path()) {
                                let config_or_err_in_file =
                                    serde_yml::from_reader::<_, Config<T>>(file);
                                if let Ok(config_in_file) = &config_or_err_in_file {
                                    self.diffshow.update(
                                        &serde_yml::to_string(config_in_file).unwrap(),
                                        current_config,
                                        ui,
                                    );
                                }
                                self.considered =
                                    Some((selected_entry.clone(), config_or_err_in_file));
                            }
                        }

                        if let Some((direntry, file_config)) = &self.considered {
                            let file_name = direntry
                                .as_path()
                                .file_name()
                                .and_then(|x| x.to_str())
                                .unwrap_or("selected file");
                            if let Err(e) = file_config {
                                ui.label(format!(
                                    "The file '{file_name}' is not a valid configuration file:"
                                ));
                                ui.separator();
                                egui::ScrollArea::both().show(ui, |ui| {
                                    let (line, column) = e
                                        .location()
                                        .map(|l| {
                                            (format!("{}", l.line()), format!("{}", l.column()))
                                        })
                                        .unwrap_or_else(|| ("unknown".into(), "unknown".into()));
                                    ui.label(format!("line {line}\ncolumn {column}\n\n{e}",));
                                });
                            } else {
                                self.diffshow.show(
                                    &format!("in '{file_name}'"),
                                    "in current configuration",
                                    &format!(
                                        "The file '{file_name}' contains a configuration that is \
                                        equivalent to the current one.",
                                    ),
                                    ui,
                                );
                            }
                        }
                    }
                } else {
                    self.considered = None {};
                }
            },
        );

        if self.as_load {
            if let Some(_) = self.file_dialog.take_picked() {
                if let Some((_, Ok(config))) = self.considered.take() {
                    return Some(config);
                }
            }
        } else {
            if let Some(path) = self.file_dialog.take_picked() {
                let ConfigByParts::Whole(config) = self.current_config.take() else {
                    panic!("somehow the config disappeared before saving?")
                };
                let res = std::fs::write(&path, config);
                if let Err(e) = res {
                    self.save_error = Some(e);
                }
            }
        }

        if self.save_error.is_some() {
            egui::Window::new("configuration save error")
                .collapsible(false)
                .show(ui.ctx(), |ui| {
                    ui.label(format!("{}", self.save_error.as_ref().unwrap()));
                    ui.separator();
                    ui.vertical_centered(|ui| {
                        if ui.button("Ok").clicked() {
                            self.save_error = None {};
                        }
                    });
                });
        }
        None {}
    }
}
