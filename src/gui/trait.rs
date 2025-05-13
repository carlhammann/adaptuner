use std::{sync::mpsc, time::Instant};

use eframe::{self, egui};

use crate::{interval::stacktype::r#trait::StackType, msg};

pub trait GUIState<T: StackType> {
    fn handle_msg(
        &mut self,
        time: Instant,
        msg: &msg::AfterProcess<T>,
        to_process: &mpsc::Sender<(Instant, msg::ToProcess)>,
        ctx: &egui::Context,
        //frame: &mut eframe::Frame,
    );
}
