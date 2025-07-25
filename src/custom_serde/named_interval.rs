use std::{fmt, marker::PhantomData};

use num_rational::Ratio;
use serde::{de::Visitor, ser::SerializeStruct, Serializer};
use serde_derive::Deserialize;

use crate::interval::stacktype::r#trait::{IntervalBasis, NamedInterval, StackCoeff};

use super::common::{NamedCoefficients, NamedCoefficientsView};

impl<T: IntervalBasis> serde::Serialize for NamedInterval<T> {
    fn serialize<S: Serializer>(&self, ser: S) -> Result<S::Ok, S::Error> {
        let mut t = ser.serialize_struct("named interval", 3)?;
        t.serialize_field("name", &self.name)?;
        t.serialize_field("short-name", &self.short_name)?;
        t.serialize_field(
            "coeffs",
            &NamedCoefficientsView::<T, _>::new(self.coeffs.view()),
        )?;
        t.end()
    }
}

impl<'de, T: IntervalBasis> serde::Deserialize<'de> for NamedInterval<T> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct NamedIntervalVisitor<T: IntervalBasis> {
            _phantom: PhantomData<T>,
        }

        impl<T: IntervalBasis> NamedIntervalVisitor<T> {
            fn new() -> Self {
                Self {
                    _phantom: PhantomData,
                }
            }
        }

        #[derive(Deserialize)]
        #[serde(field_identifier, rename_all = "kebab-case")]
        enum NamedIntervalField {
            Name,
            ShortName,
            Coeffs,
        }

        impl<'de, T> Visitor<'de> for NamedIntervalVisitor<T>
        where
            T: IntervalBasis,
        {
            type Value = NamedInterval<T>;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                write!(formatter, "named interval")
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::MapAccess<'de>,
            {
                let mut name = None {};
                let mut short_name = None {};
                let mut coeffs = None {};

                while let Some(key) = map.next_key()? {
                    match key {
                        NamedIntervalField::Name => {
                            if name.is_some() {
                                return Err(serde::de::Error::duplicate_field("name"));
                            }
                            name = Some(map.next_value()?);
                        }
                        NamedIntervalField::ShortName => {
                            if short_name.is_some() {
                                return Err(serde::de::Error::duplicate_field("short-name"));
                            }
                            short_name = Some(map.next_value()?);
                        }
                        NamedIntervalField::Coeffs => {
                            if coeffs.is_some() {
                                return Err(serde::de::Error::duplicate_field("coeffs"));
                            }
                            let NamedCoefficients { coeffs: inner, .. } =
                                map.next_value::<NamedCoefficients<T, StackCoeff>>()?;
                            coeffs = Some(inner);
                        }
                    }
                }

                if name.is_none() {
                    return Err(serde::de::Error::missing_field("name"));
                }
                if short_name.is_none() {
                    return Err(serde::de::Error::missing_field("short-name"));
                }
                if coeffs.is_none() {
                    return Err(serde::de::Error::missing_field("coeffs"));
                }

                Ok(NamedInterval::new(
                    coeffs.unwrap(),
                    name.unwrap(),
                    short_name.unwrap(),
                ))
            }
        }

        deserializer.deserialize_struct(
            "named interval",
            &["name", "short-name", "coeffs"],
            NamedIntervalVisitor::new(),
        )
    }
}
