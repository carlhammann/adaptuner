use std::{
    error::Error,
    io::{stdin, stdout, Write},
    marker::PhantomData,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc,
    },
    thread,
    time::{Duration, Instant},
};

use crossterm::{
    event, execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use midir::{MidiIO, MidiInput, MidiOutput};
use ratatui::{prelude::CrosstermBackend, Terminal};

use adaptuner::{
    backend,
    backend::r#trait::BackendState,
    config::{r#trait::Config, CompleteConfig, MidiPortConfig, TRIVIAL_CONFIG},
    interval,
    interval::Semitones,
    msg, neighbourhood, notename, process,
    process::r#trait::ProcessState,
    tui,
    tui::{Tui, UIState},
    util::dimension::{
        fixed_sizes::{Size0, Size3},
        vector, vector_from_elem, Dimension,
    },
};

fn start_process<D, T, STATE, CONFIG>(
    config: CONFIG,
    msg_rx: mpsc::Receiver<(Instant, msg::ToProcess<D, T>)>,
    backend_tx: mpsc::Sender<(Instant, msg::ToBackend)>,
    ui_tx: mpsc::Sender<(Instant, msg::ToUI<D, T>)>,
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
    msg_rx: mpsc::Receiver<(Instant, msg::ToUI<D, T>)>,
    process_tx: mpsc::Sender<(Instant, msg::ToProcess<D, T>)>,
    mut tui: Tui,
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
                    state.handle_msg(time, msg::ToUI::Stop, &process_tx, &mut tui);
                    break;
                }
                Ok((time, msg)) => state.handle_msg(time, msg, &process_tx, &mut tui),
                Err(_) => break,
            }
        }
    })
}

fn start_backend<D, T, STATE, CONFIG>(
    config: CONFIG,
    msg_rx: mpsc::Receiver<(Instant, msg::ToBackend)>,
    ui_tx: mpsc::Sender<(Instant, msg::ToUI<D, T>)>,
    midi_tx: mpsc::Sender<(Instant, Vec<u8>)>,
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
    let to_ui_tx = to_ui_tx_from_backend.clone();
    let (to_process_tx, to_process_rx) = mpsc::channel();
    let to_process_tx_from_ui = to_process_tx.clone();
    let (midi_out_tx, midi_out_rx) = mpsc::channel::<(Instant, Vec<u8>)>();

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
        move |_time, bytes, _| {
            let time = Instant::now();
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

    execute!(stdout(), EnterAlternateScreen).expect("Could not enter alternate screen");
    enable_raw_mode().expect("Could not enable raw mode");
    let tui = Terminal::new(CrosstermBackend::new(stdout()))
        .expect("Could not start a new Terminal with the crossterm backend");

    thread::spawn(move || loop {
        match event::read() {
            Err(_) => {}
            Ok(e) => {
                let time = Instant::now();
                to_ui_tx
                    .send((time, msg::ToUI::CrosstermEvent(e)))
                    .unwrap_or(());
            }
        }
    });

    let backend = start_backend(
        backend_config,
        to_backend_rx,
        to_ui_tx_from_backend,
        midi_out_tx,
    );
    let ui = start_ui(ui_config, to_ui_rx, to_process_tx_from_ui, tui);
    let process = start_process(
        process_config,
        to_process_rx,
        to_backend_tx,
        to_ui_tx_from_process,
    );

    let now = Instant::now();

    to_backend_tx_start_and_stop
        .send((now, msg::ToBackend::Start))
        .unwrap_or(());
    to_ui_tx_start_and_stop
        .send((now, msg::ToUI::Start))
        .unwrap_or(());
    to_process_tx_start_and_stop
        .send((now, msg::ToProcess::Start))
        .unwrap_or(());

    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();

    ctrlc::set_handler(move || {
        let now = Instant::now();
        r.store(false, Ordering::SeqCst);
        to_backend_tx_start_and_stop
            .send((now, msg::ToBackend::Stop))
            .unwrap_or(());
        to_ui_tx_start_and_stop
            .send((now, msg::ToUI::Stop))
            .unwrap_or(());
        to_process_tx_start_and_stop
            .send((now, msg::ToProcess::Stop))
            .unwrap_or(());
        execute!(stdout(), LeaveAlternateScreen).expect("Could not leave alternate screen");
        disable_raw_mode().expect("Could not disable raw mode");
    })
    .expect("Error setting Ctrl-C handler");

    while running.load(Ordering::SeqCst) & !backend.is_finished() & !process.is_finished() & !ui.is_finished() {
        thread::sleep(Duration::from_millis(100));
    }

    execute!(stdout(), LeaveAlternateScreen).expect("Could not leave alternate screen");
    disable_raw_mode().expect("Could not disable raw mode");

    Ok(())
}

pub fn main() -> Result<(), Box<dyn Error>> {
    let not_so_trivial_config: CompleteConfig<
        Size3,
        Size0,
        process::onlyforward::OnlyForward,
        process::onlyforward::OnlyForwardConfig,
        backend::onlyforward::OnlyForward,
        backend::onlyforward::OnlyForwardConfig,
        tui::grid::Grid<Size3, Size0>,
        tui::grid::GridConfig<Size3, Size0>,
    > = CompleteConfig {
        midi_port_config: MidiPortConfig::AskAtStartup,
        process_config: process::onlyforward::OnlyForwardConfig {},
        backend_config: backend::onlyforward::OnlyForwardConfig {},
        ui_config: tui::grid::GridConfig {
            display_config: tui::grid::DisplayConfig {
                notenamestyle: notename::NoteNameStyle::JohnstonFiveLimitFull,
                color_range: 0.2,
                gradient: colorous::VIRIDIS,
            },
            width: 7,
            height: 5,
            reference: interval::Stack::new(
                (interval::StackType::new(
                    vector(&[
                        interval::Interval {
                            name: "octave".to_string(),
                            semitones: 12.0,
                            key_distance: 12,
                        },
                        interval::Interval {
                            name: "fifth".to_string(),
                            semitones: 12.0 * (1.5 as Semitones).log2(),
                            key_distance: 7,
                        },
                        interval::Interval {
                            name: "third".to_string(),
                            semitones: 12.0 * (1.25 as Semitones).log2(),
                            key_distance: 4,
                        },
                    ])
                    .unwrap(),
                    vector(&[]).unwrap(),
                ))
                .into(),
                &vector_from_elem(false),
                vector_from_elem(0),
            ),
            neighbourhood: neighbourhood::Neighbourhood::fivelimit_new(4, 6, 1),
        },
        _phantom: PhantomData,
    };

    // run::<
    //     adaptuner::util::dimension::fixed_sizes::Size0,
    //     adaptuner::util::dimension::fixed_sizes::Size0,
    //     adaptuner::process::onlyforward::OnlyForward,
    //     adaptuner::process::onlyforward::OnlyForwardConfig,
    //     adaptuner::backend::onlyforward::OnlyForward,
    //     adaptuner::backend::onlyforward::OnlyForwardConfig,
    //     adaptuner::tui::onlynotify::OnlyNotify,
    //     adaptuner::tui::onlynotify::OnlyNotifyConfig,
    // >(
    //     TRIVIAL_CONFIG.process_config.clone(),
    //     TRIVIAL_CONFIG.backend_config.clone(),
    //     TRIVIAL_CONFIG.ui_config.clone(),
    //     TRIVIAL_CONFIG.midi_port_config.clone(),
    // )

    run
    //     ::<
    //     adaptuner::util::dimension::fixed_sizes::Size0,
    //     adaptuner::util::dimension::fixed_sizes::Size0,
    //     adaptuner::process::onlyforward::OnlyForward,
    //     adaptuner::process::onlyforward::OnlyForwardConfig,
    //     adaptuner::backend::onlyforward::OnlyForward,
    //     adaptuner::backend::onlyforward::OnlyForwardConfig,
    //     adaptuner::tui::onlynotify::OnlyNotify,
    //     adaptuner::tui::onlynotify::OnlyNotifyConfig,
    // >
        (
        not_so_trivial_config.process_config.clone(),
        not_so_trivial_config.backend_config.clone(),
        not_so_trivial_config.ui_config.clone(),
        not_so_trivial_config.midi_port_config.clone(),
    )
}
