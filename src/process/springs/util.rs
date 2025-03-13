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

type KeyDistance = i8;
type KeyNumber = u8;

/// invariants:
/// - length at least 1
/// - the key distances are always positive
/// - sorted by ascending key distance
type RodSpec = Vec<(KeyDistance, StackCoeff)>;

#[derive(PartialEq, Eq, Hash, Debug)]
enum MemoKey {
    Key(KeyNumber),
    Distance(KeyDistance),
    Rod(RodSpec),
}

struct Workspace<T: StackType> {
    _phantom: PhantomData<T>,
    n_keys: usize,
    memo_springs: bool,
    memo_anchors: bool,
    memo_rods: bool,
    memoed_springs: HashMap<KeyDistance, (Array2<Ratio<StackCoeff>>, Array1<Ratio<StackCoeff>>)>,
    memoed_anchors: HashMap<KeyNumber, (Array2<Ratio<StackCoeff>>, Array1<Ratio<StackCoeff>>)>,
    memoed_rods: HashMap<RodSpec, Array1<Ratio<StackCoeff>>>,
    current_springs: BTreeMap<(usize, usize), SpringInfo>,
    current_anchors: BTreeMap<usize, AnchorInfo>,
    current_rods: HashMap<(usize, usize), RodInfo>,
    best_solution: Array2<Ratio<StackCoeff>>,
    best_energy: Semitones, // May not be exactly zero even if `relaxed` is true, due to floating point imprecisions
    relaxed: bool,
}

impl<T: StackType> Workspace<T> {
    /// meanings of arguments:
    /// - `initial_n_keys`: How many simultaneously sounding keys do you expect this workspace to
    ///    be used for? Choosing a big value will potentially prevent re-allocations, at the cost of
    ///    wasting space.
    /// - `memo_intervals` and `memo_notes`: Should sizes of intervals or "anchor" posisitions of
    ///    notes be remembered between successive calls of [Self::compute_best_solution]?
    pub fn new(
        initial_n_keys: usize,
        memo_springs: bool,
        memo_anchors: bool,
        memo_rods: bool,
    ) -> Self {
        Workspace {
            _phantom: PhantomData,
            n_keys: 0,
            memo_springs,
            memo_anchors,
            memo_rods,
            memoed_springs: HashMap::new(),
            memoed_anchors: HashMap::new(),
            memoed_rods: HashMap::new(),
            current_springs: BTreeMap::new(),
            current_anchors: BTreeMap::new(),
            current_rods: HashMap::new(),
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

    ///  The ordering of `keys` matters: Notes that come later (and the springs between them)
    ///  are more "stable" in the sense that alternative tunings are less likely to be picked
    pub fn compute_best_solution<'a, WC, AP, PS, PA, PR>(
        &mut self,
        keys: &[KeyNumber],
        is_note_anchored: AP,
        which_connector: WC,
        provide_candidate_springs: PS,
        provide_candidate_anchors: PA,
        provide_rods: PR,
        solver_workspace: &mut solver::Workspace,
    ) -> Result<(), lu::LUErr>
    where
        WC: Fn(KeyNumber, KeyNumber) -> Connector,
        AP: Fn(KeyNumber) -> bool,
        PS: Fn(KeyDistance) -> (Array2<Ratio<StackCoeff>>, Array1<Ratio<StackCoeff>>),
        PA: Fn(KeyNumber) -> (Array2<Ratio<StackCoeff>>, Array1<Ratio<StackCoeff>>),
        PR: Fn(&RodSpec) -> Array1<Ratio<StackCoeff>>,
    {
        self.n_keys = keys.len();
        let next_index = self.collect_intervals(
            keys,
            which_connector,
            provide_candidate_springs,
            provide_rods,
        );
        self.collect_anchors(
            next_index,
            keys,
            is_note_anchored,
            provide_candidate_anchors,
        );

        println!("current_springs:\n {:?}\n\n", self.current_springs);
        println!("current_anchors:\n {:?}\n\n", self.current_anchors);
        println!("current_rods:\n {:?}\n\n", self.current_rods);
        println!("memoed_springs:\n {:?}\n\n", self.memoed_springs);
        println!("memoed_anchors:\n {:?}\n\n", self.memoed_anchors);
        println!("memoed_rods:\n {:?}\n\n", self.memoed_rods);

        self.best_energy = Semitones::MAX;

        self.solve_current_candidate(solver_workspace)?;
        while !self.relaxed & self.prepare_next_candidate() {
            self.solve_current_candidate(solver_workspace)?;
        }
        Ok(())
    }

    /// returns true iff there is a new candidate. In that case, call [Self::solve_current_candidate]
    /// again to start solving the new candidate.
    fn prepare_next_candidate(&mut self) -> bool {
        for (_, v) in self.current_anchors.iter_mut() {
            let max_ix = self
                .memoed_anchors
                .get(&v.memo_key)
                .expect("prepeare_next_candidate: found no candidates for anchor")
                .0
                .shape()[0]
                - 1;
            if v.current_candidate_index < max_ix {
                v.current_candidate_index += 1;
                return true;
            } else {
                v.current_candidate_index = 0;
            }
        }

        for (_, v) in self.current_springs.iter_mut() {
            let max_ix = self
                .memoed_springs
                .get(&v.memo_key)
                .expect("prepeare_next_candidate: found no candidates for spring")
                .0
                .shape()[0]
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

    /// expected invariants:
    /// - `self.key_distances` has been filled with [collect_rod_and_spring_key_distances].
    /// - `self.candidate_intervals`, `self.current_lengths`, and `self.current_connections` have been initialised with
    ///   [Self::collect_relative_intervals_and_connections] or updated with [Self::prepare_next_candidate].
    fn solve_current_candidate(
        &mut self,
        solver_workspace: &mut solver::Workspace,
    ) -> Result<(), lu::LUErr> {
        let n_nodes = self.n_keys;
        let n_lengths =
            self.current_springs.len() + self.current_anchors.len() + self.current_rods.len();
        let n_base_lengths = T::num_intervals();

        let mut system = solver_workspace.prepare_system(n_nodes, n_lengths, n_base_lengths);

        // Rods must be added after (fixed and relative) springs.

        for (k, v) in self.current_anchors.iter() {
            let (candidate_lengths, candidate_stiffnesses) = self
                .memoed_anchors
                .get(&v.memo_key)
                .expect("solve_current_candidate: no candidate intervals found for fixed spring.");

            let stiffness = candidate_stiffnesses[v.current_candidate_index];
            system.add_fixed_spring(*k, v.solver_length_index, stiffness);
            println!(
                "add_fixed_spring({}, {}, {})",
                k, v.solver_length_index, stiffness
            );

            let length = candidate_lengths.row(v.current_candidate_index);
            system.define_length(v.solver_length_index, length);
        }

        for ((i, j), v) in self.current_springs.iter() {
            let (candidate_lengths, candidate_stiffnesses) = self
                .memoed_springs
                .get(&v.memo_key)
                .expect("solve_current_candidate: no candidate intervals found for spring.");

            let stiffness = candidate_stiffnesses[v.current_candidate_index];
            system.add_spring(*i, *j, v.solver_length_index, stiffness);
            println!(
                "add_spring({}, {}, {}, {})",
                i, j, v.solver_length_index, stiffness
            );

            let length = candidate_lengths.row(v.current_candidate_index);
            system.define_length(v.solver_length_index, length);
        }

        for ((i, j), v) in self.current_rods.iter() {
            system.add_rod(*i, *j, v.solver_length_index);
            println!("add_rod({}, {}, {})", i, j, v.solver_length_index);

            let length = self
                .memoed_rods
                .get(&v.memo_key)
                .expect("solve_current_candidate: no interval found for rod.")
                .view();

            system.define_length(v.solver_length_index, length);
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
            let (ls, ss) = self
                .memoed_springs
                .get(&v.memo_key)
                .expect("energy_in: no candidate intervals found for spring.");
            let l = compute_length(ls.row(v.current_candidate_index));
            let s = ss[v.current_candidate_index];
            if s != Ratio::ZERO {
                res += *s.numer() as Semitones / *s.denom() as Semitones
                    * (l - self.get_relative_semitones_in(*i, *j, solution)).powi(2);
            }
        }

        for (k, v) in self.current_anchors.iter() {
            let (ps, ss) = self
                .memoed_anchors
                .get(&v.memo_key)
                .expect("energy_in: no candidates found for anchor.");
            let p = 60.0 + compute_length(ps.row(v.current_candidate_index));
            let s = ss[v.current_candidate_index];
            if s != Ratio::ZERO {
                res += *s.numer() as Semitones / *s.denom() as Semitones
                    * (p - self.get_semitones_in(*k, solution)).powi(2);
            }
        }

        res
    }

    /// returns true iff all springs have their relaxed length (that is: there are no detuned
    /// intervals or notes) in the provided solution.
    fn relaxed_in(&self, solution: ArrayView2<Ratio<StackCoeff>>) -> bool {
        for (i, v) in self.current_anchors.iter() {
            let (ps, _) = self
                .memoed_anchors
                .get(&v.memo_key)
                .expect("relaxed_in: no candidates found for anchor.");
            for k in 0..T::num_intervals() {
                if ps[[v.current_candidate_index, k]] != solution[[*i, k]] {
                    return false;
                }
            }
        }

        for ((i, j), v) in self.current_springs.iter() {
            let (ls, _) = self
                .memoed_springs
                .get(&v.memo_key)
                .expect("relaxed_in: no candidates found for spring.");
            for k in 0..T::num_intervals() {
                if ls[[v.current_candidate_index, k]] != solution[[*j, k]] - solution[[*i, k]] {
                    return false;
                }
            }
        }

        true
    }

    /// expected invariants:
    /// - start_index must be the return value of [Self::collect_intervals].
    /// - entries of `keys` are unique
    fn collect_anchors<AP, PA>(
        &mut self,
        start_index: usize,
        keys: &[KeyNumber],
        is_note_anchored: AP,
        provide_candidate_anchors: PA,
    ) where
        AP: Fn(KeyNumber) -> bool,
        PA: Fn(KeyNumber) -> (Array2<Ratio<StackCoeff>>, Array1<Ratio<StackCoeff>>),
    {
        self.current_anchors.clear();

        if !self.memo_anchors {
            self.memoed_anchors.clear();
        }

        let mut solver_length_index = start_index;

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

    /// Returns 1 plus the highest [IntervalInfo::index] that it used. This can be used to
    /// continue adding the anchored connections with [Self::collect_anchored_intervals_and_connections].
    fn collect_intervals<WC, PS, PR>(
        &mut self,
        keys: &[KeyNumber],
        which_connector: WC,
        provide_candidate_springs: PS,
        provide_rods: PR,
    ) -> usize
    where
        WC: Fn(KeyNumber, KeyNumber) -> Connector,
        PS: Fn(KeyDistance) -> (Array2<Ratio<StackCoeff>>, Array1<Ratio<StackCoeff>>),
        PR: Fn(&RodSpec) -> Array1<Ratio<StackCoeff>>,
    {
        self.current_rods.clear();
        self.current_springs.clear();

        if !self.memo_springs {
            self.memoed_springs.clear();
        }

        if !self.memo_rods {
            self.memoed_rods.clear();
        }

        let n = keys.len();

        let mut solver_length_index = 0;

        for i in 0..n {
            for j in (i + 1)..n {
                match which_connector(keys[i], keys[j]) {
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
                    Connector::Rod => {
                        let d = keys[j] as KeyDistance - keys[i] as KeyDistance;
                        self.current_rods.insert(
                            (i, j),
                            RodInfo {
                                memo_key: vec![(d, 1)],
                                solver_length_index: 0, // This is a dummy initialisation. Will be
                                                        // updated with something sensible later!
                            },
                        );
                    }
                    Connector::None => {}
                }
            }
        }

        println!("current_rods, unnormalised");
        for ((i, j), s) in self.current_rods.iter() {
            println!("({}, {}): {:?}", i, j, s);
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

        println!("current_rods, normalised:");
        for ((i, j), s) in self.current_rods.iter() {
            println!("({}, {}): {:?}", i, j, s);
        }

        for spec in self.current_rods.values() {
            if !self.memoed_rods.contains_key(&spec.memo_key) {
                let coeffs = provide_rods(&spec.memo_key);
                self.memoed_rods.insert(spec.memo_key.clone(), coeffs);
            }
        }

        solver_length_index
    }
}

#[cfg(test)]
mod test {
    use ndarray::{arr1, arr2, s};
    use pretty_assertions::assert_eq;

    use crate::interval::stacktype::fivelimit::ConcreteFiveLimitStackType;

    use super::*;

    #[test]
    fn test_collect_intervals() {
        type Irrelevant = crate::interval::stacktype::fivelimit::ConcreteFiveLimitStackType;
        let mut ws = Workspace::<Irrelevant>::new(1, false, false, false);

        ws.collect_intervals(
            &[0, 1, 2, 3],
            |_, _| Connector::Rod,
            |_| panic!("This will not be called, since there are no springs!"),
            |_| arr1(&[]), // irrelevant
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

        ws.collect_intervals(
            &[0, 1, 2, 3, 4, 5],
            |i, j| {
                if (j - i) % 2 == 0 {
                    Connector::Rod
                } else {
                    Connector::Spring
                }
            },
            |_| (arr2(&[[]]), arr1(&[])), // irrelevant
            |_| arr1(&[]),                // irrelevant
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

        ws.collect_intervals(
            &[0, 2, 5, 7, 12, 14],
            |i, j| {
                let d = j - i;
                if (d % 12 == 0) | (d % 7 == 0) {
                    Connector::Rod
                } else {
                    Connector::Spring
                }
            },
            |_| (arr2(&[[]]), arr1(&[])), // irrelevant
            |_| arr1(&[]),                // irrelevant
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
        let mut solver_workspace = solver::Workspace::new(1, 1, 1);

        let provide_candidate_springs = |d: KeyDistance| {
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

        let provide_candidate_anchors = |i| provide_candidate_springs(i as KeyDistance - 60);

        let epsilon = 0.00000000000000001; // just a very small number. I don't care precisely.

        // if nothing else is given, the first option is picked
        ws.compute_best_solution(
            &[60, 66],
            |i| i == 60,
            |_, _| Connector::Spring,
            provide_candidate_springs,
            provide_candidate_anchors,
            |_| panic!("This will never be called, since there are no rods"),
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
            provide_candidate_anchors,
            |_| panic!("This will never be called, since there are no rods"),
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
            provide_candidate_springs,
            provide_candidate_anchors,
            |_| panic!("This will never be called, since there are no rods"),
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
            provide_candidate_springs,
            provide_candidate_anchors,
            |_| panic!("This will never be called, since there are no rods"),
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
            &[64, 62, 60],
            |i| i == 60,
            |_, _| Connector::Spring,
            provide_candidate_springs,
            provide_candidate_anchors,
            |_| panic!("This will never be called, since there are no rods"),
            &mut solver_workspace,
        )
        .unwrap();
        assert_eq!(
            ws.best_solution(),
            arr2(&[
                [0.into(), 0.into(), 1.into()],
                [(-1).into(), 2.into(), 0.into()],
                [0.into(), 0.into(), 0.into()],
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
            &[60, 62, 64],
            |i| i == 60,
            |_, _| Connector::Spring,
            provide_candidate_springs,
            provide_candidate_anchors,
            |_| panic!("This will never be called, since there are no rods"),
            &mut solver_workspace,
        )
        .unwrap();
        assert_eq!(
            ws.best_solution(),
            arr2(&[
                [0.into(), 0.into(), 0.into()],
                [1.into(), (-2).into(), 1.into()],
                [0.into(), 0.into(), 1.into()],
            ])
        );
        assert!(ws.best_energy() < epsilon);
        assert!(ws.relaxed());

        // D-flat major seventh on C
        ws.compute_best_solution(
            &[60, 61, 65, 68],
            |i| i == 60,
            |_, _| Connector::Spring,
            provide_candidate_springs,
            provide_candidate_anchors,
            |_| panic!("This will never be called, since there are no rods"),
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
            provide_candidate_springs,
            provide_candidate_anchors,
            |_| panic!("This will never be called, since there are no rods"),
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
            provide_candidate_springs,
            provide_candidate_anchors,
            |_| panic!("This will never be called, since there are no rods"),
            &mut solver_workspace,
        )
        .unwrap();
        assert_eq!(
            ws.best_solution(),
            arr2(&[[1.into(), (-1).into(), 1.into()],])
        );
        assert!(ws.best_energy() < epsilon);
        assert!(ws.relaxed());

        // 69 chord cannot be in tune
        ws.compute_best_solution(
            &[60, 62, 64, 67, 69],
            |i| i == 60,
            |_, _| Connector::Spring,
            provide_candidate_springs,
            provide_candidate_anchors,
            |_| panic!("This will never be called, since there are no rods"),
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
            provide_candidate_springs,
            provide_candidate_anchors,
            |s| match s[..] {
                [(7, n)] => arr1(&[0.into(), n.into(), 0.into()]),
                _ => unreachable!(),
            },
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

        // 69 chord with rods for fifhts and fourths. This forces a pythagorean third.
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
            provide_candidate_springs,
            provide_candidate_anchors,
            |s| match s[..] {
                [(7, n)] => arr1(&[0.into(), n.into(), 0.into()]),
                [(5, n)] => arr1(&[n.into(), (-n).into(), 0.into()]),
                [(5, n), (7, m)] => arr1(&[n.into(), (m - n).into(), 0.into()]),
                _ => unreachable!(),
            },
            &mut solver_workspace,
        )
        .unwrap();
        assert_eq!(
            ws.best_solution(),
            arr2(&[
                [0.into(), 0.into(), 0.into()],
                [(-1).into(), 2.into(), 0.into()],
                [(-2).into(), 4.into(), 0.into()],
                [0.into(), 1.into(), 0.into()],
                [(-1).into(), 3.into(), 0.into()],
            ])
        );
        assert!(ws.best_energy() > epsilon);
        assert!(!ws.relaxed());
    }
}
