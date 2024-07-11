use std::marker::PhantomData;

use crate::{
    backend, backend::r#trait::BackendState, config::r#trait::Config, interval,
    interval::StackType, process, process::r#trait::ProcessState, tui, tui::r#trait::UIState,
};

pub mod r#trait;

#[derive(Clone)]
pub enum MidiPortConfig {
    AskAtStartup,
}

pub struct CompleteConfig<T, P, PCONFIG, B, BCONFIG, U, UCONFIG>
where
    T: StackType,
    P: ProcessState<T>,
    PCONFIG: Config<P>,
    B: BackendState<T>,
    BCONFIG: Config<B>,
    U: UIState<T>,
    UCONFIG: Config<U>,
{
    pub midi_port_config: MidiPortConfig,
    pub process_config: PCONFIG,
    pub backend_config: BCONFIG,
    pub ui_config: UCONFIG,
    pub _phantom: PhantomData<(T, P, B, U)>,
}

pub static TRIVIAL_CONFIG: CompleteConfig<
    interval::ConcreteStackType,
    process::onlyforward::OnlyForward,
    process::onlyforward::OnlyForwardConfig,
    backend::onlyforward::OnlyForward,
    backend::onlyforward::OnlyForwardConfig,
    tui::onlynotify::OnlyNotify,
    tui::onlynotify::OnlyNotifyConfig,
> = CompleteConfig {
    midi_port_config: MidiPortConfig::AskAtStartup,
    process_config: process::onlyforward::OnlyForwardConfig {},
    backend_config: backend::onlyforward::OnlyForwardConfig {},
    ui_config: tui::onlynotify::OnlyNotifyConfig {},
    _phantom: PhantomData,
};
