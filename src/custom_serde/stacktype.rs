#[cfg(test)]
mod test {
    use crate::interval::stacktype::fivelimit::mock::MockFiveLimitStackType;
    use adaptuner_core::interval::temperament::TemperamentDefinition;
    use ndarray::arr2;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_serialize_temperament_definition() {
        assert_eq!(
            serde_yml::to_string(&TemperamentDefinition::<MockFiveLimitStackType>::new(
                "cursed temperament".into(),
                arr2(&[[1, 0, 0], [0, 4, 0], [0, 0, 3]]),
                arr2(&[[1, 0, 0], [2, 0, 1], [1, 0, 0]]),
            ),)
            .unwrap(),
            r#"name: cursed temperament
equations:
- tempered:
    octave: 1
  pure:
    octave: 1
- tempered:
    fifth: 4
  pure:
    octave: 2
    third: 1
- tempered:
    third: 3
  pure:
    octave: 1
"#
        );
    }

    #[test]
    fn test_desserialize_temperament_definition() {
        assert_eq!(
            serde_yml::from_str::<TemperamentDefinition<_>>(
                r#"name: cursed temperament
equations:
- tempered:
    octave: 1
  pure:
    octave: 1
- tempered:
    fifth: 4
  pure:
    octave: 2
    third: 1
- tempered:
    third: 3
  pure:
    octave: 1
"#,
            )
            .unwrap(),
            TemperamentDefinition::<MockFiveLimitStackType>::new(
                "cursed temperament".into(),
                arr2(&[[1, 0, 0], [0, 4, 0], [0, 0, 3]]),
                arr2(&[[1, 0, 0], [2, 0, 1], [1, 0, 0]]),
            ),
        );
    }
}
