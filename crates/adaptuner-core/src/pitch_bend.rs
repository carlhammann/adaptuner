use crate::interval::base::Semitones;

/// Converts a semitone offset to a 14-bit MIDI Pitch Bend value.
///
/// `bend_range` is the maximum positive or negative offset in semitones.
/// The neutral value is 8192; the valid MIDI range is 0..=16383.
pub fn bend_from_semitones(bend_range: Semitones, semitones: Semitones) -> u16 {
    if !bend_range.is_finite() || bend_range <= 0.0 {
        return 8192;
    }

    if !semitones.is_finite() {
        return 8192;
    }

    let value = 8192.0 + 8191.0 * semitones / bend_range;

    // Clamp before casting: casting to u16 first can wrap for
    // out-of-range values.
    value.clamp(0.0, 16383.0) as u16
}

/// Converts a 14-bit MIDI Pitch Bend value to a semitone offset.
pub fn semitones_from_bend(bend_range: Semitones, bend: u16) -> Semitones {
    if !bend_range.is_finite() || bend_range <= 0.0 {
        return 0.0;
    }

    let bend = bend.min(16383);

    (bend as Semitones - 8192.0) / 8191.0 * bend_range
}

/// Result of deciding how a MIDI note should be retuned.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RetuningDecision {
    /// The desired 14-bit MIDI Pitch Bend value.
    pub desired_bend: u16,
    /// Whether the current bend differs from the desired bend.
    pub should_send: bool,
    /// Whether the requested tuning exceeds the configured bend range.
    pub out_of_range: bool,
    /// The resulting absolute pitch after applying the desired bend.
    pub actual_semitones: Semitones,
}

/// Calculates the Pitch Bend required to tune a MIDI note.
///
/// This function is independent of MIDI ports, clocks, locks and platform APIs.
pub fn decide_retuning(
    note: u8,
    tuning: Semitones,
    bend_range: Semitones,
    current_bend: u16,
) -> RetuningDecision {
    let offset = tuning - note as Semitones;
    let desired_bend = bend_from_semitones(bend_range, offset);

    RetuningDecision {
        desired_bend,
        should_send: current_bend != desired_bend,
        out_of_range: !bend_range.is_finite() || bend_range <= 0.0 || offset.abs() > bend_range,
        actual_semitones: note as Semitones + semitones_from_bend(bend_range, desired_bend),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn neutral_bend_is_centered() {
        assert_eq!(bend_from_semitones(2.0, 0.0), 8192);
        assert_eq!(semitones_from_bend(2.0, 8192), 0.0);
    }

    #[test]
    fn bend_range_endpoints_are_representable() {
        assert_eq!(bend_from_semitones(2.0, -2.0), 1);
        assert_eq!(bend_from_semitones(2.0, 2.0), 16383);
    }

    #[test]
    fn bend_values_are_clamped() {
        assert_eq!(bend_from_semitones(2.0, -10.0), 0);
        assert_eq!(bend_from_semitones(2.0, 10.0), 16383);
    }

    #[test]
    fn invalid_inputs_return_neutral_bend() {
        assert_eq!(bend_from_semitones(0.0, 1.0), 8192);
        assert_eq!(bend_from_semitones(2.0, f64::NAN), 8192);
    }

    #[test]
    fn retuning_decision_requests_a_bend_when_needed() {
        let decision = decide_retuning(60, 60.5, 2.0, 8192);

        assert!(decision.should_send);
        assert_eq!(decision.desired_bend, 10239);
        assert!(!decision.out_of_range);
    }

    #[test]
    fn retuning_decision_skips_an_unchanged_bend() {
        let decision = decide_retuning(60, 60.0, 2.0, 8192);

        assert!(!decision.should_send);
        assert_eq!(decision.desired_bend, 8192);
        assert!(!decision.out_of_range);
    }

    #[test]
    fn retuning_decision_reports_out_of_range_tuning() {
        let decision = decide_retuning(60, 63.0, 2.0, 8192);

        assert!(decision.out_of_range);
        assert_eq!(decision.desired_bend, 16383);
        assert_eq!(decision.actual_semitones, 62.0);
    }
}
