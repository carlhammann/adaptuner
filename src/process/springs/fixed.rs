use std::{
    collections::HashSet,
    hash::Hash,
    sync::{mpsc, Arc},
    time::Instant,
};

use midi_msg::{ChannelVoiceMsg, MidiMsg};
use num_rational::Ratio;

use super::{
    solver::Solver,
    util::{self, Connector, KeyDistance, KeyNumber, RodSpec},
};
use crate::{
    config::r#trait::Config,
    interval::{
        stack::Stack,
        stacktype::{
            fivelimit::ConcreteFiveLimitStackType,
            r#trait::{StackCoeff, StackType},
        },
    },
    msg,
    process::r#trait::ProcessState,
};

pub struct State<T: StackType, P: Provider<T>> {
    active_keys: Vec<u8>, // sorted descendingly
    solver: Solver,
    workspace: util::Workspace<T>,
    provider: P,
}

pub trait Provider<T: StackType> {
    fn candidate_springs(&self, d: KeyDistance) -> Vec<(Stack<T>, Ratio<StackCoeff>)>;
    fn candidate_anchors(&self, k: KeyNumber) -> Vec<(Stack<T>, Ratio<StackCoeff>)>;
    fn rod(&self, d: &RodSpec) -> Stack<T>;
    fn which_connector(&self, keys: &[KeyNumber], i: usize, j: usize) -> Connector;
}

pub struct ConcreteFiveLimitProvider {}

impl Provider<ConcreteFiveLimitStackType> for ConcreteFiveLimitProvider {
    fn candidate_springs(
        &self,
        d: KeyDistance,
    ) -> Vec<(Stack<ConcreteFiveLimitStackType>, Ratio<StackCoeff>)> {
        let octaves = (d as StackCoeff).div_euclid(12);
        let pitch_class = d.rem_euclid(12);

        match pitch_class {
            0 => vec![(Stack::from_target(vec![octaves, 0, 0]), 1.into())],
            1 => vec![
                (
                    Stack::from_target(vec![octaves + 1, (-1), (-1)]), // diatonic semitone
                    Ratio::new(1, 3 * 5),
                ),
                (
                    Stack::from_target(vec![octaves, (-1), 2]), // chromatic semitone
                    Ratio::new(1, 3 * 5 * 5),
                ),
            ],
            2 => vec![
                (
                    Stack::from_target(vec![octaves - 1, 2, 0]), // major whole tone 9/8
                    Ratio::new(1, 3 * 3),
                ),
                (
                    Stack::from_target(vec![octaves + 1, (-2), 1]), // minor whole tone 10/9
                    Ratio::new(1, 3 * 3 * 5),
                ),
            ],
            3 => vec![(
                Stack::from_target(vec![octaves, 1, (-1)]), // minor third
                Ratio::new(1, 3 * 5),
            )],
            4 => vec![(
                Stack::from_target(vec![octaves, 0, 1]), // major third
                Ratio::new(1, 5),
            )],
            5 => vec![(
                Stack::from_target(vec![octaves + 1, (-1), 0]), // fourth
                Ratio::new(1, 3),
            )],
            6 => vec![
                (
                    Stack::from_target(vec![octaves - 1, 2, 1]), // tritone as major tone plus major third
                    Ratio::new(1, 3 * 3 * 5),
                ),
                (
                    Stack::from_target(vec![octaves, 2, (-2)]), // tritone as chromatic semitone below fifth
                    Ratio::new(1, 3 * 3 * 5 * 5),
                ),
            ],
            7 => vec![(
                Stack::from_target(vec![octaves, 1, 0]), // fifth
                Ratio::new(1, 3),
            )],
            8 => vec![(
                Stack::from_target(vec![octaves + 1, 0, (-1)]), // minor sixth
                Ratio::new(1, 5),
            )],
            9 => vec![
                (
                    Stack::from_target(vec![octaves + 1, (-1), 1]), // major sixth
                    Ratio::new(1, 3 * 5),
                ),
                (
                    Stack::from_target(vec![octaves - 1, 3, 0]), // major tone plus fifth
                    Ratio::new(1, 3 * 3 * 3),
                ),
            ],
            10 => vec![
                (
                    Stack::from_target(vec![octaves + 2, (-2), 0]), // minor seventh as stack of two fourths
                    Ratio::new(1, 3 * 3),
                ),
                (
                    Stack::from_target(vec![octaves, 2, (-1)]), // minor seventh as fifth plus minor third
                    Ratio::new(1, 3 * 3 * 5),
                ),
            ],
            11 => vec![(
                Stack::from_target(vec![octaves, 1, 1]), // major seventh as fifth plus major third
                Ratio::new(1, 3 * 5),
            )],
            _ => unreachable!(),
        }
    }

    fn candidate_anchors(
        &self,
        k: KeyNumber,
    ) -> Vec<(Stack<ConcreteFiveLimitStackType>, Ratio<StackCoeff>)> {
        self.candidate_springs(k as KeyDistance - 60)
    }

    fn rod(&self, d: &RodSpec) -> Stack<ConcreteFiveLimitStackType> {
        match d[..] {
            [(12, n)] => Stack::from_target(vec![n, 0, 0]),
            _ => {
                println!("{d:?}");
                panic!();
            }
        }
    }

    fn which_connector(&self, keys: &[KeyNumber], i: usize, j: usize) -> Connector {
        //let d = (keys[i] as KeyDistance - keys[j] as KeyDistance).abs();
        let class = (keys[i] as KeyDistance - keys[j] as KeyDistance).abs() % 12;

        // octaves
        if class == 0 {
            return Connector::Rod(vec![(
                12,
                (keys[j] as StackCoeff - keys[i] as StackCoeff) / 12,
            )]);
        }

        if keys.len() <= 5 {
            // This means at most 32 interval candidates. That's manageable.
            return Connector::Spring;
        }

        //if i == 0 {
        //    return Connector::Spring;
        //}

        if i + 1 == j {
            return Connector::Spring;
        }

        // fifths, minor thirds, major thirds, and major seconds (and their octave complements)
        if [7, 5, 3, 9, 4, 8, 2, 10].contains(&class) {
            return Connector::Spring;
        }

        Connector::None
    }
}

impl<T: StackType + Hash + Eq + std::fmt::Debug, P: Provider<T>> State<T, P> {
    fn retune(
        &mut self,
        time: Instant,
        to_backend: &mpsc::Sender<(Instant, msg::AfterProcess<T>)>,
    ) {
        let send_to_backend =
            |msg: msg::AfterProcess<T>, time: Instant| to_backend.send((time, msg)).unwrap_or(());

        match self.workspace.compute_best_intervals(
            &self.active_keys,
            |k, i, j| self.provider.which_connector(k, i, j),
            |d| self.provider.candidate_springs(d),
            |d| self.provider.rod(d),
            &mut self.solver,
        ) {
            Ok(()) => {}
            Err(e) => {
                send_to_backend(
                    msg::AfterProcess::Notify {
                        line: format!("while computing the optimal intervals: {:?}", e),
                    },
                    time,
                );
                return;
            }
        }

        let springs_relaxed = self.workspace.relaxed();
        let springs_energy = self.workspace.current_energy();

        match self.workspace.compute_best_anchoring(
            &[0],
            |k| self.provider.candidate_anchors(k),
            &mut self.solver,
        ) {
            Ok(()) => {}
            Err(e) => {
                send_to_backend(
                    msg::AfterProcess::Notify {
                        line: format!("while computing the optimal absolute position: {:?}", e),
                    },
                    time,
                );
                return;
            }
        }
        
        let anchors_relaxed = self.workspace.relaxed();
        let anchors_energy = self.workspace.current_energy();

        match self.workspace.compute_best_solution(
            &self.active_keys,
            |k| k == *self.active_keys.last().unwrap(),
            |k, i, j| self.provider.which_connector(k, i, j),
            |d| self.provider.candidate_springs(d),
            |k| self.provider.candidate_anchors(k),
            |d| self.provider.rod(d),
            &mut self.solver,
        ) {
            Ok(()) => {}
            Err(e) => send_to_backend(
                msg::AfterProcess::Notify {
                    line: format!("{:?}", e),
                },
                time,
            ),
        };

        let dummy: Arc<HashSet<_>> = Arc::new([Stack::new_zero()].into());
        //if !self.workspace.relaxed() {
        //    self.workspace.update_anchor_options();
        //}
        let solution = self.workspace.current_solution();
        for (i, r) in solution.rows().into_iter().enumerate() {
            send_to_backend(
                msg::AfterProcess::Retune {
                    note: self.active_keys[i],
                    tuning: self.workspace.get_semitones(i),

                    tuning_stack_actual: r.to_owned(),
                    //tuning_stack_targets: self.workspace.get_anchor_options(i),
                    tuning_stack_targets: dummy.clone(),
                },
                time,
            );
        }
    }
}

impl<T: StackType + Eq + Hash + std::fmt::Debug, P: Provider<T>> State<T, P> {
    fn handle_midi(
        &mut self,
        time: Instant,
        bytes: &[u8],
        to_backend: &mpsc::Sender<(Instant, msg::AfterProcess<T>)>,
    ) {
        let send_to_backend =
            |msg: msg::AfterProcess<T>, time: Instant| to_backend.send((time, msg)).unwrap_or(());

        match MidiMsg::from_midi(&bytes) {
            Err(e) => send_to_backend(msg::AfterProcess::MidiParseErr(e.to_string()), time),
            Ok((msg, _number_of_bytes_parsed)) => match msg {
                MidiMsg::ChannelVoice {
                    channel,
                    msg: ChannelVoiceMsg::NoteOn { note, velocity },
                } => {
                    send_to_backend(
                        msg::AfterProcess::NoteOn {
                            channel,
                            note,
                            velocity,
                        },
                        time,
                    );
                    if !self.active_keys.contains(&note) {
                        self.active_keys.push(note);
                        self.active_keys.sort_by(|a, b| a.cmp(b).reverse());
                        self.retune(time, to_backend);
                    }
                }

                MidiMsg::ChannelVoice {
                    channel,
                    msg: ChannelVoiceMsg::NoteOff { note, velocity },
                } => {
                    send_to_backend(
                        msg::AfterProcess::NoteOff {
                            held_by_sustain: false, // TODO.
                            channel,
                            note,
                            velocity,
                        },
                        time,
                    );
                    match self.active_keys.iter().position(|x| *x == note) {
                        None {} => {}
                        Some(i) => {
                            self.active_keys.remove(i);
                            if self.active_keys.len() > 0 {
                                self.retune(time, to_backend);
                            }
                        }
                    }
                }

                //MidiMsg::ChannelVoice {
                //    channel,
                //    msg:
                //        ChannelVoiceMsg::ControlChange {
                //            control: ControlChange::Hold(value),
                //        },
                //} => {}
                //MidiMsg::ChannelVoice {
                //    channel,
                //    msg: ChannelVoiceMsg::ProgramChange { program },
                //} => {
                //    send_to_backend(msg::AfterProcess::ProgramChange { channel, program }, time);
                //}
                _ => send_to_backend(msg::AfterProcess::ForwardMidi { msg }, time),
            },
        }
    }
}

impl<T: StackType + Eq + Hash + std::fmt::Debug, P: Provider<T>> ProcessState<T> for State<T, P> {
    fn handle_msg(
        &mut self,
        time: Instant,
        msg: msg::ToProcess,
        to_backend: &mpsc::Sender<(Instant, msg::AfterProcess<T>)>,
    ) {
        match msg {
            msg::ToProcess::Start => {}
            msg::ToProcess::Stop => {}
            msg::ToProcess::Reset => {}
            msg::ToProcess::IncomingMidi { bytes } => self.handle_midi(time, &bytes, to_backend),
            msg::ToProcess::Consider { coefficients: _ } => {}
            msg::ToProcess::ToggleTemperament { index: _ } => {}
            msg::ToProcess::Special { code: _ } => {}
        }
    }
}

pub struct FooConfig {
    pub initial_n_keys: usize,
    pub initial_n_lengths: usize,
}

impl Config<State<ConcreteFiveLimitStackType, ConcreteFiveLimitProvider>> for FooConfig {
    fn initialise(config: &Self) -> State<ConcreteFiveLimitStackType, ConcreteFiveLimitProvider> {
        State {
            active_keys: vec![],
            solver: Solver::new(config.initial_n_keys, config.initial_n_lengths, 3),
            workspace: util::Workspace::new(config.initial_n_keys, true, true, true),
            provider: ConcreteFiveLimitProvider {},
        }
    }
}
