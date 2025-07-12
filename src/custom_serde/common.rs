use std::{fmt, marker::PhantomData};

use ndarray::{Array1, ArrayView1};
use num_rational::Ratio;
use num_traits::Zero;
use serde::{de::Visitor, ser::SerializeMap, Serializer};

use crate::interval::stacktype::r#trait::{IntervalBasis, StackCoeff};

pub struct NamedCoefficientsView<'a, T: IntervalBasis, C> {
    pub coeffs: ArrayView1<'a, C>,
    _phantom: PhantomData<T>,
}

impl<'a, T: IntervalBasis, C> NamedCoefficientsView<'a, T, C> {
    pub fn new(inner: ArrayView1<'a, C>) -> Self {
        Self {
            coeffs: inner,
            _phantom: PhantomData,
        }
    }
}

trait DeSerNumber: Sized {
    type Rep;
    fn to_rep(&self) -> Self::Rep;
    fn from_rep(rep: &Self::Rep) -> Option<Self>;
}

impl DeSerNumber for StackCoeff {
    type Rep = StackCoeff;
    fn to_rep(&self) -> Self::Rep {
        *self
    }
    fn from_rep(rep: &Self::Rep) -> Option<Self> {
        Some(*rep)
    }
}

impl DeSerNumber for Ratio<StackCoeff> {
    type Rep = String;
    fn to_rep(&self) -> Self::Rep {
        self.to_string()
    }
    fn from_rep(rep: &Self::Rep) -> Option<Self> {
        match rep.parse() {
            Ok(x) => Some(x),
            Err(_) => None {},
        }
    }
}

impl<'a, T, C> serde::Serialize for NamedCoefficientsView<'a, T, C>
where
    T: IntervalBasis,
    C: Zero + DeSerNumber,
    <C as DeSerNumber>::Rep: serde::Serialize,
{
    fn serialize<S: Serializer>(&self, ser: S) -> Result<S::Ok, S::Error> {
        let n = self.coeffs.iter().filter(|x| !x.is_zero()).count();
        let mut t = ser.serialize_map(Some(n))?;
        for (i, c) in self.coeffs.iter().enumerate() {
            if !c.is_zero() {
                t.serialize_entry(&T::intervals()[i].name, &c.to_rep())?;
            }
        }
        t.end()
    }
}

pub struct NamedCoefficients<T: IntervalBasis, C> {
    pub coeffs: Array1<C>,
    _phantom: PhantomData<T>,
}

impl<'de, T, C> serde::Deserialize<'de> for NamedCoefficients<T, C>
where
    T: IntervalBasis,
    C: 'static + Clone + DeSerNumber + Zero,
    <C as DeSerNumber>::Rep: serde::Deserialize<'de> + fmt::Display,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct NamedCoefficientsVisitor<T: IntervalBasis, C> {
            _phantom: PhantomData<(T, C)>,
        }

        impl<'de, T, C> Visitor<'de> for NamedCoefficientsVisitor<T, C>
        where
            T: IntervalBasis,
            C: 'static + Clone + DeSerNumber + Zero,
            <C as DeSerNumber>::Rep: serde::Deserialize<'de> + fmt::Display,
        {
            type Value = NamedCoefficients<T, C>;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                write!(formatter, "map of interval names ")?;
                for i in T::intervals() {
                    write!(formatter, "'{}' ", i.name)?;
                }
                write!(formatter, "to numbers")
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::MapAccess<'de>,
            {
                let mut res = Array1::zeros(T::num_intervals());
                let mut set = Array1::from_elem(T::num_intervals(), false);
                while let Some((key, value)) =
                    map.next_entry::<String, <C as DeSerNumber>::Rep>()?
                {
                    match T::interval_positions().get(&key) {
                        None {} => {
                            return Err(serde::de::Error::custom(format!(
                                "'{}' is not an interval name",
                                key
                            )))
                        }
                        Some(i) => match C::from_rep(&value) {
                            None {} => {
                                return Err(serde::de::Error::custom(format!(
                                    "'{}' is not a well-formed number",
                                    value
                                )))
                            }
                            Some(c) => {
                                if set[*i] {
                                    return Err(serde::de::Error::custom(format!(
                                        "duplicate definition for '{}'",
                                        key
                                    )));
                                }
                                res[*i] = c;
                                set[*i] = true;
                            }
                        },
                    }
                }

                Ok(Self::Value {
                    coeffs: res,
                    _phantom: PhantomData,
                })
            }
        }

        deserializer.deserialize_map(NamedCoefficientsVisitor {
            _phantom: PhantomData,
        })
    }
}
