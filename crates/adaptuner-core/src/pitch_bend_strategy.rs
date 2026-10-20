//! MIDI Pitch Bend implementation of the backend-neutral retuning strategy.

use crate::{interval::base::Semitones, notes::{ActiveNote, NoteId}, pitch_bend::bend_from_semitones, retuning::RetuningStrategy};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PitchBendCommand {
    pub note_id: NoteId,
    pub value: u16,
    pub out_of_range: bool,
    pub actual_pitch: Semitones,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PitchBendStrategy {
    bend_range: Semitones,
}

impl PitchBendStrategy {
    pub const fn new(bend_range: Semitones) -> Self { Self { bend_range } }
    pub const fn bend_range(&self) -> Semitones { self.bend_range }
}

impl RetuningStrategy for PitchBendStrategy {
    type Command = PitchBendCommand;

    fn retune(&mut self, note: &ActiveNote) -> Self::Command {
        let offset = note.target_pitch() - note.source_pitch();
        let value = bend_from_semitones(self.bend_range, offset);
        let actual_pitch = note.source_pitch()
            + crate::pitch_bend::semitones_from_bend(self.bend_range, value);
        PitchBendCommand {
            note_id: note.id(),
            value,
            out_of_range: !self.bend_range.is_finite() || self.bend_range <= 0.0 || offset.abs() > self.bend_range,
            actual_pitch,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strategy_emits_note_scoped_pitch_bend() {
        let note = ActiveNote::new(NoteId::new(12), 60.0, 60.5);
        let command = PitchBendStrategy::new(2.0).retune(&note);
        assert_eq!(command.note_id, NoteId::new(12));
        assert_eq!(command.value, 10239);
        assert!(!command.out_of_range);
        assert!((command.actual_pitch - 60.5).abs() < 0.001);
    }

    #[test]
    fn strategy_reports_clamped_pitch_when_target_exceeds_range() {
        let note = ActiveNote::new(NoteId::new(3), 60.0, 63.0);
        let command = PitchBendStrategy::new(2.0).retune(&note);
        assert_eq!(command.value, 16383);
        assert!(command.out_of_range);
        assert_eq!(command.actual_pitch, 62.0);
    }
}
