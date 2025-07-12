use std::{fmt, marker::PhantomData};

use ndarray::Array1;
use num_rational::Ratio;
use serde::{de::Visitor, ser::SerializeStruct, Serializer};
use serde_derive::Deserialize;

use crate::interval::{
    stack::Stack,
    stacktype::r#trait::{IntervalBasis, StackCoeff},
};

use super::common::{NamedCoefficients, NamedCoefficientsView};

impl<T: IntervalBasis> serde::Serialize for Stack<T> {
    fn serialize<S: Serializer>(&self, ser: S) -> Result<S::Ok, S::Error> {
        if self.is_target() {
            NamedCoefficientsView::<T, _>::new(self.target.view()).serialize(ser)
        } else {
            let mut t = ser.serialize_struct("stack", 2)?;
            t.serialize_field(
                "target",
                &NamedCoefficientsView::<T, _>::new(self.target.view()),
            )?;
            t.serialize_field(
                "actual",
                &NamedCoefficientsView::<T, _>::new(self.actual.view()),
            )?;
            t.end()
        }
    }
}

impl<'de, T: IntervalBasis> serde::Deserialize<'de> for Stack<T> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct StackVisitor<T: IntervalBasis> {
            _phantom: PhantomData<T>,
        }

        impl<T: IntervalBasis> StackVisitor<T> {
            fn new() -> Self {
                Self {
                    _phantom: PhantomData,
                }
            }
        }

        #[derive(Deserialize)]
        #[serde(field_identifier, rename_all = "lowercase")]
        enum StackField {
            Target,
            Actual,
            // #[serde(, flatten)]
            IntervalName(String),
        }

        impl<'de, T> Visitor<'de> for StackVisitor<T>
        where
            T: IntervalBasis,
        {
            type Value = Stack<T>;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                write!(formatter, "stack")
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::MapAccess<'de>,
            {
                let mut target = None {};
                let mut actual = None {};
                let mut raw_target: Array1<StackCoeff> = Array1::zeros(T::num_intervals());
                let mut raw_set = Array1::from_elem(T::num_intervals(), false);
                let mut is_raw = false;
                while let Some(key) = map.next_key()? {
                    match key {
                        StackField::Target => {
                            if target.is_some() {
                                return Err(serde::de::Error::duplicate_field("target"));
                            }
                            if is_raw {
                                return Err(serde::de::Error::custom(
                                    "don't specify 'target' on the same level as interval names",
                                ));
                            }
                            let NamedCoefficients { coeffs, .. } =
                                map.next_value::<NamedCoefficients<T, StackCoeff>>()?;
                            target = Some(coeffs);
                        }
                        StackField::Actual => {
                            if actual.is_some() {
                                return Err(serde::de::Error::duplicate_field("actual"));
                            }
                            if is_raw {
                                return Err(serde::de::Error::custom(
                                    "don't specify 'actual' on the same level as interval names",
                                ));
                            }
                            let NamedCoefficients { coeffs: inner, .. } =
                                map.next_value::<NamedCoefficients<T, Ratio<StackCoeff>>>()?;
                            actual = Some(inner);
                        }
                        StackField::IntervalName(name) => {
                            if actual.is_some() {
                                return Err(serde::de::Error::custom(format!("'actual' alreay specified, don't specify interval name '{name}' on the same level")));
                            }
                            if target.is_some() {
                                return Err(serde::de::Error::custom(format!("'target' alreay specified, don't specify interval name '{name}' on the same level")));
                            }

                            match T::interval_positions().get(&name) {
                                None {} => {
                                    return Err(serde::de::Error::custom(format!(
                                        "'{}' is not an interval name",
                                        name
                                    )))
                                }
                                Some(i) => {
                                    if raw_set[*i] {
                                        return Err(serde::de::Error::custom(format!(
                                            "duplicate definition for '{}'",
                                            name
                                        )));
                                    }
                                    raw_set[*i] = true;
                                    let c = map.next_value::<StackCoeff>()?;
                                    is_raw = true;
                                    raw_target[*i] = c;
                                }
                            }
                        }
                    }
                }

                // if is_raw is already true, then there's no change. Otherwise, the map was
                // empty, and then the zeros in raw_target are accurate.
                if target.is_none() & actual.is_none() {
                    is_raw = true;
                }

                if is_raw {
                    Ok(Stack::from_target(raw_target))
                } else {
                    if target.is_none() {
                        return Err(serde::de::Error::missing_field("target"));
                    }
                    Ok(match actual {
                        None {} => Stack::from_target(target.unwrap()),
                        Some(actual) => Stack::from_target_and_actual(target.unwrap(), actual),
                    })
                }
            }
        }

        deserializer.deserialize_struct("stack", &["target", "actual"], StackVisitor::new())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::interval::stacktype::fivelimit::mock::MockFiveLimitStackType;
    use ndarray::arr1;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_serialize_stack() {
        assert_eq!(
            serde_yml::to_string(&Stack::<MockFiveLimitStackType>::new_zero()).unwrap(),
            r#"{}
"#
        );

        assert_eq!(
            serde_yml::to_string(&Stack::<MockFiveLimitStackType>::from_target(vec![
                1, 2, -3
            ]))
            .unwrap(),
            r#"octave: 1
fifth: 2
third: -3
"#
        );

        assert_eq!(
            serde_yml::to_string(&Stack::<MockFiveLimitStackType>::from_target(vec![1, 0, 0]))
                .unwrap(),
            r#"octave: 1
"#
        );

        assert_eq!(
            serde_yml::to_string(&Stack::<MockFiveLimitStackType>::from_target_and_actual(
                arr1(&[1, 2, 3]),
                arr1(&[Ratio::new(1, 2), Ratio::new(-2, 3), Ratio::new(4, 5)])
            ))
            .unwrap(),
            r#"target:
  octave: 1
  fifth: 2
  third: 3
actual:
  octave: '1/2'
  fifth: '-2/3'
  third: '4/5'
"#
        );
    }

    #[test]
    fn test_deserialize_stack() {
        assert_eq!(
            serde_yml::from_str::<Stack<MockFiveLimitStackType>>("target: {}").unwrap(),
            Stack::new_zero(),
        );

        assert_eq!(
            serde_yml::from_str::<Stack<MockFiveLimitStackType>>("{}").unwrap(),
            Stack::new_zero(),
        );

        assert_eq!(
            serde_yml::from_str::<Stack<MockFiveLimitStackType>>("").unwrap(),
            Stack::new_zero(),
        );

        assert_eq!(
            serde_yml::from_str::<Stack<MockFiveLimitStackType>>(
                r#"octave: 1
fifth: 2
third: -3
"#
            )
            .unwrap(),
            Stack::from_target(vec![1, 2, -3]),
        );

        assert_eq!(
            serde_yml::from_str::<Stack<MockFiveLimitStackType>>(
                r#"octave: 1
"#
            )
            .unwrap(),
            Stack::from_target(vec![1, 0, 0]),
        );

        assert_eq!(
            serde_yml::from_str::<Stack<MockFiveLimitStackType>>(
                r#"target:
  octave: 1
actual:
  third: 333333333/4
"#
            )
            .unwrap(),
            Stack::from_target_and_actual(
                arr1(&[1, 0, 0]),
                arr1(&[0.into(), 0.into(), Ratio::new(333333333, 4)])
            ),
        );
    }

    #[test]
    fn test_deserialize_stack_errors() {
        let test_error_contains = |input, contained| {
            let res = format!(
                "{:?}",
                serde_yml::from_str::<Stack<MockFiveLimitStackType>>(input)
            );
            if !res.contains(contained) {
                panic!("the result\n\n{res}\n\ndoesn't contain the expected error message");
            }
        };

        test_error_contains(
            r#"target: 
  sadfio: 4
"#,
            "'sadfio' is not an interval name",
        );
        test_error_contains("sadfio: 4", "'sadfio' is not an interval name");

        test_error_contains(
            r#"target: {}
octave: 4
"#,
            "'target' alreay specified, don't specify interval name 'octave' on the same level",
        );

        test_error_contains(
            r#"actual: {}
octave: 4
"#,
            "'actual' alreay specified, don't specify interval name 'octave' on the same level",
        );

        test_error_contains(
            r#"octave: 4
target: irrelevant
"#,
            "don't specify 'target' on the same level as interval names",
        );

        test_error_contains(
            r#"octave: 4
actual: irrelevant
"#,
            "don't specify 'actual' on the same level as interval names",
        );

        test_error_contains(
            r#"target: 
  octave: 4ew1
"#,
            "invalid type",
        );

        test_error_contains(
            r#"octave: 4ew1
"#,
            "invalid type",
        );

        test_error_contains(
            r#"target: 
  octave: 1
  octave: 2
"#,
            "duplicate definition for 'octave'",
        );

        test_error_contains(
            r#"octave: 1
octave: 2
"#,
            "duplicate definition for 'octave'",
        );
    }

    #[test]
    fn stack_serde_yml_roundtrip() {
        let stacks = [
            Stack::<MockFiveLimitStackType>::new_zero(),
            Stack::from_temperaments_and_target(&[true, false], vec![-1, 2, 3]),
            Stack::from_temperaments_and_target(&[false, true], vec![4, -5, 6]),
            Stack::from_temperaments_and_target(&[true, true], vec![7, 8, -9]),
        ];

        for stack in stacks {
            assert_eq!(
                stack,
                serde_yml::from_str(&serde_yml::to_string(&stack).unwrap()).unwrap()
            );
        }
    }
}
