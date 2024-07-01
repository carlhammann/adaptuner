use std::{
    error::Error,
    io::{stdin, stdout, Write},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc,
    },
    thread,
};

use midir::{MidiIO, MidiInput, MidiOutput};

use adaptuner::{
    backend::r#trait::BackendState,
    config::{r#trait::Config, MidiPortConfig, TRIVIAL_CONFIG},
    msg,
    process::r#trait::ProcessState,
    tui::UIState,
    util::dimension::Dimension,
};

fn start_process<D, T, STATE, CONFIG>(
    config: CONFIG,
    msg_rx: mpsc::Receiver<(u64, msg::ToProcess<D, T>)>,
    backend_tx: mpsc::Sender<(u64, msg::ToBackend)>,
    ui_tx: mpsc::Sender<(u64, msg::ToUI<D, T>)>,
) -> thread::JoinHandle<()>
where
    D: Dimension + Send + Sync + 'static,
    T: Dimension + Send + Sync + 'static,
    STATE: ProcessState<D, T>,
    CONFIG: Config<STATE> + Send + Sync + 'static,
{
    thread::spawn(move || {
        let mut state: STATE = <CONFIG as Config<STATE>>::initialise(&config);
        loop {
            match msg_rx.recv() {
                Ok((time, msg::ToProcess::Stop)) => {
                    state.handle_msg(time, msg::ToProcess::Stop, &backend_tx, &ui_tx);
                    break;
                }
                Ok((time, msg)) => state.handle_msg(time, msg, &backend_tx, &ui_tx),
                Err(_) => break,
            }
        }
    })
}

fn start_ui<D, T, STATE, CONFIG>(
    config: CONFIG,
    msg_rx: mpsc::Receiver<(u64, msg::ToUI<D, T>)>,
    process_tx: mpsc::Sender<(u64, msg::ToProcess<D, T>)>,
) -> thread::JoinHandle<()>
where
    D: Dimension + Send + Sync + 'static,
    T: Dimension + Send + Sync + 'static,
    STATE: UIState<D, T>,
    CONFIG: Config<STATE> + Send + Sync + 'static,
{
    thread::spawn(move || {
        let mut state: STATE = <CONFIG as Config<STATE>>::initialise(&config);
        loop {
            match msg_rx.recv() {
                Ok((time, msg::ToUI::Stop)) => {
                    state.handle_msg(time, msg::ToUI::Stop, &process_tx);
                    break;
                }
                Ok((time, msg)) => state.handle_msg(time, msg, &process_tx),
                Err(_) => break,
            }
        }
    })
}

fn start_backend<D, T, STATE, CONFIG>(
    config: CONFIG,
    msg_rx: mpsc::Receiver<(u64, msg::ToBackend)>,
    ui_tx: mpsc::Sender<(u64, msg::ToUI<D, T>)>,
    midi_tx: mpsc::Sender<(u64, Vec<u8>)>,
) -> thread::JoinHandle<()>
where
    D: Dimension + Send + Sync + 'static,
    T: Dimension + Send + Sync + 'static,
    STATE: BackendState<D, T>,
    CONFIG: Config<STATE> + Send + Sync + 'static,
{
    thread::spawn(move || {
        let mut state: STATE = <CONFIG as Config<STATE>>::initialise(&config);
        loop {
            match msg_rx.recv() {
                Ok((time, msg::ToBackend::Stop)) => {
                    state.handle_msg(time, msg::ToBackend::Stop, &ui_tx, &midi_tx);
                    break;
                }
                Ok((time, msg)) => state.handle_msg(time, msg, &ui_tx, &midi_tx),
                Err(_) => break,
            }
        }
    })
}

fn select_port<T: MidiIO>(midi_io: &T, descr: &str) -> Result<T::Port, Box<dyn Error>> {
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

fn run<D, T, P, PCONFIG, B, BCONFIG, U, UCONFIG>(
    process_config: PCONFIG,
    backend_config: BCONFIG,
    ui_config: UCONFIG,
    _port_config: MidiPortConfig,
) -> Result<(), Box<dyn Error>>
where
    D: Dimension + Send + Sync + 'static,
    T: Dimension + Send + Sync + 'static,
    P: ProcessState<D, T>,
    PCONFIG: Config<P> + Send + Sync + 'static,
    B: BackendState<D, T>,
    BCONFIG: Config<B> + Send + Sync + 'static,
    U: UIState<D, T>,
    UCONFIG: Config<U> + Send + Sync + 'static,
{
    let (to_backend_tx, to_backend_rx) = mpsc::channel();
    let (to_ui_tx_from_process, to_ui_rx) = mpsc::channel();
    let to_ui_tx_from_backend = to_ui_tx_from_process.clone();
    let (to_process_tx, to_process_rx) = mpsc::channel();
    let to_process_tx_from_ui = to_process_tx.clone();
    let (midi_out_tx, midi_out_rx) = mpsc::channel::<(u64, Vec<u8>)>();

    // these three are for the initial "Start" messages and the "Stop" messages from the Ctrl-C
    // handler:
    let to_process_tx_start_and_stop = to_process_tx.clone();
    let to_ui_tx_start_and_stop = to_ui_tx_from_backend.clone();
    let to_backend_tx_start_and_stop = to_backend_tx.clone();

    let midi_in = MidiInput::new("midir forwarding input")?;
    let midi_out = MidiOutput::new("midir forwarding output")?;

    // match port_config {
    //     MidiPortConfig::AskAtStartup => {
    let midi_in_port = select_port(&midi_in, "input")?;
    println!();
    let midi_out_port = select_port(&midi_out, "output")?;
    //     }
    // }

    let _conn_in = midi_in.connect(
        &midi_in_port,
        "midir-forward",
        move |time, bytes, _| {
            to_process_tx
                .send((
                    time,
                    msg::ToProcess::IncomingMidi {
                        bytes: bytes.to_vec(),
                    },
                ))
                .unwrap_or(());
        },
        (),
    )?;

    let mut conn_out = midi_out.connect(&midi_out_port, "midir-forward")?;
    thread::spawn(move || loop {
        match midi_out_rx.recv() {
            Ok((_time, msg)) => {
                conn_out.send(&msg).unwrap_or(());
            }
            Err(_) => break,
        }
    });

    let _backend = start_backend(
        backend_config,
        to_backend_rx,
        to_ui_tx_from_backend,
        midi_out_tx,
    );
    let _ui = start_ui(ui_config, to_ui_rx, to_process_tx_from_ui);
    let _process = start_process(
        process_config,
        to_process_rx,
        to_backend_tx,
        to_ui_tx_from_process,
    );

    to_backend_tx_start_and_stop
        .send((0, msg::ToBackend::Start))
        .unwrap_or(());
    to_ui_tx_start_and_stop
        .send((0, msg::ToUI::Start))
        .unwrap_or(());
    to_process_tx_start_and_stop
        .send((0, msg::ToProcess::Start))
        .unwrap_or(());

    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();

    ctrlc::set_handler(move || {
        r.store(false, Ordering::SeqCst);
        to_backend_tx_start_and_stop
            .send((0, msg::ToBackend::Stop))
            .unwrap_or(());
        to_ui_tx_start_and_stop
            .send((0, msg::ToUI::Stop))
            .unwrap_or(());
        to_process_tx_start_and_stop
            .send((0, msg::ToProcess::Stop))
            .unwrap_or(());
    })
    .expect("Error setting Ctrl-C handler");

    while running.load(Ordering::SeqCst) {}

    Ok(())
}

pub fn main() -> Result<(), Box<dyn Error>> {
    run::<
        adaptuner::util::dimension::fixed_sizes::Size0,
        adaptuner::util::dimension::fixed_sizes::Size0,
        adaptuner::process::onlyforward::OnlyForward,
        adaptuner::process::onlyforward::OnlyForwardConfig,
        adaptuner::backend::onlyforward::OnlyForward,
        adaptuner::backend::onlyforward::OnlyForwardConfig,
        adaptuner::tui::onlynotify::OnlyNotify,
        adaptuner::tui::onlynotify::OnlyNotifyConfig,
    >(
        TRIVIAL_CONFIG.process_config.clone(),
        TRIVIAL_CONFIG.backend_config.clone(),
        TRIVIAL_CONFIG.ui_config.clone(),
        TRIVIAL_CONFIG.midi_port_config.clone(),
    )
}
