use std::{ops::Deref, sync::mpsc};

use eframe::egui;

use crate::{
    interval::stacktype::r#trait::StackType,
    msg::FromUi,
    process::r#trait::StackWithTuning,
    util::readerwriter::{ConcreteReader128, Reader128},
};

#[derive(Clone)]
pub struct ConcreteUiAdaptor<T: StackType> {
    pub forward: mpsc::Sender<FromUi<T>>,
    pub tunings: ConcreteReader128<StackWithTuning<T>>,
}

pub trait UiAdaptor<T: StackType>: Clone {
    fn read_tuning(&self, i:  usize) -> impl Deref<Target = StackWithTuning<T>>;
    fn send(&self, msg: FromUi<T>) -> bool;
}

impl<T: StackType> UiAdaptor<T> for ConcreteUiAdaptor<T> {
    fn read_tuning(&self, i:  usize) -> impl Deref<Target = StackWithTuning<T>> {
        self.tunings.read(i)
    }

    fn send(&self, msg: FromUi<T>) -> bool {
        self.forward.send(msg).is_ok()
    }
}

pub trait GuiShow<T: StackType> {
    fn show(&mut self, ui: &mut egui::Ui, forward: &mpsc::Sender<FromUi<T>>);
}
