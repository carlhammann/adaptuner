use std::{
    cell::RefCell,
    collections::{BTreeMap, HashMap, VecDeque},
    rc::Rc,
    time::{Duration, Instant},
};

use ndarray::{s, Array2, ArrayView1, ArrayView2, ArrayViewMut2};
use num_rational::Ratio;
use serde_derive::{Deserialize, Serialize};

use crate::{
    config::{ExtractConfig, HarmonyStrategyConfig},
    custom_serde::common::{deserialize_nonempty, deserialize_ratio, serialize_ratio},
    interval::{
        base::Semitones,
        stack::{semitones_from_actual, ScaledAdd, Stack},
        stacktype::r#trait::{IntervalBasis, StackCoeff, StackType},
    },
    keystate::KeyState,
    msg::{FromStrategy, ToHarmonyStrategy},
    neighbourhood::{Neighbourhood, Partial, SomeNeighbourhood},
    strategy::{
        r#trait::StrategyAction,
        twostep::{Harmony, HarmonyStrategy},
    },
    util::springs::Solver,
};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "kebab-case")]
#[derive(Clone)]
pub struct Spring<T: IntervalBasis> {
    length: Stack<T>,
    #[serde(
        deserialize_with = "deserialize_ratio",
        serialize_with = "serialize_ratio"
    )]
    stiffness: Ratio<StackCoeff>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "kebab-case")]
#[derive(Clone)]
pub enum RodOrSprings<T: IntervalBasis> {
    Rod(Stack<T>),
    #[serde(deserialize_with = "deserialize_nonempty_springs")]
    Springs(Vec<Spring<T>>),
}

fn deserialize_nonempty_springs<
    'de,
    D: serde::Deserializer<'de>,
    T: IntervalBasis + serde::Deserialize<'de>,
>(
    deserializer: D,
) -> Result<Vec<Spring<T>>, D::Error> {
    deserialize_nonempty("expected a non-empty list of springs", deserializer)
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "kebab-case")]
#[derive(Clone)]
pub enum HarmonySpringsProvider<T: IntervalBasis> {
    #[serde(rename_all = "kebab-case")]
    Mod12 {
        by_class: [RodOrSprings<T>; 12],
        octave: Stack<T>,
    },
}

impl<T: IntervalBasis> HarmonySpringsProvider<T> {
    fn which_connector(&self, i: (usize, u8), j: (usize, u8)) -> Connector<T> {
        match self {
            HarmonySpringsProvider::Mod12 { by_class, octave } => {
                let d = j.1 as i8 - i.1 as i8;
                let rem = d.rem_euclid(12) as usize;
                match &by_class[rem] {
                    RodOrSprings::Rod(stack) => {
                        let quot = d.div_euclid(12) as StackCoeff;
                        let mut rod = stack.clone();
                        rod.scaled_add(quot, octave);
                        Connector::Rod(rod)
                    }
                    RodOrSprings::Springs(_) => Connector::Spring,
                }
            }
        }
    }

    fn candidate_springs(&self, d: i8) -> Vec<(Stack<T>, Ratio<StackCoeff>)> {
        match self {
            HarmonySpringsProvider::Mod12 { by_class, octave } => {
                let rem = d.rem_euclid(12) as usize;
                match &by_class[rem] {
                    RodOrSprings::Rod(_) => panic!("cannot compute candidate_springs for rod"),
                    RodOrSprings::Springs(springs) => {
                        let quot = d.div_euclid(12) as StackCoeff;
                        springs
                            .iter()
                            .map(
                                |Spring {
                                     length: stack,
                                     stiffness,
                                 }| {
                                    let mut shifted_stack = stack.clone();
                                    shifted_stack.scaled_add(quot, octave);
                                    (shifted_stack, *stiffness)
                                },
                            )
                            .collect()
                    }
                }
            }
        }
    }
}
pub enum Connector<T: IntervalBasis> {
    Spring,
    Rod(Stack<T>),
    None,
}

#[derive(Debug)]
struct SpringInfo {
    solver_length_index: usize,
    memo_key: i8,
    current_candidate_index: usize,
}

type Energy = Semitones;

struct SpringSetup<T: IntervalBasis> {
    memoed_springs: HashMap<i8, Vec<(Stack<T>, Ratio<StackCoeff>)>>,
    /// invariant: the key tuples are two distinct numbers, with the smaller one first
    current_springs: BTreeMap<(usize, usize), SpringInfo>,
    /// invariant: the key tuples are two distinct numbers, with the smaller one first
    current_rods: BTreeMap<(usize, usize), Stack<T>>,
}

pub struct HarmonySprings<T: IntervalBasis> {
    keys: Vec<u8>,
    memo_springs: bool,

    spring_setup: SpringSetup<T>,
    solver: Solver,

    minimum_number_of_sounding_keys: usize,
    lower_notes_are_more_stable: bool,
    provider: HarmonySpringsProvider<T>,

    timeout: Duration,

    solve_start_time: Instant,
    relaxed: bool,
    energy: Energy,
    solution_actuals: Array2<Ratio<StackCoeff>>,

    solution_interval_targets: Array2<StackCoeff>,
    solution_target_is_set: Vec<bool>,

    /// will always be a [SomeNeighbourhood::Partial]
    solution_neighbourhood: Rc<RefCell<SomeNeighbourhood<T>>>,
}

#[derive(Clone)]
pub struct HarmonySpringsConfig<T: IntervalBasis> {
    /// should the [HarmonySpringsProvider::candidate_springs] be memoised between different calls
    /// to [HarmonyStrategy::solve]?
    pub memo_springs: bool,
    pub minimum_number_of_sounding_keys: usize,
    pub lower_notes_are_more_stable: bool,
    pub provider: HarmonySpringsProvider<T>,
    pub timeout_ms: u64,
}

impl<T: IntervalBasis> SpringSetup<T> {
    fn new() -> Self {
        Self {
            memoed_springs: HashMap::new(),
            current_springs: BTreeMap::new(),
            current_rods: BTreeMap::new(),
        }
    }

    fn n_springs(&self) -> usize {
        self.current_springs.len()
    }

    fn n_rods(&self) -> usize {
        self.current_rods.len()
    }

    fn iter_current_rods(&self) -> impl Iterator<Item = (&(usize, usize), &Stack<T>)> {
        self.current_rods.iter()
    }

    /// returns ((start_node_index, end_node_index), solver_length_index, stack, stiffness)
    fn iter_current_springs(
        &self,
    ) -> impl Iterator<Item = (&(usize, usize), usize, &Stack<T>, &Ratio<StackCoeff>)> {
        self.current_springs.iter().map(|(ix, spring_info)| {
            let (stack, stiffness) = &self
                .memoed_springs
                .get(&spring_info.memo_key)
                .expect("iter_current_springs: no candidates found for spring")
                [spring_info.current_candidate_index];
            (ix, spring_info.solver_length_index, stack, stiffness)
        })
    }

    fn collect_springs_and_rods(
        &mut self,
        keys: &[u8],
        which_connector: impl Fn((usize, u8), (usize, u8)) -> Connector<T>,
        candidate_springs: impl Fn(i8) -> Vec<(Stack<T>, Ratio<StackCoeff>)>,
        memo_springs: bool,
    ) {
        self.current_rods.clear();
        self.current_springs.clear();

        if !memo_springs {
            self.memoed_springs.clear();
        }

        let n = keys.len();

        let mut spring_index = 0;

        for i in 0..n {
            for j in (i + 1)..n {
                match which_connector((i, keys[i]), (j, keys[j])) {
                    Connector::Spring => {
                        let d = keys[j] as i8 - keys[i] as i8;
                        if !self.memoed_springs.contains_key(&d) {
                            self.memoed_springs.insert(d, candidate_springs(d));
                        }
                        self.current_springs.insert(
                            (i, j),
                            SpringInfo {
                                current_candidate_index: 0,
                                memo_key: d,
                                solver_length_index: spring_index,
                            },
                        );
                        spring_index += 1;
                    }
                    Connector::Rod(stack) => {
                        self.current_rods.insert((i, j), stack);
                    }
                    Connector::None => {}
                }
            }
        }

        // This triple loop ensures the invariant of [solver::System::add_rod]
        for k in (0..n).rev() {
            for j in (0..k).rev() {
                for i in (0..j).rev() {
                    match self.current_rods.remove(&(j, k)) {
                        None {} => {}
                        Some(b) => match (
                            self.current_rods.get(&(i, j)),
                            self.current_rods.get(&(i, k)),
                        ) {
                            (None {}, None {}) => {
                                // put it back: we can't delete information
                                self.current_rods.insert((j, k), b);
                            }
                            (Some(a), None {}) => {
                                // now we have a chain like
                                //
                                //     a       b
                                // i ----- j ----- k
                                //
                                // which we'll replace by
                                //
                                //     a
                                // i ----- j       k
                                //   --------------
                                //       a+b
                                let mut b_plus_a = b;
                                b_plus_a.scaled_add(1, a);
                                self.current_rods.insert((i, k), b_plus_a);
                            }
                            (None {}, Some(c)) => {
                                // now we have a chain like
                                //
                                //             b
                                // i       j ----- k
                                //   -------------
                                //         c
                                //
                                // which we'll replace by
                                //
                                //    c-b
                                // i ----- j       k
                                //   --------------
                                //        c
                                let mut c_minus_b = b;
                                c_minus_b.scale(-1);
                                c_minus_b.scaled_add(1, c);
                                self.current_rods.insert((i, j), c_minus_b);
                            }
                            (Some(_a), Some(_c)) => {
                                // nothing left to do: the information in `b` is redundant with the
                                // information in `a` and `c`, since i,j,k are collinear
                            }
                        },
                    }
                }
            }
        }
    }

    /// returns true iff the next candidate was prepared, false iff there are no more candidates.
    fn prepare_next_candidate(&mut self, change_from_the_back: bool) -> bool {
        let springs_iter: Box<dyn Iterator<Item = _>> = if change_from_the_back {
            Box::new(self.current_springs.iter_mut().rev())
        } else {
            Box::new(self.current_springs.iter_mut())
        };
        for (_, v) in springs_iter {
            let max_ix = self
                .memoed_springs
                .get(&v.memo_key)
                .expect("prepeare_next_spring_candidate: found no candidates for spring")
                .len()
                - 1;
            if v.current_candidate_index < max_ix {
                v.current_candidate_index += 1;
                return true;
            } else {
                v.current_candidate_index = 0;
            }
        }

        return false;
    }

    fn energy_in(&self, solution: ArrayView2<Ratio<StackCoeff>>) -> Energy {
        let compute_length = |coeffs: ArrayView1<Ratio<StackCoeff>>| {
            let mut res = 0.0;
            for (j, c) in coeffs.iter().enumerate() {
                res += T::intervals()[j].semitones * *c.numer() as Energy / *c.denom() as Energy;
            }
            res
        };

        let mut energy = 0.0;

        for ((i, j), v) in self.current_springs.iter() {
            let (stack, stiffness) = &self
                .memoed_springs
                .get(&v.memo_key)
                .expect("energy_in: no candidates found for spring.")[v.current_candidate_index];
            let length = compute_length(stack.actual_coefficients());
            if *stiffness != Ratio::ZERO {
                energy += *stiffness.numer() as Energy / *stiffness.denom() as Energy
                    * (length - relative_semitones_in_solution_rows::<T>(*i, *j, solution)).powi(2);
            }
        }

        energy
    }

    fn relaxed_in(&self, solution: ArrayView2<Ratio<StackCoeff>>) -> bool {
        for ((i, j), v) in self.current_springs.iter() {
            let (stack, _) = &self
                .memoed_springs
                .get(&v.memo_key)
                .expect("relaxed_in: no candidates found for spring.")[v.current_candidate_index];
            for k in 0..T::num_intervals() {
                if stack.actual_coefficients()[k] != solution[[*j, k]] - solution[[*i, k]] {
                    return false;
                }
            }
        }
        true
    }
}

fn relative_semitones_in_solution_rows<T: IntervalBasis>(
    i: usize,
    j: usize,
    solution: ArrayView2<Ratio<StackCoeff>>,
) -> Semitones {
    semitones_from_actual::<T>(solution.row(j)) - semitones_from_actual::<T>(solution.row(i))
}

impl<T: IntervalBasis> HarmonySprings<T> {
    pub fn new(config: HarmonySpringsConfig<T>) -> Self {
        let n = 10; // initial guess at how many keys we're playing simulatneously: both hands full.
        let big_n = n * (n - 1) / 2;
        Self {
            keys: Vec::with_capacity(n),
            memo_springs: config.memo_springs,
            spring_setup: SpringSetup::new(),
            solver: Solver::new(n, big_n, T::num_intervals()),
            minimum_number_of_sounding_keys: config.minimum_number_of_sounding_keys,
            lower_notes_are_more_stable: config.lower_notes_are_more_stable,
            provider: config.provider,
            timeout: Duration::from_millis(config.timeout_ms),
            solve_start_time: Instant::now(),
            relaxed: false,
            energy: Energy::MAX,
            solution_actuals: Array2::zeros((n, T::num_intervals())),
            solution_interval_targets: Array2::zeros((big_n, T::num_intervals())),
            solution_target_is_set: vec![false; big_n],
            solution_neighbourhood: Rc::new(RefCell::new(SomeNeighbourhood::Partial(
                Partial::new(),
            ))),
        }
    }

    fn initialise(&mut self, keys: &[KeyState; 128]) {
        self.keys.clear();
        keys.iter().enumerate().for_each(|(i, k)| {
            if k.is_sounding() {
                self.keys.push(i as u8)
            }
        });

        self.spring_setup.collect_springs_and_rods(
            &self.keys,
            |i, j| self.provider.which_connector(i, j),
            |d| self.provider.candidate_springs(d),
            self.memo_springs,
        );

        self.solve_start_time = Instant::now();
        self.relaxed = false;
        self.energy = Semitones::MAX;
        // no need to initialise `self.solution_actuals`, it will be overwritten anyway
    }

    /// returns true iff a solution was successfully computed
    fn compute_solution_actuals(&mut self) -> bool {
        let n_nodes = self.keys.len();
        let n_springs = self.spring_setup.n_springs();
        let n_rods = self.spring_setup.n_rods();
        let n_lengths = n_springs + n_rods + 1; // +1 for the anchor that fixes the first key to zero
        let n_base_lengths = T::num_intervals();

        self.solver
            .prepare_system(n_nodes, n_lengths, n_base_lengths);

        // first, add the springs, as their [SpringInfo::solver_length_index]es start at 0
        for ((i, j), solver_length_index, stack, stiffness) in
            self.spring_setup.iter_current_springs()
        {
            self.solver
                .add_spring(*i, *j, solver_length_index, *stiffness);
            self.solver
                .define_length(solver_length_index, stack.actual_coefficients());
        }

        // now, add the rods
        let mut solver_length_index = n_springs;
        for ((i, j), stack) in self.spring_setup.iter_current_rods() {
            self.solver.add_rod(*i, *j, solver_length_index);
            let length = stack.actual_coefficients();
            self.solver.define_length(solver_length_index, length);
            solver_length_index += 1;
        }

        // finally, anchor the lowest key to zero
        self.solver.define_zero_length(solver_length_index);
        self.solver
            .add_fixed_spring(0, solver_length_index, 1.into());

        if let Ok(solution) = self.solver.solve() {
            let mut copy_solution = false;
            if self.spring_setup.relaxed_in(solution) {
                self.relaxed = true;
                self.energy = 0.0;
                copy_solution = true;
            } else {
                let new_energy = self.spring_setup.energy_in(solution);
                if new_energy < self.energy {
                    self.energy = new_energy;
                    copy_solution = true;
                }
            }
            if copy_solution {
                let n = solution.shape()[0]; // == self.keys.len()
                if n > self.solution_actuals.shape()[0] {
                    self.solution_actuals = Array2::zeros((n, T::num_intervals()));
                }
                self.solution_actuals
                    .slice_mut(s![0..n, ..])
                    .assign(&solution);
            }
            true
        } else {
            false
        }
    }

    /// Computes the [Self::solution_interval_targets] for the [Self::solution_actuals] solution.
    ///
    /// The order of the intervals in [Self::solution_interval_targets] is such that the interval
    /// from the `i`-th to the `j`-th note, where `0 <= i < j`, is stored at the index computed by
    ///
    /// `
    /// let index = |i, j| n * i - i * (i + 1) / 2 + j - i - 1;
    /// `
    ///
    /// This allows easy iteration with nested loops like
    /// `
    /// let targets = ws.current_interval_targets();
    /// let index = 0;
    /// for i = 0..n {
    ///    for j = (i + 1)..n {
    ///       // targets[index] is now the interval from note `i` to note `j`
    ///       index += 1;
    ///    }
    /// }
    /// `
    ///
    /// If there are no tensioned springs, the computed target intervals correspond directly to the
    /// intervals in the [Self::solution_actuals]. Otherwise, there is no "always correct choice"
    /// to guess the intended non-detuned intervals. These choices are made:
    ///
    /// - every interval that is fixed by a rod or a combination of rods will be kept.
    /// - springs and rods that come between more "stable" notes (i.e. the ones that come last in
    ///   [Self::keys]) are preferred.
    ///
    /// expected invariants:
    /// - No zero intervals, i.e. every note occurs at most once.
    /// - Nothing is called between the computation of the [Self::solution_actuals] and this function.
    fn compute_solution_interval_targets(&mut self) {
        let n = self.keys.len();
        let big_n = n * (n - 1) / 2;

        if big_n > self.solution_interval_targets.shape()[0] {
            self.solution_interval_targets = Array2::zeros((big_n, T::num_intervals()));
            self.solution_target_is_set = vec![false; big_n];
        } else {
            for i in 0..big_n {
                self.solution_target_is_set[i] = false;
            }
        }

        let index = |i, j| n * i - i * (i + 1) / 2 + j - i - 1;

        let complete = |mut targets: ArrayViewMut2<StackCoeff>, is_set: &mut Vec<bool>| {
            for i in 0..n {
                for j in (i + 1)..n {
                    for k in (j + 1)..n {
                        let ij = index(i, j);
                        let jk = index(j, k);
                        let ik = index(i, k);
                        let (mut a, mut b, mut c) =
                            targets.multi_slice_mut((s![ij, ..], s![jk, ..], s![ik, ..]));

                        match (is_set[ij], is_set[jk], is_set[ik]) {
                            (false, true, true) => {
                                a.assign(&c);
                                a.scaled_add(-1, &b);
                                is_set[ij] = true;
                            }
                            (true, false, true) => {
                                b.assign(&c);
                                b.scaled_add(-1, &a);
                                is_set[jk] = true;
                            }
                            (true, true, false) => {
                                c.assign(&a);
                                c.scaled_add(1, &b);
                                is_set[ik] = true;
                            }
                            _ => {}
                        }
                    }
                }
            }
        };

        // Let's iterate through this back to front: This will prefer the connections between more
        // "stable" notes
        for ((i, j), stack) in self.spring_setup.iter_current_rods() {
            let ij = index(*i, *j);
            if !self.solution_target_is_set[ij] {
                self.solution_interval_targets
                    .row_mut(ij)
                    .assign(&stack.target);
                self.solution_target_is_set[ij] = true;
            }
        }

        complete(
            self.solution_interval_targets.view_mut(),
            &mut self.solution_target_is_set,
        );

        // Again, back to front. Also: after the rods have been completed.
        for ((i, j), _solver_length_index, stack, _stiffness) in
            self.spring_setup.iter_current_springs()
        {
            let ij = index(*i, *j);
            if !self.solution_target_is_set[ij] {
                self.solution_interval_targets
                    .row_mut(ij)
                    .assign(&stack.target);
                self.solution_target_is_set[ij] = true;
            }
        }

        complete(
            self.solution_interval_targets.view_mut(),
            &mut self.solution_target_is_set,
        );
    }
}

impl<T: StackType> HarmonyStrategy<T> for HarmonySprings<T> {
    fn solve(&mut self, keys: &[KeyState; 128]) -> (Option<usize>, Option<Harmony<T>>) {
        self.initialise(keys);

        if self.keys.len() < self.minimum_number_of_sounding_keys {
            return (None {}, None {})
        }

        let mut computed_at_least_one_solution = self.compute_solution_actuals();
        while Instant::now().duration_since(self.solve_start_time) <= self.timeout {
            if self.relaxed {
                break;
            }
            if !self
                .spring_setup
                .prepare_next_candidate(self.lower_notes_are_more_stable)
            {
                break;
            }
            computed_at_least_one_solution |= self.compute_solution_actuals();
        }

        if !computed_at_least_one_solution {
            return (None {}, None {});
        }

        self.compute_solution_interval_targets();

        // this will always work, since self.solution_neighbourhood is a [SomeNeighbourhood::Partial]
        self.solution_neighbourhood.borrow_mut().clear();
        self.solution_neighbourhood.borrow_mut().insert_zero();
        for i in 1..self.keys.len() {
            self.solution_neighbourhood
                .borrow_mut()
                .insert_target_actual(
                    self.solution_interval_targets.row(i - 1),
                    self.solution_actuals.row(i),
                );
        }

        (
            None {},
            Some(Harmony {
                neighbourhood: self.solution_neighbourhood.clone(),
                reference: self.keys[0] as StackCoeff,
            }),
        )
    }

    fn handle_msg(&mut self, msg: ToHarmonyStrategy<T>) -> bool {
        match msg {
            ToHarmonyStrategy::ChordListAction { .. } => {}
            ToHarmonyStrategy::PushNewChord { .. } => {}
            ToHarmonyStrategy::AllowExtraHighNotes { .. } => {}
            ToHarmonyStrategy::EnableChordList { .. } => {}
        }
        true
    }

    fn handle_action(&mut self, action: StrategyAction, _forward: &mut VecDeque<FromStrategy<T>>) {
        match action {
            StrategyAction::IncrementNeighbourhoodIndex(_) => {}
            StrategyAction::SetReferenceToLowest => {}
            StrategyAction::SetReferenceToHighest => {}
            StrategyAction::SetReferenceToCurrent => {}
            StrategyAction::ToggleChordMatching => {}
            StrategyAction::ToggleReanchor => {}
            StrategyAction::Reset => todo!(),
        }
    }
}

impl<T: StackType> ExtractConfig<HarmonyStrategyConfig<T>> for HarmonySprings<T> {
    fn extract_config(&self) -> HarmonyStrategyConfig<T> {
        HarmonyStrategyConfig::Springs(HarmonySpringsConfig {
            memo_springs: self.memo_springs,
            minimum_number_of_sounding_keys: self.minimum_number_of_sounding_keys,
            lower_notes_are_more_stable: self.lower_notes_are_more_stable,
            provider: self.provider.clone(),
            timeout_ms: self.timeout.as_millis() as u64,
        })
    }
}

#[cfg(test)]
mod test {
    use approx::abs_diff_eq;
    use midi_msg::Channel;
    use ndarray::{arr1, arr2};
    use pretty_assertions::assert_eq;

    use crate::interval::stacktype::fivelimit::mock::MockFiveLimitStackType;

    use super::*;

    #[test]
    fn test_collect_springs_and_rods() {
        let mut ws = SpringSetup::<MockFiveLimitStackType>::new();

        ws.collect_springs_and_rods(
            &[0, 1, 2, 3],
            |(i, _), (j, _)| {
                Connector::Rod(Stack::from_pure_interval(
                    0,
                    j as StackCoeff - i as StackCoeff,
                ))
            },
            |_| panic!("This will not be called, since there are no springs!"),
            false,
        );
        assert_eq!(
            ws.current_rods.iter().collect::<Vec<_>>(),
            vec![
                (&(0, 1), &Stack::from_pure_interval(0, 1)),
                (&(0, 2), &Stack::from_pure_interval(0, 2)),
                (&(0, 3), &Stack::from_pure_interval(0, 3)),
            ]
        );

        ws.collect_springs_and_rods(
            &[0, 1, 2, 3, 4, 5],
            |(i, _), (j, _)| {
                if (j - i) % 2 == 0 {
                    Connector::Rod(Stack::from_pure_interval(
                        0,
                        j as StackCoeff - i as StackCoeff,
                    ))
                } else {
                    Connector::Spring
                }
            },
            |_| vec![], // irrelevant
            false,
        );
        assert_eq!(
            ws.current_rods.iter().collect::<Vec<_>>(),
            vec![
                (&(0, 2), &Stack::from_pure_interval(0, 2)),
                (&(0, 4), &Stack::from_pure_interval(0, 4)),
                (&(1, 3), &Stack::from_pure_interval(0, 2)),
                (&(1, 5), &Stack::from_pure_interval(0, 4)),
            ]
        );
        assert_eq!(
            ws.current_springs
                .iter()
                .map(|(i, _)| i)
                .collect::<Vec<_>>(),
            vec![
                &(0, 1),
                &(0, 3),
                &(0, 5),
                &(1, 2),
                &(1, 4),
                &(2, 3),
                &(2, 5),
                &(3, 4),
                &(4, 5)
            ]
        );

        ws.collect_springs_and_rods(
            &[0, 2, 5, 7, 12, 14],
            |(i, ki), (j, kj)| {
                let d = kj as StackCoeff - ki as StackCoeff;
                if d % 12 == 0 {
                    Connector::Rod(Stack::from_pure_interval(0, d / 12))
                } else if d % 7 == 0 {
                    Connector::Rod(Stack::from_pure_interval(1, d / 7))
                } else if j - i >= 3 {
                    Connector::Spring
                } else {
                    Connector::None
                }
            },
            |_| vec![], // irrelevant
            false,
        );
        assert_eq!(
            ws.current_rods
                .iter()
                .map(|(i, s)| (i, &s.target))
                .collect::<Vec<_>>(),
            vec![
                (&(0, 1), &arr1(&[-1, 2, 0])),
                (&(0, 2), &arr1(&[1, -1, 0])),
                (&(0, 3), &arr1(&[0, 1, 0])),
                (&(0, 4), &arr1(&[1, 0, 0])),
                (&(0, 5), &arr1(&[0, 2, 0])),
            ]
        );

        assert_eq!(
            ws.current_springs
                .iter()
                .map(|(i, _)| i)
                .collect::<Vec<_>>(),
            vec![&(1, 4), &(2, 5)]
        );
    }

    fn mock_provider() -> HarmonySpringsProvider<MockFiveLimitStackType> {
        HarmonySpringsProvider::Mod12 {
            by_class: [
                RodOrSprings::Rod(Stack::from_target(vec![0, 0, 0])),
                RodOrSprings::Springs(vec![
                    Spring {
                        length: Stack::from_target(vec![1, (-1), (-1)]), // diatonic semitone
                        stiffness: Ratio::new(1, 5),
                    },
                    Spring {
                        length: Stack::from_target(vec![0, (-1), 2]), // chromatic semitone
                        stiffness: Ratio::new(1, 5),
                    },
                ]),
                RodOrSprings::Springs(vec![
                    Spring {
                        length: Stack::from_target(vec![-1, 2, 0]), // major tone 9/8
                        stiffness: Ratio::new(1, 3),
                    },
                    Spring {
                        length: Stack::from_target(vec![1, -2, 1]), // minor tone 10/9
                        stiffness: Ratio::new(1, 5),
                    },
                ]),
                RodOrSprings::Springs(vec![Spring {
                    length: Stack::from_target(vec![0, 1, (-1)]), // minor third
                    stiffness: Ratio::new(1, 5),
                }]),
                RodOrSprings::Springs(vec![Spring {
                    length: Stack::from_target(vec![0, 0, 1]), // major third
                    stiffness: Ratio::new(1, 5),
                }]),
                RodOrSprings::Springs(vec![Spring {
                    length: Stack::from_target(vec![1, (-1), 0]), // fourth
                    stiffness: Ratio::new(1, 3),
                }]),
                RodOrSprings::Springs(vec![
                    Spring {
                        length: Stack::from_target(vec![-1, 2, 1]), // tritone as major tone plus major third
                        stiffness: Ratio::new(1, 5),
                    },
                    Spring {
                        length: Stack::from_target(vec![0, 2, (-2)]), // tritone as chromatic semitone below fifth
                        stiffness: Ratio::new(1, 5),
                    },
                ]),
                RodOrSprings::Springs(vec![Spring {
                    length: Stack::from_target(vec![0, 1, 0]), // fifth
                    stiffness: Ratio::new(1, 3),
                }]),
                RodOrSprings::Springs(vec![Spring {
                    length: Stack::from_target(vec![1, 0, (-1)]), // minor sixth
                    stiffness: Ratio::new(1, 5),
                }]),
                RodOrSprings::Springs(vec![
                    Spring {
                        length: Stack::from_target(vec![1, (-1), 1]), // major sixth
                        stiffness: Ratio::new(1, 5),
                    },
                    Spring {
                        length: Stack::from_target(vec![-1, 3, 0]), // major tone plus fifth
                        stiffness: Ratio::new(1, 3),
                    },
                ]),
                RodOrSprings::Springs(vec![
                    Spring {
                        length: Stack::from_target(vec![2, (-2), 0]), // minor seventh as stack of two fourths
                        stiffness: Ratio::new(1, 3),
                    },
                    Spring {
                        length: Stack::from_target(vec![0, 2, (-1)]), // minor seventh as fifth plus minor third
                        stiffness: Ratio::new(1, 5),
                    },
                ]),
                RodOrSprings::Springs(vec![Spring {
                    length: Stack::from_target(vec![0, 1, 1]), // major seventh as fifth plus major third
                    stiffness: Ratio::new(1, 5),
                }]),
            ],
            octave: Stack::from_pure_interval(0, 1),
        }
    }

    fn mock_harmony_springs() -> HarmonySprings<MockFiveLimitStackType> {
        HarmonySprings::new(HarmonySpringsConfig {
            minimum_number_of_sounding_keys: 1,
            memo_springs: true,
            lower_notes_are_more_stable: true,
            provider: mock_provider(),
            timeout_ms: u64::MAX,
        })
    }

    #[test]
    fn test_harmony_springs_solve() {
        let mut ws = mock_harmony_springs();

        let epsilon = 0.00000000000000001; // just a very small number. I don't care precisely.

        let now = Instant::now();
        let mut keys: [KeyState; 128] = core::array::from_fn(|_| KeyState::new(now));
        let clear = |keys: &mut [KeyState]| keys.iter_mut().for_each(|k| *k = KeyState::new(now));

        // if nothing else is given, the first option is picked
        clear(&mut keys);
        keys[60].note_on(Channel::Ch1, now);
        keys[66].note_on(Channel::Ch1, now);
        ws.solve(&keys);
        assert!(ws.energy < epsilon);
        assert!(ws.relaxed);
        assert_eq!(
            *ws.solution_neighbourhood.borrow(),
            SomeNeighbourhood::Partial({
                let mut n = Partial::new();
                n.insert(&Stack::from_target(arr1(&[0, 0, 0])));
                n.insert(&Stack::from_target(arr1(&[-1, 2, 1])));
                n
            }),
        );

        // C major triad
        clear(&mut keys);
        keys[60].note_on(Channel::Ch1, now);
        keys[64].note_on(Channel::Ch1, now);
        keys[67].note_on(Channel::Ch1, now);
        ws.solve(&keys);
        assert!(ws.energy < epsilon);
        assert!(ws.relaxed);
        assert_eq!(
            *ws.solution_neighbourhood.borrow(),
            SomeNeighbourhood::Partial({
                let mut n = Partial::new();
                n.insert(&Stack::from_target(arr1(&[0, 0, 0])));
                n.insert(&Stack::from_target(arr1(&[0, 1, 0])));
                n.insert(&Stack::from_target(arr1(&[0, 0, 1])));
                n
            }),
        );

        // E major triad -- translation invariance test
        clear(&mut keys);
        keys[64].note_on(Channel::Ch1, now);
        keys[68].note_on(Channel::Ch1, now);
        keys[71].note_on(Channel::Ch1, now);
        ws.solve(&keys);
        assert!(ws.energy < epsilon);
        assert!(ws.relaxed);
        assert_eq!(
            *ws.solution_neighbourhood.borrow(),
            SomeNeighbourhood::Partial({
                let mut n = Partial::new();
                n.insert(&Stack::from_target(arr1(&[0, 0, 0])));
                n.insert(&Stack::from_target(arr1(&[0, 1, 0])));
                n.insert(&Stack::from_target(arr1(&[0, 0, 1])));
                n
            }),
        );

        // The three notes C,D,E: Because the lower notes are more stable, the interval C-D will
        // be the major tone. See the next example as well.
        clear(&mut keys);
        keys[60].note_on(Channel::Ch1, now);
        keys[62].note_on(Channel::Ch1, now);
        keys[64].note_on(Channel::Ch1, now);
        ws.solve(&keys);
        assert!(ws.energy < epsilon);
        assert!(ws.relaxed);
        assert_eq!(
            *ws.solution_neighbourhood.borrow(),
            SomeNeighbourhood::Partial({
                let mut n = Partial::new();
                n.insert(&Stack::from_target(arr1(&[0, 0, 0])));
                n.insert(&Stack::from_target(arr1(&[-1, 2, 0])));
                n.insert(&Stack::from_target(arr1(&[0, 0, 1])));
                n
            }),
        );

        // now, D-E will be the major tone.
        ws.lower_notes_are_more_stable = false;
        ws.solve(&keys);
        assert!(ws.energy < epsilon);
        assert!(ws.relaxed);
        assert_eq!(
            *ws.solution_neighbourhood.borrow(),
            SomeNeighbourhood::Partial({
                let mut n = Partial::new();
                n.insert(&Stack::from_target(arr1(&[0, 0, 0])));
                n.insert(&Stack::from_target(arr1(&[1, -2, 1])));
                n.insert(&Stack::from_target(arr1(&[0, 0, 1])));
                n
            }),
        );

        ws.lower_notes_are_more_stable = true;

        // D-flat major seventh on C
        clear(&mut keys);
        keys[60].note_on(Channel::Ch1, now);
        keys[61].note_on(Channel::Ch1, now);
        keys[65].note_on(Channel::Ch1, now);
        keys[68].note_on(Channel::Ch1, now);
        ws.solve(&keys);
        assert!(ws.energy < epsilon);
        assert!(ws.relaxed);
        assert_eq!(
            *ws.solution_neighbourhood.borrow(),
            SomeNeighbourhood::Partial({
                let mut n = Partial::new();
                n.insert(&Stack::from_target(arr1(&[0, 0, 0])));
                n.insert(&Stack::from_target(arr1(&[1, -1, -1])));
                n.insert(&Stack::from_target(arr1(&[1, -1, 0])));
                n.insert(&Stack::from_target(arr1(&[1, 0, -1])));
                n
            }),
        );

        // D dominant seventh on C
        clear(&mut keys);
        keys[60].note_on(Channel::Ch1, now);
        keys[62].note_on(Channel::Ch1, now);
        keys[66].note_on(Channel::Ch1, now);
        keys[69].note_on(Channel::Ch1, now);
        ws.solve(&keys);
        assert!(ws.energy < epsilon);
        assert!(ws.relaxed);
        assert_eq!(
            *ws.solution_neighbourhood.borrow(),
            SomeNeighbourhood::Partial({
                let mut n = Partial::new();
                n.insert(&Stack::from_target(arr1(&[0, 0, 0])));
                n.insert(&Stack::from_target(arr1(&[-1, 2, 0])));
                n.insert(&Stack::from_target(arr1(&[-1, 2, 1])));
                n.insert(&Stack::from_target(arr1(&[-1, 3, 0])));
                n
            }),
        );

        // a slightly bigger example
        clear(&mut keys);
        keys[60].note_on(Channel::Ch1, now);
        keys[62].note_on(Channel::Ch1, now);
        keys[64].note_on(Channel::Ch1, now);
        keys[67].note_on(Channel::Ch1, now);
        keys[70].note_on(Channel::Ch1, now);
        keys[73].note_on(Channel::Ch1, now);
        keys[75].note_on(Channel::Ch1, now);
        ws.solve(&keys);
        assert!(ws.energy > epsilon);
        assert!(!ws.relaxed);

        // 69 chord cannot be in tune
        clear(&mut keys);
        keys[60].note_on(Channel::Ch1, now);
        keys[62].note_on(Channel::Ch1, now);
        keys[64].note_on(Channel::Ch1, now);
        keys[67].note_on(Channel::Ch1, now);
        keys[69].note_on(Channel::Ch1, now);
        ws.solve(&keys);
        assert!(ws.energy > epsilon);
        assert!(!ws.relaxed);

        assert_eq!(
            ws.solution_interval_targets.slice(s![0..(5 * 4 / 2), ..]),
            arr2(&[
                // intervals from C
                [-1, 2, 0],
                [0, 0, 1],
                [0, 1, 0],
                [1, -1, 1],
                // intervals from D
                [-1, 2, 0],
                [1, -1, 0],
                [0, 1, 0],
                // intervals from E
                [0, 1, -1],
                [1, -1, 0],
                // intervals from G
                [-1, 2, 0],
            ])
        );

        // 69 chord with rods for fifhts
        match &mut ws.provider {
            HarmonySpringsProvider::Mod12 { by_class, .. } => {
                by_class[7] = RodOrSprings::Rod(Stack::from_pure_interval(1, 1));
            }
        }
        ws.solve(&keys);
        assert!(ws.energy > epsilon);
        assert!(!ws.relaxed);

        let mut solution = vec![];
        ws.solution_neighbourhood
            .borrow()
            .for_each_stack(|_, stack| solution.push(stack.clone()));

        //C-G fifth
        assert_eq!(solution[0], Stack::new_zero());
        assert_eq!(solution[3], Stack::from_pure_interval(1, 1));

        // D-A fifth:
        let mut delta = solution[4].clone();
        delta.scaled_add(-1, &solution[1]);
        // note that the target is maybe of a different shape, e.g. if we're considering the
        // "fifth" D..A, and not D..A+
        assert_eq!(delta.actual, arr1(&[0.into(), 1.into(), 0.into()]));

        // the D is between a minor and a major tone higher than C:
        let majortone = 12.0 * (9.0 as Semitones / 8.0).log2();
        let minortone = 12.0 * (10.0 as Semitones / 9.0).log2();
        assert!(solution[1].semitones() < majortone);
        assert!(solution[1].semitones() > minortone);

        // the distance between E and D is also between a major and minor tone:
        assert!(solution[2].semitones() - solution[1].semitones() < majortone);
        assert!(solution[2].semitones() - solution[1].semitones() > minortone);

        // the distance betwen C and D is the same as between G and A:
        let _ = abs_diff_eq!(
            solution[1].semitones() - solution[0].semitones(),
            solution[4].semitones() - solution[3].semitones(),
            epsilon = epsilon
        );

        assert_eq!(
            ws.solution_interval_targets.slice(s![0..(5 * 4 / 2), ..]),
            arr2(&[
                // intervals from C
                [-1, 2, 0],
                [0, 0, 1],
                [0, 1, 0],
                [1, -1, 1],
                // intervals from D
                [-1, 2, 0],
                [1, -1, 0],
                [0, 1, 0],
                //intervals from E
                [0, 1, -1],
                [1, -1, 0],
                //intervals from G
                [-1, 2, 0],
            ])
        );

        // 69 chord with rods for fifhts (set above) and fourths. This forces a pythagorean third.
        match &mut ws.provider {
            HarmonySpringsProvider::Mod12 { by_class, .. } => {
                by_class[5] = RodOrSprings::Rod(Stack::from_target(arr1(&[1, -1, 0])))
            }
        }
        ws.solve(&keys);
        assert!(ws.energy > epsilon);
        assert!(!ws.relaxed);
        assert_eq!(
            *ws.solution_neighbourhood.borrow(),
            SomeNeighbourhood::Partial({
                let mut n = Partial::new();
                n.insert(&Stack::from_target(arr1(&[0, 0, 0])));
                n.insert(&Stack::from_target(arr1(&[-1, 2, 0])));
                n.insert(&Stack::from_target(arr1(&[-2, 4, 0])));
                n.insert(&Stack::from_target(arr1(&[0, 1, 0])));
                n.insert(&Stack::from_target(arr1(&[-1, 3, 0])));
                n
            }),
        );
    }
}
