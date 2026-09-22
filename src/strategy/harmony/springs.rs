use std::{
    collections::{BTreeMap, HashMap},
    time::Instant,
};

use ndarray::{s, Array2, ArrayView1, ArrayView2, ArrayViewMut2};
use num_rational::Ratio;
use serde_derive::{Deserialize, Serialize};

use crate::{
    adaptors::lock_levels::{ActiveStrategyIndexLevel, KeyStateLevel, StrategyConfigLevel},
    bindable::BindableStrategyAction,
    config::{HarmonyStrategyConfig, IsHarmonyStrategyConfig, StrategyConfig},
    custom_serde::common::{deserialize_ratio, serialize_ratio},
    interval::{
        base::Semitones,
        stack::{semitones_from_actual, ScaledAdd, Stack},
        stacktype::r#trait::{IntervalBasis, StackCoeff, StackType},
    },
    msg::{ToHarmony, ToHarmonySprings},
    neighbourhood::{self, Neighbourhood},
    strategy::harmony::r#trait::{Harmony, HarmonyAdaptor, HarmonyResult, HarmonyStrategy},
    util::{
        ordered_locks::{AtMost, Succ, Zero},
        springs::Solver,
    },
};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "kebab-case")]
#[derive(Clone)]
pub struct Spring<T: IntervalBasis> {
    pub length: Stack<T>,
    #[serde(
        serialize_with = "serialize_ratio",
        deserialize_with = "deserialize_ratio"
    )]
    pub stiffness: Ratio<StackCoeff>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "kebab-case")]
#[derive(Clone)]
pub enum RodOrSprings<T: IntervalBasis> {
    Rod(Stack<T>),
    #[serde(rename_all = "kebab-case")]
    Springs {
        /// todo: ensure that this list is non-empty at deserialisation time. The approach commented out
        /// doesn't work because the type checker is too dumb.
        options: Vec<Spring<T>>,
    },
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

/// This function ensures the invariant of [Solver::add_rod]
fn normalize_rods<T: IntervalBasis>(n_keys: usize, rods: &mut BTreeMap<(usize, usize), Stack<T>>) {
    for k in (0..n_keys).rev() {
        for j in (0..k).rev() {
            for i in (0..j).rev() {
                match rods.remove(&(j, k)) {
                    None {} => {}
                    Some(b) => match (rods.get(&(i, j)), rods.get(&(i, k))) {
                        (None {}, None {}) => {
                            // put it back: we can't delete information
                            rods.insert((j, k), b);
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
                            rods.insert((i, k), b_plus_a);
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
                            rods.insert((i, j), c_minus_b);
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

impl<T: IntervalBasis> HarmonySpringsProvider<T> {
    fn collect_connectors(
        &self,
        keys: &[u8],
        lower_intervals_are_more_stable: bool,
        tmp: &mut Vec<((usize, usize), usize)>,
        rods: &mut BTreeMap<(usize, usize), Stack<T>>,
        springs: &mut BTreeMap<(usize, usize), SpringInfo>,
    ) {
        match self {
            HarmonySpringsProvider::Mod12 { by_class, octave } => {
                let n = keys.len();

                rods.clear();

                for i in 0..n {
                    for j in (i + 1)..n {
                        let d = keys[j] as i8 - keys[i] as i8;
                        let rem = d.rem_euclid(12) as usize;
                        match &by_class[rem] {
                            RodOrSprings::Rod(stack) => {
                                let quot = d.div_euclid(12) as StackCoeff;
                                let mut rod = stack.clone();
                                rod.scaled_add(quot, octave);
                                rods.insert((i, j), rod);
                            }
                            _ => {}
                        }
                    }
                }

                normalize_rods(n, rods);

                // if the node is a rod end, what's the rod's start node?
                //
                // This is useful to check if a spring can be ommited between two nodes.
                //
                // Note that normalize_rods made it so that there are a no "chains" of rods; there
                // are a number of "base" nodes, and all non-base nodes that are connected to a
                // rod are connected directly to a base node.
                let mut rod_start: Vec<Option<usize>> = vec![None {}; n];
                for (i, j) in rods.keys() {
                    rod_start[*j] = Some(*i);
                }

                tmp.clear();

                springs.clear();

                for i in 0..n {
                    for j in (i + 1)..n {
                        let d = keys[j] as i8 - keys[i] as i8;
                        let rem = d.rem_euclid(12) as usize;
                        match &by_class[rem] {
                            RodOrSprings::Springs { options, .. } => {
                                if rod_start[i].is_none() || rod_start[i] != rod_start[j] {
                                    springs.insert(
                                        (i, j),
                                        SpringInfo {
                                            current_candidate_index: 0,
                                            memo_key: d,
                                            solver_length_index: 0, // dummy initialisation; will be overwritten!
                                        },
                                    );
                                    tmp.push(((i, j), options.len()));
                                }
                            }
                            _ => {}
                        }
                    }
                }

                if lower_intervals_are_more_stable {
                    tmp.sort_by(|a, b| b.0.cmp(&a.0));
                } else {
                    tmp.sort_by(|a, b| a.0.cmp(&b.0));
                }

                let mut i = 0;
                for (_, info) in springs.iter_mut() {
                    info.solver_length_index = i;
                    i += 1;
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
                    RodOrSprings::Springs {
                        options: springs, ..
                    } => {
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
    tmp: Vec<((usize, usize), usize)>,
    spring_setup: SpringSetup<T>,
    solver: Solver,

    relaxed: bool,
    energy: Energy,
    number_of_tries: u64,
    computed_at_least_one_solution: bool,

    solution_actuals: Array2<Ratio<StackCoeff>>,

    solution_interval_targets: Array2<StackCoeff>,
    solution_target_is_set: Vec<bool>,

    solution_neighbourhood: neighbourhood::Partial<T>,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct HarmonySpringsConfig<T: IntervalBasis> {
    pub enable: bool,
    /// Should the [HarmonySpringsProvider::candidate_springs] be memoised between different calls
    /// to [HarmonyStrategy::solve]?
    pub memo_springs: bool,
    pub min_keys: usize,
    pub lower_intervals_are_more_stable: bool,
    pub provider: HarmonySpringsProvider<T>,
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

    fn update_memoed_springs(
        &mut self,
        candidate_springs: impl Fn(i8) -> Vec<(Stack<T>, Ratio<StackCoeff>)>,
        memo_springs: bool,
    ) {
        if !memo_springs {
            self.memoed_springs.clear();
        }

        for (_, SpringInfo { memo_key, .. }) in self.current_springs.iter() {
            if !self.memoed_springs.contains_key(&memo_key) {
                self.memoed_springs
                    .insert(*memo_key, candidate_springs(*memo_key));
            }
        }
    }

    /// returns true iff the next candidate was prepared, false iff there are no more candidates.
    fn prepare_next_candidate(&mut self, change_from_the_back: bool) -> bool {
        macro_rules! __prepare_nex_candidate_step {
            ($v:ident) => {
                let max_ix = self
                    .memoed_springs
                    .get(&$v.memo_key)
                    .expect("prepare_next_spring_candidate: found no candidates for spring")
                    .len()
                    - 1;
                if $v.current_candidate_index < max_ix {
                    $v.current_candidate_index += 1;
                    return true;
                } else {
                    $v.current_candidate_index = 0;
                }
            };
        }

        if change_from_the_back {
            for (_, v) in self.current_springs.iter_mut().rev() {
                __prepare_nex_candidate_step!(v);
            }
        } else {
            for (_, v) in self.current_springs.iter_mut() {
                __prepare_nex_candidate_step!(v);
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

impl<T: StackType> HarmonySprings<T> {
    fn initialise<L: AtMost<KeyStateLevel> + AtMost<StrategyConfigLevel>>(
        &mut self,
        mut adaptor: HarmonyAdaptor<T, Self, L>,
    ) -> HarmonyAdaptor<T, Self, L> {
        self.keys.clear();
        adaptor = adaptor.for_all_sounding_keys(|i, _, _| self.keys.push(i as u8));

        (_, adaptor) = adaptor.config(|conf, _| {
            conf.provider.collect_connectors(
                &self.keys,
                conf.lower_intervals_are_more_stable,
                &mut self.tmp,
                &mut self.spring_setup.current_rods,
                &mut self.spring_setup.current_springs,
            );

            self.spring_setup
                .update_memoed_springs(|d| conf.provider.candidate_springs(d), conf.memo_springs);
        });

        self.relaxed = false;
        self.energy = Semitones::MAX;
        // no need to initialise `self.solution_actuals`, it will be overwritten anyway

        adaptor
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

impl<T: StackType> IsHarmonyStrategyConfig<T> for HarmonySpringsConfig<T> {
    fn as_harmony_strategy_config(self) -> HarmonyStrategyConfig<T> {
        HarmonyStrategyConfig::Springs(self)
    }
}

impl<T: StackType> HarmonySprings<T> {
    fn send_current_solution(
        &mut self,
        mut adaptor: HarmonyAdaptor<T, Self, Zero>,
    ) -> HarmonyAdaptor<T, Self, Zero> {
        if self.computed_at_least_one_solution {
            self.compute_solution_interval_targets();

            self.solution_neighbourhood.clear();
            self.solution_neighbourhood.insert_zero();
            for i in 1..self.keys.len() {
                self.solution_neighbourhood.insert_target_actual(
                    self.solution_interval_targets.row(i - 1),
                    self.solution_actuals.row(i),
                );
            }

            (_, adaptor) = adaptor.harmony_mut(|harmony, _| {
                *harmony = Harmony::SpringSolution {
                    neighbourhood: self.solution_neighbourhood.clone(),
                    lowest_key: self.keys[0],
                    number_of_tries: self.number_of_tries,
                    relaxed: self.relaxed,
                };
            });
        }
        adaptor
    }

    fn preliminiary_result(
        &mut self,
        mut adaptor: HarmonyAdaptor<T, Self, Zero>,
    ) -> (HarmonyResult, HarmonyAdaptor<T, Self, Zero>) {
        adaptor = self.send_current_solution(adaptor);
        (
            HarmonyResult {
                finished: false,
                progress: self.computed_at_least_one_solution,
                perfect: self.relaxed,
            },
            adaptor,
        )
    }

    fn finish_solve(
        &mut self,
        mut adaptor: HarmonyAdaptor<T, Self, Zero>,
    ) -> (HarmonyResult, HarmonyAdaptor<T, Self, Zero>) {
        adaptor = self.send_current_solution(adaptor);
        (
            HarmonyResult {
                finished: true,
                progress: self.computed_at_least_one_solution,
                perfect: self.relaxed,
            },
            adaptor,
        )
    }
}

impl<T: StackType, L: AtMost<StrategyConfigLevel>> HarmonyAdaptor<T, HarmonySprings<T>, L> {
    pub fn config<R>(
        self,
        mut f: impl FnMut(
            &HarmonySpringsConfig<T>,
            HarmonyAdaptor<T, HarmonySprings<T>, Succ<ActiveStrategyIndexLevel>>,
        ) -> R,
    ) -> (R, Self) {
        self.active_strategy(|conf, adaptor| match conf {
            StrategyConfig::TwoStep {
                harmony: HarmonyStrategyConfig::Springs(conf),
                ..
            } => f(conf, adaptor),
            StrategyConfig::TwoStep {
                harmony: HarmonyStrategyConfig::List(confs),
                ..
            } => {
                for conf in confs {
                    if let HarmonyStrategyConfig::Springs(conf) = conf {
                        return f(conf, adaptor);
                    }
                }
                panic!("Wrong type of harmony strategy config: expected HarmonySprings somewhere in the list of configs")
            }
            _ => panic!("Wrong type of harmony strategy config: expected HarmonySprings"),
        })
    }
}

impl<T: StackType> HarmonyStrategy<T> for HarmonySprings<T> {
    type Config = HarmonySpringsConfig<T>;

    type Msg = ToHarmonySprings;

    fn new(_config: HarmonySpringsConfig<T>) -> Self {
        let n = 10; // initial guess at how many keys we're playing simulatneously: both hands full.
        let big_n = n * (n - 1) / 2;
        Self {
            keys: Vec::with_capacity(n),
            spring_setup: SpringSetup::new(),
            solver: Solver::new(n, big_n, T::num_intervals()),
            relaxed: false,
            energy: Energy::MAX,
            number_of_tries: 0,
            computed_at_least_one_solution: false,
            solution_actuals: Array2::zeros((n, T::num_intervals())),
            solution_interval_targets: Array2::zeros((big_n, T::num_intervals())),
            solution_target_is_set: vec![false; big_n],
            solution_neighbourhood: neighbourhood::Partial::new(),
            tmp: Vec::with_capacity(big_n),
        }
    }

    fn start_solve(
        &mut self,
        _time: Instant,
        mut adaptor: HarmonyAdaptor<T, Self, Zero>,
    ) -> (HarmonyResult, HarmonyAdaptor<T, Self, Zero>) {
        self.number_of_tries = 0;
        (_, adaptor) = adaptor.harmony_mut(|h, _| *h = Harmony::None);
        let enable;
        let min_keys;
        ((enable, min_keys), adaptor) = adaptor.config(|conf, _| (conf.enable, conf.min_keys));
        if !enable {
            return (
                HarmonyResult {
                    finished: true,
                    progress: false,
                    perfect: false,
                },
                adaptor,
            );
        }
        adaptor = self.initialise(adaptor);
        if self.keys.len() < min_keys {
            self.computed_at_least_one_solution = false;
            return self.finish_solve(adaptor);
        }

        self.computed_at_least_one_solution = self.compute_solution_actuals();
        self.number_of_tries = 1;
        if self.relaxed {
            return self.finish_solve(adaptor);
        }

        (
            HarmonyResult {
                finished: false,
                progress: self.computed_at_least_one_solution,
                perfect: true,
            },
            adaptor,
        )
    }

    fn step(
        &mut self,
        mut adaptor: HarmonyAdaptor<T, Self, Zero>,
    ) -> (HarmonyResult, HarmonyAdaptor<T, Self, Zero>) {
        let lower_intervals_are_more_stable;
        (lower_intervals_are_more_stable, adaptor) =
            adaptor.config(|conf, _| conf.lower_intervals_are_more_stable);
        if !self
            .spring_setup
            .prepare_next_candidate(lower_intervals_are_more_stable)
        {
            return self.finish_solve(adaptor);
        }

        self.computed_at_least_one_solution |= self.compute_solution_actuals();
        self.number_of_tries += 1;
        if self.relaxed {
            self.finish_solve(adaptor)
        } else {
            self.preliminiary_result(adaptor)
        }
    }

    fn filter_to_harmony(msg: ToHarmony) -> Option<Self::Msg> {
        match msg {
            ToHarmony::Springs(msg) => Some(msg),
            _ => None {},
        }
    }

    fn receive_msg(
        &mut self,
        msg: Self::Msg,
        adaptor: HarmonyAdaptor<T, Self, Zero>,
    ) -> (Option<Instant>, HarmonyAdaptor<T, Self, Zero>) {
        match msg {
            ToHarmonySprings::ReloadSprings { time } => {
                self.spring_setup.memoed_springs.clear();
                (Some(time), adaptor)
            }
            ToHarmonySprings::Recalculate { time } => (Some(time), adaptor),
        }
    }

    fn handle_bound_action(
        &mut self,
        action: BindableStrategyAction,
        _time: Instant,
        adaptor: HarmonyAdaptor<T, Self, Zero>,
    ) -> (Option<Instant>, HarmonyAdaptor<T, Self, Zero>) {
        match action {
            _ => (None {}, adaptor),
        }
    }
}

#[cfg(test)]
mod test {
    use std::sync::{mpsc, Arc};

    use approx::abs_diff_eq;
    use midi_msg::Channel;
    use ndarray::{arr1, arr2};
    use parking_lot::RwLock;
    use pretty_assertions::assert_eq;

    use crate::{
        adaptors::ConcreteLocks,
        config::{
            BackendConfig, Config, HarmonyStrategyConfig, MelodyHarmonyCoordinationConfig,
            MelodyStrategyConfig,
        },
        gui::r#trait::GuiTag,
        interval::stacktype::fivelimit::mock::MockFiveLimitStackType,
        keystate::KeyState,
        process::r#trait::{ProcessTag, StackWithTuning},
        reference::Reference,
        strategy::melody::neighbourhoods::StaticNeighbourhoodsAsMelodyConfig,
        util::ordered_locks::{OrderedLocks, Zero},
    };

    use super::*;

    fn mock_harmony_adaptor(
    ) -> HarmonyAdaptor<MockFiveLimitStackType, HarmonySprings<MockFiveLimitStackType>, Zero> {
        unsafe {
            let (from_process_tx, _) = mpsc::channel();
            let (from_ui_tx, _) = mpsc::channel();
            let (from_backend_tx, _) = mpsc::channel();

            const TEMPLATE_CONFIG: &'static str = include_str!("../../../configs/template.yaml");
            let template_config: Config<MockFiveLimitStackType> =
                serde_yml::from_str(TEMPLATE_CONFIG).unwrap();

            OrderedLocks::new(Arc::new(ConcreteLocks {
                from_process_tx,
                from_ui_tx,
                from_backend_tx,

                pedal_hold: RwLock::new([false; 16]),
                sostenuto_hold: RwLock::new([false; 16]),
                soft_hold: RwLock::new([false; 16]),
                tunings: core::array::from_fn(|i| {
                    RwLock::new(StackWithTuning {
                        stack: Stack::new_zero(),
                        semitones: i as Semitones,
                    })
                }),
                key_states: core::array::from_fn(|_| RwLock::new(KeyState::new(Instant::now()))),
                reference: RwLock::new(Stack::new_zero()),
                tuning_reference: RwLock::new(Reference::from_semitones(Stack::new_zero(), 60.0)),
                strategy_config: RwLock::new(vec![StrategyConfig::TwoStep {
                    bindings: BTreeMap::new(),
                    name: "mock twostep".into(),
                    description: "".into(),
                    harmony: HarmonyStrategyConfig::Springs(HarmonySpringsConfig {
                        enable: true,
                        min_keys: 1,
                        memo_springs: true,
                        lower_intervals_are_more_stable: true,
                        provider: mock_provider(),
                    }),
                    melody: MelodyStrategyConfig::StaticNeighbourhoods(
                        StaticNeighbourhoodsAsMelodyConfig {
                            initial_reference: Stack::new_zero(),
                            scales: vec![], // dummy initialisation: In the real world, this is never empty
                        },
                    ),
                    melody_harmony_coordination: MelodyHarmonyCoordinationConfig {
                        reanchor: true,
                        group_ms: 100,
                        tune_wait_us: 1000,
                    },
                }]),
                active_strategy_index: RwLock::new(0),
                harmony: RwLock::new(Harmony::None),
                backend_config: RwLock::new(match template_config.backend {
                    BackendConfig::Pitchbend12(c) => c,
                }),
                gui_config: RwLock::new(template_config.gui),
            }))
        }
    }

    fn mock_provider() -> HarmonySpringsProvider<MockFiveLimitStackType> {
        HarmonySpringsProvider::Mod12 {
            by_class: [
                RodOrSprings::Rod(Stack::from_target(vec![0, 0, 0])),
                RodOrSprings::Springs {
                    options: vec![
                        Spring {
                            length: Stack::from_target(vec![1, (-1), (-1)]), // diatonic semitone
                            stiffness: Ratio::new(1, 5),
                        },
                        Spring {
                            length: Stack::from_target(vec![0, (-1), 2]), // chromatic semitone
                            stiffness: Ratio::new(1, 5),
                        },
                    ],
                },
                RodOrSprings::Springs {
                    options: vec![
                        Spring {
                            length: Stack::from_target(vec![-1, 2, 0]), // major tone 9/8
                            stiffness: Ratio::new(1, 3),
                        },
                        Spring {
                            length: Stack::from_target(vec![1, -2, 1]), // minor tone 10/9
                            stiffness: Ratio::new(1, 5),
                        },
                    ],
                },
                RodOrSprings::Springs {
                    options: vec![Spring {
                        length: Stack::from_target(vec![0, 1, (-1)]), // minor third
                        stiffness: Ratio::new(1, 5),
                    }],
                },
                RodOrSprings::Springs {
                    options: vec![Spring {
                        length: Stack::from_target(vec![0, 0, 1]), // major third
                        stiffness: Ratio::new(1, 5),
                    }],
                },
                RodOrSprings::Springs {
                    options: vec![Spring {
                        length: Stack::from_target(vec![1, (-1), 0]), // fourth
                        stiffness: Ratio::new(1, 3),
                    }],
                },
                RodOrSprings::Springs {
                    options: vec![
                        Spring {
                            length: Stack::from_target(vec![-1, 2, 1]), // tritone as major tone plus major third
                            stiffness: Ratio::new(1, 5),
                        },
                        Spring {
                            length: Stack::from_target(vec![0, 2, (-2)]), // tritone as chromatic semitone below fifth
                            stiffness: Ratio::new(1, 5),
                        },
                    ],
                },
                RodOrSprings::Springs {
                    options: vec![Spring {
                        length: Stack::from_target(vec![0, 1, 0]), // fifth
                        stiffness: Ratio::new(1, 3),
                    }],
                },
                RodOrSprings::Springs {
                    options: vec![Spring {
                        length: Stack::from_target(vec![1, 0, (-1)]), // minor sixth
                        stiffness: Ratio::new(1, 5),
                    }],
                },
                RodOrSprings::Springs {
                    options: vec![
                        Spring {
                            length: Stack::from_target(vec![1, (-1), 1]), // major sixth
                            stiffness: Ratio::new(1, 5),
                        },
                        Spring {
                            length: Stack::from_target(vec![-1, 3, 0]), // major tone plus fifth
                            stiffness: Ratio::new(1, 3),
                        },
                    ],
                },
                RodOrSprings::Springs {
                    options: vec![
                        Spring {
                            length: Stack::from_target(vec![2, (-2), 0]), // minor seventh as stack of two fourths
                            stiffness: Ratio::new(1, 3),
                        },
                        Spring {
                            length: Stack::from_target(vec![0, 2, (-1)]), // minor seventh as fifth plus minor third
                            stiffness: Ratio::new(1, 5),
                        },
                    ],
                },
                RodOrSprings::Springs {
                    options: vec![Spring {
                        length: Stack::from_target(vec![0, 1, 1]), // major seventh as fifth plus major third
                        stiffness: Ratio::new(1, 5),
                    }],
                },
            ],
            octave: Stack::from_pure_interval(0, 1),
        }
    }

    fn mock_harmony_springs() -> HarmonySprings<MockFiveLimitStackType> {
        HarmonySprings::new(HarmonySpringsConfig {
            enable: true,
            min_keys: 1,
            memo_springs: true,
            lower_intervals_are_more_stable: true,
            provider: mock_provider(),
        })
    }

    #[test]
    fn test_harmony_springs_solve() {
        let mut ws = mock_harmony_springs();
        let mut adaptor = mock_harmony_adaptor();

        let epsilon = 0.00000000000000001; // just a very small number. I don't care precisely.

        let now = Instant::now();
        // let clear = |keys: &mut [KeyState]| keys.iter_mut().for_each(|k| *k = KeyState::new(now));
        macro_rules! clear_keys {
            () => {{
                let mut a: OrderedLocks<ProcessTag, _, Zero> =
                    unsafe { OrderedLocks::new(adaptor.inner_arc()) };
                for i in 0..128 {
                    (_, a) = a.key_state_mut(i, |k, _| k.note_off(Channel::Ch1, false, now));
                }
            }};
        }

        macro_rules! set_note_on {
            ($i:expr) => {{
                let a: OrderedLocks<ProcessTag, _, Zero> =
                    unsafe { OrderedLocks::new(adaptor.inner_arc()) };
                a.key_state_mut($i as usize, |k, _| k.note_on(Channel::Ch1, now));
            }};
        }

        macro_rules! solve {
            () => {{
                let mut res;
                (res, adaptor) = ws.start_solve(now, adaptor);
                while !res.finished {
                    (res, adaptor) = ws.step(adaptor);
                }
            }};
        }

        // if nothing else is given, the first option is picked
        clear_keys!();
        set_note_on!(60);
        set_note_on!(66);
        solve!();
        assert!(ws.energy < epsilon);
        assert!(ws.relaxed);
        assert_eq!(ws.solution_neighbourhood, {
            let mut n = neighbourhood::Partial::new();
            n.insert(&Stack::from_target(arr1(&[0, 0, 0])));
            n.insert(&Stack::from_target(arr1(&[-1, 2, 1])));
            n
        },);

        // C major triad
        clear_keys!();
        set_note_on!(60);
        set_note_on!(64);
        set_note_on!(67);
        solve!();
        assert!(ws.energy < epsilon);
        assert!(ws.relaxed);
        assert_eq!(ws.solution_neighbourhood, {
            let mut n = neighbourhood::Partial::new();
            n.insert(&Stack::from_target(arr1(&[0, 0, 0])));
            n.insert(&Stack::from_target(arr1(&[0, 1, 0])));
            n.insert(&Stack::from_target(arr1(&[0, 0, 1])));
            n
        });

        // E major triad -- translation invariance test
        clear_keys!();
        set_note_on!(64);
        set_note_on!(68);
        set_note_on!(71);
        solve!();
        assert!(ws.energy < epsilon);
        assert!(ws.relaxed);
        assert_eq!(ws.solution_neighbourhood, {
            let mut n = neighbourhood::Partial::new();
            n.insert(&Stack::from_target(arr1(&[0, 0, 0])));
            n.insert(&Stack::from_target(arr1(&[0, 1, 0])));
            n.insert(&Stack::from_target(arr1(&[0, 0, 1])));
            n
        });

        // The three notes C,D,E: Because the lower notes are more stable, the interval C-D will
        // be the major tone. See the next example as well.
        clear_keys!();
        set_note_on!(60);
        set_note_on!(62);
        set_note_on!(64);
        solve!();
        assert!(ws.energy < epsilon);
        assert!(ws.relaxed);
        assert_eq!(ws.solution_neighbourhood, {
            let mut n = neighbourhood::Partial::new();
            n.insert(&Stack::from_target(arr1(&[0, 0, 0])));
            n.insert(&Stack::from_target(arr1(&[-1, 2, 0])));
            n.insert(&Stack::from_target(arr1(&[0, 0, 1])));
            n
        });

        // now, D-E will be the major tone.
        {
            let a: OrderedLocks<GuiTag, _, Zero> =
                unsafe { OrderedLocks::new(adaptor.inner_arc()) };
            a.active_strategy_mut(|conf, _| match conf {
                StrategyConfig::TwoStep {
                    harmony:
                        HarmonyStrategyConfig::Springs(HarmonySpringsConfig {
                            lower_intervals_are_more_stable,
                            ..
                        }),
                    ..
                } => *lower_intervals_are_more_stable = false,
                _ => panic!(),
            });
        }
        clear_keys!();
        set_note_on!(60);
        set_note_on!(62);
        set_note_on!(64);
        solve!();
        assert!(ws.energy < epsilon);
        assert!(ws.relaxed);
        assert_eq!(ws.solution_neighbourhood, {
            let mut n = neighbourhood::Partial::new();
            n.insert(&Stack::from_target(arr1(&[0, 0, 0])));
            n.insert(&Stack::from_target(arr1(&[1, -2, 1])));
            n.insert(&Stack::from_target(arr1(&[0, 0, 1])));
            n
        },);

        // reset to lower intervals preferred again:
        {
            let a: OrderedLocks<GuiTag, _, Zero> =
                unsafe { OrderedLocks::new(adaptor.inner_arc()) };
            a.active_strategy_mut(|conf, _| match conf {
                StrategyConfig::TwoStep {
                    harmony:
                        HarmonyStrategyConfig::Springs(HarmonySpringsConfig {
                            lower_intervals_are_more_stable,
                            ..
                        }),
                    ..
                } => *lower_intervals_are_more_stable = true,
                _ => panic!(),
            });
        }

        // D-flat major seventh on C
        clear_keys!();
        set_note_on!(60);
        set_note_on!(61);
        set_note_on!(65);
        set_note_on!(68);
        solve!();
        assert!(ws.energy < epsilon);
        assert!(ws.relaxed);
        assert_eq!(ws.solution_neighbourhood, {
            let mut n = neighbourhood::Partial::new();
            n.insert(&Stack::from_target(arr1(&[0, 0, 0])));
            n.insert(&Stack::from_target(arr1(&[1, -1, -1])));
            n.insert(&Stack::from_target(arr1(&[1, -1, 0])));
            n.insert(&Stack::from_target(arr1(&[1, 0, -1])));
            n
        });

        // D dominant seventh on C
        clear_keys!();
        set_note_on!(60);
        set_note_on!(62);
        set_note_on!(66);
        set_note_on!(69);
        solve!();
        assert!(ws.energy < epsilon);
        assert!(ws.relaxed);
        assert_eq!(ws.solution_neighbourhood, {
            let mut n = neighbourhood::Partial::new();
            n.insert(&Stack::from_target(arr1(&[0, 0, 0])));
            n.insert(&Stack::from_target(arr1(&[-1, 2, 0])));
            n.insert(&Stack::from_target(arr1(&[-1, 2, 1])));
            n.insert(&Stack::from_target(arr1(&[-1, 3, 0])));
            n
        });

        // a slightly bigger example
        clear_keys!();
        set_note_on!(60);
        set_note_on!(62);
        set_note_on!(64);
        set_note_on!(67);
        set_note_on!(70);
        set_note_on!(73);
        set_note_on!(75);
        solve!();
        assert!(ws.energy > epsilon);
        assert!(!ws.relaxed);

        // 69 chord cannot be in tune
        clear_keys!();
        set_note_on!(60);
        set_note_on!(62);
        set_note_on!(64);
        set_note_on!(67);
        set_note_on!(69);
        solve!();
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
        {
            let a: OrderedLocks<GuiTag, _, Zero> =
                unsafe { OrderedLocks::new(adaptor.inner_arc()) };
            a.active_strategy_mut(|conf, _| match conf {
                StrategyConfig::TwoStep {
                    harmony:
                        HarmonyStrategyConfig::Springs(HarmonySpringsConfig {
                            provider: HarmonySpringsProvider::Mod12 { by_class, .. },
                            ..
                        }),
                    ..
                } => by_class[7] = RodOrSprings::Rod(Stack::from_pure_interval(1, 1)),
                _ => panic!(),
            });
        }
        solve!();
        assert!(ws.energy > epsilon);
        assert!(!ws.relaxed);

        let mut solution = vec![];
        ws.solution_neighbourhood
            .for_each_stack(|_, stack| solution.push(stack.clone()));

        // C-G fifth
        assert_eq!(solution[0], Stack::new_zero());
        assert_eq!(solution[3], Stack::from_pure_interval(1, 1));

        // D-A fifth
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

        // the interval D..E is also between a major and a minor tone
        assert!(solution[2].semitones() - solution[1].semitones() < majortone);
        assert!(solution[2].semitones() - solution[1].semitones() > minortone);

        // the distance between C and D is the same as between G and A:
        let _ = abs_diff_eq!(
            solution[1].semitones() - solution[0].semitones(),
            solution[4].semitones() - solution[3].semitones(),
            epsilon = epsilon
        );

        // 69 chord with rods for fifhts (set above) and fourths. This forces a pythagorean third.
        {
            let a: OrderedLocks<GuiTag, _, Zero> =
                unsafe { OrderedLocks::new(adaptor.inner_arc()) };
            a.active_strategy_mut(|conf, _| match conf {
                StrategyConfig::TwoStep {
                    harmony:
                        HarmonyStrategyConfig::Springs(HarmonySpringsConfig {
                            provider: HarmonySpringsProvider::Mod12 { by_class, .. },
                            ..
                        }),
                    ..
                } => by_class[5] = RodOrSprings::Rod(Stack::from_target(arr1(&[1, -1, 0]))),
                _ => panic!(),
            });
        }
        solve!();
        assert!(ws.energy > epsilon);
        assert!(!ws.relaxed);
        assert_eq!(ws.solution_neighbourhood, {
            let mut n = neighbourhood::Partial::new();
            n.insert(&Stack::from_target(arr1(&[0, 0, 0])));
            n.insert(&Stack::from_target(arr1(&[-1, 2, 0])));
            n.insert(&Stack::from_target(arr1(&[-2, 4, 0])));
            n.insert(&Stack::from_target(arr1(&[0, 1, 0])));
            n.insert(&Stack::from_target(arr1(&[-1, 3, 0])));
            n
        });
    }
}
