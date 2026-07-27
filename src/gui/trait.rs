use std::{
    cell::RefCell,
    ops::{Deref, DerefMut},
    sync::{mpsc, Arc},
};

use parking_lot::RwLock;

use eframe::egui;

use crate::{
    adaptors::lock_levels::*,
    backend::pitchbend12::Pitchbend12Config,
    config::{GuiConfig, StrategyConfig},
    interval::{stack::Stack, stacktype::r#trait::StackType},
    keystate::KeyState,
    msg::{FromUi, ReceiveMsg, ToUi},
    process::r#trait::StackWithTuning,
    reference::Reference,
    util::ordered_locks::{
        impl_access, impl_access_mut, impl_indexed_access, Access, AccessMut, IndexedAccess,
        OrderedLocks, Zero,
    },
};

pub trait UiAdaptor:
    IndexedAccess<KeyStateLevel, usize, KeyState>
    + IndexedAccess<TuningStateLevel, usize, StackWithTuning<Self::StackType>>
    + Access<StrategyConfigLevel, Vec<StrategyConfig<Self::StackType>>>
    + AccessMut<StrategyConfigLevel, Vec<StrategyConfig<Self::StackType>>>
    + Access<ActiveStrategyIndexLevel, usize>
    + Access<TuningReferenceLevel, Reference<Self::StackType>>
    + AccessMut<TuningReferenceLevel, Reference<Self::StackType>>
    + Access<ReferenceLevel, Stack<Self::StackType>>
    + Access<BackendConfigLevel, Pitchbend12Config>
    + AccessMut<BackendConfigLevel, Pitchbend12Config>
{
    type StackType: StackType;
    fn send(&self, msg: FromUi<Self::StackType>);

    fn config(&self) -> impl Deref<Target = GuiConfig>;
    fn config_mut(&self) -> impl DerefMut<Target = GuiConfig>;
}

pub struct ConcreteUiAdaptor<T: StackType> {
    pub forward: mpsc::Sender<FromUi<T>>,
    pub key_states: [Arc<RwLock<KeyState>>; 128],
    pub tunings: [Arc<RwLock<StackWithTuning<T>>>; 128],
    pub tuning_reference: Arc<RwLock<Reference<T>>>,
    pub reference: Arc<RwLock<Stack<T>>>,
    pub strategy_config: Arc<RwLock<Vec<StrategyConfig<T>>>>,
    pub active_strategy_index: Arc<RwLock<usize>>,
    pub gui_config: RefCell<GuiConfig>,
    pub backend_config: Arc<RwLock<Pitchbend12Config>>,
}

impl_indexed_access! {<T:StackType>, ConcreteUiAdaptor<T>, KeyStateLevel, usize, KeyState, |self, i| &self.key_states[i].read()}
impl_indexed_access! {<T:StackType>, ConcreteUiAdaptor<T>, TuningStateLevel, usize, StackWithTuning<T>, |self, i| &self.tunings[i].read()}
impl_access! {<T:StackType>, ConcreteUiAdaptor<T>, TuningReferenceLevel, Reference<T>, |self| &self.tuning_reference.read()}
impl_access_mut! {<T:StackType>, ConcreteUiAdaptor<T>, TuningReferenceLevel, Reference<T>, |self| &mut self.tuning_reference.write()}
impl_access! {<T:StackType>, ConcreteUiAdaptor<T>, StrategyConfigLevel, Vec<StrategyConfig<T>>, |self| &self.strategy_config.read()}
impl_access_mut! {<T:StackType>, ConcreteUiAdaptor<T>, StrategyConfigLevel, Vec<StrategyConfig<T>>, |self| &mut self.strategy_config.write()}
impl_access! {<T:StackType>, ConcreteUiAdaptor<T>, ActiveStrategyIndexLevel, usize, |self| &self.active_strategy_index.read()}
impl_access! {<T:StackType>, ConcreteUiAdaptor<T>, ReferenceLevel, Stack<T>, |self| &self.reference.read()}
impl_access! {<T:StackType>, ConcreteUiAdaptor<T>, BackendConfigLevel, Pitchbend12Config, |self| &self.backend_config.read()}
impl_access_mut! {<T:StackType>, ConcreteUiAdaptor<T>, BackendConfigLevel, Pitchbend12Config, |self| &mut self.backend_config.write()}

impl<T: StackType> UiAdaptor for ConcreteUiAdaptor<T> {
    type StackType = T;

    #[inline]
    fn send(&self, msg: FromUi<T>) {
        self.forward.send(msg);
    }

    #[inline]
    fn config(&self) -> impl Deref<Target = GuiConfig> {
        self.gui_config.borrow()
    }

    #[inline]
    fn config_mut(&self) -> impl DerefMut<Target = GuiConfig> {
        self.gui_config.borrow_mut()
    }
}

pub trait GuiShow<T: StackType> {
    fn show<A: UiAdaptor<StackType = T>>(
        &mut self,
        ui: &mut egui::Ui,
        adaptor: OrderedLocks<A, Zero>,
    ) -> OrderedLocks<A, Zero>;
}

pub trait Gui<T: StackType, A: UiAdaptor<StackType = T>>:
    eframe::App + ReceiveMsg<ToUi<T>>
{
    fn new(adaptor: A) -> Self;
}

pub trait ReceiveToUiRef<T: StackType, A: UiAdaptor<StackType = T>> {
    fn receive_to_ui_ref(
        &mut self,
        msg: &ToUi<T>,
        adaptor: OrderedLocks<A, Zero>,
    ) -> OrderedLocks<A, Zero>;
}
