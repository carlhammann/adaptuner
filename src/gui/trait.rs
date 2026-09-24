use std::ops::{Deref, DerefMut};

use eframe::egui;

use crate::{
    adaptors::{lock_levels::*, ConcreteLocks},
    config::GuiConfig,
    interval::stacktype::r#trait::StackType,
    msg::{FromUi, ReceiveMsg, ToUi},
    util::ordered_locks::{Nat, OrderedLocks, ReadAllowed, WriteAllowed, Zero},
};

pub struct GuiTag {}

impl ReadAllowed<PedalHoldLevel> for GuiTag {}
impl ReadAllowed<KeyStateLevel> for GuiTag {}
impl ReadAllowed<TuningStateLevel> for GuiTag {}
impl ReadAllowed<StrategyConfigLevel> for GuiTag {}
impl ReadAllowed<ActiveStrategyIndexLevel> for GuiTag {}
impl ReadAllowed<TuningReferenceLevel> for GuiTag {}
impl ReadAllowed<ReferenceLevel> for GuiTag {}
impl ReadAllowed<BackendConfigLevel> for GuiTag {}
impl ReadAllowed<HarmonyLevel> for GuiTag {}
impl ReadAllowed<AnchoringLevel> for GuiTag {}

impl WriteAllowed<StrategyConfigLevel> for GuiTag {}
impl WriteAllowed<ActiveStrategyIndexLevel> for GuiTag {}
impl WriteAllowed<TuningReferenceLevel> for GuiTag {}
impl WriteAllowed<BackendConfigLevel> for GuiTag {}

pub type UiAdaptor<T, L> = OrderedLocks<GuiTag, ConcreteLocks<T>, L>;

impl<T: StackType, L: Nat> UiAdaptor<T, L> {
    #[inline]
    pub fn send(&self, msg: FromUi<T>) {
        let _ = unsafe { self.inner() }.from_ui_tx.send(msg);
    }

    #[inline]
    pub fn config(&self) -> impl Deref<Target = GuiConfig> + use<'_, T, L> {
        unsafe { self.inner() }.gui_config.read()
    }

    #[inline]
    pub fn config_mut(&self) -> impl DerefMut<Target = GuiConfig> + use<'_, T, L> {
        unsafe { self.inner() }.gui_config.write()
    }
}

pub trait GuiShow<T: StackType> {
    fn show(&mut self, ui: &mut egui::Ui, adaptor: UiAdaptor<T, Zero>) -> UiAdaptor<T, Zero>;
}

pub trait Gui<T: StackType>: eframe::App + ReceiveMsg<ToUi<T>> {
    fn new(adaptor: UiAdaptor<T, Zero>) -> Self;
}

pub trait ReceiveToUiRef<T: StackType> {
    fn receive_to_ui_ref(
        &mut self,
        msg: &ToUi<T>,
        adaptor: UiAdaptor<T, Zero>,
    ) -> UiAdaptor<T, Zero>;
}
