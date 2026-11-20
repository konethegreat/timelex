// Written by Kone Tshivhinda & Claude
//! Threshold derivation: the lowest score that keeps measured precision at or above a floor.

use crate::{CalibrationError, Observation};

/// The chosen operating point for one tenant.
#[derive(Debug, Clone, PartialEq)]
pub struct Threshold {
    /// Lowest score that still meets the floor; decisions at or above it may post.
    pub score: f64,
    /// Measured precision of every decision at or above `score`.
    pub precision: f64,
    /// Decisions at or above `score`.
    pub selected: usize,
    /// Decisions at or above `score` that were correct.
    pub correct: usize,
}

/// Derives the lowest score whose selected set keeps precision at or above `floor`, so
/// recall is as high as the floor allows.
///
/// # Errors
///
/// Returns [`CalibrationError::Empty`] for an empty sample,
/// [`CalibrationError::FloorOutOfRange`] for a floor outside (0, 1], and
/// [`CalibrationError::ScoreOutOfRange`] for a score that cannot be calibrated.
///
/// Returns `Ok(None)` when no score reaches the floor.
pub fn derive_threshold(
    observations: &[Observation],
    floor: f64,
) -> Result<Option<Threshold>, CalibrationError> {
    if !floor.is_finite() || floor <= 0.0 || floor > 1.0 {
        return Err(CalibrationError::FloorOutOfRange);
    }
    if observations.is_empty() {
        return Err(CalibrationError::Empty);
    }
    for observation in observations {
        Observation::new(observation.raw, observation.correct)?;
    }

    let mut sorted = observations.to_vec();
    sorted.sort_by(|left, right| {
        right
            .raw
            .partial_cmp(&left.raw)
            .expect("scores are checked finite")
    });

    let mut best = None;
    let mut selected = 0_usize;
    let mut correct = 0_usize;
    let mut index = 0_usize;
    while index < sorted.len() {
        let score = sorted[index].raw;
        while index < sorted.len() && sorted[index].raw == score {
            selected += 1;
            correct += usize::from(u8::from(sorted[index].correct));
            index += 1;
        }
        let precision = correct as f64 / selected as f64;
        if precision >= floor {
            best = Some(Threshold {
                score,
                precision,
                selected,
                correct,
            });
        }
    }
    Ok(best)
}

#[cfg(test)]
mod tests {
    use super::derive_threshold;
    use crate::{CalibrationError, Observation};

    fn observations(samples: &[(f64, bool)]) -> Vec<Observation> {
        samples
            .iter()
            .map(|&(raw, correct)| Observation::new(raw, correct).expect("sample is valid"))
            .collect()
    }

    #[test]
    fn the_lowest_score_that_holds_the_floor_wins() {
        let samples = observations(&[(0.95, true), (0.9, true), (0.8, true), (0.5, false)]);

        let strict = derive_threshold(&samples, 0.8)
            .expect("the floor is valid")
            .expect("a threshold exists");
        assert!((strict.score - 0.8).abs() < 1e-12);
        assert!((strict.precision - 1.0).abs() < 1e-12);
        assert_eq!(strict.selected, 3);
        assert_eq!(strict.correct, 3);

        // 3 of 4 is exactly 0.75, so the floor admits the last decision too.
        let loose = derive_threshold(&samples, 0.75)
            .expect("the floor is valid")
            .expect("a threshold exists");
        assert!((loose.score - 0.5).abs() < 1e-12);
        assert!((loose.precision - 0.75).abs() < 1e-12);
        assert_eq!(loose.selected, 4);
        assert_eq!(loose.correct, 3);
    }

    #[test]
    fn tied_scores_are_taken_together() {
        let samples = observations(&[(0.9, true), (0.9, false), (0.9, false), (0.2, true)]);

        let chosen = derive_threshold(&samples, 0.5)
            .expect("the floor is valid")
            .expect("the lowest score is checked as a whole group");
        assert!((chosen.score - 0.2).abs() < 1e-12);
        assert_eq!(chosen.selected, 4);
        assert_eq!(chosen.correct, 2);
    }

    #[test]
    fn an_unreachable_floor_returns_none() {
        let samples = observations(&[(0.9, false), (0.4, false)]);

        assert_eq!(
            derive_threshold(&samples, 0.99).expect("the floor is valid"),
            None
        );
    }

    #[test]
    fn the_floor_must_be_in_the_open_closed_unit_interval() {
        let samples = observations(&[(0.5, true)]);

        assert_eq!(
            derive_threshold(&samples, 0.0),
            Err(CalibrationError::FloorOutOfRange)
        );
        assert_eq!(
            derive_threshold(&samples, 1.1),
            Err(CalibrationError::FloorOutOfRange)
        );
        assert_eq!(
            derive_threshold(&samples, f64::NAN),
            Err(CalibrationError::FloorOutOfRange)
        );
    }

    #[test]
    fn an_empty_sample_is_refused() {
        assert_eq!(derive_threshold(&[], 0.9), Err(CalibrationError::Empty));
    }
}
