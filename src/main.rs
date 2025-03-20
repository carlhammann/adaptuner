use std::{
    env,
    error::Error,
    fs::File,
    hash::Hash,
    io::{stdin, stdout, Write},
    path::Path,
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
    backend::r#trait::BackendState,
    config,
    config::{r#trait::Config, CompleteConfig},
    interval::stacktype::r#trait::StackType,
    msg,
    process::r#trait::ProcessState,
    tui::r#trait::{Tui, UIState},
};

fn start_process<T, STATE, CONFIG>(
    config: CONFIG,
    msg_rx: mpsc::Receiver<(Instant, msg::ToProcess)>,
    backend_tx: mpsc::Sender<(Instant, msg::AfterProcess<T>)>,
) -> thread::JoinHandle<()>
where
    T: StackType + Send + Sync + 'static + Eq + Hash,
    STATE: ProcessState<T>,
    CONFIG: Config<STATE> + Send + Sync + 'static,
{
    thread::spawn(move || {
        let mut state: STATE = <CONFIG as Config<STATE>>::initialise(&config);
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

fn start_ui<T, STATE, CONFIG>(
    config: CONFIG,
    msg_rx: mpsc::Receiver<(Instant, msg::AfterProcess<T>)>,
    process_tx: mpsc::Sender<(Instant, msg::ToProcess)>,
    mut tui: Tui,
) -> thread::JoinHandle<()>
where
    T: StackType + Send + Sync + 'static + Eq + Hash,
    STATE: UIState<T>,
    CONFIG: Config<STATE> + Send + Sync + 'static,
{
    thread::spawn(move || {
        let mut state: STATE = <CONFIG as Config<STATE>>::initialise(&config);
        loop {
            match msg_rx.recv() {
                Ok((time, msg::AfterProcess::Stop)) => {
                    let _ = tui.draw(|frame| {
                        state.handle_msg(
                            time,
                            &msg::AfterProcess::Stop,
                            &process_tx,
                            frame,
                            frame.size(),
                        );
                    });
                    break;
                }
                Ok((time, msg)) => {
                    let _ = tui.draw(|frame| {
                        state.handle_msg(time, &msg, &process_tx, frame, frame.size());
                    });
                }
                Err(_) => break,
            }
        }
    })
}

fn start_backend<T, STATE, CONFIG>(
    config: CONFIG,
    msg_rx: mpsc::Receiver<(Instant, msg::AfterProcess<T>)>,
    ui_tx: mpsc::Sender<(Instant, msg::AfterProcess<T>)>,
    midi_tx: mpsc::Sender<(Instant, Vec<u8>)>,
) -> thread::JoinHandle<()>
where
    T: StackType + Send + Sync + 'static + Eq + Hash,
    STATE: BackendState<T>,
    CONFIG: Config<STATE> + Send + Sync + 'static,
{
    thread::spawn(move || {
        let mut state: STATE = <CONFIG as Config<STATE>>::initialise(&config);
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

fn run<T, P, PCONFIG, B, BCONFIG, U, UCONFIG>(
    config: CompleteConfig<T, P, PCONFIG, B, BCONFIG, U, UCONFIG>,
) -> Result<(), Box<dyn Error>>
where
    T: StackType + Sync + Send + 'static + Clone + Eq + Hash,
    P: ProcessState<T>,
    PCONFIG: Config<P> + Send + Sync + 'static,
    B: BackendState<T>,
    BCONFIG: Config<B> + Send + Sync + 'static,
    U: UIState<T>,
    UCONFIG: Config<U> + Send + Sync + 'static,
{
    // let process_config = config
    // backend_config: BCONFIG,
    // ui_config: UCONFIG,
    // _port_config: MidiPortConfig,

    let (to_backend_tx, to_backend_rx) = mpsc::channel();
    let (to_ui_tx, to_ui_rx) = mpsc::channel();
    let (to_backend_and_ui_tx, to_backend_and_ui_rx) =
        mpsc::channel::<(Instant, msg::AfterProcess<T>)>();
    let to_backend_and_ui_tx_from_midi_out = to_backend_and_ui_tx.clone();
    let to_ui_tx_from_backend = to_ui_tx.clone();
    let to_ui_tx_from_outside = to_ui_tx.clone();

    let (to_process_tx, to_process_rx) = mpsc::channel();
    let to_process_tx_from_ui = to_process_tx.clone();

    let (midi_out_tx, midi_out_rx) = mpsc::channel::<(Instant, Vec<u8>)>();

    // these three are for the initial "Start" messages and the "Stop" messages from the Ctrl-C
    // handler:
    let to_process_tx_start_and_stop = to_process_tx.clone();
    let to_ui_tx_start_and_stop = to_ui_tx_from_backend.clone();
    let to_backend_tx_start_and_stop = to_backend_tx.clone();

    let midi_in = MidiInput::new("adaptuner input")?;
    let midi_out = MidiOutput::new("adaptuner output")?;

    // match port_config {
    //     MidiPortConfig::AskAtStartup => {
    let midi_in_port = select_port(&midi_in, "input")?;
    println!();
    let midi_out_port = select_port(&midi_out, "output")?;
    //     }
    // }

    let _conn_in = midi_in.connect(
        &midi_in_port,
        "adaptuner-forward",
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

    let mut conn_out = midi_out.connect(&midi_out_port, "adaptuner-forward")?;
    thread::spawn(move || loop {
        match midi_out_rx.recv() {
            Ok((original_time, msg)) => {
                conn_out.send(&msg).unwrap_or(());
                let time = Instant::now();
                to_backend_and_ui_tx_from_midi_out
                    .send((
                        time,
                        msg::AfterProcess::BackendLatency {
                            since_input: time.duration_since(original_time),
                        },
                    ))
                    .unwrap_or(());
            }
            Err(_) => break,
        }
    });

    execute!(stdout(), EnterAlternateScreen).expect("Could not enter alternate screen");
    execute!(stdout(), event::EnableMouseCapture).expect("Could not enable mouse capture");
    enable_raw_mode().expect("Could not enable raw mode");
    let tui = Terminal::new(CrosstermBackend::new(stdout()))
        .expect("Could not start a new Terminal with the crossterm backend");

    thread::spawn(move || loop {
        match event::read() {
            Err(_) => {}
            Ok(e) => {
                let time = Instant::now();
                to_ui_tx_from_outside
                    .send((time, msg::AfterProcess::CrosstermEvent(e)))
                    .unwrap_or(());
            }
        }
    });

    thread::spawn(move || loop {
        match to_backend_and_ui_rx.recv() {
            Ok((time, msg)) => {
                to_backend_tx.send((time, msg.clone())).unwrap_or(());
                to_ui_tx.send((time, msg)).unwrap_or(());
            }
            Err(_) => break,
        }
    });

    let backend = start_backend(
        config.backend_config,
        to_backend_rx,
        to_ui_tx_from_backend,
        midi_out_tx,
    );
    let ui = start_ui(config.ui_config, to_ui_rx, to_process_tx_from_ui, tui);
    let process = start_process(config.process_config, to_process_rx, to_backend_and_ui_tx);

    let now = Instant::now();

    to_backend_tx_start_and_stop
        .send((now, msg::AfterProcess::Start))
        .unwrap_or(());
    to_ui_tx_start_and_stop
        .send((now, msg::AfterProcess::Start))
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
            .send((now, msg::AfterProcess::Stop))
            .unwrap_or(());
        to_ui_tx_start_and_stop
            .send((now, msg::AfterProcess::Stop))
            .unwrap_or(());
        to_process_tx_start_and_stop
            .send((now, msg::ToProcess::Stop))
            .unwrap_or(());
        execute!(stdout(), LeaveAlternateScreen).expect("Could not leave alternate screen");
        execute!(stdout(), event::DisableMouseCapture).expect("Could not disable mouse capture");
        disable_raw_mode().expect("Could not disable raw mode");
    })
    .expect("Error setting Ctrl-C handler");

    while running.load(Ordering::SeqCst)
        & !backend.is_finished()
        & !process.is_finished()
        & !ui.is_finished()
    {
        thread::sleep(Duration::from_millis(100));
    }

    execute!(stdout(), LeaveAlternateScreen).expect("Could not leave alternate screen");
    execute!(stdout(), event::DisableMouseCapture).expect("Could not disable mouse capture");
    disable_raw_mode().expect("Could not disable raw mode");

    Ok(())
}

pub fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = env::args().collect();
    let initial_neighbourhood_width = 4;
    let initial_neighbourhood_index = 5;
    let initial_neighbourhood_offset = 1;

    if args.len() != 2 {
        return Err("expected exactly one argument: the path of the pattern file".into());
    }

    let pattern_path = Path::new(&args[1]);
    let file = match File::open(pattern_path) {
        Err(why) => return Err(format!("couldn't open {}: {}", pattern_path.display(), why).into()),
        Ok(file) => file,
    };
    let patterns = match deser_hjson::from_reader(file) {
        Err(why) => return Err(format!("couldn't read {}: {}", pattern_path.display(), why).into()),
        Ok(patterns) => patterns,
    };

    let the_config = config::init_walking_config(
        initial_neighbourhood_width,
        initial_neighbourhood_index,
        initial_neighbourhood_offset,
        patterns,
    );

    run(the_config)
}
