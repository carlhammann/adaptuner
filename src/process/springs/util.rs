use std::{
    collections::{BTreeMap, HashMap, HashSet},
    hash::Hash,
    ops,
    sync::Arc,
    time::Instant,
};

use ndarray::{s, Array1, Array2, ArrayView1, ArrayView2};
use num_rational::Ratio;

use super::solver::Solver;
use crate::{
    interval::{
        base::Semitones,
        stack::{ScaledAdd, Stack},
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

/// An association list of key distances of the sub-intervals and multiplicities of these intervals.
///
/// invariants:
/// - length at least 1
/// - the key distances are always positive
/// - sorted by ascending key distance
pub type RodSpec = Vec<(KeyDistance, StackCoeff)>;

#[derive(Debug, Clone)]
pub enum OneOrMany<T> {
    One(T),
    Many(HashSet<T>),
}

impl<T: Eq + Hash> PartialEq for OneOrMany<T> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (OneOrMany::One(x), OneOrMany::One(y)) => x == y,
            (OneOrMany::Many(xs), OneOrMany::Many(ys)) => xs.is_subset(ys) & ys.is_subset(xs),
            _ => false,
        }
    }
}

impl<T: Eq + Hash> Eq for OneOrMany<T> {}

enum OneOrManyIter<'a, T> {
    One { elem: &'a T, used: bool },
    Many(std::collections::hash_set::Iter<'a, T>),
}

impl<'a, T> Iterator for OneOrManyIter<'a, T> {
    type Item = &'a T;
    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::One { elem, used } => {
                if *used {
                    None
                } else {
                    *used = true;
                    Some(elem)
                }
            }
            Self::Many(xs) => xs.next(),
        }
    }
}

impl<T: Hash + Eq + Clone> OneOrMany<T> {
    fn iter(&self) -> OneOrManyIter<T> {
        match self {
            OneOrMany::Many(xs) => OneOrManyIter::Many(xs.iter()),
            OneOrMany::One(x) => OneOrManyIter::One {
                elem: x,
                used: false,
            },
        }
    }
}

impl<T: Hash + Eq> OneOrMany<T> {
    fn extend(&mut self, mut other: Self) {
        match (&mut *self, &mut other) {
            (OneOrMany::One(_), _) => {}
            (_, OneOrMany::One(_)) => *self = other,
            (OneOrMany::Many(xs), OneOrMany::Many(ys)) => {
                for y in ys.drain() {
                    xs.insert(y);
                }
            }
        }
    }
}

impl<S, T> ScaledAdd<S> for OneOrMany<T>
where
    S: Copy,
    T: Hash + Eq + Clone + ScaledAdd<S>,
{
    fn scaled_add<P: ops::Deref<Target = Self>>(&mut self, scalar: S, other: P) {
        match (&mut *self, &*other) {
            (OneOrMany::One(x), OneOrMany::One(y)) => {
                x.scaled_add(scalar, y);
            }
            (OneOrMany::One(x), OneOrMany::Many(ys)) => {
                let mut xs = HashSet::new();
                for y in ys.iter() {
                    let mut tmp = x.clone();
                    tmp.scaled_add(scalar, y);
                    xs.insert(tmp);
                }
                *self = OneOrMany::Many(xs);
            }
            (OneOrMany::Many(xs), OneOrMany::One(y)) => {
                let mut new_xs = HashSet::new();
                for mut x in xs.drain() {
                    x.scaled_add(scalar, y);
                    new_xs.insert(x);
                }
                *xs = new_xs;
            }
            (OneOrMany::Many(xs), OneOrMany::Many(ys)) => {
                let mut new_xs = HashSet::new();
                for x in xs.drain() {
                    for y in ys.iter() {
                        let mut xplusy = x.clone();
                        xplusy.scaled_add(scalar, y);
                        new_xs.insert(xplusy);
                    }
                }
                *xs = new_xs;
            }
        }
    }
}

pub struct Workspace<T: StackType> {
    n_keys: usize,
    keys: Vec<KeyNumber>,
    memo_springs: bool,
    memo_anchors: bool,
    memo_rods: bool,
    memoed_springs: HashMap<KeyDistance, Vec<(Stack<T>, Ratio<StackCoeff>)>>,
    memoed_anchors: HashMap<KeyNumber, Vec<(Stack<T>, Ratio<StackCoeff>)>>,
    memoed_rods: HashMap<RodSpec, Stack<T>>,
    current_springs: BTreeMap<(usize, usize), SpringInfo>, // invariant: the key pairs are sorted ascendingly
    current_anchors: BTreeMap<usize, AnchorInfo>,
    current_rods: HashMap<(usize, usize), RodInfo>, // invariant: the key pairs are sorted ascendingly
}

pub struct Solutions<'a, T: StackType> {
    workspace: &'a mut Workspace<T>,
    solver: &'a mut Solver,
    next_try: bool,
    next_try_prepared: bool,
}

impl<'a, T: StackType + Hash + Eq + std::fmt::Debug> Solutions<'a, T> {
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
        WC: Fn(&[KeyNumber], usize, usize) -> Connector,
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
    ) -> Result<Option<(ArrayView2<Ratio<StackCoeff>>, bool, Semitones)>, lu::LUErr> {
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

impl<'a, T: StackType + Hash + Eq + std::fmt::Debug> IntervalSolutions<'a, T> {
    pub fn new<WC, PS, PR>(
        workspace: &'a mut Workspace<T>,
        solver: &'a mut Solver,
        keys: &[KeyNumber],
        which_connector: WC,
        provide_candidate_springs: PS,
        provide_rod: PR,
    ) -> Self
    where
        WC: Fn(&[KeyNumber], usize, usize) -> Connector,
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
    ) -> Result<Option<(ArrayView2<Ratio<StackCoeff>>, bool, Semitones)>, lu::LUErr> {
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

impl<T: StackType + Hash + Eq + std::fmt::Debug> Workspace<T> {
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
            current_rods: HashMap::new(),
        }
    }

    /// meanings of arguments:
    ///
    /// - `keys`: a list of MIDI key number of currently sounding keys (or at least, keys that you
    ///   want to consider together)
    /// - `is_note_anchored` returns true iff the note with the given MIDI key number should be
    ///   attached to a "fixed spring". Use this if you have a "tuning reference" for the note.
    /// - `which_connector` returns the kind of connection that should be used between the notes
    ///   with the two given key numbers. The connection can be one of:
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
    ) -> Result<(Array2<Ratio<StackCoeff>>, bool, Semitones), lu::LUErr>
    where
        WC: Fn(&[KeyNumber], usize, usize) -> Connector,
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
        let mut energy = Semitones::MAX;
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
    ) -> Result<(Array2<Ratio<StackCoeff>>, bool, Semitones), lu::LUErr>
    where
        WC: Fn(&[KeyNumber], usize, usize) -> Connector,
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
        let mut energy = Semitones::MAX;
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
    /// invariantx:
    /// - the `keys` must be the same ones that the `interval_solution` was calculated for.
    ///   (Strictlz, it'll be logically sufficient for them to have the same relative positions.)
    pub fn best_anchoring<PA>(
        &mut self,
        interval_solution: Array2<Ratio<StackCoeff>>,
        keys: &[KeyNumber],
        anchored_key_indices: &[usize],
        provide_candidate_anchors: PA,
        solver: &mut Solver,
    ) -> Result<(Array2<Ratio<StackCoeff>>, bool, Semitones), lu::LUErr>
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

        let mut energy = Semitones::MAX;
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

    /// Returns a list [Stack] the that the `i`-th note could be interpreted as in the current
    /// solution. If [Self::relaxed()], the returned set has size exactly one, otherwise it may
    /// be bigger: Different springs might have "wanted" the note to end up in different places.
    ///
    /// Call [Self::update_anchor_options] before calling this function and after some function
    /// like [Self::compute_best_solution].
    //pub fn get_anchor_options(&self, i: usize) -> Arc<HashSet<Stack<T>>> {
    //    if self.relaxed {
    //        let actual = self.current_solution.row(i).to_owned();
    //        let target = Array1::from_shape_fn(T::num_intervals(), |i| actual[i].to_integer());
    //        Arc::new(HashSet::from([Stack::from_target_and_actual(
    //            target, actual,
    //        )]))
    //    } else {
    //        if !self.anchor_options_are_up_to_date {
    //            panic!("get_anchor_options: options are not up to date");
    //        }
    //        self.current_anchor_options[&i].clone()
    //    }
    //}

    /// Like [Self::get_possible_stacks], only for intervals.
    ///
    /// Call [Self::update_anchor_options] before calling this function and after some function
    /// like [Self::compute_best_solution].
    //pub fn get_interval_options(&mut self, i: usize, j: usize) -> Arc<OneOrMany<Stack<T>>> {
    //    if self.relaxed {
    //        let mut actual = self.current_solution.row(j).to_owned();
    //        actual.scaled_add((-1).into(), &self.current_solution.row(i));
    //        let target = Array1::from_shape_fn(T::num_intervals(), |i| actual[i].to_integer());
    //        Arc::new(OneOrMany::One(Stack::from_target_and_actual(
    //            target, actual,
    //        )))
    //    } else {
    //        if !self.interval_options_are_up_to_date {
    //            panic!("get_interval_options: options are not up to date");
    //        }
    //        self.current_interval_options[&(i, j)].clone()
    //    }
    //}

    //pub fn update_anchor_options(&mut self) {
    //    //println!("n: {}", self.n_keys);
    //    //println!(
    //    //    "solution: {}",
    //    //    self.current_solution
    //    //        .slice(s![..self.n_keys, ..T::num_intervals()])
    //    //);
    //    //println!("relaxed: {}", self.relaxed);
    //    //println!("energy: {}", self.current_energy);
    //    if !self.interval_options_are_up_to_date {
    //        //println!("updating intervals ...");
    //        self.update_interval_options();
    //        //println!("... finished updating intervals");
    //    }
    //    //println!(
    //    //    "current_interval_options (updated): {:?}",
    //    //    self.current_interval_options
    //    //);
    //
    //    self.current_anchor_options.clear();
    //    self.current_anchor_options.reserve(self.n_keys);
    //    for i in 0..self.n_keys {
    //        self.current_anchor_options
    //            .insert(i, Arc::new(HashSet::new()));
    //    }
    //
    //    for (&i, k) in self.current_anchors.iter() {
    //        let anchor = &self
    //            .memoed_anchors
    //            .get(&k.memo_key)
    //            .expect("update_anchor_options: no stack found for anchor")
    //            [k.current_candidate_index]
    //            .0;
    //        // All of this `unwrap()` here and further on is ok, we added the empty set above.
    //        Arc::get_mut(self.current_anchor_options.get_mut(&i).unwrap())
    //            .unwrap()
    //            .insert(anchor.clone());
    //        for j in 0..i {
    //            for dist in self.current_interval_options[&(j, i)].iter() {
    //                let mut other = anchor.clone();
    //                other.scaled_add(-1, dist);
    //                Arc::get_mut(self.current_anchor_options.get_mut(&j).unwrap())
    //                    .unwrap()
    //                    .insert(other);
    //            }
    //        }
    //        for j in (i + 1)..self.n_keys {
    //            for dist in self.current_interval_options[&(i, j)].iter() {
    //                let mut other = anchor.clone();
    //                other.scaled_add(1, dist);
    //                Arc::get_mut(self.current_anchor_options.get_mut(&j).unwrap())
    //                    .unwrap()
    //                    .insert(other);
    //            }
    //        }
    //    }
    //
    //    self.anchor_options_are_up_to_date = true;
    //}

    //pub fn update_interval_options(&mut self) {
    //    self.current_interval_options.clear();
    //
    //    for ((i, j), k) in self.current_springs.iter() {
    //        let (stack, _) = &self
    //            .memoed_springs
    //            .get(&k.memo_key)
    //            .expect("update_interval_options: no stack found for spring")
    //            [k.current_candidate_index];
    //        //println!("spring from {i} to {j}: {:?}", stack);
    //        self.current_interval_options
    //            .insert((*i, *j), Arc::new(OneOrMany::Many([stack.clone()].into())));
    //    }
    //
    //    // it's important to add rods later: in case we're in the strange situation where both a
    //    // rod and a spring connect the same two notes, we will want to keep the rod since it holds
    //    // definite information
    //    for ((i, j), k) in self.current_rods.iter() {
    //        let stack = self
    //            .memoed_rods
    //            .get(&k.memo_key)
    //            .expect("update_interval_options: no stack found for rod");
    //        //println!("rod from {i} to {j}: {:?}", stack);
    //        self.current_interval_options
    //            .insert((*i, *j), Arc::new(OneOrMany::One(stack.clone())));
    //    }
    //
    //    for i in 0..self.n_keys {
    //        for j in (i + 1)..self.n_keys {
    //            for k in (j + 1)..self.n_keys {
    //                match (
    //                    self.current_interval_options.get(&(i, j)),
    //                    self.current_interval_options.get(&(j, k)),
    //                    self.current_interval_options.get(&(i, k)),
    //                ) {
    //                    (None {}, None {}, None {}) => {}
    //                    (Some(_), None {}, None {}) => {}
    //                    (None {}, Some(_), None {}) => {}
    //                    (None {}, None {}, Some(_)) => {}
    //                    (Some(a), Some(b), None {}) => {
    //                        let mut c = (**a).clone();
    //                        c.scaled_add(1, &**b);
    //                        self.current_interval_options.insert((i, k), Arc::new(c));
    //                    }
    //                    (Some(a), None {}, Some(c)) => {
    //                        let mut b = (**c).clone();
    //                        b.scaled_add(-1, &**a);
    //                        self.current_interval_options.insert((j, k), Arc::new(b));
    //                    }
    //                    (None {}, Some(b), Some(c)) => {
    //                        let mut a = (**c).clone();
    //                        a.scaled_add(-1, &**b);
    //                        self.current_interval_options.insert((i, j), Arc::new(a));
    //                    }
    //                    (Some(_), Some(_), Some(_)) => {
    //                        // All of this into_inner is OK, we only added the entris above.
    //                        let mut a = Arc::into_inner(
    //                            self.current_interval_options.remove(&(i, j)).unwrap(),
    //                        )
    //                        .unwrap();
    //                        let mut b = Arc::into_inner(
    //                            self.current_interval_options.remove(&(j, k)).unwrap(),
    //                        )
    //                        .unwrap();
    //                        let mut c = Arc::into_inner(
    //                            self.current_interval_options.remove(&(i, k)).unwrap(),
    //                        )
    //                        .unwrap();
    //
    //                        let mut new_a: Option<OneOrMany<Stack<T>>> = None;
    //                        let mut new_b: Option<OneOrMany<Stack<T>>> = None;
    //                        let mut new_c: Option<OneOrMany<Stack<T>>> = None;
    //                        match a {
    //                            OneOrMany::One(_) => {}
    //                            OneOrMany::Many(_) => {
    //                                let mut x = c.clone();
    //                                x.scaled_add(-1, &b);
    //                                new_a = Some(x);
    //                            }
    //                        }
    //                        match b {
    //                            OneOrMany::One(_) => {}
    //                            OneOrMany::Many(_) => {
    //                                let mut x = c.clone();
    //                                x.scaled_add(-1, &a);
    //                                new_b = Some(x);
    //                            }
    //                        }
    //                        match c {
    //                            OneOrMany::One(_) => {}
    //                            OneOrMany::Many(_) => {
    //                                let mut x = (a).clone();
    //                                x.scaled_add(1, &b);
    //                                new_c = Some(x);
    //                            }
    //                        }
    //
    //                        new_a.map(|x| a.extend(x));
    //                        new_b.map(|x| b.extend(x));
    //                        new_c.map(|x| c.extend(x));
    //                        self.current_interval_options.insert((i, j), Arc::new(a));
    //                        self.current_interval_options.insert((j, k), Arc::new(b));
    //                        self.current_interval_options.insert((i, k), Arc::new(c));
    //                    }
    //                }
    //            }
    //        }
    //    }
    //
    //    self.interval_options_are_up_to_date = true;
    //}

    /// Compute the energy stored in tensioned springs (== detuned intervals or notes) in the
    /// provided solution.
    ///
    /// Don't compare this number to zero to find out if there are detunings; use
    /// [Self::relaxed_in] for that purpose!
    fn energy_in(&self, solution: ArrayView2<Ratio<StackCoeff>>) -> Semitones {
        self.anchor_energy_in(solution) + self.interval_energy_in(solution)
    }

    /// like [Self::energy_in], but only takes into account interval (i.e. non-anchor) springs.
    fn interval_energy_in(&self, solution: ArrayView2<Ratio<StackCoeff>>) -> Semitones {
        let compute_length = |coeffs: ArrayView1<Ratio<StackCoeff>>| {
            let mut res = 0.0;
            for (j, c) in coeffs.iter().enumerate() {
                res +=
                    T::intervals()[j].semitones * *c.numer() as Semitones / *c.denom() as Semitones;
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
                res += *stiffness.numer() as Semitones / *stiffness.denom() as Semitones
                    * (length - self.get_relative_semitones(solution, *i, *j)).powi(2);
            }
        }

        res
    }

    /// like [Self::energy_in], but only takes into account the anchor springs.
    fn anchor_energy_in(&self, solution: ArrayView2<Ratio<StackCoeff>>) -> Semitones {
        let compute_length = |coeffs: ArrayView1<Ratio<StackCoeff>>| {
            let mut res = 0.0;
            for (j, c) in coeffs.iter().enumerate() {
                res +=
                    T::intervals()[j].semitones * *c.numer() as Semitones / *c.denom() as Semitones;
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
                res += *stiffness.numer() as Semitones / *stiffness.denom() as Semitones
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
        WC: Fn(&[KeyNumber], usize, usize) -> Connector,
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
                match which_connector(keys, i, j) {
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
        fivelimit::ConcreteFiveLimitStackType, r#trait::FiveLimitStackType,
    };

    use super::*;

    #[test]
    fn test_collect_intervals() {
        type Irrelevant = crate::interval::stacktype::fivelimit::ConcreteFiveLimitStackType;
        let mut ws = Workspace::<Irrelevant>::new(1, false, false, false);

        ws.keys = vec![0, 1, 2, 3];
        ws.n_keys = ws.keys.len();
        ws.collect_intervals(
            |_, i, j| Connector::Rod(vec![(j as KeyDistance - i as KeyDistance, 1)]),
            |_| panic!("This will not be called, since there are no springs!"),
            |_| Stack::new_zero(), // irrelevant
        );
        assert_eq!(
            {
                let mut m = ws
                    .current_rods
                    .drain()
                    .map(|(a, b)| (a, b.memo_key))
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
            |_, i, j| {
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
                    .drain()
                    .map(|(a, b)| (a, b.memo_key))
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

        ws.keys = vec![0, 2, 5, 7, 12, 14];
        ws.n_keys = ws.keys.len();
        ws.collect_intervals(
            |k, i, j| {
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
                    .drain()
                    .map(|(a, b)| (a, b.memo_key))
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
        let mut ws = Workspace::<ConcreteFiveLimitStackType>::new(1, true, true, true);
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
                |_, _, _| Connector::Spring,
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

        //assert_eq!(
        //    *ws.get_anchor_options(0),
        //    [Stack::from_target(vec![0, 0, 0])].into()
        //);
        //assert_eq!(
        //    *ws.get_anchor_options(1),
        //    [Stack::from_target(vec![-1, 2, 1])].into()
        //);
        //assert_eq!(
        //    *ws.get_interval_options(0, 1),
        //    OneOrMany::One(Stack::from_target(vec![-1, 2, 1]))
        //);
        //assert_eq!(
        //    *ws.get_interval_options(1, 0),
        //    OneOrMany::One(Stack::from_target(vec![1, -2, -1]))
        //);

        // no new interval, so `provide_candidate_intervals` is never called.
        let (solution, relaxed, energy) = ws
            .best_solution(
                &[60, 66],
                |i| i == 60,
                |_, _, _| Connector::Spring,
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
                |_, _, _| Connector::Spring,
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
                |_, _, _| Connector::Spring,
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

        //assert_eq!(
        //    *ws.get_anchor_options(0),
        //    [Stack::from_target(vec![0, 0, 1])].into()
        //);
        //assert_eq!(
        //    *ws.get_anchor_options(1),
        //    [Stack::from_target(vec![0, 0, 2])].into()
        //);
        //assert_eq!(
        //    *ws.get_anchor_options(2),
        //    [Stack::from_target(vec![0, 1, 1])].into()
        //);
        //assert_eq!(
        //    *ws.get_interval_options(0, 2),
        //    OneOrMany::One(Stack::from_target(vec![0, 1, 0]))
        //);
        //assert_eq!(
        //    *ws.get_interval_options(0, 1),
        //    OneOrMany::One(Stack::from_target(vec![0, 0, 1]))
        //);
        //assert_eq!(
        //    *ws.get_interval_options(1, 0),
        //    OneOrMany::One(Stack::from_target(vec![0, 0, -1]))
        //);
        //

        // The three notes C,D,E: Because they are mentioned in this order, the interval C-D will
        // be the major tone. See the next example as well.
        let (solution, relaxed, energy) = ws
            .best_solution(
                &[64, 62, 60],
                |i| i == 60,
                |_, _, _| Connector::Spring,
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
                |_, _, _| Connector::Spring,
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
                |_, _, _| Connector::Spring,
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
                |_, _, _| Connector::Spring,
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
                |_, _, _| Connector::Spring,
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
        let (solution, relaxed, energy) = ws
            .best_solution(
                &[60, 62, 64, 67, 69],
                |i| i == 60,
                |_, _, _| Connector::Spring,
                provide_candidate_springs,
                provide_candidate_anchors,
                |_| panic!("This will never be called, since there are no rods"),
                &mut solver,
            )
            .unwrap();
        assert!(energy > epsilon);
        assert!(!relaxed);

        // 69 chord with rods for fifhts
        let (solution, relaxed, energy) = ws
            .best_solution(
                &[60, 62, 64, 67, 69],
                |i| i == 60,
                |k, i, j| {
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
                        Stack::from_pure_interval(ConcreteFiveLimitStackType::fifth_index(), n)
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

        // 69 chord with rods for fifhts and fourths. This forces a pythagorean third.
        let (solution, relaxed, energy) = ws
            .best_solution(
                &[60, 62, 64, 67, 69],
                |i| i == 60,
                |k, i, j| {
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

    //#[test]
    //fn test_interval_and_anchor_options() {
    //    let mut ws = Workspace::<ConcreteFiveLimitStackType>::new(1, false, false, false);
    //    let mut solver = Solver::new(1, 1, 1);
    //
    //    let epsilon = 0.00000000000000001; // just a very small number. I don't care precisely.
    //
    //    // a third cannot be two major tones.
    //    ws.compute_best_solution(
    //        &[60, 62, 64],
    //        |i| i == 60,
    //        |_, _, _| Connector::Spring,
    //        |d| match d {
    //            2 => vec![(Stack::from_target(vec![-1, 2, 0]), 1.into())],
    //            4 => vec![(Stack::from_target(vec![0, 0, 1]), 1.into())],
    //            _ => unreachable!(),
    //        },
    //        |_| vec![(Stack::from_target(vec![0, 0, 0]), 1.into())],
    //        |_| panic!("This will never be called, since there are no rods"),
    //        &mut solver,
    //    )
    //    .unwrap();
    //    assert!(!ws.relaxed());
    //    assert!(ws.current_energy() > epsilon);
    //
    //    ws.update_interval_options();
    //    assert_eq!(
    //        *ws.get_interval_options(0, 1),
    //        OneOrMany::Many(
    //            [
    //                Stack::from_target(vec![-1, 2, 0]),
    //                Stack::from_target(vec![1, -2, 1])
    //            ]
    //            .into()
    //        )
    //    );
    //    assert_eq!(
    //        *ws.get_interval_options(1, 2),
    //        OneOrMany::Many(
    //            [
    //                Stack::from_target(vec![-1, 2, 0]),
    //                Stack::from_target(vec![1, -2, 1])
    //            ]
    //            .into()
    //        )
    //    );
    //    assert_eq!(
    //        *ws.get_interval_options(0, 2),
    //        OneOrMany::Many(
    //            [
    //                Stack::from_target(vec![0, 0, 1]),
    //                Stack::from_target(vec![-2, 4, 0])
    //            ]
    //            .into()
    //        )
    //    );
    //
    //    ws.update_anchor_options();
    //    assert_eq!(
    //        *ws.get_anchor_options(0),
    //        [Stack::from_target(vec![0, 0, 0])].into()
    //    );
    //    assert_eq!(
    //        *ws.get_anchor_options(1),
    //        [
    //            Stack::from_target(vec![-1, 2, 0]),
    //            Stack::from_target(vec![1, -2, 1])
    //        ]
    //        .into()
    //    );
    //    assert_eq!(
    //        *ws.get_anchor_options(2),
    //        [
    //            Stack::from_target(vec![0, 0, 1]),
    //            Stack::from_target(vec![-2, 4, 0])
    //        ]
    //        .into()
    //    );
    //}
}
