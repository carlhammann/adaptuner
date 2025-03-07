use std::{
    collections::{BTreeMap, HashMap},
    marker::PhantomData,
};

use ndarray::{s, Array1, Array2, ArrayView1, ArrayView2};
use num_rational::Ratio;

use super::solver;
use crate::interval::{
    base::Semitones,
    stacktype::r#trait::{StackCoeff, StackType},
};

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
    key_distances: Array2<KeyDistance>,
    candidate_intervals:
        HashMap<KeyOrDistance, (Array2<Ratio<StackCoeff>>, Array1<Ratio<StackCoeff>>)>,
    current_connections: BTreeMap<ConnectedKeys, IntervalInfo>,
    pub best_solution: Array2<Ratio<StackCoeff>>,
    pub best_energy: Semitones,
}

impl<T: StackType> Workspace<T> {
    /// meanings of arguments:
    /// - `initial_n_keys`: How many simultaneously sounding keys do you expect this workspace to
    ///    be used for? Choosing a big value will potentially prevent re-allocations, at the cost of
    ///    wasting space.
    pub fn new(initial_n_keys: usize) -> Self {
        Workspace {
            _phantom: PhantomData,
            key_distances: Array2::zeros((initial_n_keys, initial_n_keys)),
            candidate_intervals: HashMap::new(),
            current_connections: BTreeMap::new(),
            best_solution: Array2::zeros((initial_n_keys, T::num_intervals())),
            best_energy: Semitones::MAX,
        }
    }

    /// expected invariants:
    /// - `keys` is sorted by ascending key number, and each key number occurs at most once
    pub fn compute_best_solution<'a, RP, PC>(
        &mut self,
        keys: &[(
            KeyNumber,
            Option<(
                ArrayView2<'a, Ratio<StackCoeff>>,
                ArrayView1<'a, Ratio<StackCoeff>>,
            )>,
        )],
        rodp: RP,
        provide_candidates: PC,
        solver_workspace: &mut solver::Workspace,
    ) -> Result<(), fractionfree::LinalgErr>
    where
        RP: Fn(KeyDistance) -> bool,
        PC: Fn(KeyDistance) -> (Array2<Ratio<StackCoeff>>, Array1<Ratio<StackCoeff>>),
    {
        self.collect_rod_and_spring_key_distances(&keys, rodp);
        let next_index = self.collect_relative_intervals_and_connections(provide_candidates);
        self.collect_anchored_intervals_and_connections(next_index, &keys);

        println!("current_connections:\n {:?}\n\n", self.current_connections);
        println!("candidate_intervals:\n {:?}\n\n", self.candidate_intervals);

        self.best_energy = Semitones::MAX;

        self.solve_current_candidate(solver_workspace)?;
        while self.prepare_next_candidate() {
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
            let (intervals, _) = &self.candidate_intervals[keydist];
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
    ) -> Result<(), fractionfree::LinalgErr> {
        let n_nodes = self.key_distances.shape()[0];
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
                        .candidate_intervals
                        .get(&conn.key_or_distance)
                        .expect("solve_current_candidate: no candidate intervals found for rod key distance. This should never happen.");

                    let length = candidate_lengths.row(conn.current_candidate_index);
                    system.define_length(conn.index, length);
                }
                ConnectedKeys::Spring(i, j) => {
                    let (candidate_lengths, candidate_stiffnesses) = self
                        .candidate_intervals
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
                        .candidate_intervals
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

        // `self.best_solution.shape()[1]` and `solution.shape()[1]` are always equal to `T::num_intervals()`.
        if self.best_solution.shape()[0] < solution.shape()[0] {
            self.best_solution = Array2::zeros(solution.raw_dim());
        }

        if energy < self.best_energy {
            self.best_solution.assign(&solution);
            self.best_energy = energy;
        }

        Ok(())
    }

    /// Return the fractional MIDI note number of the `i`-th currently considered note, as
    /// prescribed by the current best solution.
    pub fn get_semitones(&self, i: usize) -> Semitones {
        self.get_semitones_in(i, self.best_solution.view())
    }

    fn get_semitones_in(&self, i: usize, solution: ArrayView2<Ratio<StackCoeff>>) -> Semitones {
        let mut res = 0.0;
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

    /// Compute the energy stored in tensioned springs (== detuned intervals) in the provided
    /// solution.
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
                .candidate_intervals
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
                            * (l - self.get_relative_semitones_in(*i, *j, solution));
                    }
                }
                ConnectedKeys::FixedSpring(i) => {
                    let d = &conn.key_or_distance;
                    let ci = conn.current_candidate_index;
                    let (l, s) = target_length_and_stiffness(d, ci);
                    if s != Ratio::ZERO {
                        res += *s.numer() as Semitones / *s.denom() as Semitones
                            * (l - self.get_semitones_in(*i, solution));
                    }
                }
            }
        }

        res
    }

    /// expected invariants:
    /// -  [Self::collect_rod_and_spring_key_distances] was called before
    ///
    /// Returns 1 plus the highest [IntervalInfo::index] that it used. This can be used to
    /// continue adding the anchored connections with [Self::collect_anchored_intervals_and_connections].
    fn collect_relative_intervals_and_connections<F>(&mut self, provide_candidates: F) -> usize
    where
        F: Fn(KeyDistance) -> (Array2<Ratio<StackCoeff>>, Array1<Ratio<StackCoeff>>),
    {
        self.current_connections.clear();

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

            if !self
                .candidate_intervals
                .contains_key(&KeyOrDistance::Distance(d))
            {
                self.candidate_intervals
                    .insert(KeyOrDistance::Distance(d), provide_candidates(d));
            }
        };

        let n = self.key_distances.shape()[0];

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

    /// expected invariants:
    /// - start_index must be the return value of [Self::collect_relative_intervals_and_connections].
    /// - `keys` is sorted by ascending KeyNumber, and its entries are unique by key number
    fn collect_anchored_intervals_and_connections(
        &mut self,
        start_index: usize,
        keys: &[(
            KeyNumber,
            Option<(ArrayView2<Ratio<StackCoeff>>, ArrayView1<Ratio<StackCoeff>>)>,
        )],
    ) {
        let mut index = start_index;

        // we can delete the entries for absolute note positions; they'll be different from time to time.
        // The sizes of intervals remain the same, so we keep them. (note that in
        // [Self::collect_relative_intervals_and_connections], we don't overwrite!)
        self.candidate_intervals.retain(|k, _| match k {
            KeyOrDistance::Distance(_) => true,
            KeyOrDistance::Key(_) => false,
        });

        for (i, (k, candidates)) in keys.iter().enumerate() {
            match candidates {
                None => {}
                Some((intervals, stiffnesses)) => {
                    self.current_connections.insert(
                        ConnectedKeys::FixedSpring(i),
                        IntervalInfo {
                            index,
                            key_or_distance: KeyOrDistance::Key(*k),
                            current_candidate_index: 0,
                        },
                    );
                    index += 1;

                    self.candidate_intervals.insert(
                        KeyOrDistance::Key(*k),
                        (intervals.to_owned(), stiffnesses.to_owned()),
                    );
                }
            }
        }
    }

    /// meanings of the arguments:
    /// - `keys` contains MIDI key number of currently sounding keys (or in any case, keys that should
    ///    be "considered together")
    /// - `rodp` takes a key distance (i.e. a difference of MIDI key numbers) and returns whether an
    ///    interval of that size should be realized by a rod.
    ///
    /// expected invariants:
    /// - the entries of `keys` are unique, and sorted by ascending MIDI key number
    ///
    /// provided invariants:
    /// -
    /// - Within the square matrix of size `keys.len()` in `self.key_distances`,
    ///   - the upper triangle contains the key distances of rods that should be added: for
    ///     `i<j` the rod's length is in `key_distances[[i,j]]`.
    ///   - the lower triangle contains the key distances of springs that should be added: for
    ///     `i<j` the spring's lengthh is in `key_distances[[j,i]]`.
    ///   - the rod distances are such that every node is only the endpoint of at most one rod, and every
    ///     node that is an endpoint is not a start point of a rod (this is the expected invariant of
    ///     [solver::System::add_rod])
    fn collect_rod_and_spring_key_distances<X, RP>(&mut self, keys: &[(KeyNumber, X)], rodp: RP)
    where
        RP: Fn(KeyDistance) -> bool,
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
                let d = keys[j].0 as KeyDistance - keys[i].0 as KeyDistance;
                if rodp(d) {
                    self.key_distances[[i, j]] = d;
                } else {
                    self.key_distances[[j, i]] = d;
                }
            }
        }

        //println!("before normalise:\n{}", distances);

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
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn test_rod_and_spring_key_distances() {
        type Irrelevant = crate::interval::stacktype::fivelimit::ConcreteFiveLimitStackType;
        let mut ws = Workspace::<Irrelevant>::new(1);

        ws.collect_rod_and_spring_key_distances(&[(0, ()), (1, ()), (2, ()), (3, ())], |_| true);
        assert_eq!(
            ws.key_distances.slice(s![..4, ..4]),
            arr2(&[[0, 1, 2, 3], [0, 0, 0, 0], [0, 0, 0, 0], [0, 0, 0, 0],])
        );

        ws.collect_rod_and_spring_key_distances(
            &[(0, ()), (1, ()), (2, ()), (3, ()), (4, ()), (5, ())],
            |d| d % 2 == 0,
        );
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

        ws.collect_rod_and_spring_key_distances(&[(0, ()), (5, ()), (7, ()), (12, ())], |d| {
            (d % 12 == 0) | (d % 7 == 0)
        });
        assert_eq!(
            ws.key_distances.slice(s![..4, ..4]),
            arr2(&[[0, 5, 7, 12], [5, 0, 0, 0], [0, 2, 0, 0], [0, 0, 5, 0],])
        )
    }

    #[test]
    fn test_compute_best_solution() {
        type Irrelevant = crate::interval::stacktype::fivelimit::ConcreteFiveLimitStackType;
        let mut ws = Workspace::<Irrelevant>::new(1);
        let mut sws = solver::Workspace::new(1, 1, 1);

        ws.compute_best_solution(
            &[(
                0,
                Some((
                    arr2(&[[0.into(), 0.into(), 0.into()]]).view(),
                    arr1(&[1.into()]).view(),
                )),
            )],
            |_| true,
            |_| panic!("this should not be called"),
            &mut sws,
        )
        .unwrap();
        assert_eq!(ws.best_solution, arr2(&[[0.into(), 0.into(), 0.into()]]));

        ws.compute_best_solution(
            &[
                (
                    0,
                    Some((
                        arr2(&[[0.into(), 0.into(), 0.into()]]).view(),
                        arr1(&[1.into()]).view(),
                    )),
                ),
                (1, None),
            ],
            |_| false,
            |_| (arr2(&[[2.into(), 3.into(), 5.into()]]), arr1(&[1.into()])),
            &mut sws,
        )
        .unwrap();
        assert_eq!(ws.best_energy, 0.0);
        assert_eq!(
            ws.best_solution,
            arr2(&[
                [0.into(), 0.into(), 0.into()],
                [2.into(), 3.into(), 5.into()]
            ])
        );

        ws.compute_best_solution(
            &[
                (0, None),
                (
                    1,
                    Some((
                        arr2(&[[0.into(), 0.into(), 0.into()]]).view(),
                        arr1(&[1.into()]).view(),
                    )),
                ),
            ],
            |_| true,
            |_| panic!("this should not be called"),
            &mut sws,
        )
        .unwrap();
        assert_eq!(ws.best_energy, 0.0);
        assert_eq!(
            ws.best_solution,
            arr2(&[
                [(-2).into(), (-3).into(), (-5).into()],
                [0.into(), 0.into(), 0.into()],
            ])
        );

        ws.compute_best_solution(
            &[
                (
                    0,
                    Some((
                        arr2(&[[0.into(), 0.into(), 0.into()]]).view(),
                        arr1(&[1.into()]).view(),
                    )),
                ),
                (
                    1,
                    Some((
                        arr2(&[[2.into(), 3.into(), 5.into()]]).view(),
                        arr1(&[1.into()]).view(),
                    )),
                ),
            ],
            |_| false,
            |_| panic!("this should not be called"),
            &mut sws,
        )
        .unwrap();
        assert_eq!(ws.best_energy, 0.0);
        assert_eq!(
            ws.best_solution,
            arr2(&[
                [0.into(), 0.into(), 0.into()],
                [2.into(), 3.into(), 5.into()],
            ])
        );
    }
}
