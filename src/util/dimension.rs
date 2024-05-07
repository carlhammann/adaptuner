use ndarray::{arr1, arr2, s, Array1, Array2, ArrayView1, FixedInitializer};
use std::{
    any::TypeId,
    collections::HashMap,
    error::Error,
    fmt,
    marker::PhantomData,
    ops,
    sync::{LazyLock, RwLock},
};

pub trait Dimension {
    fn value() -> usize;
}

pub trait AtLeast<const N: usize>: Dimension {}

pub struct RuntimeDimension<T> {
    _phantom: PhantomData<T>,
}

static RUNTIME_DIMENSIONS_INITIALISED: LazyLock<RwLock<HashMap<TypeId, usize>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

pub fn initialise_runtime_dimension<T: 'static>(value: usize) -> RuntimeDimension<T> {
    let id = TypeId::of::<T>();
    let l = LazyLock::force(&RUNTIME_DIMENSIONS_INITIALISED);
    if !l.read().unwrap().contains_key(&id) {
        l.write().unwrap().insert(id, value);
        RuntimeDimension {
            _phantom: PhantomData,
        }
    } else {
        panic!("attempt to initialise RuntimeDimension twice!",)
    }
}

impl<T: 'static> Dimension for RuntimeDimension<T> {
    fn value() -> usize {
        let id = TypeId::of::<T>();
        let l = LazyLock::force(&RUNTIME_DIMENSIONS_INITIALISED);
        match l.read().unwrap().get(&id) {
            None => {
                panic!("attempt to use RuntimeDimension without initialisation")
            }
            Some(n) => *n,
        }
    }
}

pub struct RuntimeAtLeast<const N: usize, T> {
    _phantom: PhantomData<T>,
}

pub fn initialise_runtime_at_least<const N: usize, T: 'static>(
    value: usize,
) -> RuntimeAtLeast<N, T> {
    let id = TypeId::of::<T>();
    let l = LazyLock::force(&RUNTIME_DIMENSIONS_INITIALISED);
    if !l.read().unwrap().contains_key(&id) {
        if value < N {
            panic!("attempt to initialise RuntimeAtLeast<{N}, T> with a value less than {N}")
        }
        l.write().unwrap().insert(id, value);
        RuntimeAtLeast {
            _phantom: PhantomData,
        }
    } else {
        panic!("attempt to initialise RuntimeDimension or RuntimeAtLeast twice!",)
    }
}

impl<const N: usize, T: 'static> Dimension for RuntimeAtLeast<N, T> {
    fn value() -> usize {
        let id = TypeId::of::<T>();
        let l = LazyLock::force(&RUNTIME_DIMENSIONS_INITIALISED);
        match l.read().unwrap().get(&id) {
            None => panic!("attempt to use RuntimeAtLeast without initialisation"),
            Some(n) => *n,
        }
    }
}

impl<const N: usize, T: 'static> AtLeast<N> for RuntimeAtLeast<N, T> {}

#[derive(Clone, Copy)]
pub struct Bounded<D> {
    inner: usize,
    _phantom: PhantomData<D>,
}

impl<D: Dimension> Bounded<D> {
    pub fn new(inner: usize) -> Result<Self, DimensionErr> {
        if D::value() <= inner {
            Err(DimensionErr::OutOfBound(
                "initialising Bounded",
                D::value(),
                inner,
            ))
        } else {
            Ok(Bounded {
                inner,
                _phantom: PhantomData,
            })
        }
    }
    pub fn get(&self) -> usize {
        self.inner
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Vector<D: Dimension, I> {
    inner: Array1<I>,
    _phantom: PhantomData<D>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct VectorView<'a, D: Dimension, I> {
    inner: ArrayView1<'a, I>,
    _phantom: PhantomData<D>,
}

pub fn vector<D: Dimension, I: Clone>(values: &[I]) -> Result<Vector<D, I>, DimensionErr> {
    let inner = arr1(values);
    Vector::new(inner)
}

pub fn vector_from_elem<D: Dimension, I: Clone>(elem: I) -> Vector<D, I> {
    Vector::new(Array1::from_elem(D::value(), elem)).unwrap()
}

impl<D: Dimension, I> Vector<D, I> {
    pub fn new(inner: Array1<I>) -> Result<Self, DimensionErr> {
        let d = D::value();
        if inner.len() != d {
            return Err(DimensionErr::Mismatch(
                "initialising Vector",
                d,
                inner.len(),
            ));
        } else {
            Ok(Vector {
                inner,
                _phantom: PhantomData,
            })
        }
    }

    pub fn view(&self) -> VectorView<D, I> {
        VectorView {
            inner: self.inner.view(),
            _phantom: PhantomData,
        }
    }
}

impl<D: Dimension, I> ops::Index<Bounded<D>> for Vector<D, I> {
    type Output = I;
    fn index(&self, i: Bounded<D>) -> &I {
        &self.inner[i.inner]
    }
}

impl<D: Dimension, I> ops::IndexMut<Bounded<D>> for Vector<D, I> {
    fn index_mut(&mut self, i: Bounded<D>) -> &mut I {
        &mut self.inner[i.inner]
    }
}

impl<'a, D: Dimension, I> ops::Index<Bounded<D>> for VectorView<'a, D, I> {
    type Output = I;
    fn index(&self, i: Bounded<D>) -> &I {
        &self.inner[i.inner]
    }
}

pub struct VectorIterator<'a, D: Dimension, I> {
    inner: ndarray::iter::IndexedIter<'a, I, ndarray::Ix1>,
    _phantom: PhantomData<D>,
}

impl<'a, D: Dimension, I> Iterator for VectorIterator<'a, D, I> {
    type Item = (Bounded<D>, &'a I);
    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|(i, x)| {
            (
                Bounded {
                    inner: i,
                    _phantom: PhantomData,
                },
                x,
            )
        })
    }
}

impl<'a, D: Dimension, I> IntoIterator for &'a Vector<D, I> {
    type Item = (Bounded<D>, &'a I);
    type IntoIter = VectorIterator<'a, D, I>;
    fn into_iter(self) -> Self::IntoIter {
        VectorIterator {
            inner: self.inner.indexed_iter(),
            _phantom: PhantomData,
        }
    }
}

pub struct VectorIteratorMut<'a, D: Dimension, I: 'static> {
    inner: ndarray::iter::IndexedIterMut<'a, I, ndarray::Ix1>,
    _phantom: PhantomData<D>,
}

impl<'a, D: Dimension, I: 'static> Iterator for VectorIteratorMut<'a, D, I> {
    type Item = (Bounded<D>, &'a mut I);
    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|(i, x)| {
            (
                Bounded {
                    inner: i,
                    _phantom: PhantomData,
                },
                x,
            )
        })
    }
}

impl<'a, D: Dimension, I: 'static> IntoIterator for &'a mut Vector<D, I> {
    type Item = (Bounded<D>, &'a mut I);
    type IntoIter = VectorIteratorMut<'a, D, I>;
    fn into_iter(self) -> Self::IntoIter {
        VectorIteratorMut {
            inner: self.inner.indexed_iter_mut(),
            _phantom: PhantomData,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Matrix<R: Dimension, C: Dimension, I> {
    inner: Array2<I>,
    _phantom: PhantomData<(R, C)>,
}

pub fn matrix<R, C, I, V>(values: &[V]) -> Result<Matrix<R, C, I>, DimensionErr>
where
    R: Dimension,
    C: Dimension,
    V: FixedInitializer<Elem = I> + Clone,
    I: Clone,
{
    let inner = arr2(values);
    Matrix::new(inner)
}

impl<R: Dimension, C: Dimension, I> Matrix<R, C, I> {
    pub fn new(inner: Array2<I>) -> Result<Self, DimensionErr> {
        let rows = R::value();
        let cols = C::value();

        if inner.raw_dim()[0] != rows {
            return Err(DimensionErr::Mismatch(
                "initialising Matrix, number of rows",
                rows,
                inner.raw_dim()[0],
            ));
        }

        if inner.raw_dim()[1] != cols {
            return Err(DimensionErr::Mismatch(
                "initialising Matrix, number of columns",
                rows,
                inner.raw_dim()[1],
            ));
        }

        Ok(Matrix {
            inner,
            _phantom: PhantomData,
        })
    }

    pub fn from_fn<F>(mut f: F) -> Self
    where
        F: FnMut((Bounded<R>, Bounded<C>)) -> I,
    {
        Matrix {
            inner: Array2::from_shape_fn((R::value(), C::value()), |(i, j)| {
                f((Bounded::new(i).unwrap(), Bounded::new(j).unwrap()))
            }),
            _phantom: PhantomData,
        }
    }

    pub fn get_array2(&self) -> &Array2<I> {
        &self.inner
    }

    pub fn into_array2(self) -> Array2<I> {
        self.inner
    }

    pub fn row_ref(&self, i: Bounded<R>) -> VectorView<C, I> {
        VectorView {
            inner: self.inner.slice(s![i.get(), ..]),
            _phantom: PhantomData,
        }
    }
}

impl<R: Dimension, C: Dimension, I> ops::Index<(Bounded<R>, Bounded<C>)> for Matrix<R, C, I> {
    type Output = I;
    fn index(&self, i: (Bounded<R>, Bounded<C>)) -> &I {
        &self.inner[[i.0.inner, i.1.inner]]
    }
}

impl<R: Dimension, C: Dimension, I> ops::IndexMut<(Bounded<R>, Bounded<C>)> for Matrix<R, C, I> {
    fn index_mut(&mut self, i: (Bounded<R>, Bounded<C>)) -> &mut I {
        &mut self.inner[[i.0.inner, i.1.inner]]
    }
}

pub struct MatrixIterator<'a, R: Dimension, C: Dimension, I> {
    inner: ndarray::iter::IndexedIter<'a, I, ndarray::Ix2>,
    _phantom: PhantomData<(R, C)>,
}

impl<'a, R: Dimension, C: Dimension, I> Iterator for MatrixIterator<'a, R, C, I> {
    type Item = ((Bounded<R>, Bounded<C>), &'a I);
    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|((i, j), x)| {
            (
                (
                    Bounded {
                        inner: i,
                        _phantom: PhantomData,
                    },
                    Bounded {
                        inner: j,
                        _phantom: PhantomData,
                    },
                ),
                x,
            )
        })
    }
}

impl<'a, R: Dimension, C: Dimension, I> IntoIterator for &'a Matrix<R, C, I> {
    type Item = ((Bounded<R>, Bounded<C>), &'a I);
    type IntoIter = MatrixIterator<'a, R, C, I>;
    fn into_iter(self) -> Self::IntoIter {
        MatrixIterator {
            inner: self.inner.indexed_iter(),
            _phantom: PhantomData,
        }
    }
}

pub struct MatrixIteratorMut<'a, R: Dimension, C: Dimension, I> {
    inner: ndarray::iter::IndexedIterMut<'a, I, ndarray::Ix2>,
    _phantom: PhantomData<(R, C)>,
}

impl<'a, R: Dimension, C: Dimension, I> Iterator for MatrixIteratorMut<'a, R, C, I> {
    type Item = ((Bounded<R>, Bounded<C>), &'a mut I);
    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|((i, j), x)| {
            (
                (
                    Bounded {
                        inner: i,
                        _phantom: PhantomData,
                    },
                    Bounded {
                        inner: j,
                        _phantom: PhantomData,
                    },
                ),
                x,
            )
        })
    }
}

impl<'a, R: Dimension, C: Dimension, I> IntoIterator for &'a mut Matrix<R, C, I> {
    type Item = ((Bounded<R>, Bounded<C>), &'a mut I);
    type IntoIter = MatrixIteratorMut<'a, R, C, I>;
    fn into_iter(self) -> Self::IntoIter {
        MatrixIteratorMut {
            inner: self.inner.indexed_iter_mut(),
            _phantom: PhantomData,
        }
    }
}

#[derive(Debug)]
pub enum DimensionErr {
    Mismatch(&'static str, usize, usize),
    OutOfBound(&'static str, usize, usize),
}

impl fmt::Display for DimensionErr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DimensionErr::Mismatch(info, expected, actual) => write!(
                f,
                "Dimension mismatch ({info}): Expected {expected}, got {actual}."
            ),
            DimensionErr::OutOfBound(info, bound, actual) => write!(
                f,
                "Out of bounds ({info}): Allowed rage is 0..({bound}-1), got {actual}."
            ),
        }
    }
}

impl Error for DimensionErr {}

pub mod fixed_sizes {
    use super::{AtLeast, Dimension};

    #[derive(Clone, Copy, Debug, PartialEq)]
    pub struct Size2 {}
    impl Dimension for Size2 {
        fn value() -> usize {
            2
        }
    }
    impl AtLeast<2> for Size2 {}
    impl AtLeast<1> for Size2 {}

    #[derive(Clone, Copy, Debug, PartialEq)]
    pub struct Size3 {}
    impl Dimension for Size3 {
        fn value() -> usize {
            3
        }
    }
    impl AtLeast<3> for Size3 {}
    impl AtLeast<2> for Size3 {}
    impl AtLeast<1> for Size3 {}
}
