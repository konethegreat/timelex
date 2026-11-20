// Written by Kone Tshivhinda & Claude
//! Isotonic calibration: a non-decreasing map from raw score to observed frequency.

use std::error::Error;
use std::fmt;

/// One measured decision: the raw score a stage produced and whether it was correct.
#[derive(Debug, Clone, PartialEq)]
pub struct Observation {
    /// Raw score in [0, 1]; higher means the stage was surer.
    pub raw: f64,
    /// Whether the decision matched the label.
    pub correct: bool,
}

impl Observation {
    /// Builds an observation, rejecting scores that cannot be calibrated.
    ///
    /// # Errors
    ///
    /// Returns [`CalibrationError::ScoreOutOfRange`] when `raw` is NaN, infinite
    /// or outside [0, 1].
    pub fn new(raw: f64, correct: bool) -> Result<Self, CalibrationError> {
        check_score(raw)?;
        Ok(Self { raw, correct })
    }
}

/// A refused calibration input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CalibrationError {
    /// No observations were supplied.
    Empty,
    /// A raw score was NaN, infinite or outside [0, 1].
    ScoreOutOfRange,
    /// A precision floor was outside (0, 1].
    FloorOutOfRange,
}

impl fmt::Display for CalibrationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => formatter.write_str("calibration needs at least one observation"),
            Self::ScoreOutOfRange => formatter.write_str("a raw score must be finite and in [0, 1]"),
            Self::FloorOutOfRange => formatter.write_str("a precision floor must be in (0, 1]"),
        }
    }
}

impl Error for CalibrationError {}

/// One step of the fitted map: every raw score in this group maps to `probability`.
#[derive(Debug, Clone, PartialEq)]
pub struct Point {
    /// Raw score at this step.
    pub raw: f64,
    /// Observed correctness frequency for the fitted block this step belongs to.
    pub probability: f64,
    /// Observations in the fitted block this step belongs to.
    pub weight: usize,
}

/// A fitted non-decreasing map from raw score to observed frequency.
#[derive(Debug, Clone, PartialEq)]
pub struct Calibrator {
    points: Vec<Point>,
}

struct Group {
    raw: f64,
    successes: usize,
    weight: usize,
}

struct Block {
    successes: usize,
    weight: usize,
    first: usize,
    last: usize,
}

impl Calibrator {
    /// Fits with pool-adjacent-violators so the map never decreases as the score rises.
    ///
    /// # Errors
    ///
    /// Returns [`CalibrationError::Empty`] for an empty sample and
    /// [`CalibrationError::ScoreOutOfRange`] for an uncalibratable score.
    pub fn fit(observations: &[Observation]) -> Result<Self, CalibrationError> {
        if observations.is_empty() {
            return Err(CalibrationError::Empty);
        }
        for observation in observations {
            check_score(observation.raw)?;
        }

        let mut sorted = observations.to_vec();
        sorted.sort_by(|left, right| {
            left.raw
                .partial_cmp(&right.raw)
                .expect("scores are checked finite")
        });

        let mut groups: Vec<Group> = Vec::new();
        for observation in &sorted {
            let successes = usize::from(u8::from(observation.correct));
            match groups.last_mut() {
                Some(last) if last.raw == observation.raw => {
                    last.successes += successes;
                    last.weight += 1;
                }
                _ => groups.push(Group {
                    raw: observation.raw,
                    successes,
                    weight: 1,
                }),
            }
        }

        let mut blocks: Vec<Block> = Vec::with_capacity(groups.len());
        for (index, group) in groups.iter().enumerate() {
            blocks.push(Block {
                successes: group.successes,
                weight: group.weight,
                first: index,
                last: index,
            });
            while blocks.len() >= 2 {
                let right = &blocks[blocks.len() - 1];
                let left = &blocks[blocks.len() - 2];
                if left.successes * right.weight <= right.successes * left.weight {
                    break;
                }
                let merged = Block {
                    successes: left.successes + right.successes,
                    weight: left.weight + right.weight,
                    first: left.first,
                    last: right.last,
                };
                blocks.truncate(blocks.len() - 2);
                blocks.push(merged);
            }
        }

        let mut points = Vec::with_capacity(groups.len());
        for block in &blocks {
            let probability = block.successes as f64 / block.weight as f64;
            for group in &groups[block.first..=block.last] {
                points.push(Point {
                    raw: group.raw,
                    probability,
                    weight: block.weight,
                });
            }
        }
        Ok(Self { points })
    }

    /// The calibrated probability for a raw score: the fitted step at or below it.
    ///
    /// # Errors
    ///
    /// Returns [`CalibrationError::ScoreOutOfRange`] when `raw` cannot be calibrated.
    pub fn calibrate(&self, raw: f64) -> Result<f64, CalibrationError> {
        check_score(raw)?;
        let mut chosen = self.points[0].probability;
        for point in &self.points {
            if point.raw > raw {
                break;
            }
            chosen = point.probability;
        }
        Ok(chosen)
    }

    /// The fitted steps, ascending by raw score and non-decreasing in probability.
    pub fn points(&self) -> &[Point] {
        &self.points
    }
}

fn check_score(raw: f64) -> Result<(), CalibrationError> {
    if raw.is_finite() && (0.0..=1.0).contains(&raw) {
        Ok(())
    } else {
        Err(CalibrationError::ScoreOutOfRange)
    }
}

#[cfg(test)]
mod tests {
    use super::{CalibrationError, Calibrator, Observation};

    fn observations(samples: &[(f64, bool)]) -> Vec<Observation> {
        samples
            .iter()
            .map(|&(raw, correct)| Observation::new(raw, correct).expect("sample is valid"))
            .collect()
    }

    #[test]
    fn the_fitted_map_never_decreases_and_stays_in_the_unit_interval() {
        let samples = observations(&[
            (0.2, true),
            (0.2, false),
            (0.4, false),
            (0.6, true),
            (0.6, true),
            (0.9, false),
        ]);
        let calibrator = Calibrator::fit(&samples).expect("the sample fits");

        let points = calibrator.points();
        assert!(!points.is_empty());
        for point in points {
            assert!((0.0..=1.0).contains(&point.probability));
        }
        for pair in points.windows(2) {
            assert!(
                pair[0].probability <= pair[1].probability,
                "probability fell from {} to {} between {} and {}",
                pair[0].probability,
                pair[1].probability,
                pair[0].raw,
                pair[1].raw
            );
        }
    }

    #[test]
    fn the_map_pools_adjacent_violations_and_holds_them_between_steps() {
        let samples = observations(&[(0.2, true), (0.2, false), (0.4, false), (0.6, true), (0.6, true)]);
        let calibrator = Calibrator::fit(&samples).expect("the sample fits");

        // 0.2 (half correct) pooled with 0.4 (all wrong) averages to one in three.
        let expected = 1.0 / 3.0;
        assert!((calibrator.calibrate(0.1).unwrap() - expected).abs() < 1e-12);
        assert!((calibrator.calibrate(0.2).unwrap() - expected).abs() < 1e-12);
        assert!((calibrator.calibrate(0.4).unwrap() - expected).abs() < 1e-12);
        assert!((calibrator.calibrate(0.5).unwrap() - expected).abs() < 1e-12);
        assert!((calibrator.calibrate(0.6).unwrap() - 1.0).abs() < 1e-12);
        assert!((calibrator.calibrate(1.0).unwrap() - 1.0).abs() < 1e-12);
    }

    #[test]
    fn a_sample_that_is_already_monotone_is_left_alone() {
        let samples = observations(&[(0.1, false), (0.5, false), (0.5, true), (0.8, true)]);
        let calibrator = Calibrator::fit(&samples).expect("the sample fits");

        assert_eq!(calibrator.points().len(), 3);
        assert!((calibrator.calibrate(0.1).unwrap() - 0.0).abs() < 1e-12);
        assert!((calibrator.calibrate(0.5).unwrap() - 0.5).abs() < 1e-12);
        assert!((calibrator.calibrate(0.8).unwrap() - 1.0).abs() < 1e-12);
    }

    #[test]
    fn an_empty_sample_is_refused() {
        assert_eq!(Calibrator::fit(&[]), Err(CalibrationError::Empty));
    }

    #[test]
    fn scores_outside_the_unit_interval_are_refused() {
        assert_eq!(
            Observation::new(1.5, true),
            Err(CalibrationError::ScoreOutOfRange)
        );
        assert_eq!(
            Observation::new(-0.1, true),
            Err(CalibrationError::ScoreOutOfRange)
        );
        assert_eq!(
            Observation::new(f64::NAN, true),
            Err(CalibrationError::ScoreOutOfRange)
        );

        let samples = observations(&[(0.5, true)]);
        let calibrator = Calibrator::fit(&samples).expect("the sample fits");
        assert_eq!(
            calibrator.calibrate(f64::INFINITY),
            Err(CalibrationError::ScoreOutOfRange)
        );
    }
}
