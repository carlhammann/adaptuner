use std::{
    cell::RefCell,
    ops::{Deref, DerefMut},
    sync::{
        atomic::{AtomicUsize, Ordering},
        mpsc, Arc, RwLock,
    },
    time::Instant,
};

use eframe::egui;

use crate::{
    config::{GuiConfig, MelodyStrategyConfig, StrategyConfig},
    gui::common::CorrectionSystemChooser,
    interval::{stack::Stack, stacktype::r#trait::StackType},
    keystate::KeyState,
    msg::{
        FromUi, ReceiveMsg, ToMelody, ToStaticNeighbourhoods, ToStaticNeighbourhoodsAsMelody, ToStrategy, ToTwoStep, ToUi
    },
    process::r#trait::StackWithTuning,
    reference::Reference,
};

pub trait UiAdaptor<T: StackType> {
    fn send(&self, msg: FromUi<T>) -> bool;
    fn key_states(&self) -> impl Deref<Target = [KeyState; 128]>;
    fn tunings(&self) -> impl Deref<Target = [StackWithTuning<T>; 128]>;
    fn reference(&self) -> impl Deref<Target = Stack<T>>;
    fn config(&self) -> impl Deref<Target = GuiConfig>;
    fn strategy_config(&self) -> impl Deref<Target = Vec<StrategyConfig<T>>>;
    fn tuning_reference(&self) -> impl DerefMut<Target = Reference<T>>;
    fn correction_system_chooser(&self) -> impl DerefMut<Target = CorrectionSystemChooser<T>>;
    fn active_strategy_index(&self) -> usize;

    fn send_consider(&self, stack: &Stack<T>, time: Instant) -> bool {
        self.send(FromUi::ToStrategy(
            match self.strategy_config()[self.active_strategy_index()] {
                StrategyConfig::StaticNeighbourhoods { .. } => {
                    ToStrategy::StaticNeighbourhoods(ToStaticNeighbourhoods::Consider {
                        stack: stack.clone(),
                        time,
                    })
                }
                StrategyConfig::TwoStep {
                    melody: MelodyStrategyConfig::StaticNeighbourhoods { .. },
                    ..
                } => ToStrategy::TwoStep(ToTwoStep::ToMelodyStrategy(
                    ToMelody::StaticNeighbourhoods(ToStaticNeighbourhoodsAsMelody::Consider {
                        stack: stack.clone(),
                        time,
                    }),
                )),
            },
        ))
    }
}

pub struct ConcreteUiAdaptor<T: StackType> {
    pub forward: mpsc::Sender<FromUi<T>>,
    pub tunings: Arc<RwLock<[StackWithTuning<T>; 128]>>,
    pub reference: Arc<RwLock<Stack<T>>>,
    pub key_states: Arc<RwLock<[KeyState; 128]>>,
    pub gui_config: RefCell<GuiConfig>,
    pub strategies: Arc<RwLock<Vec<StrategyConfig<T>>>>,
    pub active_strategy_index: Arc<AtomicUsize>,
    pub tuning_reference: Arc<RwLock<Reference<T>>>,
    pub correction_system_chooser: RefCell<CorrectionSystemChooser<T>>,
}

impl<T: StackType> UiAdaptor<T> for ConcreteUiAdaptor<T> {
    fn send(&self, msg: FromUi<T>) -> bool {
        self.forward.send(msg).is_ok()
    }

    fn key_states(&self) -> impl Deref<Target = [KeyState; 128]> {
        self.key_states.read().unwrap()
    }

    fn tunings(&self) -> impl Deref<Target = [StackWithTuning<T>; 128]> {
        self.tunings.read().unwrap()
    }

    fn reference(&self) -> impl Deref<Target = Stack<T>> {
        self.reference.read().unwrap()
    }

    fn config(&self) -> impl Deref<Target = GuiConfig> {
        self.gui_config.borrow()
    }

    fn strategy_config(&self) -> impl Deref<Target = Vec<StrategyConfig<T>>> {
        self.strategies.read().unwrap()
    }

    fn tuning_reference(&self) -> impl DerefMut<Target = Reference<T>> {
        self.tuning_reference.write().unwrap()
    }

    fn correction_system_chooser(&self) -> impl DerefMut<Target = CorrectionSystemChooser<T>> {
        self.correction_system_chooser.borrow_mut()
    }

    fn active_strategy_index(&self) -> usize {
        self.active_strategy_index.load(Ordering::Acquire)
    }
}

pub trait GuiShow<T: StackType> {
    fn show(&mut self, ui: &mut egui::Ui, adaptor: &impl UiAdaptor<T>);
}

pub trait Gui<T: StackType, A: UiAdaptor<T>>: eframe::App + ReceiveMsg<ToUi<T>> {
    fn new(config: GuiConfig, adaptor: A) -> Self;
}
