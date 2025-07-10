use std::collections::{BTreeMap, HashMap};

use ndarray::{Array1, Array2, ArrayView1, ArrayView2};
use num_rational::Ratio;

use super::solver::Solver;
use crate::{
    interval::{
        base::Semitones,
        stack::Stack,
        stacktype::r#trait::{StackCoeff, StackType},
    },
    util::lu,
};

pub enum Connector {
    Spring,
    Rod(RodSpec),
    None,
}

#[derive(Debug)]
struct SpringInfo {
    solver_length_index: usize,
    memo_key: KeyDistance,
    current_candidate_index: usize,
}

#[derive(Debug)]
struct AnchorInfo {
    solver_length_index: usize,
    memo_key: KeyNumber,
    current_candidate_index: usize,
}

#[derive(Debug, PartialEq)]
struct RodInfo {
    solver_length_index: usize,
    memo_key: RodSpec,
}

pub type KeyDistance = i8;
pub type KeyNumber = u8;
pub type Energy = Semitones;

/// An association list of key distances of the sub-intervals and multiplicities of these
/// intervals.
///
/// invariants:
/// - length at least 1
/// - the key distances are always positive
/// - sorted by ascending key distance
pub type RodSpec = Vec<(KeyDistance, StackCoeff)>;

pub struct Workspace<T: StackType> {
    n_keys: usize,
    keys: Vec<KeyNumber>,
    memo_springs: bool,
    memo_anchors: bool,
    memo_rods: bool,
    memoed_springs: HashMap<KeyDistance, Vec<(Stack<T>, Ratio<StackCoeff>)>>,
    memoed_anchors: HashMap<KeyNumber, Vec<(Stack<T>, Ratio<StackCoeff>)>>,
    memoed_rods: HashMap<RodSpec, Stack<T>>,
    current_springs: BTreeMap<(usize, usize), SpringInfo>, // invariant: the key tuples are two distinct numbers, with the smaller one first
    current_anchors: BTreeMap<usize, AnchorInfo>,
    current_rods: BTreeMap<(usize, usize), RodInfo>, // invariant: the key tuples are two distinct numbers, with the smaller one first
}

pub struct Solutions<'a, T: StackType> {
    workspace: &'a mut Workspace<T>,
    solver: &'a mut Solver,
    next_try: bool,
    next_try_prepared: bool,
}

impl<'a, T: StackType + Eq + std::fmt::Debug> Solutions<'a, T> {
    pub fn new<WC, AP, PS, PA, PR>(
        workspace: &'a mut Workspace<T>,
        solver: &'a mut Solver,
        keys: &[KeyNumber],
        is_note_anchored: AP,
        which_connector: WC,
        provide_candidate_springs: PS,
        provide_candidate_anchors: PA,
        provide_rod: PR,
    ) -> Self
    where
        WC: Fn(usize, usize) -> Connector,
        AP: Fn(KeyNumber) -> bool,
        PS: Fn(KeyDistance) -> Vec<(Stack<T>, Ratio<StackCoeff>)>,
        PA: Fn(KeyNumber) -> Vec<(Stack<T>, Ratio<StackCoeff>)>,
        PR: Fn(&RodSpec) -> Stack<T>,
    {
        workspace.n_keys = keys.len();
        workspace.keys.clear();
        workspace.keys.extend_from_slice(keys);

        let next_index =
            workspace.collect_intervals(which_connector, provide_candidate_springs, provide_rod);
        workspace.collect_anchors(next_index, is_note_anchored, provide_candidate_anchors);

        Self {
            workspace,
            solver,
            next_try: true,
            next_try_prepared: true,
        }
    }

    pub fn next(
        &mut self,
    ) -> Result<Option<(ArrayView2<Ratio<StackCoeff>>, bool, Energy)>, lu::LUErr> {
        if !self.next_try_prepared {
            self.next_try = self.workspace.prepare_next_candidate();
        }

        if self.next_try {
            let n_nodes = self.workspace.n_keys;
            let n_lengths = self.workspace.current_springs.len()
                + self.workspace.current_anchors.len()
                + self.workspace.current_rods.len();
            let n_base_lengths = T::num_intervals();

            self.solver
                .prepare_system(n_nodes, n_lengths, n_base_lengths);

            for (k, v) in self.workspace.current_anchors.iter() {
                let (position, stiffness) =
                    &self.workspace.memoed_anchors.get(&v.memo_key).expect(
                        "Solutions::next(): no candidate intervals found for fixed spring.",
                    )[v.current_candidate_index];
                self.solver
                    .add_fixed_spring(*k, v.solver_length_index, *stiffness);
                self.solver
                    .define_length(v.solver_length_index, position.actual_coefficients());
            }

            for ((i, j), v) in self.workspace.current_springs.iter() {
                let (length, stiffness) = &self
                    .workspace
                    .memoed_springs
                    .get(&v.memo_key)
                    .expect("Solutions::next(): no candidate intervals found for spring.")
                    [v.current_candidate_index];
                self.solver
                    .add_spring(*i, *j, v.solver_length_index, *stiffness);
                self.solver
                    .define_length(v.solver_length_index, length.actual_coefficients());
            }

            for ((i, j), v) in self.workspace.current_rods.iter() {
                self.solver.add_rod(*i, *j, v.solver_length_index);

                let length = self
                    .workspace
                    .memoed_rods
                    .get(&v.memo_key)
                    .expect("Solutions::next(): no stack found for rod.")
                    .actual_coefficients();

                self.solver.define_length(v.solver_length_index, length);
            }

            let solution = self.solver.solve()?;
            let energy = self.workspace.energy_in(solution);
            let relaxed = self.workspace.relaxed_in(solution);

            self.next_try_prepared = false;

            Ok(Some((solution, relaxed, energy)))
        } else {
            Ok(None {})
        }
    }
}

pub struct IntervalSolutions<'a, T: StackType> {
    workspace: &'a mut Workspace<T>,
    solver: &'a mut Solver,
    anchor_length_index: usize,
    zero_coeffs: Array1<Ratio<StackCoeff>>, // constant zeros
    next_try: bool,
    next_try_prepared: bool,
}

impl<'a, T: StackType + Eq + std::fmt::Debug> IntervalSolutions<'a, T> {
    pub fn new<WC, PS, PR>(
        workspace: &'a mut Workspace<T>,
        solver: &'a mut Solver,
        keys: &[KeyNumber],
        which_connector: WC,
        provide_candidate_springs: PS,
        provide_rod: PR,
    ) -> Self
    where
        WC: Fn(usize, usize) -> Connector,
        PS: Fn(KeyDistance) -> Vec<(Stack<T>, Ratio<StackCoeff>)>,
        PR: Fn(&RodSpec) -> Stack<T>,
    {
        workspace.n_keys = keys.len();
        workspace.keys.clear();
        workspace.keys.extend_from_slice(keys);

        let anchor_length_index =
            workspace.collect_intervals(which_connector, provide_candidate_springs, provide_rod);

        IntervalSolutions {
            workspace,
            solver,
            anchor_length_index,
            zero_coeffs: Array1::zeros(T::num_intervals()),
            next_try: true,
            next_try_prepared: true,
        }
    }

    pub fn next(
        &mut self,
    ) -> Result<Option<(ArrayView2<Ratio<StackCoeff>>, bool, Energy)>, lu::LUErr> {
        if !self.next_try_prepared {
            self.next_try = self.workspace.prepare_next_spring_candidate();
        }

        if self.next_try {
            let n_nodes = self.workspace.n_keys;
            let n_lengths = self.anchor_length_index + 2;
            let n_base_lengths = T::num_intervals();
            self.solver
                .prepare_system(n_nodes, n_lengths, n_base_lengths);

            self.solver
                .define_length(self.anchor_length_index, self.zero_coeffs.view());
            self.solver
                .add_fixed_spring(0, self.anchor_length_index, 1.into());

            for ((i, j), v) in self.workspace.current_springs.iter() {
                let (length, stiffness) =
                    &self.workspace.memoed_springs.get(&v.memo_key).expect(
                        "IntervalSolutions::next(): no candidate intervals found for spring.",
                    )[v.current_candidate_index];
                self.solver
                    .add_spring(*i, *j, v.solver_length_index, *stiffness);
                self.solver
                    .define_length(v.solver_length_index, length.actual_coefficients());
            }

            for ((i, j), v) in self.workspace.current_rods.iter() {
                self.solver.add_rod(*i, *j, v.solver_length_index);

                let length = self
                    .workspace
                    .memoed_rods
                    .get(&v.memo_key)
                    .expect("IntervalSolutions::next(): no stack found for rod.")
                    .actual_coefficients();

                self.solver.define_length(v.solver_length_index, length);
            }

            let solution = self.solver.solve()?;
            let relaxed = self.workspace.interval_relaxed_in(solution);
            let energy = self.workspace.interval_energy_in(solution);

            self.next_try_prepared = false;

            Ok(Some((solution, relaxed, energy)))
        } else {
            Ok(None {})
        }
    }
}

impl<T: StackType + Eq + std::fmt::Debug> Workspace<T> {
    /// meanings of arguments:
    /// - `initial_n_keys`: How many simultaneously sounding keys do you expect this workspace to
    ///    be used for? Choosing a big value will potentially prevent re-allocations, at the cost of
    ///    wasting space.
    /// - `memo_springs`, `memo_anchors` and `memo_rodss`: Should sizes, anchor posisitions (and
    ///    their stiffnesses) and the lengths of rods be remembered between successive
    ///    calls to [Self::compute_best_solution]?
    pub fn new(
        initial_n_keys: usize,
        memo_springs: bool,
        memo_anchors: bool,
        memo_rods: bool,
    ) -> Self {
        Workspace {
            n_keys: 0,
            keys: Vec::with_capacity(initial_n_keys),
            memo_springs,
            memo_anchors,
            memo_rods,
            memoed_springs: HashMap::new(),
            memoed_anchors: HashMap::new(),
            memoed_rods: HashMap::new(),
            current_springs: BTreeMap::new(),
            current_anchors: BTreeMap::new(),
            current_rods: BTreeMap::new(),
        }
    }

    /// meanings of arguments:
    ///
    /// - `keys`: a list of MIDI key number of currently sounding keys (or at least, keys that you
    ///   want to consider together)
    /// - `is_note_anchored` returns true iff the note with the given MIDI key number should be
    ///   attached to a "fixed spring". Use this if you have a "tuning reference" for the note.
    /// - `which_connector` returns the kind of connection that should be used between the notes
    ///   with the two given indices in `keys`. The connection can be one of:
    ///   - [Connector::None]: The tuning of the two notes is not (directly) related.
    ///   - [Connector::Rod]: The two notes must be tuned a specific interval apart.
    ///   - [Connector::Spring]: The tuning of the notes is related, but the interval between them
    ///     is flexible; it may be detuned if necessary.
    /// - `provide_candidate_springs` returns for each key distance several options for detune-able
    ///   intervals that might be used to instantiate the key distance. These are given as a
    ///   [Stack] together with a "stiffness" (i.e. how hard to detune)
    /// - `provide_candidate_anchors` does the same for absolute positions of notes.
    /// - `provide_rod` does the same for non-detuneable intervals.
    /// - `solver` is where the actual calculations happen.
    ///
    /// invariants:
    ///
    /// - The entries of `keys` must be unique.
    /// - The ordering of `keys` matters: Notes that come later (and the springs between them) are
    ///   more "stable" in the sense that alternative tunings are less likely to be picked.
    /// - The `provide_*``functions are only called when needed. In particular if the corresponding
    ///  `memo_*` argments were set to true in [Self::new], any spring, rod, or anchor candidates
    ///   will be computed at most once for each key number or key didstance. There are internal
    ///   fields in [Self] that (can) keep track of everything seen before, even between successive
    ///   calls to this function.
    pub fn best_solution<WC, AP, PS, PA, PR>(
        &mut self,
        keys: &[KeyNumber],
        is_note_anchored: AP,
        which_connector: WC,
        provide_candidate_springs: PS,
        provide_candidate_anchors: PA,
        provide_rod: PR,
        solver: &mut Solver,
    ) -> Result<(Array2<Ratio<StackCoeff>>, bool, Energy), lu::LUErr>
    where
        WC: Fn(usize, usize) -> Connector,
        AP: Fn(KeyNumber) -> bool,
        PS: Fn(KeyDistance) -> Vec<(Stack<T>, Ratio<StackCoeff>)>,
        PA: Fn(KeyNumber) -> Vec<(Stack<T>, Ratio<StackCoeff>)>,
        PR: Fn(&RodSpec) -> Stack<T>,
    {
        let mut solutions = Solutions::new(
            self,
            solver,
            keys,
            is_note_anchored,
            which_connector,
            provide_candidate_springs,
            provide_candidate_anchors,
            provide_rod,
        );

        let mut solution = Array2::zeros((keys.len(), T::num_intervals()));
        let mut energy = Energy::MAX;
        let mut relaxed = false;

        while !relaxed {
            match solutions.next()? {
                None {} => break,
                Some((new_solution, new_relaxed, new_energy)) => {
                    if new_relaxed {
                        relaxed = true;
                        energy = new_energy;
                        solution.assign(&new_solution);
                    } else {
                        if new_energy < energy {
                            energy = new_energy;
                            solution.assign(&new_solution);
                        }
                    }
                }
            }
        }

        Ok((solution, relaxed, energy))
    }

    /// This function anchors the position of the first key to the zero [Stack], and then tries to
    /// find the optimal intervals, given the connectors specified by the other arguments, which
    /// have the same meaning as for [Self::compute_best_solution].
    ///
    /// Changes [Self::current_energy] and [Self::relaxed]. These will pertain only to the state of
    /// non-anchor springs.
    ///
    /// Invariants:
    /// - won't touch [Self::current_anchors] and [Self::memoed_anchors]
    /// - will touch [Self::current_springs], [Self::current_rods], [Self::memoed_springs],
    ///   [Self::memoed_rods]
    pub fn best_intervals<WC, PS, PR>(
        &mut self,
        keys: &[KeyNumber],
        which_connector: WC,
        provide_candidate_springs: PS,
        provide_rod: PR,
        solver: &mut Solver,
    ) -> Result<(Array2<Ratio<StackCoeff>>, bool, Energy), lu::LUErr>
    where
        WC: Fn(usize, usize) -> Connector,
        PS: Fn(KeyDistance) -> Vec<(Stack<T>, Ratio<StackCoeff>)>,
        PR: Fn(&RodSpec) -> Stack<T>,
    {
        let mut solutions = IntervalSolutions::new(
            self,
            solver,
            keys,
            which_connector,
            provide_candidate_springs,
            provide_rod,
        );

        let mut solution = Array2::zeros((keys.len(), T::num_intervals()));
        let mut energy = Energy::MAX;
        let mut relaxed = false;

        while !relaxed {
            match solutions.next()? {
                None {} => break,
                Some((new_solution, new_relaxed, new_energy)) => {
                    if new_relaxed {
                        relaxed = true;
                        energy = new_energy;
                        solution.assign(&new_solution);
                    } else {
                        if new_energy < energy {
                            energy = new_energy;
                            solution.assign(&new_solution);
                        }
                    }
                }
            }
        }

        Ok((solution, relaxed, energy))
    }

    /// If you do anything mutating to [self] (in particular [Workspace::keys]) between
    /// calculating the IntervalSolution and calling this function, prepare for pain.
    ///
    /// invariants:
    /// - the `keys` must be the same ones that the `interval_solution` was calculated for.
    ///   (Strictly, it'll be logically sufficient for them to have the same relative positions.)
    pub fn best_anchoring<PA>(
        &mut self,
        interval_solution: Array2<Ratio<StackCoeff>>,
        keys: &[KeyNumber],
        anchored_key_indices: &[usize],
        provide_candidate_anchors: PA,
        solver: &mut Solver,
    ) -> Result<(Array2<Ratio<StackCoeff>>, bool, Energy), lu::LUErr>
    where
        PA: Fn(KeyNumber) -> Vec<(Stack<T>, Ratio<StackCoeff>)>,
    {
        self.n_keys = keys.len();
        self.keys.clear();
        self.keys.extend_from_slice(keys);

        let mut solver_length_index = self.n_keys - 1;

        self.current_anchors.clear();
        if !self.memo_anchors {
            self.memoed_anchors.clear();
        }
        for i in anchored_key_indices {
            let k = self.keys[*i];

            if !self.memoed_anchors.contains_key(&k) {
                self.memoed_anchors.insert(k, provide_candidate_anchors(k));
            }

            self.current_anchors.insert(
                *i,
                AnchorInfo {
                    current_candidate_index: 0,
                    memo_key: k,
                    solver_length_index,
                },
            );
            solver_length_index += 1;
        }

        let n_nodes = self.n_keys;
        let n_lengths = solver_length_index;
        let n_base_lengths = T::num_intervals();

        let mut energy = Energy::MAX;
        let mut relaxed = false;
        let mut solution = interval_solution;
        let mut next_try = true;

        let mut tmp = Array1::zeros(T::num_intervals());

        while next_try {
            solver.prepare_system(n_nodes, n_lengths, n_base_lengths);

            // Rods must be added after anchors (this is an invariant of [solver::Workspace::add_rod])
            for (k, v) in self.current_anchors.iter() {
                let (position, stiffness) = &self.memoed_anchors.get(&v.memo_key).expect(
                    "compute_best_anchoring: no candidate intervals found for fixed spring.",
                )[v.current_candidate_index];
                solver.add_fixed_spring(*k, v.solver_length_index, *stiffness);
                solver.define_length(v.solver_length_index, position.actual_coefficients());
            }

            for i in 1..self.n_keys {
                tmp.assign(&solution.row(i));
                tmp.scaled_add((-1).into(), &solution.row(0));
                solver.define_length(i - 1, tmp.view());
                solver.add_rod(0, i, i - 1);
            }

            let new_solution = solver.solve()?;

            let new_energy = self.anchor_energy_in(new_solution.view());
            let new_relaxed = self.anchor_relaxed_in(new_solution.view());

            if new_relaxed | (new_energy < energy) {
                solution.assign(&new_solution);
                energy = new_energy;
                relaxed = new_relaxed;
            }

            if relaxed {
                break;
            }

            next_try = self.prepare_next_anchor_candidate();
        }

        Ok((solution, relaxed, energy))
    }

    /// returns true iff there is a new candidate. Will try to change anchors first and then
    /// springs
    fn prepare_next_candidate(&mut self) -> bool {
        let anchors_changed = self.prepare_next_anchor_candidate();
        if anchors_changed {
            true
        } else {
            self.prepare_next_spring_candidate()
        }
    }

    /// like [Self::prepare_next_candidate], but only takes into account anchor springs
    fn prepare_next_anchor_candidate(&mut self) -> bool {
        for (_, v) in self.current_anchors.iter_mut() {
            let max_ix = self
                .memoed_anchors
                .get(&v.memo_key)
                .expect("prepeare_next_anchor_candidate: found no candidates for anchor")
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

    /// like [Self::prepare_next_candidate], but only takes into account interval (i.e.
    /// non-anchor) springs
    fn prepare_next_spring_candidate(&mut self) -> bool {
        for (_, v) in self.current_springs.iter_mut() {
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

    pub fn get_semitones(&self, solution: ArrayView2<Ratio<StackCoeff>>, i: usize) -> Semitones {
        let mut res = 60.0;
        for (j, c) in solution.row(i).iter().enumerate() {
            res += T::intervals()[j].semitones * *c.numer() as Semitones / *c.denom() as Semitones;
        }
        res
    }

    pub fn get_relative_semitones(
        &self,
        solution: ArrayView2<Ratio<StackCoeff>>,
        i: usize,
        j: usize,
    ) -> Semitones {
        self.get_semitones(solution, j) - self.get_semitones(solution, i)
    }

    /// Compute the energy stored in tensioned springs (== detuned intervals or notes) in the
    /// provided solution.
    ///
    /// Don't compare this number to zero to find out if there are detunings; use
    /// [Self::relaxed_in] for that purpose!
    fn energy_in(&self, solution: ArrayView2<Ratio<StackCoeff>>) -> Energy {
        self.anchor_energy_in(solution) + self.interval_energy_in(solution)
    }

    /// like [Self::energy_in], but only takes into account interval (i.e. non-anchor) springs.
    fn interval_energy_in(&self, solution: ArrayView2<Ratio<StackCoeff>>) -> Energy {
        let compute_length = |coeffs: ArrayView1<Ratio<StackCoeff>>| {
            let mut res = 0.0;
            for (j, c) in coeffs.iter().enumerate() {
                res += T::intervals()[j].semitones * *c.numer() as Energy / *c.denom() as Energy;
            }
            res
        };

        let mut res = 0.0;

        for ((i, j), v) in self.current_springs.iter() {
            let (stack, stiffness) = &self
                .memoed_springs
                .get(&v.memo_key)
                .expect("energy_in: no candidates found for spring.")[v.current_candidate_index];
            let length = compute_length(stack.actual_coefficients());
            if *stiffness != Ratio::ZERO {
                res += *stiffness.numer() as Energy / *stiffness.denom() as Energy
                    * (length - self.get_relative_semitones(solution, *i, *j)).powi(2);
            }
        }

        res
    }

    /// like [Self::energy_in], but only takes into account the anchor springs.
    fn anchor_energy_in(&self, solution: ArrayView2<Ratio<StackCoeff>>) -> Energy {
        let compute_length = |coeffs: ArrayView1<Ratio<StackCoeff>>| {
            let mut res = 0.0;
            for (j, c) in coeffs.iter().enumerate() {
                res += T::intervals()[j].semitones * *c.numer() as Energy / *c.denom() as Energy;
            }
            res
        };

        let mut res = 0.0;

        for (k, v) in self.current_anchors.iter() {
            let (stack, stiffness) = &self
                .memoed_anchors
                .get(&v.memo_key)
                .expect("anchor_energy_in: no candidates found for anchor.")
                [v.current_candidate_index];
            let position = 60.0 + compute_length(stack.actual_coefficients());
            if *stiffness != Ratio::ZERO {
                res += *stiffness.numer() as Energy / *stiffness.denom() as Energy
                    * (position - self.get_semitones(solution, *k)).powi(2);
            }
        }

        res
    }

    /// returns true iff all springs have their relaxed length (that is: there are no detuned
    /// intervals or notes) in the provided solution.
    fn relaxed_in(&self, solution: ArrayView2<Ratio<StackCoeff>>) -> bool {
        self.anchor_relaxed_in(solution) & self.interval_relaxed_in(solution)
    }

    /// like [Self::relaxed_in], but only takes into account anchor springs.
    fn anchor_relaxed_in(&self, solution: ArrayView2<Ratio<StackCoeff>>) -> bool {
        for (i, v) in self.current_anchors.iter() {
            let (stack, _) = &self
                .memoed_anchors
                .get(&v.memo_key)
                .expect("relaxed_in: no candidates found for anchor.")[v.current_candidate_index];
            for k in 0..T::num_intervals() {
                if stack.actual_coefficients()[k] != solution[[*i, k]] {
                    return false;
                }
            }
        }

        true
    }

    /// like [Self::relaxed_in], but only takes into account interval (i.e. non-anchor) springs.
    fn interval_relaxed_in(&self, solution: ArrayView2<Ratio<StackCoeff>>) -> bool {
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

    /// Computes a list of [Stack::target]s for the intervals in the current solution.
    ///
    /// If there are no tensioned sprrings, these correspond directly to the intervals in the
    /// solution. Otherwise, there is no "always correct choice" to guess the intended non-detuned
    /// intervals. These choices are made:
    ///
    /// - every interval that is fixed by a rod or a combination of rods will be kept.
    /// - springs and rods that come between more "stable" notes (i.e. the ones that come last in
    /// the `keys` arugments to functions like [Self::compute_best_solution]) are preferred.
    ///
    /// The order of the intervals is such that the interval from the `i`-th to the `j`-th note,
    /// where `0 <= i < j`, is stored at the index computed by
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
    /// expected invariants:
    /// - No zero intervals, i.e. every note occurs at most once.
    /// - Nothing is called between the computation of the solution and this function.
    pub fn current_interval_targets(&self) -> Vec<Array1<StackCoeff>> {
        let n = self.n_keys;
        let big_n = n * (n - 1) / 2;
        let mut res = vec![Array1::zeros(T::num_intervals()); big_n];
        let mut is_set = vec![false; big_n];

        let index = |i, j| n * i - i * (i + 1) / 2 + j - i - 1;

        let complete = |res: &mut Vec<Array1<StackCoeff>>, is_set: &mut Vec<bool>| {
            for i in 0..n {
                for j in (i + 1)..n {
                    for k in (j + 1)..n {
                        let ij = index(i, j);
                        let jk = index(j, k);
                        let ik = index(i, k);
                        let (s12, s3) = res.split_at_mut(jk);
                        let (s1, s2) = s12.split_at_mut(ik);
                        let a = &mut s1[ij];
                        let b = &mut s3[0];
                        let c = &mut s2[0];

                        match (is_set[ij], is_set[jk], is_set[ik]) {
                            (false, true, true) => {
                                a.clone_from(&c);
                                a.scaled_add(-1, &b);
                                is_set[ij] = true;
                            }
                            (true, false, true) => {
                                b.clone_from(&c);
                                b.scaled_add(-1, &a);
                                is_set[jk] = true;
                            }
                            (true, true, false) => {
                                c.clone_from(&a);
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
        for ((i, j), v) in self.current_rods.iter().rev() {
            let ij = index(*i, *j);
            if !is_set[ij] {
                res[ij].clone_from(
                    &self
                        .memoed_rods
                        .get(&v.memo_key)
                        .expect("current_interval_targets: no candidate found for rod.")
                        .target,
                );
                is_set[ij] = true;
            }
        }

        complete(&mut res, &mut is_set);

        // Again, back to front. Also: after the rods have been completed.
        for ((i, j), v) in self.current_springs.iter().rev() {
            let ij = index(*i, *j);
            if !is_set[ij] {
                res[ij].clone_from(
                    &self
                        .memoed_springs
                        .get(&v.memo_key)
                        .expect("current_interval_targets: no candidates found for spring.")
                        [v.current_candidate_index]
                        .0
                        .target,
                );
                is_set[ij] = true;
            }
        }

        complete(&mut res, &mut is_set);

        res
    }

    /// Return a list of [Stack::target]s for the notes in the current solution.
    ///
    /// If there are no tensioned sprrings, these correspond directly to the solution. Otherwise,
    /// there is no "always correct choice" to guess the intended non-detuned notes. This function
    /// uses the intervals to the first note, as provided by the `interval_targets` argument to
    /// determine the positions of all other notes.
    ///
    /// Expected invariants:
    /// - the `interval_targets` the result returned by [Self::current_interval_targets]. (Or, at
    /// least the prefix of the first `n-1` entries of that vector)
    pub fn current_anchor_targets(
        &self,
        interval_targets: &[Array1<StackCoeff>],
    ) -> Vec<Array1<StackCoeff>> {
        let n = self.n_keys;

        let (&first_anchor_index, first_anchor) = self
            .current_anchors
            .iter()
            .next()
            .expect("current_anchor_targets: No anchored notes");

        let first_anchor_target = &self
            .memoed_anchors
            .get(&first_anchor.memo_key)
            .expect("current_anchor_targets: no candidates found for anchor")
            [first_anchor.current_candidate_index]
            .0
            .target;

        let mut res = vec![Array1::zeros(T::num_intervals()); n];
        res[first_anchor_index].clone_from(first_anchor_target);
        if first_anchor_index != 0 {
            res[0].clone_from(first_anchor_target);
            res[0].scaled_add(-1, &interval_targets[first_anchor_index - 1]);
        }

        let (head, tail) = res.split_at_mut(1);
        for i in 0..(n - 1) {
            tail[i].clone_from(&head[0]);
            tail[i].scaled_add(1, &interval_targets[i]);
        }

        res
    }

    /// start_index must be the return value of [Self::collect_intervals].
    fn collect_anchors<AP, PA>(
        &mut self,
        start_index: usize,
        is_note_anchored: AP,
        provide_candidate_anchors: PA,
    ) where
        AP: Fn(KeyNumber) -> bool,
        PA: Fn(KeyNumber) -> Vec<(Stack<T>, Ratio<StackCoeff>)>,
    {
        self.current_anchors.clear();

        if !self.memo_anchors {
            self.memoed_anchors.clear();
        }

        let mut solver_length_index = start_index;
        let keys = &self.keys;

        for (i, &k) in keys.iter().enumerate() {
            if is_note_anchored(k) {
                if !self.memoed_anchors.contains_key(&k) {
                    self.memoed_anchors.insert(k, provide_candidate_anchors(k));
                }

                self.current_anchors.insert(
                    i,
                    AnchorInfo {
                        solver_length_index,
                        memo_key: k,
                        current_candidate_index: 0,
                    },
                );
                solver_length_index += 1;
            }
        }
    }

    /// Returns 1 plus the highest [SpringInfo::solver_length_index] of
    /// [RodInfo::solver_length_index] that it used. This can be used to continue adding the
    /// anchored connections with [Self::collect_anchors].
    fn collect_intervals<WC, PS, PR>(
        &mut self,
        which_connector: WC,
        provide_candidate_springs: PS,
        provide_rod: PR,
    ) -> usize
    where
        WC: Fn(usize, usize) -> Connector,
        PS: Fn(KeyDistance) -> Vec<(Stack<T>, Ratio<StackCoeff>)>,
        PR: Fn(&RodSpec) -> Stack<T>,
    {
        self.current_rods.clear();
        self.current_springs.clear();

        if !self.memo_springs {
            self.memoed_springs.clear();
        }

        if !self.memo_rods {
            self.memoed_rods.clear();
        }

        let keys = &self.keys;
        let n = self.n_keys;

        let mut solver_length_index = 0;

        for i in 0..n {
            for j in (i + 1)..n {
                match which_connector(i, j) {
                    Connector::Spring => {
                        let d = keys[j] as KeyDistance - keys[i] as KeyDistance;
                        if !self.memoed_springs.contains_key(&d) {
                            self.memoed_springs.insert(d, provide_candidate_springs(d));
                        }
                        self.current_springs.insert(
                            (i, j),
                            SpringInfo {
                                current_candidate_index: 0,
                                memo_key: d,
                                solver_length_index,
                            },
                        );
                        solver_length_index += 1;
                    }
                    Connector::Rod(spec) => {
                        //let d = keys[j] as KeyDistance - keys[i] as KeyDistance;
                        self.current_rods.insert(
                            (i, j),
                            RodInfo {
                                memo_key: spec, //vec![if d < 0 { (-d, -1) } else { (d, 1) }],
                                solver_length_index: 0, // This is a dummy initialisation. Will be
                                                // updated with something sensible later!
                            },
                        );
                    }
                    Connector::None => {}
                }
            }
        }

        let add_to_rodspec = |a: &mut RodSpec, d: KeyDistance, c: StackCoeff| {
            let mut d = d;
            let mut c = c;
            if d < 0 {
                d *= -1;
                c *= -1;
            }

            // the simmple linear search is best here: [RodSpec]s will be short. In the most common
            // case, they'll have length 1.
            match a.iter().position(|(x, _)| *x >= d) {
                Some(i) => {
                    if a[i].0 == d {
                        a[i].1 += c;
                    } else {
                        // a[i].0 > d
                        a.insert(i, (d, c));
                    }
                }
                None {} => a.push((d, c)),
            }
        };

        // This triple loop ensures the invariant of [solver::System::add_rod]
        for k in (0..n).rev() {
            for j in (0..k).rev() {
                for i in (0..j).rev() {
                    match self.current_rods.remove(&(j, k)) {
                        None => {}
                        Some(b) => match (
                            self.current_rods.get(&(i, j)),
                            self.current_rods.get(&(i, k)),
                        ) {
                            (None, None) => {
                                // put it back: we can't delete information
                                self.current_rods.insert((j, k), b);
                            }
                            (Some(a), None) => {
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
                                for (d, x) in a.memo_key.iter() {
                                    add_to_rodspec(&mut b_plus_a.memo_key, *d, *x);
                                }
                                self.current_rods.insert((i, k), b_plus_a);
                            }
                            (None, Some(c)) => {
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
                                for (_, x) in c_minus_b.memo_key.iter_mut() {
                                    *x *= -1;
                                }
                                for (d, x) in c.memo_key.iter() {
                                    add_to_rodspec(&mut c_minus_b.memo_key, *d, *x);
                                }
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

        for v in self.current_rods.values_mut() {
            v.solver_length_index = solver_length_index;
            solver_length_index += 1;
        }

        for spec in self.current_rods.values() {
            if !self.memoed_rods.contains_key(&spec.memo_key) {
                self.memoed_rods
                    .insert(spec.memo_key.clone(), provide_rod(&spec.memo_key));
            }
        }

        solver_length_index
    }
}

#[cfg(test)]
mod test {
    use ndarray::{arr1, arr2};
    use pretty_assertions::assert_eq;

    use crate::interval::stacktype::{
        fivelimit::mock::MockFiveLimitStackType, r#trait::FiveLimitStackType,
    };

    use super::*;

    #[test]
    fn test_collect_intervals() {
        type Irrelevant = crate::interval::stacktype::fivelimit::mock::MockFiveLimitStackType;
        let mut ws = Workspace::<Irrelevant>::new(1, false, false, false);

        ws.keys = vec![0, 1, 2, 3];
        ws.n_keys = ws.keys.len();
        ws.collect_intervals(
            |i, j| Connector::Rod(vec![(j as KeyDistance - i as KeyDistance, 1)]),
            |_| panic!("This will not be called, since there are no springs!"),
            |_| Stack::new_zero(), // irrelevant
        );
        assert_eq!(
            {
                let mut m = ws
                    .current_rods
                    .iter()
                    .map(|(a, b)| (*a, b.memo_key.clone()))
                    .collect::<Vec<_>>();
                m.sort_by(|a, b| a.0.cmp(&b.0));
                m
            },
            vec![
                ((0, 1), vec![(1, 1)]),
                ((0, 2), vec![(2, 1)]),
                ((0, 3), vec![(3, 1)]),
            ]
        );

        ws.keys = vec![0, 1, 2, 3, 4, 5];
        ws.n_keys = ws.keys.len();
        ws.collect_intervals(
            |i, j| {
                if (j - i) % 2 == 0 {
                    Connector::Rod(vec![(j as KeyDistance - i as KeyDistance, 1)])
                } else {
                    Connector::Spring
                }
            },
            |_| vec![],            // irrelevant
            |_| Stack::new_zero(), // irrelevant
        );
        assert_eq!(
            {
                let mut m = ws
                    .current_rods
                    .iter()
                    .map(|(a, b)| (*a, b.memo_key.clone()))
                    .collect::<Vec<_>>();
                m.sort_by(|a, b| a.0.cmp(&b.0));
                m
            },
            vec![
                ((0, 2), vec![(2, 1)]),
                ((0, 4), vec![(4, 1)]),
                ((1, 3), vec![(2, 1)]),
                ((1, 5), vec![(4, 1)]),
            ]
        );

        let k = vec![0, 2, 5, 7, 12, 14];
        ws.keys = k.clone();
        ws.n_keys = ws.keys.len();
        ws.collect_intervals(
            |i, j| {
                let d = k[j] - k[i];
                if (d % 12 == 0) | (d % 7 == 0) {
                    Connector::Rod(vec![(k[j] as KeyDistance - k[i] as KeyDistance, 1)])
                } else {
                    Connector::Spring
                }
            },
            |_| vec![],            // irrelevant
            |_| Stack::new_zero(), // irrelevant
        );
        assert_eq!(
            {
                let mut m = ws
                    .current_rods
                    .iter()
                    .map(|(a, b)| (*a, b.memo_key.clone()))
                    .collect::<Vec<_>>();
                m.sort_by(|a, b| a.0.cmp(&b.0));
                m
            },
            vec![
                ((0, 1), vec![(12, -1), (14, 1)]),
                ((0, 2), vec![(7, -1), (12, 1)]),
                ((0, 3), vec![(7, 1)]),
                ((0, 4), vec![(12, 1)]),
                ((0, 5), vec![(14, 1)]),
            ]
        );
    }

    #[test]
    fn test_compute_best_solution() {
        let mut ws = Workspace::<MockFiveLimitStackType>::new(1, true, true, true);
        let mut solver = Solver::new(1, 1, 1);

        let provide_candidate_springs = |d: KeyDistance| {
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
        };

        let provide_candidate_anchors = |i| provide_candidate_springs(i as KeyDistance - 60);

        let epsilon = 0.00000000000000001; // just a very small number. I don't care precisely.

        // if nothing else is given, the first option is picked
        let (solution, relaxed, energy) = ws
            .best_solution(
                &[60, 66],
                |i| i == 60,
                |_, _| Connector::Spring,
                provide_candidate_springs,
                provide_candidate_anchors,
                |_| panic!("This will never be called, since there are no rods"),
                &mut solver,
            )
            .unwrap();
        assert_eq!(
            solution,
            arr2(&[
                [0.into(), 0.into(), 0.into()],
                [(-1).into(), 2.into(), 1.into()], // tritone as major tone plus major third
            ])
        );
        assert!(energy < epsilon);
        assert!(relaxed);

        let interval_targets = ws.current_interval_targets();
        assert_eq!(interval_targets, vec![arr1(&[-1, 2, 1])]);
        assert_eq!(
            ws.current_anchor_targets(&interval_targets),
            vec![
                arr1(&[0.into(), 0.into(), 0.into()]),
                arr1(&[(-1).into(), 2.into(), 1.into()]),
            ]
        );

        // no new interval, so `provide_candidate_intervals` is never called.
        let (solution, relaxed, energy) = ws
            .best_solution(
                &[60, 66],
                |i| i == 60,
                |_, _| Connector::Spring,
                |_| panic!("This should not be called"),
                provide_candidate_anchors,
                |_| panic!("This will never be called, since there are no rods"),
                &mut solver,
            )
            .unwrap();
        assert_eq!(
            solution,
            arr2(&[
                [0.into(), 0.into(), 0.into()],
                [(-1).into(), 2.into(), 1.into()], // tritone as major tone plus major third
            ])
        );
        assert!(energy < epsilon);
        assert!(relaxed);

        // C major triad
        let (solution, relaxed, energy) = ws
            .best_solution(
                &[60, 64, 67],
                |i| i == 60,
                |_, _| Connector::Spring,
                provide_candidate_springs,
                provide_candidate_anchors,
                |_| panic!("This will never be called, since there are no rods"),
                &mut solver,
            )
            .unwrap();
        assert_eq!(
            solution,
            arr2(&[
                [0.into(), 0.into(), 0.into()],
                [0.into(), 0.into(), 1.into()],
                [0.into(), 1.into(), 0.into()],
            ])
        );
        assert!(energy < epsilon);
        assert!(relaxed);

        // E major triad
        let (solution, relaxed, energy) = ws
            .best_solution(
                &[64, 68, 71],
                |i| i == 64,
                |_, _| Connector::Spring,
                provide_candidate_springs,
                provide_candidate_anchors,
                |_| panic!("This will never be called, since there are no rods"),
                &mut solver,
            )
            .unwrap();
        assert_eq!(
            solution,
            arr2(&[
                [0.into(), 0.into(), 1.into()],
                [0.into(), 0.into(), 2.into()],
                [0.into(), 1.into(), 1.into()],
            ])
        );
        assert!(energy < epsilon);
        assert!(relaxed);

        let interval_targets = ws.current_interval_targets();
        assert_eq!(
            interval_targets,
            vec![arr1(&[0, 0, 1]), arr1(&[0, 1, 0]), arr1(&[0, 1, -1])]
        );
        assert_eq!(
            ws.current_anchor_targets(&interval_targets),
            vec![
                arr1(&[0.into(), 0.into(), 1.into()]),
                arr1(&[0.into(), 0.into(), 2.into()]),
                arr1(&[0.into(), 1.into(), 1.into()]),
            ]
        );

        // The three notes C,D,E: Because they are mentioned in this order, the interval C-D will
        // be the major tone. See the next example as well.
        let (solution, relaxed, energy) = ws
            .best_solution(
                &[64, 62, 60],
                |i| i == 60,
                |_, _| Connector::Spring,
                provide_candidate_springs,
                provide_candidate_anchors,
                |_| panic!("This will never be called, since there are no rods"),
                &mut solver,
            )
            .unwrap();
        assert_eq!(
            solution,
            arr2(&[
                [0.into(), 0.into(), 1.into()],
                [(-1).into(), 2.into(), 0.into()],
                [0.into(), 0.into(), 0.into()],
            ])
        );
        assert!(energy < epsilon);
        assert!(relaxed);

        // This is the same as before, but illustrates the relevance of the order in the `keys`
        // argument: Now, the tuning that makes the step from C to D a minor tone is preferred.
        //
        // Generally, intervals between notes that are mentioned early are less likely to have the
        // alternative sizes.
        let (solution, relaxed, energy) = ws
            .best_solution(
                &[60, 62, 64],
                |i| i == 60,
                |_, _| Connector::Spring,
                provide_candidate_springs,
                provide_candidate_anchors,
                |_| panic!("This will never be called, since there are no rods"),
                &mut solver,
            )
            .unwrap();
        assert_eq!(
            solution,
            arr2(&[
                [0.into(), 0.into(), 0.into()],
                [1.into(), (-2).into(), 1.into()],
                [0.into(), 0.into(), 1.into()],
            ])
        );
        assert!(energy < epsilon);
        assert!(relaxed);

        // D-flat major seventh on C
        let (solution, relaxed, energy) = ws
            .best_solution(
                &[60, 61, 65, 68],
                |i| i == 60,
                |_, _| Connector::Spring,
                provide_candidate_springs,
                provide_candidate_anchors,
                |_| panic!("This will never be called, since there are no rods"),
                &mut solver,
            )
            .unwrap();
        assert_eq!(
            solution,
            arr2(&[
                [0.into(), 0.into(), 0.into()],
                [1.into(), (-1).into(), (-1).into()], // diatonic semitone
                [1.into(), (-1).into(), 0.into()],
                [1.into(), 0.into(), (-1).into()],
            ])
        );
        assert!(energy < epsilon);
        assert!(relaxed);

        // D dominant seventh on C
        let (solution, relaxed, energy) = ws
            .best_solution(
                &[60, 62, 66, 69],
                |i| i == 60,
                |_, _| Connector::Spring,
                provide_candidate_springs,
                provide_candidate_anchors,
                |_| panic!("This will never be called, since there are no rods"),
                &mut solver,
            )
            .unwrap();
        assert_eq!(
            solution,
            arr2(&[
                [0.into(), 0.into(), 0.into()],
                [(-1).into(), 2.into(), 0.into()],
                [(-1).into(), 2.into(), 1.into()],
                [(-1).into(), 3.into(), 0.into()],
            ])
        );
        assert!(energy < epsilon);
        assert!(relaxed);

        // a single note: the first option is choosen
        let (solution, relaxed, energy) = ws
            .best_solution(
                &[69],
                |i| i == 69,
                |_, _| Connector::Spring,
                provide_candidate_springs,
                provide_candidate_anchors,
                |_| panic!("This will never be called, since there are no rods"),
                &mut solver,
            )
            .unwrap();
        assert_eq!(solution, arr2(&[[1.into(), (-1).into(), 1.into()],]));
        assert!(energy < epsilon);
        assert!(relaxed);

        // 69 chord cannot be in tune
        let (_solution, relaxed, energy) = ws
            .best_solution(
                &[60, 62, 64, 67, 69],
                |i| i == 60,
                |_, _| Connector::Spring,
                provide_candidate_springs,
                provide_candidate_anchors,
                |_| panic!("This will never be called, since there are no rods"),
                &mut solver,
            )
            .unwrap();
        assert!(energy > epsilon);
        assert!(!relaxed);

        let interval_targets = ws.current_interval_targets();
        assert_eq!(
            interval_targets,
            vec![
                // intervals from C
                arr1(&[-1, 2, 0]),
                arr1(&[0, 0, 1]),
                arr1(&[0, 1, 0]),
                arr1(&[1, -1, 1]),
                // intervals from D
                arr1(&[-1, 2, 0]),
                arr1(&[1, -1, 0]),
                arr1(&[0, 1, 0]),
                //intervals from E
                arr1(&[0, 1, -1]),
                arr1(&[1, -1, 0]),
                //intervals from G
                arr1(&[-1, 2, 0]),
            ]
        );
        assert_eq!(
            ws.current_anchor_targets(&interval_targets),
            vec![
                arr1(&[0, 0, 0]),
                arr1(&[-1, 2, 0]),
                arr1(&[0, 0, 1]),
                arr1(&[0, 1, 0]),
                arr1(&[1, -1, 1]),
            ]
        );

        // 69 chord with rods for fifhts
        let k = [60, 62, 64, 67, 69];
        let (solution, relaxed, energy) = ws
            .best_solution(
                &k,
                |i| i == 60,
                |i, j| {
                    if k[j] - k[i] == 7 {
                        Connector::Rod(vec![(k[j] as KeyDistance - k[i] as KeyDistance, 1)])
                    } else {
                        Connector::Spring
                    }
                },
                provide_candidate_springs,
                provide_candidate_anchors,
                |s| match s[..] {
                    [(7, n)] => {
                        Stack::from_pure_interval(MockFiveLimitStackType::fifth_index(), n)
                    }
                    _ => unreachable!(),
                },
                &mut solver,
            )
            .unwrap();

        //C-D fifth
        assert_eq!(solution.row(0), arr1(&[0.into(), 0.into(), 0.into()]));
        assert_eq!(solution.row(3), arr1(&[0.into(), 1.into(), 0.into()]));

        // D-A fifth:
        let mut delta = solution.row(4).to_owned();
        delta.scaled_add((-1).into(), &solution.row(1));
        assert_eq!(delta, arr1(&[0.into(), 1.into(), 0.into()]));

        // the D is between a minor and a major tone higher than C:
        let majortone = 12.0 * (9.0 as Semitones / 8.0).log2();
        let minortone = 12.0 * (10.0 as Semitones / 9.0).log2();
        assert!(ws.get_semitones(solution.view(), 1) < 60.0 + majortone);
        assert!(ws.get_semitones(solution.view(), 1) > 60.0 + minortone);

        // the distance between E and D is also between a major and minor tone:
        assert!(ws.get_relative_semitones(solution.view(), 1, 2) < majortone);
        assert!(ws.get_relative_semitones(solution.view(), 1, 2) > minortone);

        // the distance betwen C and D is the same as between G and A:
        assert_eq!(
            ws.get_relative_semitones(solution.view(), 0, 1),
            ws.get_relative_semitones(solution.view(), 3, 4)
        );

        assert!(energy > epsilon);
        assert!(!relaxed);

        let interval_targets = ws.current_interval_targets();
        assert_eq!(
            interval_targets,
            vec![
                // intervals from C
                arr1(&[-1, 2, 0]),
                arr1(&[0, 0, 1]),
                arr1(&[0, 1, 0]),
                arr1(&[1, -1, 1]),
                // intervals from D
                arr1(&[-1, 2, 0]),
                arr1(&[1, -1, 0]),
                arr1(&[0, 1, 0]),
                //intervals from E
                arr1(&[0, 1, -1]),
                arr1(&[1, -1, 0]),
                //intervals from G
                arr1(&[-1, 2, 0]),
            ]
        );
        assert_eq!(
            ws.current_anchor_targets(&interval_targets),
            vec![
                arr1(&[0, 0, 0]),
                arr1(&[-1, 2, 0]),
                arr1(&[0, 0, 1]),
                arr1(&[0, 1, 0]),
                arr1(&[1, -1, 1]),
            ]
        );

        // 69 chord with rods for fifhts and fourths. This forces a pythagorean third.
        let k = [60, 62, 64, 67, 69];
        let (solution, relaxed, energy) = ws
            .best_solution(
                &k,
                |i| i == 62,
                |i, j| {
                    if (k[j] - k[i] == 5) | (k[j] - k[i] == 7) {
                        Connector::Rod(vec![(k[j] as KeyDistance - k[i] as KeyDistance, 1)])
                    } else {
                        Connector::Spring
                    }
                },
                provide_candidate_springs,
                provide_candidate_anchors,
                |s| match s[..] {
                    [(7, n)] => Stack::from_target(vec![0.into(), n.into(), 0.into()]),
                    [(5, n)] => Stack::from_target(vec![n.into(), (-n).into(), 0.into()]),
                    [(5, n), (7, m)] => {
                        Stack::from_target(vec![n.into(), (m - n).into(), 0.into()])
                    }
                    _ => unreachable!(),
                },
                &mut solver,
            )
            .unwrap();
        assert_eq!(
            solution,
            arr2(&[
                [0.into(), 0.into(), 0.into()],
                [(-1).into(), 2.into(), 0.into()],
                [(-2).into(), 4.into(), 0.into()],
                [0.into(), 1.into(), 0.into()],
                [(-1).into(), 3.into(), 0.into()],
            ])
        );
        assert!(energy > epsilon);
        assert!(!relaxed);

        let interval_targets = ws.current_interval_targets();
        assert_eq!(
            interval_targets,
            vec![
                // intervals from C
                arr1(&[-1, 2, 0]),
                arr1(&[-2, 4, 0]),
                arr1(&[0, 1, 0]),
                arr1(&[-1, 3, 0]),
                // intervals from D
                arr1(&[-1, 2, 0]),
                arr1(&[1, -1, 0]),
                arr1(&[0, 1, 0]),
                //intervals from E
                arr1(&[2, -3, 0]),
                arr1(&[1, -1, 0]),
                //intervals from G
                arr1(&[-1, 2, 0]),
            ]
        );
        assert_eq!(
            ws.current_anchor_targets(&interval_targets),
            vec![
                arr1(&[0, 0, 0]),
                arr1(&[-1, 2, 0]),
                arr1(&[-2, 4, 0]),
                arr1(&[0, 1, 0]),
                arr1(&[-1, 3, 0]),
            ]
        );

        //// a slightly bigger example -- this overflows!
        //ws.compute_best_solution(
        //    //&[60, 62, 64, 67, 68, 73, 75],
        //    //&[75, 73, 68, 67, 64, 62, 60],
        //    &[75, 73, 70, 67, 64, 62, 60],
        //    |i| i == 60,
        //    |_, _, _| Connector::Spring,
        //    provide_candidate_springs,
        //    provide_candidate_anchors,
        //    |_| panic!("This will never be called, since there are no rods"),
        //    &mut solver,
        //)
        //.unwrap();
        //assert!(ws.current_energy() > epsilon);
        //assert!(!ws.relaxed());
    }
}
