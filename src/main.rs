use std::{
    collections::HashMap,
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
use midi_msg::Channel;
use midir::{MidiIO, MidiInput, MidiOutput};
use ratatui::{prelude::CrosstermBackend, Terminal};

use adaptuner::{
    backend,
    backend::r#trait::BackendState,
    config::{r#trait::Config, CompleteConfig, MidiPortConfig},
    interval,
    interval::{
        stack::Stack,
        stacktype::r#trait::{PeriodicStackType, StackType},
    },
    msg, neighbourhood,
    neighbourhood::{PeriodicPartial, SomeNeighbourhood},
    notename,
    pattern::{KeyShape, Pattern},
    process,
    process::r#trait::ProcessState,
    tui,
    tui::r#trait::{Tui, UIState},
};

fn start_process<T, STATE, CONFIG>(
    config: CONFIG,
    msg_rx: mpsc::Receiver<(Instant, msg::ToProcess)>,
    backend_tx: mpsc::Sender<(Instant, msg::AfterProcess<T>)>,
) -> thread::JoinHandle<()>
where
    T: StackType + Send + Sync + 'static,
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
    T: StackType + Send + Sync + 'static,
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
    T: StackType + Send + Sync + 'static,
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
    process_config: PCONFIG,
    backend_config: BCONFIG,
    ui_config: UCONFIG,
    _port_config: MidiPortConfig,
) -> Result<(), Box<dyn Error>>
where
    T: StackType + Sync + Send + 'static + Clone,
    P: ProcessState<T>,
    PCONFIG: Config<P> + Send + Sync + 'static,
    B: BackendState<T>,
    BCONFIG: Config<B> + Send + Sync + 'static,
    U: UIState<T>,
    UCONFIG: Config<U> + Send + Sync + 'static,
{
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
        backend_config,
        to_backend_rx,
        to_ui_tx_from_backend,
        midi_out_tx,
    );
    let ui = start_ui(ui_config, to_ui_rx, to_process_tx_from_ui, tui);
    let process = start_process(process_config, to_process_rx, to_backend_and_ui_tx);

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
    type TheStackType = crate::interval::stacktype::fivelimit::ConcreteFiveLimitStackType;
    let initial_neighbourhood_width = 4;
    let initial_neighbourhood_index = 5;
    let initial_neighbourhood_offset = 1;
    let no_active_temperaments = vec![false; TheStackType::num_temperaments()];
    let initial_neighbourhood = neighbourhood::new_fivelimit_neighbourhood(
        &no_active_temperaments,
        initial_neighbourhood_width,
        initial_neighbourhood_index,
        initial_neighbourhood_offset,
    );

    let not_so_trivial_config: CompleteConfig<
        TheStackType,
        // process::static12::Static12<TheStackType, neighbourhood::PeriodicCompleteAligned<TheStackType>>,
        // process::static12::Static12Config<TheStackType, neighbourhood::PeriodicCompleteAligned<TheStackType>>,
        process::walking::Walking<
            TheStackType,
            neighbourhood::PeriodicCompleteAligned<TheStackType>,
        >,
        process::walking::WalkingConfig<
            TheStackType,
            neighbourhood::PeriodicCompleteAligned<TheStackType>,
        >,
        backend::pitchbend::Pitchbend<15>,
        backend::pitchbend::PitchbendConfig<15>,
        // tui::onlynotify::OnlyNotify,
        // tui::onlynotify::OnlyNotifyConfig,
        tui::wrappedgrid::WrappedGrid<
            TheStackType,
            neighbourhood::PeriodicCompleteAligned<TheStackType>,
        >,
        tui::wrappedgrid::WrappedGridConfig<
            TheStackType,
            neighbourhood::PeriodicCompleteAligned<TheStackType>,
        >,
    > = CompleteConfig {
        midi_port_config: MidiPortConfig::AskAtStartup,
        process_config: process::walking::WalkingConfig {
            _phantom: PhantomData,
            temper_pattern_neighbourhoods: false,
            initial_neighbourhood: initial_neighbourhood.clone(),
            patterns: vec![
                Pattern {
                    name: String::from("major"),
                    keyshape: KeyShape::ClassesRelative {
                        period_keys: 12,
                        classes: vec![0, 4, 7],
                    },
                    neighbourhood: SomeNeighbourhood::PeriodicPartial(PeriodicPartial {
                        period: Stack::from_pure_interval(TheStackType::period_index()),
                        stacks: HashMap::from([
                            (0, Stack::new_zero()),
                            (4, Stack::new(&no_active_temperaments, vec![0, 0, 1])),
                            (7, Stack::new(&no_active_temperaments, vec![0, 1, 0])),
                        ]),
                    }),
                },
                Pattern {
                    name: String::from("minor"),
                    keyshape: KeyShape::ClassesRelative {
                        period_keys: 12,
                        classes: vec![0, 3, 7],
                    },
                    neighbourhood: SomeNeighbourhood::PeriodicPartial(PeriodicPartial {
                        period: Stack::from_pure_interval(TheStackType::period_index()),
                        stacks: HashMap::from([
                            (0, Stack::new_zero()),
                            (3, Stack::new(&no_active_temperaments, vec![0, 1, -1])),
                            (7, Stack::new(&no_active_temperaments, vec![0, 1, 0])),
                        ]),
                    }),
                },
            ],
            consider_played: false,
        },
        // process_config: process::static12::Static12Config {
        //     initial_reference_key,
        //     initial_neighbourhood: initial_neighbourhood.clone(),
        //     _phantom: PhantomData,
        // },
        backend_config: backend::pitchbend::PitchbendConfig {
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
                Channel::Ch14,
                Channel::Ch15,
                Channel::Ch16,
            ],
            bend_range: 2.0,
        },
        ui_config: tui::wrappedgrid::WrappedGridConfig {
            gridconfig: tui::grid::GridConfig {
                display_config: tui::grid::DisplayConfig {
                    notenamestyle: notename::NoteNameStyle::JohnstonFiveLimitClass,
                    color_range: 0.2,
                    gradient: colorous::RED_BLUE,
                },
                initial_reference_key: 60,
                initial_neighbourhood,
                horizontal_index: 1,
                vertical_index: 2,
                fifth_index: 1,
                third_index: 2,
                _phantom: PhantomData,
            },
            latencyreporterconfig: tui::latencyreporter::LatencyReporterConfig { nsamples: 20 },
        },
        // ui_config: tui::onlynotify::OnlyNotifyConfig {},
        _phantom: PhantomData,
    };

    run(
        not_so_trivial_config.process_config.clone(),
        not_so_trivial_config.backend_config.clone(),
        not_so_trivial_config.ui_config.clone(),
        not_so_trivial_config.midi_port_config.clone(),
    )
}
