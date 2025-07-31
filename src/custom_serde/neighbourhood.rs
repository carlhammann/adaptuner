use std::collections::BTreeMap;

use serde::ser::SerializeMap;
use serde::Deserializer;
use serde_derive::Deserialize;
use serde_with::{serde_as, DeserializeAs, MapPreventDuplicates};

use crate::neighbourhood::{Partial, PeriodicPartial};
use crate::{
    interval::{stack::Stack, stacktype::r#trait::IntervalBasis},
    neighbourhood::{Neighbourhood, PeriodicComplete},
};

fn serialize_as_map<T, N, S>(neigh: &N, ser: S) -> Result<S::Ok, S::Error>
where
    T: IntervalBasis + serde::Serialize,
    N: Neighbourhood<T>,
    S: serde::Serializer,
{
    let mut t = ser.serialize_map(T::try_period_keys().map(|x| x as usize))?;
    neigh.for_each_stack_failing(|offset, stack| t.serialize_entry(&offset, stack))?;
    t.end()
}

impl<T: IntervalBasis + serde::Serialize> serde::Serialize for PeriodicComplete<T> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serialize_as_map(self, serializer)
    }
}

impl<T: IntervalBasis + serde::Serialize> serde::Serialize for PeriodicPartial<T> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serialize_as_map(self, serializer)
    }
}

impl<T: IntervalBasis + serde::Serialize> serde::Serialize for Partial<T> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serialize_as_map(self, serializer)
    }
}

/// needed for the DeserializeAs magic. Otherwise, serde_as will try to deserialize map keys as
/// Strings...
struct AnI8<'de>(&'de str);

impl<'de> serde::Deserialize<'de> for AnI8<'de> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let i = <_ as serde::Deserialize<'de>>::deserialize(deserializer)?;
        Ok(AnI8(i))
    }
}

impl<'de> DeserializeAs<'de, i8> for AnI8<'de> {
    fn deserialize_as<D>(deserializer: D) -> Result<i8, D::Error>
    where
        D: Deserializer<'de>,
    {
        let AnI8(str) = <AnI8 as serde::Deserialize<'de>>::deserialize(deserializer)?;
        match str.parse() {
            Ok(i) => Ok(i),
            Err(e) => Err(serde::de::Error::custom(e)),
        }
    }
}

#[serde_as]
#[derive(Deserialize)]
struct NoDuplicates<T: IntervalBasis> {
    #[serde(flatten)]
    #[serde_as(as = "MapPreventDuplicates<AnI8, _>")]
    stacks: BTreeMap<i8, Stack<T>>,
}

impl<'de, T: IntervalBasis + serde::Deserialize<'de>> serde::Deserialize<'de>
    for PeriodicComplete<T>
{
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        if let Some(n) = T::try_period_keys() {
            let NoDuplicates { stacks: mut map } = <_>::deserialize(deserializer)?;

            if map.len() != n as usize {
                return Err(serde::de::Error::custom(format!(
                    "expected {n} entries, but got {}",
                    map.len()
                )));
            }

            if let Some((&lo, _)) = map.first_key_value() {
                if lo != 0 {
                    return Err(serde::de::Error::custom(format!(
                        "the lowest entry must be 0, but it is {lo}",
                    )));
                }
            }
            if let Some((&hi, _)) = map.last_key_value() {
                if hi != n as i8 - 1 {
                    return Err(serde::de::Error::custom(format!(
                        "the highest entry must be {}, but it is {hi}",
                        n as i8 - 1,
                    )));
                }
            }

            if let Some((offset, stack)) = map
                .iter()
                .find(|(i, stack)| stack.key_distance() as i8 != **i)
            {
                return Err(serde::de::Error::custom(format!(
                    "the stack for entry {offset} describes an interval spanning {} keys",
                    stack.key_distance()
                )));
            }

            Ok(PeriodicComplete {
                stacks: {
                    let mut v = Vec::with_capacity(n as usize);
                    while let Some((_, stack)) = map.pop_first() {
                        v.push(stack);
                    }
                    v
                },
                // unwrap is ok, because `T::try_period_keys()` was `Some`
                period: Stack::from_pure_interval(T::try_period_index().unwrap(), 1),
                period_index: T::try_period_index(),
            })
        } else {
            panic!(
                "cannot deserialize PeriodicComplete neighbourhood for non-periodic IntervalBasis"
            );
        }
    }
}

impl<'de, T: IntervalBasis + serde::Deserialize<'de>> serde::Deserialize<'de>
    for PeriodicPartial<T>
{
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        if let Some(n) = T::try_period_keys() {
            let NoDuplicates { mut stacks } = <_>::deserialize(deserializer)?;

            if stacks.len() > n as usize {
                return Err(serde::de::Error::custom(format!(
                    "expected at most {n} entries, but got {}",
                    stacks.len()
                )));
            }

            if let Some((&lo, _)) = stacks.first_key_value() {
                if lo < 0 {
                    return Err(serde::de::Error::custom(format!(
                        "the lowest entry must be at least 0, but it is {lo}",
                    )));
                }
            }
            if let Some((&hi, _)) = stacks.last_key_value() {
                if hi >= n as i8 {
                    return Err(serde::de::Error::custom(format!(
                        "the highest entry must be at most {}, but it is {hi}",
                        n as i8 - 1,
                    )));
                }
            }

            if let Some((offset, stack)) = stacks
                .iter()
                .find(|(i, stack)| stack.key_distance() as i8 != **i)
            {
                return Err(serde::de::Error::custom(format!(
                    "the stack for entry {offset} describes an interval spanning {} keys",
                    stack.key_distance()
                )));
            }

            Ok(PeriodicPartial {
                stacks: {
                    let mut v = vec![(Stack::new_zero(), false); n as usize];
                    while let Some((i, stack)) = stacks.pop_first() {
                        v[i as usize] = (stack, true);
                    }
                    v
                },
                // unwrap is ok, because `T::try_period_keys()` was `Some`
                period: Stack::from_pure_interval(T::try_period_index().unwrap(), 1),
                period_index: T::try_period_index(),
            })
        } else {
            panic!(
                "cannot deserialize PeriodicPartial neighbourhood for non-periodic IntervalBasis"
            );
        }
    }
}

impl<'de, T: IntervalBasis + serde::Deserialize<'de>> serde::Deserialize<'de> for Partial<T> {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let NoDuplicates { stacks } = <_>::deserialize(deserializer)?;

        if let Some((offset, stack)) = stacks
            .iter()
            .find(|(i, stack)| stack.key_distance() as i8 != **i)
        {
            return Err(serde::de::Error::custom(format!(
                "the stack for entry {offset} describes an interval spanning {} keys",
                stack.key_distance()
            )));
        }

        Ok(Partial { stacks })
    }
}

#[cfg(test)]
mod test {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::{
        interval::stacktype::fivelimit::mock::MockFiveLimitStackType,
        neighbourhood::SomeNeighbourhood,
    };

    #[test]
    fn test_serialize_neighbourhood() {
        let neigh = SomeNeighbourhood::PeriodicComplete(PeriodicComplete::new_periodic(vec![
            Stack::<MockFiveLimitStackType>::new_zero(), // C
            Stack::from_target(vec![0, -1, 2]),          // C#
            Stack::from_target(vec![-1, 2, 0]),          // D
            Stack::from_target(vec![0, 1, -1]),          // Eb
            Stack::from_target(vec![0, 0, 1]),           // E
            Stack::from_target(vec![1, -1, 0]),          // F
            Stack::from_target(vec![-1, 2, 1]),          // F#
            Stack::from_target(vec![0, 1, 0]),           // G
            Stack::from_target(vec![0, 0, 2]),           // G#
            Stack::from_target(vec![1, -1, 1]),          // A
            Stack::from_target(vec![0, 2, -1]),          // Bb
            Stack::from_target(vec![0, 1, 1]),           // B
        ]));

        assert_eq!(
            serde_yml::to_string(&neigh).unwrap(),
            r#"!periodic-complete
0: {}
1:
  fifth: -1
  third: 2
2:
  octave: -1
  fifth: 2
3:
  fifth: 1
  third: -1
4:
  third: 1
5:
  octave: 1
  fifth: -1
6:
  octave: -1
  fifth: 2
  third: 1
7:
  fifth: 1
8:
  third: 2
9:
  octave: 1
  fifth: -1
  third: 1
10:
  fifth: 2
  third: -1
11:
  fifth: 1
  third: 1
"#
        );
    }

    #[test]
    fn test_deserialize_neighbourhood_errors() {
        let test_error_contains = |input, contained| {
            let res = format!(
                "{:?}",
                serde_yml::from_str::<SomeNeighbourhood<MockFiveLimitStackType>>(input)
            );
            if !res.contains(contained) {
                panic!("the result\n\n{res}\n\ndoesn't contain the expected error message");
            }
        };

        test_error_contains(
            r#"!periodic-complete
0: {}
1:
  fifth: -1
  third: 2
"#,
            "expected 12 entries, but got 2",
        );

        test_error_contains(
            r#"!periodic-partial
        0: {}
        0: {}
        "#,
            "duplicate",
        );

        test_error_contains(
            r#"!periodic-partial
        -7:
          fifth: -1
        "#,
            "the lowest entry must be at least 0, but it is -7",
        );

        test_error_contains(
            r#"!periodic-partial
        3:
          fifth: 1
        "#,
            "the stack for entry 3 describes an interval spanning 7 keys",
        );
    }
}
