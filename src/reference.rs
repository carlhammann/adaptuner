use crate::interval::{base::Semitones, stack::Stack, stacktype::r#trait::StackType};

#[derive(Clone, Debug)]
pub struct Reference<T: StackType> {
    pub stack: Stack<T>,
    pub semitones: Semitones,

    /// convenience: the [Stack::key_number()] of the [Self::stack]
    pub key: u8,
}

pub fn semitones_from_frequency(frequency: f64) -> Semitones {
    69.0 + 12.0 * (frequency as Semitones / 440.0).log2()
}

pub fn frequency_from_semitones(semitones: Semitones) -> f64 {
    440.0 * ((semitones - 69.0) / 12.0).exp2()
}

impl<T: StackType> Reference<T> {
    pub fn from_semitones(stack: Stack<T>, semitones: Semitones) -> Self {
        let key = stack.key_number() as u8;
        Self {
            stack,
            semitones,
            key,
        }
    }

    pub fn from_frequency(stack: Stack<T>, frequency: f64) -> Self {
        let semitones = semitones_from_frequency(frequency);

        let key = stack.key_number() as u8;
        Self {
            stack,
            semitones,
            key,
        }
    }

    pub fn get_frequency(&self) -> f64 {
        frequency_from_semitones(self.semitones)
    }

    /// The fractional MIDI note number that middle C is tuned to with this reference.
    pub fn c4_semitones(&self) -> Semitones {
        60.0 + self.semitones - self.key as Semitones
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use approx::assert_relative_eq;

    #[test]
    fn test_semitones_from_frequency() {
        let one_test = |freq, expected| {
            assert_relative_eq!(
                semitones_from_frequency(freq),
                expected,
                max_relative = 1e-10
            )
        };

        let examples = [
            (440.0, 69.0),
            (880.0, 81.0),
            (330.0, 64.01955000865388),
            (550.0, 72.86313713864834),
        ];

        for (freq, expected) in examples {
            one_test(freq, expected)
        }
    }

    #[test]
    fn test_frequency_from_semitones() {
        let one_test = |semitones, expected| {
            assert_relative_eq!(
                frequency_from_semitones(semitones),
                expected,
                max_relative = 1e-10
            )
        };

        let examples = [
            (69.0, 440.0),
            (68.0, 415.3046975799451),
            (81.0, 880.0),
            (64.01955000865388, 330.0),
            (72.86313713864834, 550.0),
        ];

        for (freq, expected) in examples {
            one_test(freq, expected)
        }
    }
}
