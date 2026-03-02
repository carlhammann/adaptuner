use std::{sync::mpsc, thread, time::Instant};

use eframe::egui;
use midir::{MidiInput, MidiOutput};

use crate::{
    backend::r#trait::ConcreteBackendAdaptor,
    config::{BackendConfig, FromConfigAndState, ProcessConfig},
    gui::r#trait::ConcreteUiAdaptor,
    interval::{base::Semitones, stack::Stack, stacktype::r#trait::StackType},
    keystate::KeyState,
    maybeconnected::{input::MidiInputOrConnection, output::MidiOutputOrConnection},
    msg::{
        FromProcess, HasStop, MessageTranslate, MessageTranslate2, MessageTranslate3,
        MessageTranslate4, ReceiveMsg, ToBackend, ToMidiIn, ToMidiOut, ToProcess, ToUi,
    },
    process::r#trait::{ConcreteProcessAdaptor, StackWithTuning},
    util::readerwriter::ConcreteReaderWriter128,
};

fn start_receiver_thread<I, H, NH>(
    new_state: NH,
    rx: mpsc::Receiver<I>,
) -> thread::JoinHandle<mpsc::Receiver<I>>
where
    H: ReceiveMsg<I>,
    I: HasStop + Send + 'static,
    NH: FnOnce() -> H + Send + 'static,
{
    thread::spawn(move || {
        let mut state = new_state();
        loop {
            match rx.recv() {
                Ok(msg) => {
                    let stop = msg.is_stop();
                    state.receive_msg(msg);
                    if stop {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
        rx
    })
}

struct GuiWithConnections<T: StackType, G> {
    gui: G,
    rx: mpsc::Receiver<ToUi<T>>,
}

impl<T: StackType + Send, G> GuiWithConnections<T, G> {
    fn new(cc: &eframe::CreationContext, gui: G, rx: mpsc::Receiver<ToUi<T>>) -> Self {
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
        }
    }
}

impl<T, G> eframe::App for GuiWithConnections<T, G>
where
    T: StackType,
    G: ReceiveMsg<ToUi<T>> + eframe::App,
{
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        for msg in self.rx.try_iter() {
            self.gui.receive_msg(msg);
        }
        self.gui.update(ctx, frame);
    }
}

fn setup_fonts(ctx: &egui::Context) {
    use egui::{FontData, FontDefinitions, FontFamily};

    let mut fonts = FontDefinitions::default();

    fonts.font_data.insert(
        "inter_music".to_owned(),
        FontData::from_static(include_bytes!("../assets/InterMusic.ttf")).into(),
    );

    fonts
        .families
        .get_mut(&FontFamily::Proportional)
        .unwrap()
        .insert(0, "inter_music".to_owned());

    ctx.set_fonts(fonts);
}

fn start_gui<T, H, NH>(
    new_gui: NH,
    rx: mpsc::Receiver<ToUi<T>>,
    adaptor: ConcreteUiAdaptor<T>,
) -> Result<(), eframe::Error>
where
    H: ReceiveMsg<ToUi<T>> + eframe::App,
    NH: FnOnce(&egui::Context, ConcreteUiAdaptor<T>) -> H + Send + 'static,
    T: StackType + Send + 'static,
{
    // create icon.rgba using something like
    //
    // magick stream -map rgba -storage-type char icon.png icon.rgba
    //
    // you can inspect it using
    //
    // magick display -depth 8 -size 512x512 rgba:icon.rgba
    let icon_bytes = include_bytes!("../assets/icon.rgba");
    let icon_data = egui::IconData {
        width: 512,
        height: 512,
        rgba: icon_bytes.into(),
    };
    eframe::run_native(
        "com.carlhammann.adaptuner",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_icon(icon_data)
                .with_title("adaptuner")
                .with_app_id("com.carlhammann.adaptuner"),
            ..eframe::NativeOptions::default()
        },
        Box::new(|cc| {
            egui_extras::install_image_loaders(&cc.egui_ctx);

            // load fonts
            setup_fonts(&cc.egui_ctx);

            let gui = new_gui(&cc.egui_ctx, adaptor);
            Ok(Box::new(GuiWithConnections::new(cc, gui, rx)))
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
    midi_input: thread::JoinHandle<mpsc::Receiver<ToMidiIn>>,
    midi_output: thread::JoinHandle<mpsc::Receiver<ToMidiOut>>,
    process: thread::JoinHandle<mpsc::Receiver<ToProcess<T>>>,
    backend: thread::JoinHandle<mpsc::Receiver<ToBackend>>,
    to_process_tx: mpsc::Sender<ToProcess<T>>,
    to_backend_tx: mpsc::Sender<ToBackend>,
    to_midi_input_tx: mpsc::Sender<ToMidiIn>,
    to_midi_output_tx: mpsc::Sender<ToMidiOut>,
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
        T: Send + Sync + 'static,
        P: ReceiveMsg<ToProcess<T>>
            + FromConfigAndState<ProcessConfig<T>, ConcreteProcessAdaptor<T>>,
        B: ReceiveMsg<ToBackend> + FromConfigAndState<BackendConfig, ConcreteBackendAdaptor<T>>,
        U: ReceiveMsg<ToUi<T>> + eframe::App,
        NU: FnOnce(&egui::Context, ConcreteUiAdaptor<T>) -> U + Send + 'static,
    {
        let (to_midi_input_tx, to_midi_input_rx) = mpsc::channel();
        let (from_midi_input_tx, from_midi_input_rx) = mpsc::channel();
        let midi_input = MidiInputOrConnection::new(midi_in, from_midi_input_tx.clone());

        let (to_midi_output_tx, to_midi_output_rx) = mpsc::channel();
        let (from_midi_output_tx, from_midi_output_rx) = mpsc::channel();
        let midi_output = MidiOutputOrConnection::new(midi_out, from_midi_output_tx);

        let (to_process_tx, to_process_rx) = mpsc::channel();
        let (from_process_tx, from_process_rx) = mpsc::channel::<FromProcess<T>>();

        let (to_backend_tx, to_backend_rx) = mpsc::channel();
        let (from_backend_tx, from_backend_rx) = mpsc::channel();

        let (to_ui_tx, to_ui_rx) = mpsc::channel();
        let (from_ui_tx, from_ui_rx) = mpsc::channel();

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

        let now = Instant::now();

        let process_adaptor = ConcreteProcessAdaptor {
            forward: from_process_tx,
            tunings: ConcreteReaderWriter128::new(core::array::from_fn(|i| StackWithTuning {
                stack: Stack::new_zero(),
                tuning: i as Semitones,
            })),
            key_states: ConcreteReaderWriter128::new(core::array::from_fn(|_| KeyState::new(now))),
        };

        let gui_adaptor = ConcreteUiAdaptor {
            forward: from_ui_tx,
            tunings: process_adaptor.tunings.clone().into_reader(),
        };

        let backend_adaptor = ConcreteBackendAdaptor {
            forward: from_backend_tx,
            tunings: process_adaptor.tunings.clone().into_reader(),
            key_states: process_adaptor.key_states.clone().into_reader(),
        };

        let res = Self {
            midi_input: start_receiver_thread(|| midi_input, to_midi_input_rx),
            midi_output: start_receiver_thread(|| midi_output, to_midi_output_rx),
            process: start_receiver_thread(
                || P::initialise(process_config, process_adaptor),
                to_process_rx,
            ),
            backend: start_receiver_thread(
                || B::initialise(backend_config, backend_adaptor),
                to_backend_rx,
            ),
            to_process_tx: to_process_tx.clone(),
            to_backend_tx,
            to_midi_input_tx: to_midi_input_tx.clone(),
            to_midi_output_tx: to_midi_output_tx.clone(),
        };

        let _ = to_midi_input_tx.send(ToMidiIn::Start);
        let _ = to_midi_output_tx.send(ToMidiOut::Start);
        let _ = to_process_tx.send(ToProcess::Start {
            time: Instant::now(),
        });
        // TODO: send more start messages?

        start_gui(new_ui_state, to_ui_rx, gui_adaptor)?;

        Ok(res)
    }
}
