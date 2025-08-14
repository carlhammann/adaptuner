use std::time::SystemTime;

use eframe::egui::{self, vec2};
use egui_file_dialog::{DirectoryEntry, FileDialog, FileDialogConfig};
use serde::{Deserialize, Serialize};

use crate::{
    config::{BackendConfig, Config, GuiConfig, ProcessConfig},
    gui::diffshow::DiffShow,
    interval::stacktype::r#trait::{IntervalBasis, StackType},
};

enum Phase {
    ShowingDialog,
    ShowingError,
    Closed,
}

pub struct ConfigFileDialog<T: IntervalBasis> {
    phase: Phase,
    as_load: bool,
    file_dialog: FileDialog,
    considered: Option<(DirectoryEntry, Result<Config<T>, serde_yml::Error>)>,
    considered_time: SystemTime,
    diffshow: DiffShow,
    error: Option<String>,
}

impl<T: StackType + Serialize> ConfigFileDialog<T> {
    pub fn new() -> Self {
        Self {
            phase: Phase::Closed,
            as_load: false,
            file_dialog: FileDialog::with_config(
                FileDialogConfig {
                    anchor: Some((egui::Align2::CENTER_TOP, vec2(0.0, 0.0))),
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
            // current_config: UpdateCell::new(ConfigByParts::empty()),
            diffshow: DiffShow::new(),
            error: None {},
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

    pub fn open(&mut self) {
        // , gui_config: GuiConfig<T>, forward: &mpsc::Sender<FromUi<T>>) {
        // self.current_config.set(ConfigByParts::empty());
        // self.current_config
        //     .update(|x| x.add_gui_if_nonexistent(gui_config));
        self.phase = Phase::ShowingDialog;
        self.considered = None {};
        if self.as_load {
            self.file_dialog.pick_file();
        } else {
            self.file_dialog.save_file();
        }
        // let _ = forward.send(FromUi::GetCurrentProcessConfig);
        // let _ = forward.send(FromUi::GetCurrentBackendConfig);
    }
}

fn show_waiting(ui: &mut egui::Ui) {
    egui::Window::new("waiting for backend configurations")
        .collapsible(false)
        .show(ui.ctx(), |ui| ui.spinner());
}

impl<T: StackType + Serialize + for<'a> Deserialize<'a>> ConfigFileDialog<T> {
    fn show_config_file_dialog(
        &mut self,
        ui: &mut egui::Ui,
        gui_config: &GuiConfig<T>,
        process_config: &ProcessConfig<T>,
        backend_config: &BackendConfig,
    ) -> Option<Config<T>> {
        self.file_dialog.update_with_right_panel_ui(
            ui.ctx(),
            &mut |ui: &mut egui::Ui, file_dialog| {
                ui.set_min_width(200.0);
                if let Some(selected_entry) = file_dialog.selected_entry() {
                    if !selected_entry.is_file() {
                        self.considered = None {};
                        return;
                    }

                    let mut update = false;

                    if let Some((old_entry, _)) = &self.considered {
                        update |= !selected_entry.path_eq(old_entry);
                        if let Ok(metadata) = std::fs::metadata(old_entry.as_path()) {
                            if let Ok(modified) = metadata.modified() {
                                update |= self.considered_time.duration_since(modified).is_err();
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
                                    &serde_yml::to_string(&Config::join(
                                        process_config.clone(),
                                        backend_config.clone(),
                                        gui_config.clone(),
                                        T::temperament_definitions().clone(),
                                        T::named_intervals().clone(),
                                    ))
                                    .unwrap(),
                                    ui,
                                );
                            }
                            self.considered = Some((selected_entry.clone(), config_or_err_in_file));
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
                                    .map(|l| (format!("{}", l.line()), format!("{}", l.column())))
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
                } else {
                    self.considered = None {};
                }
            },
        );

        if self.as_load {
            if let Some(path) = self.file_dialog.take_picked() {
                if let Some((_, Ok(config))) = self.considered.take() {
                    self.phase = Phase::Closed;
                    return Some(config);
                } else {
                    self.phase = Phase::ShowingError;
                    self.error = Some(format!(
                        "The path '{path:?}' does not contain a valid configuration file"
                    ));
                }
            }
            None {}
        } else {
            if let Some(path) = self.file_dialog.take_picked() {
                let config = Config::join(
                    process_config.clone(),
                    backend_config.clone(),
                    gui_config.clone(),
                    T::temperament_definitions().clone(),
                    T::named_intervals().clone(),
                );

                let res: Result<(), String> = {
                    match std::fs::File::create(path) {
                        Ok(file) => serde_yml::to_writer(file, &config).map_err(|e| format!("{e}")),
                        Err(e) => Err(format!("{e}")),
                    }
                };
                if let Err(e) = res {
                    self.error = Some(e);
                    self.phase = Phase::ShowingError;
                } else {
                    self.phase = Phase::Closed;
                }
            }
            None {}
        }
    }

    fn show_error(&mut self, ui: &mut egui::Ui) {
        if self.error.is_some() {
            egui::Window::new("Error saving or loading configuration")
                .collapsible(false)
                .show(ui.ctx(), |ui| {
                    ui.label(format!("{}", self.error.as_ref().unwrap()));
                    ui.separator();
                    ui.vertical_centered(|ui| {
                        if ui.button("Ok").clicked() {
                            self.error = None {};
                        }
                    });
                });
        }
    }

    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        gui_config: &GuiConfig<T>,
        process_config: &Option<ProcessConfig<T>>,
        backend_config: &Option<BackendConfig>,
    ) -> Option<Config<T>> {
        match &self.phase {
            Phase::ShowingDialog => {
                if let (Some(process_config), Some(backend_config)) =
                    (process_config, backend_config)
                {
                    self.show_config_file_dialog(ui, gui_config, process_config, backend_config)
                } else {
                    show_waiting(ui);
                    None {}
                }
            }
            Phase::ShowingError => {
                self.show_error(ui);
                None {}
            }
            Phase::Closed => None {},
        }
    }
}
