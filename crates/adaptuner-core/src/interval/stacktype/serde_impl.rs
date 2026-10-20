use alloc::{format, string::String};
use core::{fmt, marker::PhantomData};

use ndarray::Array1;
use num_rational::Ratio;
use serde::{de::Visitor, ser::SerializeStruct, Deserialize, Deserializer, Serialize, Serializer};

use crate::interval::{
    stack::Stack,
    stacktype::{
        r#trait::{IntervalBasis, NamedInterval, StackCoeff},
        serde_support::{NamedCoefficients, NamedCoefficientsView},
    },
};

impl<T: IntervalBasis> Serialize for NamedInterval<T> {
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

impl<'de, T: IntervalBasis> Deserialize<'de> for NamedInterval<T> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
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
                                map.next_value::<NamedCoefficients<T, Ratio<StackCoeff>>>()?;
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

impl<T: IntervalBasis> Serialize for Stack<T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        if self.is_target() {
            return NamedCoefficientsView::<T, _>::new(self.target.view()).serialize(serializer);
        }

        let mut state = serializer.serialize_struct("stack", 2)?;

        state.serialize_field(
            "target",
            &NamedCoefficientsView::<T, _>::new(self.target.view()),
        )?;

        state.serialize_field(
            "actual",
            &NamedCoefficientsView::<T, _>::new(self.actual.view()),
        )?;

        state.end()
    }
}

impl<'de, T: IntervalBasis> Deserialize<'de> for Stack<T> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct StackVisitor<T: IntervalBasis> {
            _phantom: PhantomData<T>,
        }

        impl<'de, T: IntervalBasis> Visitor<'de> for StackVisitor<T> {
            type Value = Stack<T>;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                write!(
                    formatter,
                    "a stack represented by interval coefficients, \
                     or a map containing target and actual coefficients"
                )
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::MapAccess<'de>,
            {
                let mut target = None;
                let mut actual = None;

                let mut raw_target = Array1::zeros(T::num_intervals());
                let mut raw_seen = Array1::from_elem(T::num_intervals(), false);
                let mut has_raw_coefficients = false;

                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "target" => {
                            if target.is_some() {
                                return Err(serde::de::Error::duplicate_field("target"));
                            }

                            if has_raw_coefficients {
                                return Err(serde::de::Error::custom(
                                    "cannot mix raw interval coefficients with target/actual fields",
                                ));
                            }

                            target =
                                Some(map.next_value::<NamedCoefficients<T, StackCoeff>>()?.coeffs);
                        }

                        "actual" => {
                            if actual.is_some() {
                                return Err(serde::de::Error::duplicate_field("actual"));
                            }

                            if has_raw_coefficients {
                                return Err(serde::de::Error::custom(
                                    "cannot mix raw interval coefficients with target/actual fields",
                                ));
                            }

                            actual = Some(
                                map.next_value::<NamedCoefficients<T, Ratio<StackCoeff>>>()?
                                    .coeffs,
                            );
                        }

                        _ => {
                            if target.is_some() || actual.is_some() {
                                return Err(serde::de::Error::custom(
                                    "cannot mix raw interval coefficients with target/actual fields",
                                ));
                            }

                            let index = T::interval_positions()
                                .iter()
                                .find(|(name, _)| *name == key)
                                .map(|(_, index)| *index)
                                .ok_or_else(|| {
                                    serde::de::Error::custom(format!(
                                        "'{key}' is not an interval name"
                                    ))
                                })?;

                            if raw_seen[index] {
                                return Err(serde::de::Error::custom(format!(
                                    "duplicate definition for '{key}'"
                                )));
                            }

                            let coefficient = map.next_value::<StackCoeff>()?;

                            raw_target[index] = coefficient;
                            raw_seen[index] = true;
                            has_raw_coefficients = true;
                        }
                    }
                }

                match (target, actual, has_raw_coefficients) {
                    (Some(target), Some(actual), false) => {
                        Ok(Stack::from_target_and_actual(target, actual))
                    }

                    (Some(target), None, false) => Ok(Stack::from_target(target)),

                    (None, None, true) => Ok(Stack::from_target(raw_target)),

                    (None, None, false) => Ok(Stack::new_zero()),

                    (None, Some(_), false) => Err(serde::de::Error::custom(
                        "an actual stack requires target coefficients",
                    )),

                    _ => Err(serde::de::Error::custom(
                        "invalid combination of stack fields",
                    )),
                }
            }
        }

        deserializer.deserialize_map(StackVisitor {
            _phantom: PhantomData,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interval::base::Interval;
    use crate::interval::stacktype::r#trait::IntervalBasis;
    use alloc::{vec, vec::Vec};
    use ndarray::arr1;
    use std::sync::LazyLock;

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    struct TestBasis;

    static TEST_INTERVALS: LazyLock<Vec<Interval>> = LazyLock::new(|| {
        vec![
            Interval {
                name: "octave".into(),
                semitones: 12.0,
                key_distance: 12,
            },
            Interval {
                name: "fifth".into(),
                semitones: 7.0,
                key_distance: 7,
            },
        ]
    });

    impl IntervalBasis for TestBasis {
        fn intervals() -> &'static [Interval] {
            TEST_INTERVALS.as_slice()
        }

        fn try_period_index() -> Option<usize> {
            Some(0)
        }

        fn interval_positions() -> &'static [(&'static str, usize)] {
            &[("octave", 0), ("fifth", 1)]
        }
    }

    #[test]
    fn test_target_stack_roundtrip() {
        let stack = Stack::<TestBasis>::from_target(arr1(&[1, -1]));

        let serialized = serde_json::to_string(&stack).unwrap();
        let deserialized: Stack<TestBasis> = serde_json::from_str(&serialized).unwrap();

        assert_eq!(deserialized.target, stack.target);
        assert_eq!(deserialized.actual, stack.actual);
    }

    #[test]
    fn test_stack_with_actual_values_roundtrip() {
        let stack = Stack::<TestBasis>::from_target_and_actual(
            arr1(&[1, 0]),
            arr1(&[Ratio::new(3, 2), Ratio::from_integer(0)]),
        );

        assert!(!stack.is_target());

        let serialized = serde_json::to_string(&stack).unwrap();
        let deserialized: Stack<TestBasis> = serde_json::from_str(&serialized).unwrap();

        assert_eq!(deserialized.target, stack.target);
        assert_eq!(deserialized.actual, stack.actual);
    }

    #[test]
    fn test_empty_map_deserializes_to_zero_stack() {
        let stack: Stack<TestBasis> = serde_json::from_str("{}").unwrap();

        assert_eq!(stack, Stack::<TestBasis>::new_zero());
    }

    #[test]
    fn test_unknown_interval_name_is_rejected() {
        let result = serde_json::from_str::<Stack<TestBasis>>(r#"{"unknown":1}"#);

        assert!(result.is_err());
    }

    #[test]
    fn test_actual_without_target_is_rejected() {
        let result = serde_json::from_str::<Stack<TestBasis>>(r#"{"actual":{"octave":"3/2"}}"#);

        assert!(result.is_err());
    }
}
