use std::{fmt, marker::PhantomData};

use serde::de::Visitor;
use serde::ser::{SerializeSeq, SerializeStruct};
use serde_derive::{Deserialize, Serialize};

use crate::{
    interval::{stack::Stack, stacktype::r#trait::IntervalBasis},
    neighbourhood::{Neighbourhood, PeriodicComplete},
};

impl<T: IntervalBasis + serde::Serialize> serde::Serialize for PeriodicComplete<T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        #[derive(Serialize)]
        struct EntryView<'a, T: IntervalBasis> {
            offset: i8,
            stack: &'a Stack<T>,
        }

        struct EntriesView<'a, T: IntervalBasis, N: Neighbourhood<T>> {
            _phantom: PhantomData<T>,
            view: &'a N,
        }

        impl<'a, T: IntervalBasis + serde::Serialize, N: Neighbourhood<T>> serde::Serialize
            for EntriesView<'a, T, N>
        {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: serde::Serializer,
            {
                let mut seq = serializer.serialize_seq(None {})?;
                self.view.for_each_stack_failing(|offset, stack| {
                    seq.serialize_element(&EntryView { offset, stack })
                })?;
                seq.end()
            }
        }

        let mut t = serializer.serialize_struct("periodic complete aligned neighbourhood", 2)?;
        t.serialize_field("name", self.name())?;
        t.serialize_field(
            "entries",
            &EntriesView {
                _phantom: PhantomData,
                view: self,
            },
        )?;
        t.end()
    }
}

impl<'de, T> serde::Deserialize<'de> for PeriodicComplete<T>
where
    T: IntervalBasis + serde::Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct TheVisitor<T: IntervalBasis> {
            _phantom: PhantomData<T>,
        }

        #[derive(Deserialize)]
        #[serde(field_identifier, rename_all = "lowercase")]
        enum NeighbourhoodField {
            Name,
            Entries,
        }

        #[derive(Deserialize)]
        struct Entry<T: IntervalBasis> {
            offset: i8,
            stack: Stack<T>,
        }

        impl<'de, T> Visitor<'de> for TheVisitor<T>
        where
            T: IntervalBasis + serde::Deserialize<'de>,
        {
            type Value = PeriodicComplete<T>;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                write!(formatter, "periodic complete aligned neighbourhood")
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::MapAccess<'de>,
            {
                let m_period_keys = T::try_period_keys();
                if m_period_keys.is_none() {
                    todo!()
                }
                let period_keys = m_period_keys.unwrap();

                let mut name = None {};
                let mut stacks = None {};
                while let Some(key) = map.next_key()? {
                    match key {
                        NeighbourhoodField::Name => {
                            if name.is_some() {
                                return Err(serde::de::Error::duplicate_field("name"));
                            }
                            name = Some(map.next_value()?);
                        }
                        NeighbourhoodField::Entries => {
                            if stacks.is_some() {
                                return Err(serde::de::Error::duplicate_field("entries"));
                            }
                            let mut entries = map.next_value::<Vec<Entry<T>>>()?;
                            if entries.len() != period_keys as usize {
                                return Err(serde::de::Error::custom(format!(
                                    "there must be {} entries, but I found {}",
                                    period_keys,
                                    entries.len()
                                )));
                            }
                            entries.sort_by(|a, b| a.offset.cmp(&b.offset));
                            if entries
                                .iter()
                                .enumerate()
                                .any(|(i, a)| a.offset as usize != i)
                            {
                                return Err(serde::de::Error::custom(format!(
                                    "every offset in the (inclusive) range 0 to {} must occur exactly once",
                                    period_keys - 1,
                                )));
                            }
                            match entries
                                .iter()
                                .enumerate()
                                .find(|(i, a)| a.stack.key_distance() as usize != *i)
                            {
                                None {} => {}
                                Some((_, a)) => {
                                    return Err(serde::de::Error::custom(format!(
                                        "the stack for offset {} describes an interval that spans {} keys",
                                        a.offset,
                                        a.stack.key_distance(),
                                    )));
                                }
                            }
                            stacks = Some(entries.drain(..).map(|a| a.stack).collect());
                        }
                    }
                }
                if name.is_none() {
                    return Err(serde::de::Error::missing_field("name"));
                }
                if stacks.is_none() {
                    return Err(serde::de::Error::missing_field("entries"));
                }
                Ok(PeriodicComplete::new(
                    stacks.unwrap(),
                    Stack::from_pure_interval(T::try_period_index().unwrap(), 1),
                    name.unwrap(),
                ))
            }
        }

        deserializer.deserialize_struct(
            "neighbourhood",
            &["target", "actual"],
            TheVisitor {
                _phantom: PhantomData,
            },
        )
    }
}

#[cfg(test)]
mod test {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::interval::stacktype::fivelimit::mock::MockFiveLimitStackType;

    #[test]
    fn test_serialize_neighbourhood() {
        let neigh = PeriodicComplete::from_octave_tunings(
            "foo".into(),
            [
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
            ],
        );

        assert_eq!(
            serde_yml::to_string(&neigh).unwrap(),
            r#"name: foo
entries:
- offset: 0
  stack: {}
- offset: 1
  stack:
    fifth: -1
    third: 2
- offset: 2
  stack:
    octave: -1
    fifth: 2
- offset: 3
  stack:
    fifth: 1
    third: -1
- offset: 4
  stack:
    third: 1
- offset: 5
  stack:
    octave: 1
    fifth: -1
- offset: 6
  stack:
    octave: -1
    fifth: 2
    third: 1
- offset: 7
  stack:
    fifth: 1
- offset: 8
  stack:
    third: 2
- offset: 9
  stack:
    octave: 1
    fifth: -1
    third: 1
- offset: 10
  stack:
    fifth: 2
    third: -1
- offset: 11
  stack:
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
                serde_yml::from_str::<PeriodicComplete<MockFiveLimitStackType>>(input)
            );
            if !res.contains(contained) {
                panic!("the result\n\n{res}\n\ndoesn't contain the expected error message");
            }
        };

        test_error_contains(
            r#"name: foo
entries:
- offset: 0
  stack: {}
- offset: 1
  stack:
    fifth: -1
    third: 2
"#,
            "there must be 12 entries, but I found 2",
        );

        test_error_contains(
            r#"name: foo
entries:
- offset: 0
  stack: {}
- offset: 0
  stack:
    fifth: -1
    third: 2
- offset: 2
  stack:
    octave: -1
    fifth: 2
- offset: 3
  stack:
    fifth: 1
    third: -1
- offset: 4
  stack:
    third: 1
- offset: 5
  stack:
    octave: 1
    fifth: -1
- offset: 6
  stack:
    octave: -1
    fifth: 2
    third: 1
- offset: 7
  stack:
    fifth: 1
- offset: 8
  stack:
    third: 2
- offset: 9
  stack:
    octave: 1
    fifth: -1
    third: 1
- offset: 10
  stack:
    fifth: 2
    third: -1
- offset: 11
  stack:
    fifth: 1
    third: 1
"#,
            "every offset in the (inclusive) range 0 to 11 must occur exactly once",
        );

        test_error_contains(
            r#"name: foo
entries:
- offset: 0
  stack: {}
- offset: 1
  stack:
    fifth: -1
    third: 2
- offset: 2
  stack:
    octave: -1
    fifth: 2
- offset: 3
  stack:
    fifth: 1
- offset: 4
  stack:
    third: 1
- offset: 5
  stack:
    octave: 1
    fifth: -1
- offset: 6
  stack:
    octave: -1
    fifth: 2
    third: 1
- offset: 7
  stack:
    fifth: 1
- offset: 8
  stack:
    third: 2
- offset: 9
  stack:
    octave: 1
    fifth: -1
    third: 1
- offset: 10
  stack:
    fifth: 2
    third: -1
- offset: 11
  stack:
    fifth: 1
    third: 1
"#,
            "the stack for offset 3 describes an interval that spans 7 keys",
        );
    }
}
