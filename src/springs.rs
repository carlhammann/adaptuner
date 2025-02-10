//! For motivation, see doc/springs.tex

use fractionfree;
use ndarray::{
    azip, linalg::general_mat_mul, s, Array1, Array2, ArrayView1, ArrayView2, ArrayViewMut1,
    ArrayViewMut2,
};

type Coeff = i32;

#[derive(Debug)]
struct Workspace {
    a: Array2<Coeff>,
    ainv: Array2<Coeff>,
    b: Array2<Coeff>,
    l: Array2<Coeff>,
    bl: Array2<Coeff>,
    res_denominators: Array1<Coeff>,
    res_numerators: Array2<Coeff>,
}

struct System<'a> {
    a: ArrayViewMut2<'a, Coeff>,
    ainv: ArrayViewMut2<'a, Coeff>,
    b: ArrayViewMut2<'a, Coeff>,
    l: ArrayViewMut2<'a, Coeff>,
    bl: ArrayViewMut2<'a, Coeff>,
    res_denominators: ArrayViewMut1<'a, Coeff>,
    res_numerators: ArrayViewMut2<'a, Coeff>,
}

impl Workspace {
    fn new(n_nodes: usize, n_lengths: usize, n_base_lengths: usize) -> Self {
        Workspace {
            a: Array2::zeros((n_nodes, n_nodes)),
            ainv: Array2::eye(n_nodes),
            b: Array2::zeros((n_nodes, n_lengths)),
            l: Array2::zeros((n_lengths, n_base_lengths)),
            bl: Array2::zeros((n_nodes, n_base_lengths)),
            res_denominators: Array1::ones(n_nodes),
            res_numerators: Array2::zeros((n_nodes, n_base_lengths)),
        }
    }

    fn prepare_system(&mut self, n_nodes: usize, n_lengths: usize) -> System {
        let n_base_lengths = self.l.shape()[1];

        if n_nodes > self.a.shape()[0] {
            self.a = Array2::zeros((n_nodes, n_nodes));
            self.ainv = Array2::eye(n_nodes);
            self.b = Array2::zeros((n_nodes, n_lengths));
            self.bl = Array2::zeros((n_nodes, n_base_lengths));
            self.res_denominators = Array1::ones(n_nodes);
            self.res_numerators = Array2::zeros((n_nodes, n_base_lengths));
        }

        if n_lengths > self.l.shape()[0] {
            if n_nodes <= self.a.shape()[0] {
                // we already resized this above:
                self.b = Array2::zeros((n_nodes, n_lengths));
            }
            self.l = Array2::zeros((n_lengths, n_base_lengths));
        }

        let mut sys = System {
            a: self.a.slice_mut(s![..n_nodes, ..n_nodes]),
            ainv: self.ainv.slice_mut(s![..n_nodes, ..n_nodes]),
            b: self.b.slice_mut(s![..n_nodes, ..n_lengths]),
            l: self.l.slice_mut(s![..n_lengths, ..n_base_lengths]),
            bl: self.bl.slice_mut(s![..n_nodes, ..n_base_lengths]),
            res_denominators: self.res_denominators.slice_mut(s![..n_nodes]),
            res_numerators: self
                .res_numerators
                .slice_mut(s![..n_nodes, ..n_base_lengths]),
        };
        sys.reset();
        sys
    }
}

impl<'a> System<'a> {
    fn reset(&mut self) {
        self.a.fill(0);
        self.b.fill(0);
        self.l.fill(0);
        // the other members are for intermediate results and will be cleared/initialised by
        // [solve]
    }

    /// Expected invariants:
    /// - `0 <= i < n_lengths`
    /// - `coefficients` has lengths `n_base_lengths`
    fn define_length(&mut self, i: usize, coefficients: &ArrayView1<Coeff>) {
        self.l.row_mut(i).assign(coefficients);
    }

    /// Expected invariants:
    /// - `0 <= start < end < n_nodes`
    /// - `0 <= length < n_lengths`
    /// - called at most once for each pair `start < end`
    fn add_spring(&mut self, start: usize, end: usize, length: usize, stiffness: Coeff) {
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
    fn add_fixed_spring(&mut self, node: usize, length: usize, stiffness: Coeff) {
        self.a[[node, node]] -= stiffness;

        self.b[[node, length]] -= stiffness;
    }

    /// Expected invariants:
    /// - `0 <= start < end < n_nodes`
    /// - `0 <= length < n_lengths`
    /// - called at most once for each value of `end`, and then that value of `end` may never again be an
    ///   argument in the `start` or `end` position.
    /// - called after [add_fixed_spring] and [add_spring]
    fn add_rod(&mut self, start: usize, end: usize, length: usize) {
        let (mut start_row, mut end_row) = self.a.multi_slice_mut((s![start, ..], s![end, ..]));
        azip!((a in &mut start_row, b in &end_row) *a += b);
        azip!((a in &mut end_row) *a = 0);
        end_row[start] = -1;
        end_row[end] = 1;

        let (mut start_row, mut end_row) = self.b.multi_slice_mut((s![start, ..], s![end, ..]));
        azip!((a in &mut start_row, b in &end_row) *a += b);
        azip!((a in &mut end_row) *a = 0);
        end_row[length] = 1;
    }

    fn solve(
        mut self,
    ) -> Result<(ArrayViewMut1<'a, Coeff>, ArrayViewMut2<'a, Coeff>), fractionfree::LinalgErr> {
        // Make bl the product b.l
        general_mat_mul(1, &self.b, &self.l, 0, &mut self.bl);
        let lu = fractionfree::lu(self.a)?;
        lu.inverse_inplace(&mut self.res_denominators[0], &mut self.ainv)?;

        // Make res the product a^{-1}.b.l
        general_mat_mul(1, &self.ainv, &self.bl, 0, &mut self.res_numerators);

        // normalise
        let d = self.res_denominators[0];
        self.res_denominators.fill(d);
        fractionfree::normalise(&mut self.res_denominators, &mut self.res_numerators)?;

        Ok((self.res_denominators, self.res_numerators))
    }
}

#[cfg(test)]
mod test {
    use ndarray::{arr1, arr2};
    use pretty_assertions::assert_eq;

    use super::*;

    struct SystemSpec {
        lengths: Array2<Coeff>,
        n_nodes: usize,
        springs: Vec<(usize, usize, usize, Coeff)>,
        fixed_springs: Vec<(usize, usize, Coeff)>,
        rods: Vec<(usize, usize, usize)>,
    }

    fn initialise_and_solve<'a>(
        workspace: &'a mut Workspace,
        spec: &SystemSpec,
    ) -> (ArrayViewMut1<'a, Coeff>, ArrayViewMut2<'a, Coeff>) {
        let n_lengths = spec.lengths.shape()[0];

        let mut system = workspace.prepare_system(spec.n_nodes, n_lengths);

        for (i, row) in spec.lengths.rows().into_iter().enumerate() {
            system.define_length(i, &row);
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

        let (d, n) = system.solve().unwrap();

        println!("(d,n)=({},{})", d, n);

        (d, n)
    }

    fn one_case(
        workspace: &mut Workspace,
        spec: &SystemSpec,
        expected: &(Array1<Coeff>, Array2<Coeff>), // expected to be [normalise]d
    ) {
        let (actual_denoms, actual_numers) = initialise_and_solve(workspace, spec); // these are normalised
        let (expected_denoms, expected_numers) = expected;
        assert_eq!(
            (expected_denoms.view(), expected_numers.view()),
            (actual_denoms.view(), actual_numers.view())
        )
    }

    #[test]
    fn foo() {
        let cases = [
            (
                // one node anchored to the origin
                SystemSpec {
                    lengths: arr2(&[[0, 0, 0]]),
                    n_nodes: 1,
                    springs: vec![],
                    fixed_springs: vec![(0, 0, 1)],
                    rods: vec![],
                },
                (arr1(&[1]), arr2(&[[0, 0, 0]])),
            ),
            (
                // one node anchored to a point that is not the origin
                SystemSpec {
                    lengths: arr2(&[[1, 0, 0]]),
                    n_nodes: 1,
                    springs: vec![],
                    fixed_springs: vec![(0, 0, 1)],
                    rods: vec![],
                },
                (arr1(&[1]), arr2(&[[1, 0, 0]])),
            ),
            (
                // one anchored node with one node attached to it
                SystemSpec {
                    lengths: arr2(&[[1, 0, 3], [0, 2, 0]]),
                    n_nodes: 2,
                    springs: vec![(0, 1, 1, 1)],
                    fixed_springs: vec![(0, 0, 1)],
                    rods: vec![],
                },
                (arr1(&[1, 1]), arr2(&[[1, 0, 3], [1, 2, 3]])),
            ),
            (
                // now, the right node is anchored
                SystemSpec {
                    lengths: arr2(&[[1, 0, 3], [0, 2, 0]]),
                    n_nodes: 2,
                    springs: vec![(0, 1, 0, 1)],
                    fixed_springs: vec![(1, 1, 1)],
                    rods: vec![],
                },
                (arr1(&[1, 1]), arr2(&[[-1, 2, -3], [0, 2, 0]])),
            ),
            (
                // three nodes a,b,c, with the a anchored, b attached to a, and c to b
                SystemSpec {
                    lengths: arr2(&[[2, 0, 0], [0, 3, 0]]),
                    n_nodes: 3,
                    springs: vec![(0, 1, 0, 1), (1, 2, 1, 1)],
                    fixed_springs: vec![(0, 0, 1)],
                    rods: vec![],
                },
                (arr1(&[1, 1, 1]), arr2(&[[2, 0, 0], [4, 0, 0], [4, 3, 0]])),
            ),
            (
                // three nodes each connected to the other two; all springs have the same length
                // and stiffness
                SystemSpec {
                    lengths: arr2(&[[0, 0, 0], [1, 0, 0]]),
                    n_nodes: 3,
                    springs: vec![(0, 1, 1, 1), (1, 2, 1, 1), (0, 2, 1, 1)],
                    fixed_springs: vec![(0, 0, 1)],
                    rods: vec![],
                },
                (arr1(&[1, 3, 3]), arr2(&[[0, 0, 0], [2, 0, 0], [4, 0, 0]])),
            ),
            (
                // three nodes each connected to the other two; the spring connecting the last to
                // the first node is twice as long as the other two
                SystemSpec {
                    lengths: arr2(&[[0, 0, 0], [1, 0, 0], [2, 0, 0]]),
                    n_nodes: 3,
                    springs: vec![(0, 1, 1, 1), (1, 2, 1, 1), (0, 2, 2, 1)],
                    fixed_springs: vec![(0, 0, 1)],
                    rods: vec![],
                },
                (arr1(&[1, 1, 1]), arr2(&[[0, 0, 0], [1, 0, 0], [2, 0, 0]])),
            ),
            (
                // three nodes each connected to the other two; all springs have the same length,
                // but the spring connecting the first to the last node is half as strong as the
                // other two
                SystemSpec {
                    lengths: arr2(&[[0, 0, 0], [1, 0, 0]]),
                    n_nodes: 3,
                    springs: vec![(0, 1, 1, 2), (1, 2, 1, 2), (0, 2, 1, 1)],
                    fixed_springs: vec![(0, 0, 1)],
                    rods: vec![],
                },
                (arr1(&[1, 4, 2]), arr2(&[[0, 0, 0], [3, 0, 0], [3, 0, 0]])),
            ),
            (
                // a rod with both ends attached to the origin
                SystemSpec {
                    lengths: arr2(&[[0, 0, 0], [1, 0, 0]]),
                    n_nodes: 2,
                    springs: vec![],
                    fixed_springs: vec![(0, 0, 1), (1, 0, 1)],
                    rods: vec![(0, 1, 1)],
                },
                (arr1(&[2, 2]), arr2(&[[-1, 0, 0], [1, 0, 0]])),
            ),
            (
                // three springs of equal strength compressed between the two ends of a rod
                SystemSpec {
                    lengths: arr2(&[[0, 0, 0], [1, 0, 0], [7, -13, 5]]),
                    n_nodes: 4,
                    springs: vec![(0, 1, 2, 1), (1, 2, 2, 1), (2, 3, 2, 1)],
                    fixed_springs: vec![(0, 0, 1)],
                    rods: vec![(0, 3, 1)],
                },
                (
                    arr1(&[1, 3, 3, 1]),
                    arr2(&[[0, 0, 0], [1, 0, 0], [2, 0, 0], [1, 0, 0]]),
                ),
            ),
            (
                // three springs of unequal strength compressed between the two ends of a rod, the
                // middle spring is twice as stiff as the other two
                SystemSpec {
                    lengths: arr2(&[[0, 0, 0], [1, 0, 0]]),
                    n_nodes: 4,
                    springs: vec![(0, 1, 1, 1), (1, 2, 1, 2), (2, 3, 1, 1)],
                    fixed_springs: vec![(0, 0, 1)],
                    rods: vec![(0, 3, 1)],
                },
                (
                    arr1(&[1, 5, 5, 1]),
                    arr2(&[[0, 0, 0], [1, 0, 0], [4, 0, 0], [1, 0, 0]]),
                ),
            ),
            (
                // Two rods, connected by a spring, with the rod's free ends connected to the
                // origin. The middle spring will be squashed completely.
                SystemSpec {
                    lengths: arr2(&[[0, 0, 0], [1, 0, 0]]),
                    n_nodes: 4,
                    springs: vec![(1, 2, 1, 1)],
                    fixed_springs: vec![(0, 0, 1), (3, 0, 1)],
                    rods: vec![(0, 1, 1), (2, 3, 1)],
                },
                (
                    arr1(&[1, 1, 1, 1]),
                    arr2(&[[-1, 0, 0], [0, 0, 0], [0, 0, 0], [1, 0, 0]]),
                ),
            ),
            (
                // A triangle of two rods and a spring under tension
                SystemSpec {
                    lengths: arr2(&[[0, 0, 0], [1, 0, 0], [3, 0, 0]]),
                    n_nodes: 3,
                    springs: vec![(1, 2, 1, 1)],
                    fixed_springs: vec![(0, 0, 1)],
                    rods: vec![(0, 1, 1), (0, 2, 2)],
                },
                (arr1(&[1, 1, 1]), arr2(&[[0, 0, 0], [1, 0, 0], [3, 0, 0]])),
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
