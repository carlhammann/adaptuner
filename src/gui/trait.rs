use std::{
    cell::RefCell,
    ops::{Deref, DerefMut},
    sync::{
        atomic::{AtomicUsize, Ordering},
        mpsc, Arc,
    },
    time::Instant,
};

use parking_lot::RwLock;

use eframe::egui;

use crate::{
    adaptors::{ViewKeyStates, ViewTunings},
    backend::pitchbend12::Pitchbend12Config,
    config::{GuiConfig, MelodyStrategyConfig, StrategyConfig},
    interval::{stack::Stack, stacktype::r#trait::StackType},
    keystate::KeyState,
    msg::{
        FromUi, ReceiveMsg, ToMelody, ToStaticNeighbourhoods, ToStaticNeighbourhoodsAsMelody,
        ToStrategy, ToTwoStep, ToUi,
    },
    process::r#trait::StackWithTuning,
    reference::Reference,
};

/// Things must be locked in the order in which the functions in this trait are defined.
pub trait UiAdaptor<T: StackType>: ViewKeyStates + ViewTunings<T> {
    fn send(&self, msg: FromUi<T>) -> bool;

    fn tuning_reference(&self) -> impl Deref<Target = Reference<T>>;
    fn tuning_reference_mut(&self) -> impl DerefMut<Target = Reference<T>>;

    fn strategy_config(&self) -> impl Deref<Target = Vec<StrategyConfig<T>>>;
    fn strategy_config_mut(&self) -> impl DerefMut<Target = Vec<StrategyConfig<T>>>;
    fn active_strategy_index(&self) -> usize;
    fn reference(&self) -> impl Deref<Target = Stack<T>>;
    fn config(&self) -> impl Deref<Target = GuiConfig>;
    fn config_mut(&self) -> impl DerefMut<Target = GuiConfig>;

    fn backend_config(&self) -> impl Deref<Target = Pitchbend12Config>;
    fn backend_config_mut(&self) -> impl DerefMut<Target = Pitchbend12Config>;

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
    pub key_states: [Arc<RwLock<KeyState>>; 128],
    pub tunings: [Arc<RwLock<StackWithTuning<T>>>; 128],
    pub tuning_reference: Arc<RwLock<Reference<T>>>,
    pub reference: Arc<RwLock<Stack<T>>>,
    pub strategies: Arc<RwLock<Vec<StrategyConfig<T>>>>,
    pub active_strategy_index: Arc<AtomicUsize>,
    pub gui_config: RefCell<GuiConfig>,
    pub backend_config: Arc<RwLock<Pitchbend12Config>>,
}

impl<T: StackType> ViewKeyStates for ConcreteUiAdaptor<T> {
    #[inline]
    fn key_state(&self, i: usize) -> KeyState {
        *self.key_states[i].read()
    }
}

impl<T: StackType> ViewTunings<T> for ConcreteUiAdaptor<T> {
    #[inline]
    fn tuning(&self, i: usize) -> impl Deref<Target = StackWithTuning<T>> {
        self.tunings[i].read()
    }
}

impl<T: StackType> UiAdaptor<T> for ConcreteUiAdaptor<T> {
    #[inline]
    fn send(&self, msg: FromUi<T>) -> bool {
        self.forward.send(msg).is_ok()
    }

    #[inline]
    fn tuning_reference(&self) -> impl Deref<Target = Reference<T>> {
        self.tuning_reference.read()
    }

    #[inline]
    fn tuning_reference_mut(&self) -> impl DerefMut<Target = Reference<T>> {
        self.tuning_reference.write()
    }

    #[inline]
    fn reference(&self) -> impl Deref<Target = Stack<T>> {
        self.reference.read()
    }

    #[inline]
    fn config(&self) -> impl Deref<Target = GuiConfig> {
        self.gui_config.borrow()
    }

    #[inline]
    fn config_mut(&self) -> impl DerefMut<Target = GuiConfig> {
        self.gui_config.borrow_mut()
    }

    #[inline]
    fn strategy_config(&self) -> impl Deref<Target = Vec<StrategyConfig<T>>> {
        self.strategies.read()
    }

    #[inline]
    fn strategy_config_mut(&self) -> impl DerefMut<Target = Vec<StrategyConfig<T>>> {
        self.strategies.write()
    }

    #[inline]
    fn active_strategy_index(&self) -> usize {
        self.active_strategy_index.load(Ordering::Acquire)
    }

    #[inline]
    fn backend_config(&self) -> impl Deref<Target = Pitchbend12Config> {
        self.backend_config.read()
    }

    #[inline]
    fn backend_config_mut(&self) -> impl DerefMut<Target = Pitchbend12Config> {
        self.backend_config.write()
    }
}

pub trait GuiShow<T: StackType> {
    fn show(&mut self, ui: &mut egui::Ui, adaptor: &impl UiAdaptor<T>);
}

pub trait Gui<T: StackType, A: UiAdaptor<T>>: eframe::App + ReceiveMsg<ToUi<T>> {
    fn new(adaptor: A) -> Self;
}

pub trait ReceiveToUiRef<T: StackType, A: UiAdaptor<T>> {
    fn receive_to_ui_ref(&mut self, msg: &ToUi<T>, adaptor: &A);
}
