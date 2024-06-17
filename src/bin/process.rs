use std::error::Error;
use std::io::{stdin, stdout, Write};
use std::{sync::mpsc, thread};

use midir::{MidiIO, MidiInput, MidiInputPort, MidiOutput, MidiOutputPort};

use adaptuner::{
    backend::BackendState, config::Config, msg, process::r#trait::ProcessState, tui::UIState,
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
                Ok((time, msg)) => state.handle_msg(time, msg, &ui_tx, &midi_tx),
                Err(_) => break,
            }
        }
    })
}

fn run<D, T, P, PCONFIG, B, BCONFIG, U, UCONFIG>(
    process_config: PCONFIG,
    backend_config: BCONFIG,
    ui_config: UCONFIG,
    midi_in_port: MidiInputPort,
    midi_out_port: MidiOutputPort,
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
    let (midi_out_tx, midi_out_rx) = mpsc::channel();

    start_backend(
        backend_config,
        to_backend_rx,
        to_ui_tx_from_backend,
        midi_out_tx,
    );
    start_ui(ui_config, to_ui_rx, to_process_tx_from_ui);
    start_process(
        process_config,
        to_process_rx,
        to_backend_tx,
        to_ui_tx_from_process,
    );

    // initialise MIDI connections
    let midi_in = MidiInput::new("midir forwarding input")?;
    let midi_out = MidiOutput::new("midir forwarding output")?;

    // _conn_in needs to be a named parameter, because it needs to be kept alive until the end of the scope
    let _conn_in = midi_in.connect(
        &midi_in_port,
        "midir-forward",
        move |time, bytes, _| {
            // send will only fail if the receiver has disconnected. In that case, there's nothing
            // we can do from inside this thread, so we ignore the error. Likely, this will only
            // happen close to the termination of a regular run of the program.
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
                // no error checking here, we assume that the messages are corect.
                conn_out.send(&msg).unwrap_or(());

                //println!("{time}: {:?}", MidiMsg::from_midi(&msg));
            }
            Err(_) => break,
        }
    });

    Ok(())
}

pub fn main() -> Result<(), Box<dyn Error>> {
    // // initialise MIDI connections
    // let mut midi_in = MidiInput::new("midir forwarding input")?;
    // midi_in.ignore(Ignore::None);
    // let midi_out = MidiOutput::new("midir forwarding output")?;
    //
    // let in_port = select_port(&midi_in, "input")?;
    // println!();
    // let out_port = select_port(&midi_out, "output")?;
    //
    // // println!("\nOpening connections");
    // // let in_port_name = midi_in.port_name(&in_port)?;
    // // let out_port_name = midi_out.port_name(&out_port)?;
    //
    // let mut conn_out = midi_out.connect(&out_port, "midir-forward")?;
    //
    // // _conn_in needs to be a named parameter, because it needs to be kept alive until the end of the scope
    // let _conn_in = midi_in.connect(
    //     &in_port,
    //     "midir-forward",
    //     move |time, msg, _| {
    //         // send will only fail if the receiver has disconnected. In that case, there's nothing
    //         // we can do from inside this thread, so we ignore the error. Likely, this will only
    //         // happen close to the termination of a regular run of the program.
    //         midi_in_tx.send((time, msg.to_vec())).unwrap_or(());
    //     },
    //     (),
    // )?;
    //
    // thread::spawn(move || loop {
    //     match midi_out_rx.recv() {
    //         Ok((_time, msg)) => {
    //             // no error checking here, we assume that the messages are corect.
    //             conn_out.send(&msg).unwrap_or(());
    //
    //             //println!("{time}: {:?}", MidiMsg::from_midi(&msg));
    //         }
    //         Err(_) => break,
    //     }
    // });
    //
    // // println!(
    // //     "Connections open, forwarding from '{}' to '{}' (press enter to exit) ...",
    // //     in_port_name, out_port_name
    // // );
    //
    // let mut terminal = tui::init().unwrap();
    //
    // thread::spawn(move || {
    //     let mut grid = Grid {
    //         min_fifth: -4,
    //         max_fifth: 3,
    //         min_third: -2,
    //         max_third: 3,
    //
    //         reference: Stack::new(stype_ui, &vector_from_elem(true), vector_from_elem(0)),
    //
    //         active_temperaments: vector_from_elem(false),
    //         active_classes: [false; 12],
    //         neighbourhood: Neighbourhood::fivelimit_new(4, 5, 1),
    //
    //         config: tui::grid::DisplayConfig {
    //             notenamestyle: adaptuner::notename::NoteNameStyle::JohnstonFiveLimitFull,
    //             color_range: 0.2,
    //             gradient: colorous::RED_BLUE,
    //         },
    //     };
    //
    //     // let _ = terminal.draw(|frame| frame.render_widget(&grid, frame.size()));
    //     //
    //     // loop {
    //     //     match to_ui_rx.recv() {
    //     //         Ok(msg) => {
    //     //             grid.handle_msg(msg);
    //     //             let _ = terminal.draw(|frame| frame.render_widget(&grid, frame.size()));
    //     //         }
    //     //         Err(_) => break,
    //     //     }
    //     // }
    // });
    //
    // loop {
    //     let ev = event::read().unwrap();
    //
    //     if let event::Event::Key(k) = ev {
    //         if k.kind == event::KeyEventKind::Press {
    //             match k.code {
    //                 event::KeyCode::Char('q') => break,
    //                 _ => {}
    //             }
    //         }
    //     } else {
    //     }
    //
    //     to_ui_tx.send(msg::ToUI::CrosstermEvent(ev)).unwrap_or(());
    // }
    //
    // tui::restore().unwrap();
    //
    // // let mut input = String::new();
    // // stdin().read_line(&mut input)?; // wait for next enter key press
    // // println!("Closing connections");
    //
    Ok(())
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
