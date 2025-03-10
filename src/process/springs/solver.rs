//! For motivation, see doc/springs.tex

use ndarray::{
    azip, linalg::general_mat_mul, s, Array1, Array2, ArrayView1, ArrayViewMut1, ArrayViewMut2,
};
use num_rational::Ratio;

use crate::{interval::stacktype::r#trait::StackCoeff, util::lu};

#[derive(Debug)]
pub struct Workspace {
    a: Array2<Ratio<StackCoeff>>,
    perm: Array1<usize>,
    ainv: Array2<Ratio<StackCoeff>>,
    b: Array2<Ratio<StackCoeff>>,
    l: Array2<Ratio<StackCoeff>>,
    bl: Array2<Ratio<StackCoeff>>,
    res: Array2<Ratio<StackCoeff>>,
}

pub struct System<'a> {
    a: ArrayViewMut2<'a, Ratio<StackCoeff>>,
    perm: ArrayViewMut1<'a, usize>,
    ainv: ArrayViewMut2<'a, Ratio<StackCoeff>>,
    b: ArrayViewMut2<'a, Ratio<StackCoeff>>,
    l: ArrayViewMut2<'a, Ratio<StackCoeff>>,
    bl: ArrayViewMut2<'a, Ratio<StackCoeff>>,
    res: ArrayViewMut2<'a, Ratio<StackCoeff>>,
}

impl Workspace {
    pub fn new(n_nodes: usize, n_lengths: usize, n_base_lengths: usize) -> Self {
        Workspace {
            a: Array2::zeros((n_nodes, n_nodes)),
            perm: Array1::zeros(n_nodes + 1),
            ainv: Array2::eye(n_nodes),
            b: Array2::zeros((n_nodes, n_lengths)),
            l: Array2::zeros((n_lengths, n_base_lengths)),
            bl: Array2::zeros((n_nodes, n_base_lengths)),
            res: Array2::zeros((n_nodes, n_base_lengths)),
        }
    }

    pub fn prepare_system(
        &mut self,
        n_nodes: usize,
        n_lengths: usize,
        n_base_lengths: usize,
    ) -> System {
        if n_nodes > self.a.shape()[0] {
            self.a = Array2::zeros((n_nodes, n_nodes));
            self.perm = Array1::zeros(n_nodes + 1);
            self.ainv = Array2::eye(n_nodes);
            self.b = Array2::zeros((n_nodes, n_lengths));
            self.bl = Array2::zeros((n_nodes, n_base_lengths));
            self.res = Array2::zeros((n_nodes, n_base_lengths));
        }

        if n_lengths > self.l.shape()[0] {
            if n_nodes <= self.a.shape()[0] {
                self.b = Array2::zeros((n_nodes, n_lengths));
            }
            self.l = Array2::zeros((n_lengths, n_base_lengths));
        }

        if n_base_lengths > self.l.shape()[1] {
            if n_nodes <= self.a.shape()[0] {
                self.bl = Array2::zeros((n_nodes, n_base_lengths));
                self.res = Array2::zeros((n_nodes, n_base_lengths));
            }
            if n_lengths <= self.l.shape()[0] {
                self.l = Array2::zeros((n_lengths, n_base_lengths));
            }
        }

        let mut sys = System {
            a: self.a.slice_mut(s![..n_nodes, ..n_nodes]),
            perm: self.perm.slice_mut(s![..(n_nodes + 1)]),
            ainv: self.ainv.slice_mut(s![..n_nodes, ..n_nodes]),
            b: self.b.slice_mut(s![..n_nodes, ..n_lengths]),
            l: self.l.slice_mut(s![..n_lengths, ..n_base_lengths]),
            bl: self.bl.slice_mut(s![..n_nodes, ..n_base_lengths]),
            res: self.res.slice_mut(s![..n_nodes, ..n_base_lengths]),
        };
        sys.reset();
        sys
    }
}

impl<'a> System<'a> {
    fn reset(&mut self) {
        self.a.fill(0.into());
        self.b.fill(0.into());
        self.l.fill(0.into());
        // the other members are for intermediate results and will be cleared/initialised by
        // [solve]
    }

    /// Expected invariants:
    /// - `0 <= i < n_lengths`
    /// - `coefficients` has lengths `n_base_lengths`
    pub fn define_length(&mut self, i: usize, coefficients: ArrayView1<Ratio<StackCoeff>>) {
        self.l.row_mut(i).assign(&coefficients);
    }

    /// Expected invariants:
    /// - `0 <= start < end < n_nodes`
    /// - `0 <= length < n_lengths`
    /// - called at most once for each pair `start < end`
    pub fn add_spring(
        &mut self,
        start: usize,
        end: usize,
        length: usize,
        stiffness: Ratio<StackCoeff>,
    ) {
        self.a[[start, end]] = stiffness;
        self.a[[end, start]] = stiffness;
        self.a[[start, start]] -= stiffness;
        self.a[[end, end]] -= stiffness;

        if start < end {
            self.b[[start, length]] += stiffness;
            self.b[[end, length]] -= stiffness;
        } else {
            self.b[[start, length]] -= stiffness;
            self.b[[end, length]] += stiffness;
        }
    }

    /// Expected invariants:
    /// - `0 <= node < n_nodes`
    /// - `0 <= length < n_lengths`
    /// - called at most once for each `node`
    pub fn add_fixed_spring(&mut self, node: usize, length: usize, stiffness: Ratio<StackCoeff>) {
        self.a[[node, node]] -= stiffness;

        self.b[[node, length]] -= stiffness;
    }

    /// Expected invariants:
    /// - `0 <= start < end < n_nodes`
    /// - `0 <= length < n_lengths`
    /// - called at most once for each value of `end`, and then that value of `end` may never again be an
    ///   argument in the `start` or `end` position.
    /// - called after [add_fixed_spring] and [add_spring]
    pub fn add_rod(&mut self, start: usize, end: usize, length: usize) {
        let (mut start_row, mut end_row) = self.a.multi_slice_mut((s![start, ..], s![end, ..]));
        azip!((a in &mut start_row, b in &end_row) *a += b);
        azip!((a in &mut end_row) *a = 0.into());
        end_row[start] = Ratio::from_integer(-1);
        end_row[end] = Ratio::from_integer(1);

        let (mut start_row, mut end_row) = self.b.multi_slice_mut((s![start, ..], s![end, ..]));
        azip!((a in &mut start_row, b in &end_row) *a += b);
        azip!((a in &mut end_row) *a = Ratio::from_integer(0));
        end_row[length] = Ratio::from_integer(1);
    }

    /// Will write the solution to `self.res`
    pub fn solve(mut self) -> Result<ArrayViewMut2<'a, Ratio<StackCoeff>>, lu::LUErr> {
        println!("a:\n {}\n\n", self.a);
        println!("b:\n {}\n\n", self.b);
        println!("l:\n {}\n\n", self.l);

        // Make bl the product b.l
        general_mat_mul(
            Ratio::from_integer(1),
            &self.b,
            &self.l,
            Ratio::from_integer(0),
            &mut self.bl,
        );

        // make ainv the inverse of a
        let lu = lu::lu(self.a, self.perm)?;
        lu.inverse_inplace(&mut self.ainv);

        // Make res the product a^{-1}.b.l
        general_mat_mul(
            Ratio::from_integer(1),
            &self.ainv,
            &self.bl,
            Ratio::from_integer(0),
            &mut self.res,
        );

        Ok(self.res)
    }
}

#[cfg(test)]
mod test {
    use ndarray::arr2;
    use pretty_assertions::assert_eq;

    use super::*;

    struct SystemSpec {
        lengths: Array2<Ratio<StackCoeff>>,
        n_nodes: usize,
        springs: Vec<(usize, usize, usize, Ratio<StackCoeff>)>,
        fixed_springs: Vec<(usize, usize, Ratio<StackCoeff>)>,
        rods: Vec<(usize, usize, usize)>,
    }

    fn initialise_and_solve<'a>(
        workspace: &'a mut Workspace,
        spec: &SystemSpec,
    ) -> ArrayViewMut2<'a, Ratio<StackCoeff>> {
        let n_lengths = spec.lengths.shape()[0];
        let n_base_lengths = spec.lengths.shape()[1];

        let mut system = workspace.prepare_system(spec.n_nodes, n_lengths, n_base_lengths);

        for (i, row) in spec.lengths.rows().into_iter().enumerate() {
            system.define_length(i, row);
        }

        for (start, end, length, stiffness) in &spec.springs {
            system.add_spring(*start, *end, *length, *stiffness);
        }

        for (node, length, stiffness) in &spec.fixed_springs {
            system.add_fixed_spring(*node, *length, *stiffness);
        }

        for (start, end, length) in &spec.rods {
            system.add_rod(*start, *end, *length);
        }

        println!("a={}", system.a);
        println!("b={}", system.b);
        println!("l={}", system.l);

        let res = system.solve().unwrap();

        println!("res={}", res);

        res
    }

    fn one_case(
        workspace: &mut Workspace,
        spec: &SystemSpec,
        expected: &Array2<Ratio<StackCoeff>>,
    ) {
        let actual = initialise_and_solve(workspace, spec);
        assert_eq!(expected.view(), actual.view())
    }

    #[test]
    fn test_result_lengths() {
        let cases = [
            (
                // one node anchored to the origin
                SystemSpec {
                    lengths: arr2(&[[0.into()]]),
                    n_nodes: 1,
                    springs: vec![],
                    fixed_springs: vec![(0, 0, 1.into())],
                    rods: vec![],
                },
                arr2(&[[0.into()]]),
            ),
            (
                // one node anchored to a point that is not the origin
                SystemSpec {
                    lengths: arr2(&[[1.into(), 0.into(), 0.into()]]),
                    n_nodes: 1,
                    springs: vec![],
                    fixed_springs: vec![(0, 0, 1.into())],
                    rods: vec![],
                },
                arr2(&[[1.into(), 0.into(), 0.into()]]),
            ),
            (
                // one anchored node with one node attached to it
                SystemSpec {
                    lengths: arr2(&[
                        [1.into(), 0.into(), 3.into()],
                        [0.into(), 2.into(), 0.into()],
                    ]),
                    n_nodes: 2,
                    springs: vec![(0, 1, 1, 1.into())],
                    fixed_springs: vec![(0, 0, 1.into())],
                    rods: vec![],
                },
                arr2(&[
                    [1.into(), 0.into(), 3.into()],
                    [1.into(), 2.into(), 3.into()],
                ]),
            ),
            (
                // now, the right node is anchored
                SystemSpec {
                    lengths: arr2(&[
                        [1.into(), 0.into(), 3.into()],
                        [0.into(), 2.into(), 0.into()],
                    ]),
                    n_nodes: 2,
                    springs: vec![(0, 1, 0, 1.into())],
                    fixed_springs: vec![(1, 1, 1.into())],
                    rods: vec![],
                },
                arr2(&[
                    [(-1).into(), 2.into(), (-3).into()],
                    [0.into(), 2.into(), 0.into()],
                ]),
            ),
            (
                // three nodes a,b,c, with the a anchored, b attached to a, and c to b
                SystemSpec {
                    lengths: arr2(&[
                        [2.into(), 0.into(), 0.into()],
                        [0.into(), 3.into(), 0.into()],
                    ]),
                    n_nodes: 3,
                    springs: vec![(0, 1, 0, 1.into()), (1, 2, 1, 1.into())],
                    fixed_springs: vec![(0, 0, 1.into())],
                    rods: vec![],
                },
                arr2(&[
                    [2.into(), 0.into(), 0.into()],
                    [4.into(), 0.into(), 0.into()],
                    [4.into(), 3.into(), 0.into()],
                ]),
            ),
            (
                // three nodes each connected to the other two; all springs have the same length
                // and stiffness
                SystemSpec {
                    lengths: arr2(&[[0.into()], [1.into()]]),
                    n_nodes: 3,
                    springs: vec![
                        (0, 1, 1, 1.into()),
                        (1, 2, 1, 1.into()),
                        (0, 2, 1, 1.into()),
                    ],
                    fixed_springs: vec![(0, 0, 1.into())],
                    rods: vec![],
                },
                arr2(&[[0.into()], [Ratio::new(2, 3)], [Ratio::new(4, 3)]]),
            ),
            (
                // three nodes each connected to the other two; the spring connecting the last to
                // the first node is twice as long as the other two
                SystemSpec {
                    lengths: arr2(&[
                        [0.into(), 0.into(), 0.into()],
                        [1.into(), 0.into(), 0.into()],
                        [2.into(), 0.into(), 0.into()],
                    ]),
                    n_nodes: 3,
                    springs: vec![
                        (0, 1, 1, 1.into()),
                        (1, 2, 1, 1.into()),
                        (0, 2, 2, 1.into()),
                    ],
                    fixed_springs: vec![(0, 0, 1.into())],
                    rods: vec![],
                },
                arr2(&[
                    [0.into(), 0.into(), 0.into()],
                    [1.into(), 0.into(), 0.into()],
                    [2.into(), 0.into(), 0.into()],
                ]),
            ),
            (
                // three nodes each connected to the other two; all springs have the same length,
                // but the spring connecting the first to the last node is half as strong as the
                // other two
                SystemSpec {
                    lengths: arr2(&[
                        [0.into(), 0.into(), 0.into()],
                        [1.into(), 0.into(), 0.into()],
                    ]),
                    n_nodes: 3,
                    springs: vec![
                        (0, 1, 1, 2.into()),
                        (1, 2, 1, 2.into()),
                        (0, 2, 1, 1.into()),
                    ],
                    fixed_springs: vec![(0, 0, 1.into())],
                    rods: vec![],
                },
                arr2(&[
                    [0.into(), 0.into(), 0.into()],
                    [Ratio::new(3, 4), 0.into(), 0.into()],
                    [Ratio::new(3, 2), 0.into(), 0.into()],
                ]),
            ),
            (
                // a rod with both ends attached to the origin
                SystemSpec {
                    lengths: arr2(&[
                        [0.into(), 0.into(), 0.into()],
                        [1.into(), 0.into(), 0.into()],
                    ]),
                    n_nodes: 2,
                    springs: vec![],
                    fixed_springs: vec![(0, 0, 1.into()), (1, 0, 1.into())],
                    rods: vec![(0, 1, 1)],
                },
                arr2(&[
                    [Ratio::new(-1, 2), 0.into(), 0.into()],
                    [Ratio::new(1, 2), 0.into(), 0.into()],
                ]),
            ),
            (
                // three springs of equal strength compressed between the two ends of a rod
                SystemSpec {
                    lengths: arr2(&[
                        [0.into(), 0.into(), 0.into()],
                        [1.into(), 0.into(), 0.into()],
                        [7.into(), (-13).into(), 5.into()],
                    ]),
                    n_nodes: 4,
                    springs: vec![
                        (0, 1, 2, 1.into()),
                        (1, 2, 2, 1.into()),
                        (2, 3, 2, 1.into()),
                    ],
                    fixed_springs: vec![(0, 0, 1.into())],
                    rods: vec![(0, 3, 1)],
                },
                arr2(&[
                    [0.into(), 0.into(), 0.into()],
                    [Ratio::new(1, 3), 0.into(), 0.into()],
                    [Ratio::new(2, 3), 0.into(), 0.into()],
                    [1.into(), 0.into(), 0.into()],
                ]),
            ),
            (
                // three springs of unequal strength compressed between the two ends of a rod, the
                // middle spring is twice as stiff as the other two
                SystemSpec {
                    lengths: arr2(&[
                        [0.into(), 0.into(), 0.into()],
                        [1.into(), 0.into(), 0.into()],
                    ]),
                    n_nodes: 4,
                    springs: vec![
                        (0, 1, 1, 1.into()),
                        (1, 2, 1, 2.into()),
                        (2, 3, 1, 1.into()),
                    ],
                    fixed_springs: vec![(0, 0, 1.into())],
                    rods: vec![(0, 3, 1)],
                },
                arr2(&[
                    [0.into(), 0.into(), 0.into()],
                    [Ratio::new(1, 5), 0.into(), 0.into()],
                    [Ratio::new(4, 5), 0.into(), 0.into()],
                    [1.into(), 0.into(), 0.into()],
                ]),
            ),
            (
                // Two rods, connected by a spring, with the rod's free ends connected to the
                // origin. The middle spring will be squashed completely.
                SystemSpec {
                    lengths: arr2(&[
                        [0.into(), 0.into(), 0.into()],
                        [1.into(), 0.into(), 0.into()],
                    ]),
                    n_nodes: 4,
                    springs: vec![(1, 2, 1, 1.into())],
                    fixed_springs: vec![(0, 0, 1.into()), (3, 0, 1.into())],
                    rods: vec![(0, 1, 1), (2, 3, 1)],
                },
                arr2(&[
                    [(-1).into(), 0.into(), 0.into()],
                    [0.into(), 0.into(), 0.into()],
                    [0.into(), 0.into(), 0.into()],
                    [1.into(), 0.into(), 0.into()],
                ]),
            ),
            (
                // A triangle of two rods and a spring under tension
                SystemSpec {
                    lengths: arr2(&[
                        [0.into(), 0.into(), 0.into()],
                        [1.into(), 0.into(), 0.into()],
                        [3.into(), 0.into(), 0.into()],
                    ]),
                    n_nodes: 3,
                    springs: vec![(1, 2, 1, 1.into())],
                    fixed_springs: vec![(0, 0, 1.into())],
                    rods: vec![(0, 1, 1), (0, 2, 2)],
                },
                arr2(&[
                    [0.into(), 0.into(), 0.into()],
                    [1.into(), 0.into(), 0.into()],
                    [3.into(), 0.into(), 0.into()],
                ]),
            ),
        ];

        let n_nodes_initial = 1;
        let n_lengths_initial = 1;
        let n_base_lengths = 3;
        let mut workspace = Workspace::new(n_nodes_initial, n_lengths_initial, n_base_lengths);

        for (spec, expected) in cases.iter() {
            one_case(&mut workspace, spec, expected);
        }
    }
}
