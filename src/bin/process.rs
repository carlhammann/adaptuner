use std::error::Error;
use std::fs;
use std::io::{stdin, stdout, Write};
use std::{
    sync::{mpsc, Arc},
    thread,
};

use serde_json;

use midir::{Ignore, MidiIO, MidiInput, MidiOutput};

use adaptuner::{
    backend,
    backend::BackendState,
    config::{validate, Config, RawConfig},
    interval::{Stack, StackType},
    neighbourhood::Neighbourhood,
    process,
    process::ProcessState,
    util::dimension::{fixed_sizes::Size3, vector_from_elem},
};

#[derive(Debug, Copy, Clone)]
struct TTag {}

pub fn main() {
    let raw_config: RawConfig =
        serde_json::from_str(&fs::read_to_string("config.json").unwrap()).unwrap();
    let config: Config<Size3, TTag> = validate(raw_config);

    let stype_process = Arc::new(StackType::new(
        config.intervals.clone(),
        config.temperaments.clone(),
    ));
    let stype_backend = stype_process.clone();


    let (midi_in_tx, midi_in_rx) = mpsc::channel();
    let (midi_out_tx, midi_out_rx) = mpsc::channel();
    let (to_backend_tx, to_backend_rx) = mpsc::channel();
    let (to_ui_tx, to_ui_rx) = mpsc::channel();

    let to_ui_tx_from_process = to_ui_tx.clone();
    let to_backend_tx_from_process = to_backend_tx.clone();

    thread::spawn(move || {
        let initial_tuning_frame = process::TuningFrame {
            reference_key: 0,
            reference_stack: Stack::new(stype_process, &vector_from_elem(false), vector_from_elem(0)),
            neighbourhood: Neighbourhood::fivelimit_new(4, 7, 1),
            active_temperaments: vector_from_elem(false),
        };

        let mut state = process::State {
            current: initial_tuning_frame.clone(),
            old: initial_tuning_frame,
            birthday: 0,

            active_notes: [false; 128],
            sustain: false,

            config: process::Config {
                patterns: &config.patterns,
                minimum_age: 10000,
            },
        };

        loop {
            match midi_in_rx.recv() {
                Ok((time, msg)) => {
                    state.handle_midi_msg(
                        time,
                        &msg,
                        &to_backend_tx_from_process,
                        &to_ui_tx_from_process,
                    );
                }
                Err(_) => break,
            }
        }
    });

    thread::spawn(move || {
        let mut state = backend::OnlyForward { st: stype_backend };
        loop {
            match to_backend_rx.recv() {
                Ok(msg) => state.handle_msg(msg, &to_ui_tx, &midi_out_tx),
                Err(_) => break,
            }
        }
    });

    // println!("{state:?}");
    match run(midi_in_tx, midi_out_rx) {
        Ok(_) => {}
        Err(err) => println!("{err}"),
    }
}

fn run(
    sender: mpsc::Sender<(u64, Vec<u8>)>,
    receiver: mpsc::Receiver<(u64, Vec<u8>)>,
) -> Result<(), Box<dyn Error>> {
    let mut midi_in = MidiInput::new("midir forwarding input")?;
    midi_in.ignore(Ignore::None);
    let midi_out = MidiOutput::new("midir forwarding output")?;

    let in_port = select_port(&midi_in, "input")?;
    println!();
    let out_port = select_port(&midi_out, "output")?;

    println!("\nOpening connections");
    let in_port_name = midi_in.port_name(&in_port)?;
    let out_port_name = midi_out.port_name(&out_port)?;

    let mut conn_out = midi_out.connect(&out_port, "midir-forward")?;

    // _conn_in needs to be a named parameter, because it needs to be kept alive until the end of the scope
    let _conn_in = midi_in.connect(
        &in_port,
        "midir-forward",
        move |time, msg, _| {
            // send will only fail if the receiver has disconnected. In that case, there's nothing
            // we can do from inside this thread, so we ignore the error. Likely, this will only
            // happen close to the termination of a regular run of the program.
            sender.send((time, msg.to_vec())).unwrap_or(());
        },
        (),
    )?;

    thread::spawn(move || loop {
        match receiver.recv() {
            Ok((time, msg)) => {
                // no error checking here, we assume that the messages are corect.
                conn_out.send(&msg).unwrap_or(());
                println!("{}: {:?} (len = {})", time, msg, msg.len());
            }
            Err(_) => break,
        }
    });

    println!(
        "Connections open, forwarding from '{}' to '{}' (press enter to exit) ...",
        in_port_name, out_port_name
    );

    let mut input = String::new();
    stdin().read_line(&mut input)?; // wait for next enter key press

    println!("Closing connections");
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
