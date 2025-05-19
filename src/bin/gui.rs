use std::{
    error::Error,
    io::{stdin, stdout, Write},
    sync::mpsc,
    thread,
    time::Instant,
};

use eframe::{
    self,
    egui::{self},
};
use midi_msg::Channel;

use adaptuner::{
    backend::{pitchbend12::Pitchbend12Config, r#trait::BackendState},
    config::r#trait::Config,
    gui::{
        connectionwindow::MidiConnections, latencywindow::LatencyWindow, manywindows::ManyWindows,
        notewindow::NoteWindow, r#trait::GuiState,
    },
    interval::{
        stack::Stack,
        stacktype::{
            fivelimit::ConcreteFiveLimitStackType,
            r#trait::{FiveLimitStackType, StackType},
        },
    },
    msg,
    process::{fromstrategy, r#trait::ProcessState},
    reference::Reference,
    strategy::{r#static::*, r#trait::Strategy},
};

struct GuiWithConnections<T: StackType, G: GuiState<T>> {
    gui: G,
    incoming_msgs: mpsc::Receiver<(Instant, msg::AfterProcess<T>)>,
    msgs_to_process: mpsc::Sender<(Instant, msg::ToProcess)>,
}

impl<T: StackType, G: GuiState<T> + eframe::App> eframe::App for GuiWithConnections<T, G> {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        for (time, msg) in self.incoming_msgs.try_iter() {
            self.gui.handle_msg(time, &msg, &self.msgs_to_process, ctx);
        }
        self.gui.update(ctx, frame);
    }
}

fn start_gui<
    T: StackType + Send + 'static,
    G: GuiState<T> + eframe::App,
    NG: FnOnce(&egui::Context) -> G,
>(
    app_name: &str,
    new_gui: NG,
    incoming_msgs: mpsc::Receiver<(Instant, msg::AfterProcess<T>)>,
    msgs_to_process: mpsc::Sender<(Instant, msg::ToProcess)>,
) -> Result<(), eframe::Error> {
    eframe::run_native(
        app_name,
        eframe::NativeOptions::default(),
        Box::new(|cc| {
            egui_extras::install_image_loaders(&cc.egui_ctx);
            let gui = new_gui(&cc.egui_ctx);

            let ctx = cc.egui_ctx.clone();
            let (forward_tx, forward_rx) = mpsc::channel::<(Instant, msg::AfterProcess<T>)>();

            // This extra thread is needed to really request the repaint. If `request_repaint` is
            // called from outside of an UI thread, the UI thread wakes up and runs.
            thread::spawn(move || loop {
                match incoming_msgs.recv() {
                    Ok((time, msg)) => {
                        ctx.request_repaint();
                        forward_tx.send((time, msg)).unwrap_or(());
                    }
                    Err(_) => break,
                }
            });

            Ok(Box::new(GuiWithConnections {
                gui,
                incoming_msgs: forward_rx,
                msgs_to_process,
            }))
        }),
    )
}

fn start_process<T, S, C>(
    config: C,
    msg_rx: mpsc::Receiver<(Instant, msg::ToProcess)>,
    backend_tx: mpsc::Sender<(Instant, msg::AfterProcess<T>)>,
) -> thread::JoinHandle<()>
where
    T: StackType + Send + 'static,
    S: Strategy<T>,
    C: Config<S> + Clone + Send + 'static,
{
    thread::spawn(move || {
        let mut state: fromstrategy::State<T, S, C> = fromstrategy::State::new(&config);
        loop {
            match msg_rx.recv() {
                Ok((time, msg::ToProcess::Stop)) => {
                    state.handle_msg(time, msg::ToProcess::Stop, &backend_tx);
                    break;
                }
                Ok((time, msg)) => state.handle_msg(time, msg, &backend_tx),
                Err(_) => break,
            }
        }
    })
}

fn start_backend<T, S, C>(
    config: C,
    msg_rx: mpsc::Receiver<(Instant, msg::AfterProcess<T>)>,
    ui_tx: mpsc::Sender<(Instant, msg::AfterProcess<T>)>,
    midi_tx: mpsc::Sender<(Instant, msg::ToMidiOut)>,
) -> thread::JoinHandle<()>
where
    T: StackType + Send + 'static,
    S: BackendState<T>,
    C: Config<S> + Send + 'static,
{
    thread::spawn(move || {
        let mut state: S = <C as Config<S>>::initialise(&config);
        loop {
            match msg_rx.recv() {
                Ok((time, msg::AfterProcess::Stop)) => {
                    state.handle_msg(time, msg::AfterProcess::Stop, &ui_tx, &midi_tx);
                    break;
                }
                Ok((time, msg)) => state.handle_msg(time, msg, &ui_tx, &midi_tx),
                Err(_) => break,
            }
        }
    })
}

fn select_port<T: midir::MidiIO>(midi_io: &T, descr: &str) -> Result<T::Port, Box<dyn Error>> {
    println!("Available {} ports:", descr);
    let midi_ports = midi_io.ports();
    for (i, p) in midi_ports.iter().enumerate() {
        println!("{}: {}", i, midi_io.port_name(p)?);
    }
    print!("Please select {} port: ", descr);
    stdout().flush()?;
    let mut input = String::new();
    stdin().read_line(&mut input)?;
    let port = midi_ports
        .get(input.trim().parse::<usize>()?)
        .ok_or("Invalid port number")?;
    Ok(port.clone())
}

fn run<T, PS, SC, B, BC>(strategy_config: SC, backend_config: BC) -> Result<(), Box<dyn Error>>
where
    T: FiveLimitStackType + Send + 'static + Clone,
    PS: Strategy<T>,
    SC: Config<PS> + Clone + Send + 'static,
    B: BackendState<T>,
    BC: Config<B> + Send + 'static,
{
    let midi_in = midir::MidiInput::new("adaptuner input")?;
    let midi_out = midir::MidiOutput::new("adaptuner output")?;
    let (midi_conncections, to_process_rx, backend_to_midi_out_tx, midi_out_latency_rx) =
        MidiConnections::new(midi_in, midi_out);

    let ui_to_process_tx = midi_conncections.new_sender_to_process();
    let (process_tx, process_forward_rx) = mpsc::channel::<(Instant, msg::AfterProcess<T>)>();
    let (backend_to_ui_tx, to_ui_rx) = mpsc::channel();
    let process_to_ui_tx = backend_to_ui_tx.clone();
    let (to_backend_tx, to_backend_rx) = mpsc::channel();

    let _process_forward = thread::spawn(move || loop {
        match process_forward_rx.recv() {
            Ok((time, msg)) => {
                process_to_ui_tx.send((time, msg.clone())).unwrap_or(());
                to_backend_tx.send((time, msg)).unwrap_or(());
            }

            Err(_) => break,
        }
    });

    let _backend = start_backend(
        backend_config,
        to_backend_rx,
        backend_to_ui_tx,
        backend_to_midi_out_tx,
    );

    let _process = start_process(strategy_config, to_process_rx, process_tx);

    start_gui(
        "adaptuner",
        |ctx| ManyWindows::new(ctx, midi_conncections, 20),
        to_ui_rx,
        ui_to_process_tx,
    )?;

    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    let global_reference = Reference::<ConcreteFiveLimitStackType>::from_frequency(
        Stack::from_target(vec![1, -1, 1]),
        440.0,
    );
    let no_active_temperaments = vec![false; 2];
    let initial_neighbourhood_width = 4;
    let initial_neighbourhood_index = 5;
    let initial_neighbourhood_offset = 1;
    let strategy_config = StaticTuningConfig {
        active_temperaments: no_active_temperaments,
        width: initial_neighbourhood_width,
        index: initial_neighbourhood_index,
        offset: initial_neighbourhood_offset,
        global_reference,
    };

    let backend_config = Pitchbend12Config {
        channels: [
            Channel::Ch1,
            Channel::Ch2,
            Channel::Ch3,
            Channel::Ch4,
            Channel::Ch5,
            Channel::Ch6,
            Channel::Ch7,
            Channel::Ch8,
            Channel::Ch9,
            Channel::Ch11,
            Channel::Ch12,
            Channel::Ch13,
        ],
        bend_range: 2.0,
    };

    run(strategy_config, backend_config)
}
