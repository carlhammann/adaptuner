use std::marker::PhantomData;

use crate::{
    backend,
    backend::r#trait::BackendState,
    config::r#trait::Config,
    process,
    process::r#trait::ProcessState,
    tui,
    tui::r#trait::UIState,
    util::dimension::{fixed_sizes::Size0, Dimension},
};

pub mod r#trait;

#[derive(Clone)]
pub enum MidiPortConfig {
    AskAtStartup,
}

pub struct CompleteConfig<D, T, P, PCONFIG, B, BCONFIG, U, UCONFIG>
where
    D: Dimension,
    T: Dimension,
    P: ProcessState<D, T>,
    PCONFIG: Config<P>,
    B: BackendState<D, T>,
    BCONFIG: Config<B>,
    U: UIState<D, T>,
    UCONFIG: Config<U>,
{
    pub midi_port_config: MidiPortConfig,
    pub process_config: PCONFIG,
    pub backend_config: BCONFIG,
    pub ui_config: UCONFIG,
    _phantom: PhantomData<(D, T, P, B, U)>,
}

pub static TRIVIAL_CONFIG: CompleteConfig<
    Size0,
    Size0,
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
