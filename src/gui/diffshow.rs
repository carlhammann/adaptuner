use std::sync::Arc;

use eframe::egui::{self, pos2, vec2};
use similar::{ChangeTag, TextDiff};

const DIFF_FONT_ID: egui::FontId = egui::FontId {
    family: egui::FontFamily::Monospace,
    size: 14.0,
};

pub struct DiffShow {
    galley: Option<Arc<egui::Galley>>,
    first_lines_of_changed_blocks: Vec<usize>,
    change_block_index: usize,
    num_lines: usize,
    show_line: usize,
    scroll_to_line: bool,
    position_pointer: String,
}

impl DiffShow {
    pub fn new() -> Self {
        Self {
            galley: None {},
            first_lines_of_changed_blocks: vec![],
            change_block_index: 0,
            num_lines: 0,
            show_line: 0,
            scroll_to_line: false,
            position_pointer: "".into(),
        }
    }

    pub fn update(&mut self, left: &str, right: &str, ui: &egui::Ui) {
        let diff = TextDiff::from_lines(left, right);

        let format_both = egui::text::TextFormat {
            font_id: DIFF_FONT_ID,
            extra_letter_spacing: 0.0,
            line_height: None {},
            color: ui.style().visuals.text_color(),
            background: ui.style().visuals.window_fill,
            italics: false,
            underline: egui::Stroke::NONE,
            strikethrough: egui::Stroke::NONE,
            valign: egui::Align::Min,
        };

        let format_right = egui::TextFormat {
            color: egui::Color32::GREEN,
            ..format_both.clone()
        };

        let format_left = egui::TextFormat {
            color: egui::Color32::RED,
            // strikethrough: egui::Stroke {
            //     color: egui::Color32::RED,
            //     width: 1.0,
            // },
            ..format_right.clone()
        };

        let mut block_started = false;
        self.first_lines_of_changed_blocks.clear();
        let mut layout_job = egui::text::LayoutJob::default();
        self.num_lines = 0;

        for (i, change) in diff.iter_all_changes().enumerate() {
            self.num_lines += 1;
            let (is_change, format) = match change.tag() {
                ChangeTag::Equal => (false, format_both.clone()),
                ChangeTag::Insert => (true, format_right.clone()),
                ChangeTag::Delete => (true, format_left.clone()),
            };
            if is_change {
                if !block_started {
                    self.first_lines_of_changed_blocks.push(i);
                    block_started = true;
                }
            } else {
                block_started = false;
            }
            layout_job.append(change.as_str().unwrap(), 0.0, format);
        }

        if !self.first_lines_of_changed_blocks.is_empty() {
            self.galley = Some(ui.ctx().fonts(|f| f.layout_job(layout_job)));
            self.show_line = self.first_lines_of_changed_blocks[0];
            self.change_block_index = 0;
        } else {
            self.galley = None {};
            self.show_line = 0;
            self.change_block_index = 0;
        }

        self.scroll_to_line = true;
        self.update_position_pointer();
    }

    fn update_position_pointer(&mut self) {
        self.position_pointer.clear();
        for _ in 0..self.show_line {
            self.position_pointer.push('\n');
        }
        self.position_pointer.push('⏵');
        self.position_pointer.push('\n');

        for _ in (self.show_line + 1)..self.num_lines {
            self.position_pointer.push('\n');
        }
    }

    pub fn show(
        &mut self,
        left_name: &str,
        right_name: &str,
        same_message: &str,
        ui: &mut egui::Ui,
    ) {
        if self.first_lines_of_changed_blocks.is_empty() {
            ui.label(same_message);
        } else {
            ui.label(
                egui::RichText::from(left_name)
                    .monospace()
                    .color(egui::Color32::RED),
            );
            ui.label(
                egui::RichText::from(right_name)
                    .monospace()
                    .color(egui::Color32::GREEN),
            );
            ui.label(egui::RichText::from("in both").monospace());
            if ui.button("scroll to next change").clicked() {
                self.change_block_index =
                    (self.change_block_index + 1) % self.first_lines_of_changed_blocks.len();
                self.show_line = self.first_lines_of_changed_blocks[self.change_block_index];
                self.scroll_to_line = true;
                self.update_position_pointer();
            }
            ui.separator();

            egui::ScrollArea::both().show(ui, |ui| {
                if let Some(galley) = &self.galley {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::from(&self.position_pointer).font(DIFF_FONT_ID));
                        let text_rect = ui.label(galley.clone()).rect;
                        if self.scroll_to_line {
                            let row_height = ui.ctx().fonts(|f| f.row_height(&DIFF_FONT_ID));
                            let line_rect = egui::Rect::from_min_size(
                                pos2(
                                    text_rect.left(),
                                    text_rect.top() + row_height * self.show_line as f32,
                                ),
                                vec2(1.0, row_height),
                            );
                            ui.scroll_to_rect(line_rect, Some(egui::Align::Center));
                            self.scroll_to_line = false;
                        }
                    });
                }
            });
        }
    }
}
