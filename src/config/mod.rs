use std::marker::PhantomData;

use crate::{
    backend::r#trait::BackendState, config::r#trait::Config,
    interval::stacktype::r#trait::StackType, process::r#trait::ProcessState, tui::r#trait::UIState,
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
