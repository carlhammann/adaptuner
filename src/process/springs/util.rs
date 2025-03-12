use std::{
    collections::{BTreeMap, HashMap},
    marker::PhantomData,
};

use ndarray::{s, Array1, Array2, ArrayView1, ArrayView2};
use num_rational::Ratio;

use super::solver;
use crate::{
    interval::{
        base::Semitones,
        stacktype::r#trait::{StackCoeff, StackType},
    },
    util::lu,
};

pub enum Connector {
    Spring,
    Rod,
    None,
}

#[derive(Hash, PartialEq, Eq, Debug)]
enum ConnectedKeys {
    FixedSpring(usize),
    Spring(usize, usize),
    Rod(usize, usize),
}

/// The order matters:
///
/// 1. Rods must come after springs and fixed springs. Otherwise, [Workspace::solve_current_candidate]
///    will add rods before springs, and thus break the invariant of [solver::System::add_rod].
///
/// 2. Rods and springs should be ordered from top to bottom, so that
///    [Workspace::try_next_candidate] will choose the alternative candidates first for high notes.
///    The thinking is that it is better to have a detuned interval between two high notes than
///    between two low notes.
///
/// 3. Similarly, fixed srpings sould come before normal springs. The latter correspond to harmonic
///    intervals and should be preferred over the melodic intervals described bz fixed springs.
impl PartialOrd for ConnectedKeys {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        use std::cmp::Ordering::{Greater, Less};

        Some(match (self, other) {
            (ConnectedKeys::FixedSpring(a), ConnectedKeys::FixedSpring(x)) => a.cmp(x).reverse(),
            (ConnectedKeys::FixedSpring(_), ConnectedKeys::Spring(_, _)) => Less,
            (ConnectedKeys::FixedSpring(_), ConnectedKeys::Rod(_, _)) => Less,
            (ConnectedKeys::Spring(_, _), ConnectedKeys::FixedSpring(_)) => Greater,
            (ConnectedKeys::Spring(a, b), ConnectedKeys::Spring(x, y)) => {
                (a, b).cmp(&(x, y)).reverse()
            }
            (ConnectedKeys::Spring(_, _), ConnectedKeys::Rod(_, _)) => Less,
            (ConnectedKeys::Rod(_, _), ConnectedKeys::FixedSpring(_)) => Greater,
            (ConnectedKeys::Rod(_, _), ConnectedKeys::Spring(_, _)) => Greater,
            (ConnectedKeys::Rod(a, b), ConnectedKeys::Rod(x, y)) => (a, b).cmp(&(x, y)).reverse(),
        })
    }
}

impl Ord for ConnectedKeys {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.partial_cmp(other).unwrap()
    }
}

#[derive(Debug)]
struct IntervalInfo {
    index: usize,
    key_or_distance: KeyOrDistance,
    current_candidate_index: usize,
}

type KeyDistance = i8;
type KeyNumber = u8;

#[derive(PartialEq, Eq, Hash, Debug)]
enum KeyOrDistance {
    Key(KeyNumber),
    Distance(KeyDistance),
}

struct Workspace<T: StackType> {
    _phantom: PhantomData<T>,
    memo_intervals: bool,
    memo_note_anchors: bool,
    n_keys: usize,
    key_distances: Array2<KeyDistance>,
    candidates: HashMap<KeyOrDistance, (Array2<Ratio<StackCoeff>>, Array1<Ratio<StackCoeff>>)>,
    current_connections: BTreeMap<ConnectedKeys, IntervalInfo>,
    best_solution: Array2<Ratio<StackCoeff>>,
    best_energy: Semitones, // May not be exactly zero even if `relaxed` is true, due to
    // floating point imprecisions
    relaxed: bool,
}

impl<T: StackType> Workspace<T> {
    /// meanings of arguments:
    /// - `initial_n_keys`: How many simultaneously sounding keys do you expect this workspace to
    ///    be used for? Choosing a big value will potentially prevent re-allocations, at the cost of
    ///    wasting space.
    /// - `memo_intervals` and `memo_notes`: Should sizes of intervals or "anchor" posisitions of
    ///    notes be remembered between successive calls of [Self::compute_best_solution]?
    pub fn new(initial_n_keys: usize, memo_intervals: bool, memo_note_anchors: bool) -> Self {
        Workspace {
            _phantom: PhantomData,
            memo_intervals,
            memo_note_anchors,
            n_keys: 0,
            key_distances: Array2::zeros((initial_n_keys, initial_n_keys)),
            candidates: HashMap::new(),
            current_connections: BTreeMap::new(),
            best_solution: Array2::zeros((initial_n_keys, T::num_intervals())),
            best_energy: Semitones::MAX,
            relaxed: false,
        }
    }

    /// Call [Self::compute_best_solution] first.
    pub fn best_solution(&self) -> ArrayView2<Ratio<StackCoeff>> {
        self.best_solution
            .slice(s![..self.n_keys, ..T::num_intervals()])
    }

    /// Call [Self::compute_best_solution] first.
    pub fn best_energy(&self) -> Semitones {
        self.best_energy
    }

    /// Call [Self::compute_best_solution] first.
    pub fn relaxed(&self) -> bool {
        self.relaxed
    }

    ///  The ordering of `keys` matters: Notes that come earlier (and the intervals between them)
    ///  are more "stable" in the sense that alternative tunings are less likely to be picked
    pub fn compute_best_solution<'a, WC, AP, PI, PN>(
        &mut self,
        keys: &[KeyNumber],
        is_note_anchored: AP,
        which_connector: WC,
        provide_candidate_intervals: PI,
        provide_candidate_notes: PN,
        solver_workspace: &mut solver::Workspace,
    ) -> Result<(), lu::LUErr>
    where
        WC: Fn(KeyNumber, KeyNumber) -> Connector,
        AP: Fn(KeyNumber) -> bool,
        PI: Fn(KeyDistance) -> (Array2<Ratio<StackCoeff>>, Array1<Ratio<StackCoeff>>),
        PN: Fn(KeyNumber) -> (Array2<Ratio<StackCoeff>>, Array1<Ratio<StackCoeff>>),
    {
        self.n_keys = keys.len();
        self.collect_rod_and_spring_key_distances(&keys, which_connector);
        let next_index =
            self.collect_relative_intervals_and_connections(provide_candidate_intervals);
        self.collect_anchored_intervals_and_connections(
            next_index,
            keys,
            is_note_anchored,
            provide_candidate_notes,
        );

        println!("current_connections:\n {:?}\n\n", self.current_connections);
        println!("candidate_intervals:\n {:?}\n\n", self.candidates);

        self.best_energy = Semitones::MAX;

        self.solve_current_candidate(solver_workspace)?;
        while !self.relaxed & self.prepare_next_candidate() {
            self.solve_current_candidate(solver_workspace)?;
        }
        Ok(())
    }

    /// returns true iff there is a new candidate. In that case, call [Self::solve_current_candidate]
    /// again to start solving the new candidate.
    ///
    /// The correct behaviour of this function depends on the implementation of `PartialOrd` for
    /// [ConnectedKeys].
    fn prepare_next_candidate(&mut self) -> bool {
        for (_, conn) in self.current_connections.iter_mut() {
            let keydist = &conn.key_or_distance;
            let (intervals, _) = &self.candidates[keydist];
            let max_ix = intervals.shape()[0] - 1;
            if conn.current_candidate_index < max_ix {
                conn.current_candidate_index += 1;
                return true;
            } else {
                conn.current_candidate_index = 0;
            }
        }

        return false;
    }

    /// expected invariants:
    /// - `self.key_distances` has been filled with [collect_rod_and_spring_key_distances].
    /// - `self.candidate_intervals`, `self.current_lengths`, and `self.current_connections` have been initialised with
    ///   [Self::collect_relative_intervals_and_connections] or updated with [Self::prepare_next_candidate].
    fn solve_current_candidate(
        &mut self,
        solver_workspace: &mut solver::Workspace,
    ) -> Result<(), lu::LUErr> {
        let n_nodes = self.n_keys;
        let n_lengths = self.current_connections.len();
        let n_base_lengths = T::num_intervals();

        let mut system = solver_workspace.prepare_system(n_nodes, n_lengths, n_base_lengths);

        // Rods must be added after springs. The Ord implementation of [ConnectedKeys] ensures
        // this.
        for (keys, conn) in self.current_connections.iter() {
            match keys {
                ConnectedKeys::Rod(i, j) => {
                    system.add_rod(*i, *j, conn.index);
                    println!("add_rod({}, {}, {})", i, j, conn.index);

                    let (candidate_lengths,_) = self
                        .candidates
                        .get(&conn.key_or_distance)
                        .expect("solve_current_candidate: no candidate intervals found for rod key distance. This should never happen.");

                    let length = candidate_lengths.row(conn.current_candidate_index);
                    system.define_length(conn.index, length);
                }
                ConnectedKeys::Spring(i, j) => {
                    let (candidate_lengths, candidate_stiffnesses) = self
                        .candidates
                        .get(&conn.key_or_distance)
                        .expect("solve_current_candidate: no candidate intervals found for spring key distance. This should never happen.");

                    let stiffness = candidate_stiffnesses[conn.current_candidate_index];
                    system.add_spring(*i, *j, conn.index, stiffness);
                    println!("add_spring({}, {}, {}, {})", i, j, conn.index, stiffness);

                    let length = candidate_lengths.row(conn.current_candidate_index);
                    system.define_length(conn.index, length);
                }
                ConnectedKeys::FixedSpring(i) => {
                    let (candidate_lengths, candidate_stiffnesses) = self
                        .candidates
                        .get(&conn.key_or_distance)
                        .expect("solve_current_candidate: no candidate intervals found for fixed spring key. This should never happen.");

                    let stiffness = candidate_stiffnesses[conn.current_candidate_index];
                    system.add_fixed_spring(*i, conn.index, stiffness);
                    println!("add_fixed_spring({}, {}, {})", i, conn.index, stiffness);

                    let length = candidate_lengths.row(conn.current_candidate_index);
                    system.define_length(conn.index, length);
                }
            }
        }

        let solution = system.solve()?;

        let energy = self.energy_in(solution.view());
        let relaxed = self.relaxed_in(solution.view());

        println!("solution:\n{}", solution);
        println!("energy: {}\n\n\n", energy);

        // `self.best_solution.shape()[1]` and `solution.shape()[1]` are always equal to `T::num_intervals()`.
        if self.best_solution.shape()[0] < solution.shape()[0] {
            self.best_solution = Array2::zeros(solution.raw_dim());
        }

        if relaxed | (energy < self.best_energy) {
            self.best_solution
                .slice_mut(s![..self.n_keys, ..T::num_intervals()])
                .assign(&solution);
            self.best_energy = energy;
            self.relaxed = relaxed;
        }

        Ok(())
    }

    /// Return the fractional MIDI note number of the `i`-th currently considered note, as
    /// prescribed by the current best solution.
    ///
    /// The origin is middle C, MIDI note number 60.0
    pub fn get_semitones(&self, i: usize) -> Semitones {
        self.get_semitones_in(i, self.best_solution.view())
    }

    fn get_semitones_in(&self, i: usize, solution: ArrayView2<Ratio<StackCoeff>>) -> Semitones {
        let mut res = 60.0;
        for (j, c) in solution.row(i).iter().enumerate() {
            res += T::intervals()[j].semitones * *c.numer() as Semitones / *c.denom() as Semitones;
        }
        res
    }

    /// Return the size of the interval from the `i`-th to the `j`-th currently considered note, as
    /// a fractional MIDI note number, as prescribed by the current best solution.
    pub fn get_relative_semitones(&self, i: usize, j: usize) -> Semitones {
        self.get_semitones(j) - self.get_semitones(i)
    }

    pub fn get_relative_semitones_in(
        &self,
        i: usize,
        j: usize,
        solution: ArrayView2<Ratio<StackCoeff>>,
    ) -> Semitones {
        self.get_semitones_in(j, solution) - self.get_semitones_in(i, solution)
    }

    /// Compute the energy stored in tensioned springs (== detuned intervals or notes) in the
    /// provided solution.
    ///
    /// Don't compare this number to zero to find out if there are detunings; use
    /// [Self::relaxed_in] for that purpose!
    fn energy_in(&self, solution: ArrayView2<Ratio<StackCoeff>>) -> Semitones {
        let mut res = 0.0;

        let compute_length = |coeffs: ArrayView1<Ratio<StackCoeff>>| {
            let mut res = 0.0;
            for (j, c) in coeffs.iter().enumerate() {
                res +=
                    T::intervals()[j].semitones * *c.numer() as Semitones / *c.denom() as Semitones;
            }
            res
        };

        let target_length_and_stiffness = |d: &KeyOrDistance, i: usize| {
            let (intervals, stiffnesses) = self
                .candidates
                .get(d)
                .expect("energy_in: no candidate intervals found for key distance. This should never happen.");
            let length = compute_length(intervals.row(i));
            let stiffness = stiffnesses[i];
            (length, stiffness)
        };

        for (keys, conn) in self.current_connections.iter() {
            match keys {
                ConnectedKeys::Rod(_, _) => {}
                ConnectedKeys::Spring(i, j) => {
                    let d = &conn.key_or_distance;
                    let ci = conn.current_candidate_index;
                    let (l, s) = target_length_and_stiffness(d, ci);
                    if s != Ratio::ZERO {
                        res += *s.numer() as Semitones / *s.denom() as Semitones
                            * (l - self.get_relative_semitones_in(*i, *j, solution)).powi(2);
                    }
                }
                ConnectedKeys::FixedSpring(i) => {
                    let d = &conn.key_or_distance;
                    let ci = conn.current_candidate_index;
                    let (l, s) = target_length_and_stiffness(d, ci);
                    if s != Ratio::ZERO {
                        res += *s.numer() as Semitones / *s.denom() as Semitones
                            * (60.0 + l - self.get_semitones_in(*i, solution)).powi(2);
                    }
                }
            }
        }

        res
    }

    /// returns true iff all springs have their relaxed length (that is: there are no detuned
    /// intervals or notes) in the provided solution.
    fn relaxed_in(&self, solution: ArrayView2<Ratio<StackCoeff>>) -> bool {
        let target_coefficients = |d: &KeyOrDistance, i: usize| {
            let (intervals, _) = self
                .candidates
                .get(d)
                .expect("relaxed_in: no candidate intervals found for key distance. This should never happen.");
            intervals.row(i)
        };

        for (keys, conn) in self.current_connections.iter() {
            match keys {
                ConnectedKeys::Rod(_, _) => {}
                ConnectedKeys::Spring(i, j) => {
                    let d = &conn.key_or_distance;
                    let ci = conn.current_candidate_index;
                    let l = target_coefficients(d, ci);

                    for k in 0..T::num_intervals() {
                        if l[k] != solution[[*j, k]] - solution[[*i, k]] {
                            return false;
                        }
                    }
                }
                ConnectedKeys::FixedSpring(i) => {
                    let d = &conn.key_or_distance;
                    let ci = conn.current_candidate_index;
                    let l = target_coefficients(d, ci);

                    for k in 0..T::num_intervals() {
                        if l[k] != solution[[*i, k]] {
                            return false;
                        }
                    }
                }
            }
        }

        true
    }

    /// expected invariants:
    /// - start_index must be the return value of [Self::collect_relative_intervals_and_connections].
    /// - entries of `keys` are unique
    fn collect_anchored_intervals_and_connections<AP, PN>(
        &mut self,
        start_index: usize,
        keys: &[KeyNumber],
        is_note_anchored: AP,
        provide_candidate_notes: PN,
    ) where
        AP: Fn(KeyNumber) -> bool,
        PN: Fn(KeyNumber) -> (Array2<Ratio<StackCoeff>>, Array1<Ratio<StackCoeff>>),
    {
        let mut index = start_index;

        if !self.memo_note_anchors {
            self.candidates.retain(|k, _| match k {
                KeyOrDistance::Distance(_) => true,
                KeyOrDistance::Key(_) => false,
            });
        }

        for (i, &k) in keys.iter().enumerate() {
            if is_note_anchored(k) {
                self.current_connections.insert(
                    ConnectedKeys::FixedSpring(i),
                    IntervalInfo {
                        index,
                        key_or_distance: KeyOrDistance::Key(k),
                        current_candidate_index: 0,
                    },
                );
                index += 1;

                if self.memo_note_anchors {
                    if !self.candidates.contains_key(&KeyOrDistance::Key(k)) {
                        self.candidates
                            .insert(KeyOrDistance::Key(k), provide_candidate_notes(k));
                    }
                } else {
                    self.candidates
                        .insert(KeyOrDistance::Key(k), provide_candidate_notes(k));
                }
            }
        }
    }

    /// expected invariants:
    /// -  [Self::collect_rod_and_spring_key_distances] was called before
    ///
    /// Returns 1 plus the highest [IntervalInfo::index] that it used. This can be used to
    /// continue adding the anchored connections with [Self::collect_anchored_intervals_and_connections].
    fn collect_relative_intervals_and_connections<F>(
        &mut self,
        provide_candidate_intervals: F,
    ) -> usize
    where
        F: Fn(KeyDistance) -> (Array2<Ratio<StackCoeff>>, Array1<Ratio<StackCoeff>>),
    {
        self.current_connections.clear();

        if !self.memo_intervals {
            self.candidates.retain(|k, _| match k {
                KeyOrDistance::Distance(_) => false,
                KeyOrDistance::Key(_) => true,
            });
        }

        let mut index = 0;
        let mut insert_connection = |d: KeyDistance, keys: ConnectedKeys| {
            self.current_connections.insert(
                keys,
                IntervalInfo {
                    index,
                    key_or_distance: KeyOrDistance::Distance(d),
                    current_candidate_index: 0,
                },
            );
            index += 1;

            if self.memo_intervals {
                if !self.candidates.contains_key(&KeyOrDistance::Distance(d)) {
                    self.candidates
                        .insert(KeyOrDistance::Distance(d), provide_candidate_intervals(d));
                }
            } else {
                self.candidates
                    .insert(KeyOrDistance::Distance(d), provide_candidate_intervals(d));
            }
        };

        let n = self.n_keys;

        for i in 0..n {
            for j in (i + 1)..n {
                if self.key_distances[[i, j]] != 0 {
                    let d = self.key_distances[[i, j]];
                    insert_connection(d, ConnectedKeys::Rod(i, j))
                }
                if self.key_distances[[j, i]] != 0 {
                    let d = self.key_distances[[j, i]];
                    insert_connection(d, ConnectedKeys::Spring(i, j))
                }
            }
        }

        index
    }

    /// meanings of the arguments:
    /// - `keys` contains MIDI key number of currently sounding keys (or in any case, keys that should
    ///    be "considered together")
    /// - `rodp` takes a key distance (i.e. a difference of MIDI key numbers) and returns whether an
    ///    interval of that size should be realized by a rod.
    ///
    /// expected invariants:
    /// - the entries of `keys` are unique
    ///
    /// provided invariants:
    /// - TODO write something about how the ordering of `keys` affects the results of
    ///   [Self::compute_best_solution]. For now, see the comment there.
    /// - Within the square matrix of size `keys.len()` in `self.key_distances`,
    ///   - the upper triangle contains the key distances of rods that should be added: for
    ///     `i<j` the rod's length is in `key_distances[[i,j]]`.
    ///   - the lower triangle contains the key distances of springs that should be added: for
    ///     `i<j` the spring's lengthh is in `key_distances[[j,i]]`.
    ///   - the rod distances are such that every node is only the endpoint of at most one rod, and every
    ///     node that is an endpoint is not a start point of a rod (this is the expected invariant of
    ///     [solver::System::add_rod])
    fn collect_rod_and_spring_key_distances<WC>(&mut self, keys: &[KeyNumber], which_connector: WC)
    where
        WC: Fn(KeyNumber, KeyNumber) -> Connector,
    {
        let n = keys.len();
        if n > self.key_distances.shape()[0] {
            self.key_distances = Array2::zeros((n, n));
        } else {
            self.key_distances.slice_mut(s![..n, ..n]).fill(0);
        }

        // key distances for rods in the upper half, springs in the lower half
        for i in 0..n {
            for j in (i + 1)..n {
                let d = keys[j] as KeyDistance - keys[i] as KeyDistance;
                match which_connector(keys[i], keys[j]) {
                    Connector::Rod => self.key_distances[[i, j]] = d,
                    Connector::Spring => self.key_distances[[j, i]] = d,
                    Connector::None => {}
                }
            }
        }

        println!(
            "before normalise:\n{}",
            self.key_distances.slice(s![..n, ..n])
        );

        // normalise the rod configuration.
        //
        // We have to enforce the invariant of [System::add_rod]: Every node can only be the endpoint
        // of at most one rod, and every endpoint cant be the start point of another rod.
        for k in (0..n).rev() {
            for j in (0..k).rev() {
                for i in (0..j).rev() {
                    let ij = self.key_distances[[i, j]];
                    let jk = self.key_distances[[j, k]];
                    let ik = self.key_distances[[i, k]];

                    match (ij, jk, ik) {
                        (0, 0, 0) => {}
                        (_a, 0, 0) => {}
                        (0, _b, 0) => {}
                        (0, 0, _c) => {}
                        (a, b, 0) => {
                            self.key_distances[[i, k]] = a + b;
                            self.key_distances[[j, k]] = 0;
                        }
                        (_a, 0, _c) => {}
                        (0, b, c) => {
                            self.key_distances[[i, j]] = c - b;
                            self.key_distances[[j, k]] = 0;
                        }
                        (_a, _b, _c) => {
                            self.key_distances[[j, k]] = 0;
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod test {
    use ndarray::{arr1, arr2, s};
    use num_traits::Float;
    use pretty_assertions::assert_eq;

    use crate::interval::stacktype::fivelimit::ConcreteFiveLimitStackType;

    use super::*;

    #[test]
    fn test_rod_and_spring_key_distances() {
        type Irrelevant = crate::interval::stacktype::fivelimit::ConcreteFiveLimitStackType;
        let mut ws = Workspace::<Irrelevant>::new(1, true, true);

        ws.collect_rod_and_spring_key_distances(&[0, 1, 2, 3], |_, _| Connector::Rod);
        assert_eq!(
            ws.key_distances.slice(s![..4, ..4]),
            arr2(&[[0, 1, 2, 3], [0, 0, 0, 0], [0, 0, 0, 0], [0, 0, 0, 0],])
        );

        ws.collect_rod_and_spring_key_distances(&[0, 1, 2, 3, 4, 5], |i, j| {
            if (j - i) % 2 == 0 {
                Connector::Rod
            } else {
                Connector::Spring
            }
        });
        assert_eq!(
            ws.key_distances.slice(s![..6, ..6]),
            arr2(&[
                [0, 0, 2, 0, 4, 0],
                [1, 0, 0, 2, 0, 4],
                [0, 1, 0, 0, 0, 0],
                [3, 0, 1, 0, 0, 0],
                [0, 3, 0, 1, 0, 0],
                [5, 0, 3, 0, 1, 0],
            ])
        );

        ws.collect_rod_and_spring_key_distances(&[0, 5, 7, 12], |i, j| {
            let d = j - i;
            if (d % 12 == 0) | (d % 7 == 0) {
                Connector::Rod
            } else {
                Connector::Spring
            }
        });
        assert_eq!(
            ws.key_distances.slice(s![..4, ..4]),
            arr2(&[[0, 5, 7, 12], [5, 0, 0, 0], [0, 2, 0, 0], [0, 0, 5, 0],])
        )
    }

    #[test]
    fn test_compute_best_solution() {
        let mut ws = Workspace::<ConcreteFiveLimitStackType>::new(1, true, false);
        let mut solver_workspace = solver::Workspace::new(1, 1, 1);

        let provide_candidate_intervals = |d: KeyDistance| {
            let octaves = Ratio::new((d as StackCoeff).div_euclid(12), 1);
            let pitch_class = d.rem_euclid(12);

            match pitch_class {
                0 => (arr2(&[[octaves, 0.into(), 0.into()]]), arr1(&[1.into()])),
                1 => (
                    arr2(&[
                        [octaves + 1, (-1).into(), (-1).into()], // diatonic semitone
                        [octaves, (-1).into(), 2.into()],        // chromatic semitone
                    ]),
                    arr1(&[
                        Ratio::new(1, 2 * 3 * 4 * 5),
                        Ratio::new(1, 2 * 3 * 4 * 5 * 4 * 5),
                    ]),
                ),
                2 => (
                    arr2(&[
                        [octaves - 1, 2.into(), 0.into()],    // major whole tone 9/8
                        [octaves + 1, (-2).into(), 1.into()], // minor whole tone 10/9
                    ]),
                    arr1(&[
                        Ratio::new(1, 2 * 3 * 2 * 3),
                        Ratio::new(1, 2 * 3 * 2 * 3 * 4 * 5),
                    ]),
                ),
                3 => (
                    arr2(&[[octaves, 1.into(), (-1).into()]]), // minor third
                    arr1(&[Ratio::new(1, 2 * 3 * 4 * 5)]),
                ),
                4 => (
                    arr2(&[[octaves, 0.into(), 1.into()]]), // major third
                    arr1(&[Ratio::new(1, 4 * 5)]),
                ),
                5 => (
                    arr2(&[[octaves + 1, (-1).into(), 0.into()]]), // fourth
                    arr1(&[Ratio::new(1, 2 * 3)]),
                ),
                6 => (
                    arr2(&[
                        [octaves - 1, 2.into(), 1.into()], // tritone as major tone plus major third
                        [octaves, 2.into(), (-2).into()], // tritone as chromatic semitone below fifth
                    ]),
                    arr1(&[
                        Ratio::new(1, 2 * 3 * 2 * 3 * 4 * 5),
                        Ratio::new(1, 2 * 3 * 2 * 3 * 4 * 5 * 4 * 5),
                    ]),
                ),
                7 => (
                    arr2(&[[octaves, 1.into(), 0.into()]]), // fifth
                    arr1(&[Ratio::new(1, 2 * 3)]),
                ),
                8 => (
                    arr2(&[[octaves + 1, 0.into(), (-1).into()]]), // minor sixth
                    arr1(&[Ratio::new(1, 4 * 5)]),
                ),
                9 => (
                    arr2(&[
                        [octaves + 1, (-1).into(), 1.into()], // major sixth
                        [octaves - 1, 3.into(), 0.into()],    // major tone plus fifth
                    ]),
                    arr1(&[
                        Ratio::new(1, 2 * 3 * 4 * 5),
                        Ratio::new(1, 2 * 3 * 2 * 3 * 2 * 3),
                    ]),
                ),
                10 => (
                    arr2(&[
                        [octaves + 2, (-2).into(), 0.into()], // minor seventh as stack of two fourths
                        [octaves, 2.into(), (-1).into()], // minor seventh as fifth plus minor third
                    ]),
                    arr1(&[
                        Ratio::new(1, 2 * 3 * 2 * 3),
                        Ratio::new(1, 2 * 3 * 2 * 3 * 4 * 5),
                    ]),
                ),
                11 => (
                    arr2(&[
                        [octaves, 1.into(), 1.into()], // major seventh as fifth plus major third
                    ]),
                    arr1(&[Ratio::new(1, 2 * 3 * 4 * 5)]),
                ),
                _ => unreachable!(),
            }
        };

        let provide_candidate_notes = |i| provide_candidate_intervals(i as KeyDistance - 60);

        let epsilon = 0.00000000000000001; // just a very small number. I don't care precisely.

        // if nothing else is given, the first option is picked
        ws.compute_best_solution(
            &[60, 66],
            |i| i == 60,
            |_, _| Connector::Spring,
            provide_candidate_intervals,
            provide_candidate_notes,
            &mut solver_workspace,
        )
        .unwrap();
        assert_eq!(
            ws.best_solution(),
            arr2(&[
                [0.into(), 0.into(), 0.into()],
                [(-1).into(), 2.into(), 1.into()], // tritone as major tone plus major third
            ])
        );
        assert!(ws.best_energy() < epsilon);
        assert!(ws.relaxed());

        // no new interval, so `provide_candidate_intervals` is never called.
        ws.compute_best_solution(
            &[60, 66],
            |i| i == 60,
            |_, _| Connector::Spring,
            |_| panic!("This should not be called"),
            provide_candidate_notes,
            &mut solver_workspace,
        )
        .unwrap();
        assert_eq!(
            ws.best_solution(),
            arr2(&[
                [0.into(), 0.into(), 0.into()],
                [(-1).into(), 2.into(), 1.into()], // tritone as major tone plus major third
            ])
        );
        assert!(ws.best_energy() < epsilon);
        assert!(ws.relaxed);

        // C major triad
        ws.compute_best_solution(
            &[60, 64, 67],
            |i| i == 60,
            |_, _| Connector::Spring,
            provide_candidate_intervals,
            provide_candidate_notes,
            &mut solver_workspace,
        )
        .unwrap();
        assert_eq!(
            ws.best_solution(),
            arr2(&[
                [0.into(), 0.into(), 0.into()],
                [0.into(), 0.into(), 1.into()],
                [0.into(), 1.into(), 0.into()],
            ])
        );
        assert!(ws.best_energy() < epsilon);
        assert!(ws.relaxed());

        // E major triad
        ws.compute_best_solution(
            &[64, 68, 71],
            |i| i == 64,
            |_, _| Connector::Spring,
            provide_candidate_intervals,
            provide_candidate_notes,
            &mut solver_workspace,
        )
        .unwrap();
        assert_eq!(
            ws.best_solution(),
            arr2(&[
                [0.into(), 0.into(), 1.into()],
                [0.into(), 0.into(), 2.into()],
                [0.into(), 1.into(), 1.into()],
            ])
        );
        assert!(ws.best_energy() < epsilon);
        assert!(ws.relaxed());

        // The three notes C,D,E: Because they are mentioned in this order, the interval C-D will
        // be the major tone. See the next example as well.
        ws.compute_best_solution(
            &[60, 62, 64],
            |i| i == 60,
            |_, _| Connector::Spring,
            provide_candidate_intervals,
            provide_candidate_notes,
            &mut solver_workspace,
        )
        .unwrap();
        assert_eq!(
            ws.best_solution(),
            arr2(&[
                [0.into(), 0.into(), 0.into()],
                [(-1).into(), 2.into(), 0.into()],
                [0.into(), 0.into(), 1.into()]
            ])
        );
        assert!(ws.best_energy() < epsilon);
        assert!(ws.relaxed());

        // This is the same as before, but illustrates the relevance of the order in the `keys`
        // argument: Now, the tuning that makes the step from C to D a minor tone is preferred.
        //
        // Generally, intervals between notes that are mentioned early are less likely to have the
        // alternative sizes.
        ws.compute_best_solution(
            &[64, 62, 60],
            |i| i == 60,
            |_, _| Connector::Spring,
            provide_candidate_intervals,
            provide_candidate_notes,
            &mut solver_workspace,
        )
        .unwrap();
        assert_eq!(
            ws.best_solution(),
            arr2(&[
                [0.into(), 0.into(), 1.into()],
                [1.into(), (-2).into(), 1.into()],
                [0.into(), 0.into(), 0.into()],
            ])
        );
        assert!(ws.best_energy() < epsilon);
        assert!(ws.relaxed());

        // D-flat major seventh on C
        ws.compute_best_solution(
            &[60, 61, 65, 68],
            |i| i == 60,
            |_, _| Connector::Spring,
            provide_candidate_intervals,
            provide_candidate_notes,
            &mut solver_workspace,
        )
        .unwrap();
        assert_eq!(
            ws.best_solution(),
            arr2(&[
                [0.into(), 0.into(), 0.into()],
                [1.into(), (-1).into(), (-1).into()], // diatonic semitone
                [1.into(), (-1).into(), 0.into()],
                [1.into(), 0.into(), (-1).into()],
            ])
        );
        assert!(ws.best_energy() < epsilon);
        assert!(ws.relaxed());

        // D dominant seventh on C
        ws.compute_best_solution(
            &[60, 62, 66, 69],
            |i| i == 60,
            |_, _| Connector::Spring,
            provide_candidate_intervals,
            provide_candidate_notes,
            &mut solver_workspace,
        )
        .unwrap();
        assert_eq!(
            ws.best_solution(),
            arr2(&[
                [0.into(), 0.into(), 0.into()],
                [(-1).into(), 2.into(), 0.into()],
                [(-1).into(), 2.into(), 1.into()],
                [(-1).into(), 3.into(), 0.into()],
            ])
        );
        assert!(ws.best_energy() < epsilon);
        assert!(ws.relaxed());

        // a single note: the first option is choosen
        ws.compute_best_solution(
            &[69],
            |i| i == 69,
            |_, _| Connector::Spring,
            provide_candidate_intervals,
            provide_candidate_notes,
            &mut solver_workspace,
        )
        .unwrap();
        assert_eq!(
            ws.best_solution(),
            arr2(&[[1.into(), (-1).into(), 1.into()],])
        );
        assert!(ws.best_energy() < epsilon);
        assert!(ws.relaxed());

        // Chromatic cluster of three notes is bounded by a minor tone
        ws.compute_best_solution(
            &[60, 61, 62],
            |i| i == 60,
            |_, _| Connector::Spring,
            provide_candidate_intervals,
            provide_candidate_notes,
            &mut solver_workspace,
        )
        .unwrap();
        assert_eq!(
            ws.best_solution(),
            arr2(&[
                [0.into(), 0.into(), 0.into()],
                [1.into(), (-1).into(), (-1).into()],
                [1.into(), (-2).into(), 1.into()],
            ])
        );
        assert!(ws.best_energy() < epsilon);
        assert!(ws.relaxed());

        // 69 chord cannot be in tune
        ws.compute_best_solution(
            &[60, 62, 64, 67, 69],
            |i| i == 60,
            |_, _| Connector::Spring,
            provide_candidate_intervals,
            provide_candidate_notes,
            &mut solver_workspace,
        )
        .unwrap();
        assert!(ws.best_energy() > epsilon);
        assert!(!ws.relaxed());

        // 69 chord with rods for fifhts
        ws.compute_best_solution(
            &[60, 62, 64, 67, 69],
            |i| i == 60,
            |i, j| {
                if j - i == 7 {
                    Connector::Rod
                } else {
                    Connector::Spring
                }
            },
            provide_candidate_intervals,
            provide_candidate_notes,
            &mut solver_workspace,
        )
        .unwrap();

        //C-D fifth
        assert_eq!(
            ws.best_solution().row(0),
            arr1(&[0.into(), 0.into(), 0.into()])
        );
        assert_eq!(
            ws.best_solution().row(3),
            arr1(&[0.into(), 1.into(), 0.into()])
        );

        // D-A fifth:
        let mut delta = ws.best_solution().row(4).to_owned();
        delta.scaled_add((-1).into(), &ws.best_solution().row(1));
        assert_eq!(delta, arr1(&[0.into(), 1.into(), 0.into()]));

        // the D is between a minor and a major tone higher than C:
        let majortone = 12.0 * (9.0 as Semitones / 8.0).log2();
        let minortone = 12.0 * (10.0 as Semitones / 9.0).log2();
        assert!(ws.get_semitones(1) < 60.0 + majortone);
        assert!(ws.get_semitones(1) > 60.0 + minortone);

        // the distance between E and D is also between a major and minor tone:
        assert!(ws.get_relative_semitones(1, 2) < majortone);
        assert!(ws.get_relative_semitones(1, 2) > minortone);

        // the distance betwen C and D is the same as between G and A:
        assert_eq!(
            ws.get_relative_semitones(0, 1),
            ws.get_relative_semitones(3, 4)
        );

        assert!(ws.best_energy() > epsilon);
        assert!(!ws.relaxed());

        // 69 chord with rods for fifhts and fourths. This forces a pythagorean third. Failing at
        //    the moment because of the way I reduce rod configurations
        ws.compute_best_solution(
            &[60, 62, 64, 67, 69],
            |i| i == 60,
            |i, j| {
                if (j - i == 5) | (j - i == 7) {
                    Connector::Rod
                } else {
                    Connector::Spring
                }
            },
            provide_candidate_intervals,
            provide_candidate_notes,
            &mut solver_workspace,
        )
        .unwrap();
        assert_eq!(
            ws.best_solution(),
            arr2(&[
                [0.into(), 0.into(), 0.into()],
                [(-1).into(), 2.into(), 0.into()],
                [0.into(), 0.into(), 1.into()],
                [0.into(), 1.into(), 0.into()],
                [(-1).into(), 3.into(), 0.into()],
            ])
        );
        assert!(ws.best_energy() > epsilon);
        assert!(!ws.relaxed());
    }
}
