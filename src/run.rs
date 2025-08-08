use std::{
    sync::{mpsc, Arc, Mutex},
    thread,
    time::Instant,
};

use eframe::egui;
use midir::{MidiInput, MidiOutput};

use crate::{
    config::{
        BackendConfig, ExtractConfig, FromConfigAndState, GuiConfig, MidiInputConfig,
        MidiOutputConfig, ProcessConfig,
    },
    interval::stacktype::r#trait::StackType,
    maybeconnected::{input::MidiInputOrConnection, output::MidiOutputOrConnection},
    msg::{
        FromBackend, FromMidiIn, FromMidiOut, FromProcess, FromUi, HandleMsg, HasStop,
        MessageTranslate, MessageTranslate2, MessageTranslate3, MessageTranslate4, ReceiveMsg,
        ToBackend, ToMidiIn, ToMidiOut, ToProcess, ToUi,
    },
};

fn start_handler_thread<I, O, H, C, NH>(
    new_state: NH,
    rx: mpsc::Receiver<I>,
    tx: mpsc::Sender<O>,
) -> thread::JoinHandle<(C, mpsc::Receiver<I>, mpsc::Sender<O>)>
where
    H: HandleMsg<I, O>,
    H: ExtractConfig<C>,
    I: HasStop + Send + 'static,
    O: Send + 'static,
    NH: FnOnce() -> H + Send + 'static,
    C: Send + 'static,
{
    thread::spawn(move || {
        let mut state = new_state();
        loop {
            match rx.recv() {
                Ok(msg) => {
                    let stop = msg.is_stop();
                    state.handle_msg(msg, &tx);
                    if stop {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
        (state.extract_config(), rx, tx)
    })
}

struct GuiWithConnections<T: StackType, G> {
    gui: G,
    rx: mpsc::Receiver<ToUi<T>>,
    config_return: Arc<Mutex<Option<GuiConfig<T>>>>,
}

impl<T: StackType + Send, G> GuiWithConnections<T, G> {
    fn new(
        cc: &eframe::CreationContext,
        gui: G,
        rx: mpsc::Receiver<ToUi<T>>,
        config_return: Arc<Mutex<Option<GuiConfig<T>>>>,
    ) -> Self {
        let ctx = cc.egui_ctx.clone();
        let (forward_tx, forward_rx) = mpsc::channel::<ToUi<T>>();

        // This extra thread is needed to really request the repaint. If `request_repaint` is
        // called from outside of an UI thread, the UI thread wakes up and runs.
        thread::spawn(move || loop {
            match rx.recv() {
                Ok(msg) => {
                    ctx.request_repaint();
                    let _ = forward_tx.send(msg);
                }
                Err(_) => break,
            }
        });

        Self {
            gui,
            rx: forward_rx,
            config_return,
        }
    }
}

impl<T, G> eframe::App for GuiWithConnections<T, G>
where
    T: StackType,
    G: ReceiveMsg<ToUi<T>> + ExtractConfig<GuiConfig<T>> + eframe::App,
{
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        for msg in self.rx.try_iter() {
            self.gui.receive_msg(msg);
        }
        self.gui.update(ctx, frame);
        if ctx.input(|i| i.viewport().close_requested()) {
            *self.config_return.lock().unwrap() = Some(self.gui.extract_config());
        }
    }
}

fn start_gui<T, H, NH>(
    app_name: &str,
    new_gui: NH,
    rx: mpsc::Receiver<ToUi<T>>,
    tx: mpsc::Sender<FromUi<T>>,
    config_return: Arc<Mutex<Option<GuiConfig<T>>>>,
) -> Result<(), eframe::Error>
where
    H: ReceiveMsg<ToUi<T>> + eframe::App + ExtractConfig<GuiConfig<T>>,
    NH: FnOnce(&egui::Context, mpsc::Sender<FromUi<T>>) -> H + Send + 'static,
    T: StackType + Send + 'static,
{
    eframe::run_native(
        app_name,
        eframe::NativeOptions::default(),
        Box::new(|cc| {
            egui_extras::install_image_loaders(&cc.egui_ctx);
            let gui = new_gui(&cc.egui_ctx, tx.clone());
            Ok(Box::new(GuiWithConnections::new(
                cc,
                gui,
                rx,
                config_return,
            )))
        }),
    )
}

fn start_translate_thread<B, A>(
    rxa: mpsc::Receiver<A>,
    txb: &mpsc::Sender<B>,
) -> thread::JoinHandle<()>
where
    B: Send + 'static,
    A: MessageTranslate<B> + Send + 'static,
{
    let txb_clone = txb.clone();
    thread::spawn(move || loop {
        match rxa.recv() {
            Ok(msg) => {
                let tb = msg.translate();
                match tb {
                    Some(tb) => {
                        let _ = txb_clone.send(tb);
                    }
                    None {} => {}
                }
            }
            Err(_) => break,
        }
    })
}

fn start_translate_2_thread<B, C, A>(
    rxa: mpsc::Receiver<A>,
    txb: &mpsc::Sender<B>,
    txc: &mpsc::Sender<C>,
) -> thread::JoinHandle<()>
where
    B: Send + 'static,
    C: Send + 'static,
    A: MessageTranslate2<B, C> + Send + 'static,
{
    let txb_clone = txb.clone();
    let txc_clone = txc.clone();
    thread::spawn(move || loop {
        match rxa.recv() {
            Ok(msg) => {
                let (tb, tc) = msg.translate2();
                match tb {
                    Some(tb) => {
                        let _ = txb_clone.send(tb);
                    }
                    None {} => {}
                }
                match tc {
                    Some(tc) => {
                        let _ = txc_clone.send(tc);
                    }
                    None {} => {}
                }
            }
            Err(_) => break,
        }
    })
}

fn start_translate_3_thread<B, C, D, A>(
    rxa: mpsc::Receiver<A>,
    txb: &mpsc::Sender<B>,
    txc: &mpsc::Sender<C>,
    txd: &mpsc::Sender<D>,
) -> thread::JoinHandle<()>
where
    B: Send + 'static,
    C: Send + 'static,
    D: Send + 'static,
    A: MessageTranslate3<B, C, D> + Send + 'static,
{
    let txb_clone = txb.clone();
    let txc_clone = txc.clone();
    let txd_clone = txd.clone();
    thread::spawn(move || loop {
        match rxa.recv() {
            Ok(msg) => {
                let (tb, tc, td) = msg.translate3();
                match tb {
                    Some(tb) => {
                        let _ = txb_clone.send(tb);
                    }
                    None {} => {}
                }
                match tc {
                    Some(tc) => {
                        let _ = txc_clone.send(tc);
                    }
                    None {} => {}
                }
                match td {
                    Some(td) => {
                        let _ = txd_clone.send(td);
                    }
                    None {} => {}
                }
            }
            Err(_) => break,
        }
    })
}
fn start_translate_4_thread<B, C, D, E, A>(
    rxa: mpsc::Receiver<A>,
    txb: &mpsc::Sender<B>,
    txc: &mpsc::Sender<C>,
    txd: &mpsc::Sender<D>,
    txe: &mpsc::Sender<E>,
) -> thread::JoinHandle<()>
where
    B: Send + 'static,
    C: Send + 'static,
    D: Send + 'static,
    E: Send + 'static,
    A: MessageTranslate4<B, C, D, E> + Send + 'static,
{
    let txb_clone = txb.clone();
    let txc_clone = txc.clone();
    let txd_clone = txd.clone();
    let txe_clone = txe.clone();
    thread::spawn(move || loop {
        match rxa.recv() {
            Ok(msg) => {
                let (tb, tc, td, te) = msg.translate4();
                match tb {
                    Some(tb) => {
                        let _ = txb_clone.send(tb);
                    }
                    None {} => {}
                }
                match tc {
                    Some(tc) => {
                        let _ = txc_clone.send(tc);
                    }
                    None {} => {}
                }
                match td {
                    Some(td) => {
                        let _ = txd_clone.send(td);
                    }
                    None {} => {}
                }
                match te {
                    Some(te) => {
                        let _ = txe_clone.send(te);
                    }
                    None {} => {}
                }
            }
            Err(_) => break,
        }
    })
}

pub struct RunState<T: StackType> {
    midi_input: thread::JoinHandle<(
        MidiInputConfig,
        mpsc::Receiver<ToMidiIn>,
        mpsc::Sender<FromMidiIn>,
    )>,
    midi_output: thread::JoinHandle<(
        MidiOutputConfig,
        mpsc::Receiver<ToMidiOut>,
        mpsc::Sender<FromMidiOut>,
    )>,
    process: thread::JoinHandle<(
        ProcessConfig<T>,
        mpsc::Receiver<ToProcess<T>>,
        mpsc::Sender<FromProcess<T>>,
    )>,
    backend: thread::JoinHandle<(
        BackendConfig,
        mpsc::Receiver<ToBackend>,
        mpsc::Sender<FromBackend>,
    )>,
    to_process_tx: mpsc::Sender<ToProcess<T>>,
    to_backend_tx: mpsc::Sender<ToBackend>,
    to_midi_input_tx: mpsc::Sender<ToMidiIn>,
    to_midi_output_tx: mpsc::Sender<ToMidiOut>,
    gui_config_return: Arc<Mutex<Option<GuiConfig<T>>>>,
}

#[derive(Debug)]
pub enum JoinError {
    Process,
    Backend,
    Gui,
    MidiInput,
    MidiOutput,
}

impl std::fmt::Display for JoinError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            JoinError::Process => write!(f, "couldn't join the process thread"),
            JoinError::Backend => write!(f, "couldn't join the backend thread"),
            JoinError::Gui => write!(f, "couldn't join the GUI thread"),
            JoinError::MidiInput => write!(f, "couldn't join the midi input thread"),
            JoinError::MidiOutput => write!(f, "couldn't join the midi output thread"),
        }
    }
}

impl std::error::Error for JoinError {}

impl<T: StackType> RunState<T> {
    pub fn new<P, B, U, NU>(
        midi_in: MidiInput,
        midi_out: MidiOutput,
        process_config: ProcessConfig<T>,
        backend_config: BackendConfig,
        new_ui_state: NU,
    ) -> Result<Self, eframe::Error>
    where
        T: Send + 'static,
        P: HandleMsg<ToProcess<T>, FromProcess<T>>
            + ExtractConfig<ProcessConfig<T>>
            + FromConfigAndState<ProcessConfig<T>, ()>,
        B: HandleMsg<ToBackend, FromBackend>
            + ExtractConfig<BackendConfig>
            + FromConfigAndState<BackendConfig, ()>,
        U: ReceiveMsg<ToUi<T>> + eframe::App + ExtractConfig<GuiConfig<T>>,
        NU: FnOnce(&egui::Context, mpsc::Sender<FromUi<T>>) -> U + Send + 'static,
    {
        let (to_midi_input_tx, to_midi_input_rx) = mpsc::channel();
        let (from_midi_input_tx, from_midi_input_rx) = mpsc::channel();
        let midi_input = MidiInputOrConnection::new(midi_in, from_midi_input_tx.clone());

        let (to_midi_output_tx, to_midi_output_rx) = mpsc::channel();
        let (from_midi_output_tx, from_midi_output_rx) = mpsc::channel();
        let midi_output = MidiOutputOrConnection::new(midi_out);

        let (to_process_tx, to_process_rx) = mpsc::channel();
        let (from_process_tx, from_process_rx) = mpsc::channel::<FromProcess<T>>();

        let (to_backend_tx, to_backend_rx) = mpsc::channel();
        let (from_backend_tx, from_backend_rx) = mpsc::channel();

        let (to_ui_tx, to_ui_rx) = mpsc::channel();
        let (from_ui_tx, from_ui_rx) = mpsc::channel();

        let gui_config_return = Arc::new(Mutex::new(None {}));

        let _midi_output_forward = start_translate_thread(from_midi_output_rx, &to_ui_tx);
        let _midi_input_forward =
            start_translate_2_thread(from_midi_input_rx, &to_process_tx, &to_ui_tx);
        let _process_forward = start_translate_3_thread(
            from_process_rx,
            &to_backend_tx,
            &to_midi_output_tx,
            &to_ui_tx,
        );
        let _backend_forward =
            start_translate_2_thread(from_backend_rx, &to_midi_output_tx, &to_ui_tx);
        let _ui_forward = start_translate_4_thread(
            from_ui_rx,
            &to_process_tx,
            &to_backend_tx,
            &to_midi_input_tx,
            &to_midi_output_tx,
        );

        let res = Self {
            midi_input: start_handler_thread(|| midi_input, to_midi_input_rx, from_midi_input_tx),
            midi_output: start_handler_thread(
                || midi_output,
                to_midi_output_rx,
                from_midi_output_tx,
            ),
            process: start_handler_thread(
                || P::initialise(process_config, ()),
                to_process_rx,
                from_process_tx,
            ),
            backend: start_handler_thread(
                || B::initialise(backend_config, ()),
                to_backend_rx,
                from_backend_tx,
            ),
            to_process_tx: to_process_tx.clone(),
            to_backend_tx,
            to_midi_input_tx: to_midi_input_tx.clone(),
            to_midi_output_tx: to_midi_output_tx.clone(),
            gui_config_return: gui_config_return.clone(),
        };

        let _ = to_midi_input_tx.send(ToMidiIn::Start);
        let _ = to_midi_output_tx.send(ToMidiOut::Start);
        let _ = to_process_tx.send(ToProcess::Start {
            time: Instant::now(),
        });
        // TODO: send more start messages?

        start_gui(
            "adaptuner",
            new_ui_state,
            to_ui_rx,
            from_ui_tx,
            gui_config_return,
        )?;

        Ok(res)
    }

    pub fn stop(
        self,
    ) -> Result<
        (
            ProcessConfig<T>,
            BackendConfig,
            GuiConfig<T>,
            MidiInputConfig,
            MidiOutputConfig,
        ),
        JoinError,
    > {
        let _ = self.to_process_tx.send(ToProcess::Stop);
        let Ok((process_config, _, _)) = self.process.join() else {
            return Err(JoinError::Process);
        };

        let _ = self.to_backend_tx.send(ToBackend::Stop);
        let Ok((backend_config, _, _)) = self.backend.join() else {
            return Err(JoinError::Backend);
        };

        let _ = self.to_midi_input_tx.send(ToMidiIn::Stop);
        let Ok((midi_input_config, _, _)) = self.midi_input.join() else {
            return Err(JoinError::MidiInput);
        };

        let _ = self.to_midi_output_tx.send(ToMidiOut::Stop);
        let Ok((midi_output_config, _, _)) = self.midi_output.join() else {
            return Err(JoinError::MidiOutput);
        };

        if let Some(gui_config) = self.gui_config_return.lock().unwrap().as_ref() {
            Ok((
                process_config,
                backend_config,
                gui_config.clone(),
                midi_input_config,
                midi_output_config,
            ))
        } else {
            Err(JoinError::Gui)
        }
    }
}
