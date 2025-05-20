use std::{cell::Cell, sync::mpsc, thread};

use eframe::egui;
use midir::{MidiInput, MidiOutput};

use crate::{
    interval::stacktype::r#trait::StackType,
    maybeconnected::{input::MidiInputOrConnection, output::MidiOutputOrConnection},
    msg::{
        FromBackend, FromMidiIn, FromMidiOut, FromProcess, FromUi, HandleMsg, HasStop,
        MessageTranslate, MessageTranslate2, MessageTranslate4, ToBackend, ToMidiIn, ToMidiOut,
        ToProcess, ToUi,
    },
};

fn start_handler_thread<I, O, H, NH>(
    new_state: NH,
    rx: mpsc::Receiver<I>,
    tx: mpsc::Sender<O>,
) -> thread::JoinHandle<(mpsc::Receiver<I>, mpsc::Sender<O>)>
where
    H: HandleMsg<I, O>,
    I: HasStop + Send + 'static,
    O: Send + 'static,
    NH: FnOnce() -> H + Send + 'static,
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
        (rx, tx)
    })
}

struct GuiWithConnections<T: StackType, G: HandleMsg<ToUi<T>, FromUi> + eframe::App> {
    gui: G,
    rx: mpsc::Receiver<ToUi<T>>,
    tx: mpsc::Sender<FromUi>,
}

impl<T: StackType, G: HandleMsg<ToUi<T>, FromUi> + eframe::App> eframe::App
    for GuiWithConnections<T, G>
{
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        for msg in self.rx.try_iter() {
            self.gui.handle_msg(msg, &self.tx);
        }
        self.gui.update(ctx, frame);
    }
}

fn start_gui<T, H, NH>(
    app_name: &str,
    new_gui: NH,
    rx: mpsc::Receiver<ToUi<T>>,
    tx: mpsc::Sender<FromUi>,
) -> Result<(), eframe::Error>
where
    H: HandleMsg<ToUi<T>, FromUi> + eframe::App,
    NH: FnOnce(&egui::Context, mpsc::Sender<FromUi>) -> H + Send + 'static,
    T: StackType + Send + 'static,
{
    eframe::run_native(
        app_name,
        eframe::NativeOptions::default(),
        Box::new(|cc| {
            egui_extras::install_image_loaders(&cc.egui_ctx);
            let gui = new_gui(&cc.egui_ctx, tx.clone());

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

            Ok(Box::new(GuiWithConnections {
                gui,
                tx,
                rx: forward_rx,
            }))
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
    midi_input: thread::JoinHandle<(mpsc::Receiver<ToMidiIn>, mpsc::Sender<FromMidiIn>)>,
    midi_output: thread::JoinHandle<(mpsc::Receiver<ToMidiOut>, mpsc::Sender<FromMidiOut>)>,
    process: thread::JoinHandle<(mpsc::Receiver<ToProcess>, mpsc::Sender<FromProcess<T>>)>,
    backend: Cell<thread::JoinHandle<(mpsc::Receiver<ToBackend>, mpsc::Sender<FromBackend>)>>,
    // ui_thread: thread::JoinHandle<(mpsc::Receiver<ToUi<T>>, mpsc::Sender<FromUi>)>,
    midi_output_forward: thread::JoinHandle<()>,
    midi_input_forward: thread::JoinHandle<()>,
    process_forward: thread::JoinHandle<()>,
    backend_forward: thread::JoinHandle<()>,
    ui_forward: thread::JoinHandle<()>,
    to_process_tx: mpsc::Sender<ToProcess>,
    to_backend_tx: mpsc::Sender<ToBackend>,
    to_ui_tx: mpsc::Sender<ToUi<T>>,
}

impl<T: StackType> RunState<T> {
    pub fn new<P, NP, B, NB, U, NU>(
        midi_in: MidiInput,
        midi_out: MidiOutput,
        new_process_state: NP,
        new_backend_state: NB,
        new_ui_state: NU,
    ) -> Result<Self, eframe::Error>
    where
        T: Send + 'static,
        P: HandleMsg<ToProcess, FromProcess<T>>,
        NP: FnOnce() -> P + Send + 'static,
        B: HandleMsg<ToBackend, FromBackend>,
        NB: FnOnce() -> B + Send + 'static,
        U: HandleMsg<ToUi<T>, FromUi> + eframe::App,
        NU: FnOnce(&egui::Context, mpsc::Sender<FromUi>) -> U + Send + 'static,
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

        let res = Self {
            midi_input: start_handler_thread(|| midi_input, to_midi_input_rx, from_midi_input_tx),
            midi_output: start_handler_thread(
                || midi_output,
                to_midi_output_rx,
                from_midi_output_tx,
            ),
            process: start_handler_thread(new_process_state, to_process_rx, from_process_tx),
            backend: Cell::new(start_handler_thread(
                new_backend_state,
                to_backend_rx,
                from_backend_tx,
            )),
            midi_output_forward: start_translate_thread(from_midi_output_rx, &to_ui_tx),
            midi_input_forward: start_translate_2_thread(
                from_midi_input_rx,
                &to_process_tx,
                &to_ui_tx,
            ),
            process_forward: start_translate_2_thread(from_process_rx, &to_backend_tx, &to_ui_tx),
            backend_forward: start_translate_2_thread(
                from_backend_rx,
                &to_midi_output_tx,
                &to_ui_tx,
            ),
            ui_forward: start_translate_4_thread(
                from_ui_rx,
                &to_process_tx,
                &to_backend_tx,
                &to_midi_input_tx,
                &to_midi_output_tx,
            ),
            to_process_tx,
            to_backend_tx,
            to_ui_tx,
        };

        let _ = to_midi_input_tx.send(ToMidiIn::Start);
        let _ = to_midi_output_tx.send(ToMidiOut::Start);
        // TODO: send more start messages?

        start_gui("adaptuner", new_ui_state, to_ui_rx, from_ui_tx)?;

        Ok(res)
    }

    // pub fn replace_backend<B, NB>(&mut self, new_backend_state: NB) -> thread::Result<()>
    // where
    //     T: Send + 'static,
    //     B: HandleMsg<ToBackend<T>, FromBackend>,
    //     NB: Fn() -> B + Send + 'static,
    // {
    //     match self.to_backend_tx.send(ToBackend::mk_stop()) {
    //         Ok(_) => {}
    //         Err(e) => return Err(Box::new(e)),
    //     }
    //
    //     let make_placeholder = || {
    //         thread::spawn(|| {
    //             let (tx, _) = mpsc::channel();
    //             let (_, rx) = mpsc::channel();
    //             (rx, tx)
    //         })
    //     };
    //     let handle = self.backend.replace(make_placeholder());
    //     let (to_backend_rx, from_backend_tx) = handle.join()?;
    //
    //     self.backend.set(start_handler_thread(
    //         new_backend_state,
    //         to_backend_rx,
    //         from_backend_tx,
    //     ));
    //
    //     Ok(())
    // }
}
