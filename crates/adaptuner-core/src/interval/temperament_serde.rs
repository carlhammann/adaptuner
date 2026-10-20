use alloc::{format, string::String, vec::Vec};
use core::{fmt, marker::PhantomData};

use ndarray::Array2;
use serde::{
    de::Visitor,
    ser::{SerializeStruct, Serializer},
    Deserialize, Deserializer, Serialize,
};

use crate::interval::{
    stack::key_distance_from_coefficients,
    stacktype::{
        r#trait::{IntervalBasis, StackCoeff},
        serde_support::{NamedCoefficients, NamedCoefficientsView},
    },
    temperament::TemperamentDefinition,
};

impl<T> Serialize for TemperamentDefinition<T>
where
    T: IntervalBasis + Serialize,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        #[derive(Serialize)]
        struct TemperamentEquationView<'a, T: IntervalBasis> {
            tempered: NamedCoefficientsView<'a, T, StackCoeff>,
            pure: NamedCoefficientsView<'a, T, StackCoeff>,
        }

        let mut t = serializer.serialize_struct("temperament definition", 3)?;
        t.serialize_field("name", &self.name)?;
        t.serialize_field(
            "equations",
            &(0..T::num_intervals())
                .map(|i| TemperamentEquationView::<T> {
                    tempered: NamedCoefficientsView::new(self.tempered.row(i)),
                    pure: NamedCoefficientsView::new(self.pure.row(i)),
                })
                .collect::<Vec<_>>(),
        )?;
        t.end()
    }
}

impl<'de, T: IntervalBasis + Deserialize<'de>> Deserialize<'de> for TemperamentDefinition<T> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct TemperamentEquation<T: IntervalBasis> {
            tempered: NamedCoefficients<T, StackCoeff>,
            pure: NamedCoefficients<T, StackCoeff>,
        }

        impl<'de, T: IntervalBasis + Deserialize<'de>> Deserialize<'de> for TemperamentEquation<T> {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                #[derive(Deserialize)]
                #[serde(field_identifier, rename_all = "lowercase")]
                enum TemperamentEquationField {
                    Tempered,
                    Pure,
                }

                struct TemperamentEquationVisitor<T: IntervalBasis> {
                    _phantom: PhantomData<T>,
                }

                impl<'de, T: IntervalBasis + Deserialize<'de>> Visitor<'de> for TemperamentEquationVisitor<T> {
                    type Value = TemperamentEquation<T>;

                    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                        write!(formatter, "temperament equation")
                    }

                    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
                    where
                        A: serde::de::MapAccess<'de>,
                    {
                        let mut pure: Option<NamedCoefficients<T, StackCoeff>> = None {};
                        let mut tempered: Option<NamedCoefficients<T, StackCoeff>> = None {};

                        while let Some(key) = map.next_key()? {
                            match key {
                                TemperamentEquationField::Pure => {
                                    if pure.is_some() {
                                        return Err(serde::de::Error::duplicate_field("pure"));
                                    }
                                    pure = Some(map.next_value()?);
                                }
                                TemperamentEquationField::Tempered => {
                                    if tempered.is_some() {
                                        return Err(serde::de::Error::duplicate_field("tempered"));
                                    }
                                    tempered = Some(map.next_value()?);
                                }
                            }
                        }

                        if pure.is_none() {
                            return Err(serde::de::Error::missing_field("pure"));
                        }
                        if tempered.is_none() {
                            return Err(serde::de::Error::missing_field("tempered"));
                        }

                        let unwrapped_tempered = tempered.unwrap();
                        let unwrapped_pure = pure.unwrap();
                        let td =
                            key_distance_from_coefficients::<T>(unwrapped_tempered.coeffs.view());
                        let pd = key_distance_from_coefficients::<T>(unwrapped_pure.coeffs.view());
                        if td != pd {
                            return Err(serde::de::Error::custom(format!(
                            "the 'tempered' coefficients describe an interval spanning {} keys, and the 'pure' coefficients an interval spanning {} keys",
                            td, pd
                        )));
                        }

                        Ok(TemperamentEquation {
                            pure: unwrapped_pure,
                            tempered: unwrapped_tempered,
                        })
                    }
                }

                deserializer.deserialize_struct(
                    "temperament equation",
                    &["tempered", "pure"],
                    TemperamentEquationVisitor {
                        _phantom: PhantomData,
                    },
                )
            }
        }

        struct TemperamentDefinitionVisitor<T: IntervalBasis> {
            _phantom: PhantomData<T>,
        }

        #[derive(Deserialize)]
        #[serde(field_identifier, rename_all = "lowercase")]
        enum TemperamentDefinitionField {
            Name,
            Equations,
        }

        impl<'de, T: IntervalBasis + serde::Deserialize<'de>> Visitor<'de>
            for TemperamentDefinitionVisitor<T>
        {
            type Value = TemperamentDefinition<T>;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                write!(formatter, "temperament definition")
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::MapAccess<'de>,
            {
                let mut name: Option<String> = None {};
                let mut equations: Option<Vec<TemperamentEquation<T>>> = None {};
                while let Some(key) = map.next_key()? {
                    match key {
                        TemperamentDefinitionField::Name => {
                            if name.is_some() {
                                return Err(serde::de::Error::duplicate_field("name"));
                            }
                            name = Some(map.next_value()?);
                        }
                        TemperamentDefinitionField::Equations => {
                            if equations.is_some() {
                                return Err(serde::de::Error::duplicate_field("equations"));
                            }
                            let collected = map.next_value::<Vec<TemperamentEquation<T>>>()?;
                            if collected.len() != T::num_intervals() as usize {
                                return Err(serde::de::Error::custom(format!(
                                    "there must be {} equations, but I found {}",
                                    T::num_intervals(),
                                    collected.len()
                                )));
                            }
                            equations = Some(collected);
                        }
                    }
                }
                if name.is_none() {
                    return Err(serde::de::Error::missing_field("name"));
                }
                if equations.is_none() {
                    return Err(serde::de::Error::missing_field("equations"));
                }

                // this cloning would be avoidable if we had a non-owning, modifiable version of
                // NamedCoefficients. But that's an over-optimisation. We won't deserialize
                // often...
                let unwrapped_equations = equations.unwrap();
                let mut tempered = Array2::zeros((T::num_intervals(), T::num_intervals()));
                let mut pure = Array2::zeros((T::num_intervals(), T::num_intervals()));
                for i in 0..T::num_intervals() {
                    tempered
                        .row_mut(i)
                        .assign(&unwrapped_equations[i].tempered.coeffs);
                    pure.row_mut(i).assign(&unwrapped_equations[i].pure.coeffs);
                }

                Ok(TemperamentDefinition::new(name.unwrap(), tempered, pure))
            }
        }

        deserializer.deserialize_struct(
            "temperament defintion",
            &["name", "equations"],
            TemperamentDefinitionVisitor {
                _phantom: PhantomData,
            },
        )
    }
}
