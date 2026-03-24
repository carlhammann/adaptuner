use std::{
    ops::{Deref, DerefMut},
    sync::mpsc,
};

use eframe::egui;

use crate::{
    config::{ExtendedStrategyConfig, GuiConfigWithoutStrategies, NamedAndDescribed},
    interval::stacktype::r#trait::StackType,
    msg::FromUi,
    process::r#trait::StackWithTuning,
    util::readerwriter::{
        ConcreteReader, ConcreteReader128, ConcreteReaderWriter, Reader, Reader128, ReaderWriter,
    },
};

pub trait UiAdaptor<T: StackType>:
    Clone
    + Reader128<StackWithTuning<T>>
    + ReaderWriter<GuiConfigWithoutStrategies>
    + Reader<Vec<NamedAndDescribed<ExtendedStrategyConfig<T>>>>
{
    fn read_tuning(&self, i: usize) -> impl Deref<Target = StackWithTuning<T>> {
        <Self as Reader128<StackWithTuning<T>>>::read(self, i)
    }
    fn send(&self, msg: FromUi<T>) -> bool;
}

#[derive(Clone)]
pub struct ConcreteUiAdaptor<T: StackType> {
    forward: mpsc::Sender<FromUi<T>>,
    tunings: ConcreteReader128<StackWithTuning<T>>,
    gui_config: ConcreteReaderWriter<GuiConfigWithoutStrategies>,
    strategies: ConcreteReader<Vec<NamedAndDescribed<ExtendedStrategyConfig<T>>>>,
}

impl<T: StackType> Reader<Vec<NamedAndDescribed<ExtendedStrategyConfig<T>>>>
    for ConcreteUiAdaptor<T>
{
    fn read(&self) -> impl Deref<Target = Vec<NamedAndDescribed<ExtendedStrategyConfig<T>>>> {
        self.strategies.read()
    }
}

impl<T: StackType> Reader<GuiConfigWithoutStrategies> for ConcreteUiAdaptor<T> {
    fn read(&self) -> impl Deref<Target = GuiConfigWithoutStrategies> {
        self.gui_config.read()
    }
}

impl<T: StackType> ReaderWriter<GuiConfigWithoutStrategies> for ConcreteUiAdaptor<T> {
    fn write(&self) -> impl DerefMut<Target = GuiConfigWithoutStrategies> {
        self.gui_config.write()
    }
}

impl<T: StackType> Reader128<StackWithTuning<T>> for ConcreteUiAdaptor<T> {
    fn read(&self, i: usize) -> impl Deref<Target = StackWithTuning<T>> {
        self.tunings.read(i)
    }

    fn read_all(&self) -> impl Deref<Target = [impl AsRef<StackWithTuning<T>>; 128]> {
        self.tunings.read_all()
    }
}

impl<T: StackType> UiAdaptor<T> for ConcreteUiAdaptor<T> {
    fn send(&self, msg: FromUi<T>) -> bool {
        self.forward.send(msg).is_ok()
    }
}

pub trait GuiShow<T: StackType> {
    fn show(&mut self, ui: &mut egui::Ui, forward: &mpsc::Sender<FromUi<T>>);
}
