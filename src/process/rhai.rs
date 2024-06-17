use rhai::{Engine, FuncArgs, Scope, AST};
use std::sync::mpsc;

use crate::{msg, process::r#trait::ProcessState, util::dimension::Dimension};

pub struct State<'a> {
    engine: Engine,
    ast: AST,
    scope: Scope<'a>,
}

impl<D: Dimension, T: Dimension> FuncArgs for msg::ToProcess<D, T> {
    fn parse<ARGS: Extend<rhai::Dynamic>>(self, args: &mut ARGS) {
        todo!()
    }
}

impl<D: Dimension, T: Dimension> ProcessState<D, T> for State<'_> {
    fn handle_msg(
        &mut self,
        time: u64,
        msg: msg::ToProcess<D, T>,
        to_backend: &mpsc::Sender<(u64, msg::ToBackend)>,
        to_ui: &mpsc::Sender<(u64, msg::ToUI<D, T>)>,
    ) {
        let _ = self
            .engine
            .call_fn::<()>(&mut self.scope, &self.ast, "process", msg);
    }
}
