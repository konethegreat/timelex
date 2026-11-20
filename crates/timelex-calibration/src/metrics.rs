// Written by Kone Tshivhinda & Claude
//! Reliability metrics: expected calibration error and Brier score over fixed bins.

use crate::{CalibrationError, Observation};

/// Number of equal-width confidence bins used by the reliability diagram.
pub const BINS: usize = 10;

/// One occupied confidence bin.
#[derive(Debug, Clone, PartialEq)]
pub struct ReliabilityBin {
    /// Lower confidence edge, included.
    pub lower: f64,
    /// Upper confidence edge, excluded except for the top bin.
    pub upper: f64,
    /// Observations that fell in the bin.
    pub count: usize,
    /// Mean confidence of the observations in the bin.
    pub mean_confidence: f64,
    /// Share of the observations in the bin that were correct.
    pub accuracy: f64,
}

/// How far a set of scores sits from the outcomes it predicted.
#[derive(Debug, Clone, PartialEq)]
pub struct Reliability {
    /// Occupied bins, ascending by confidence.
    pub bins: Vec<ReliabilityBin>,
    /// Observations measured.
    pub count: usize,
    /// Expected calibration error, weighted by bin population.
    pub ece: f64,
    /// Mean squared error between score and outcome.
    pub brier: f64,
}

/// Measures the score set's calibration and sharpness.
///
/// # Errors
///
/// Returns [`CalibrationError::Empty`] for an empty sample and
/// [`CalibrationError::ScoreOutOfRange`] for a score that cannot be calibrated.
pub fn reliability(observations: &[Observation]) -> Result<Reliability, CalibrationError> {
    if observations.is_empty() {
        return Err(CalibrationError::Empty);
    }

    let mut counts = [0_usize; BINS];
    let mut confidence_sums = [0.0_f64; BINS];
    let mut correct_counts = [0_usize; BINS];
    let mut brier_sum = 0.0;
    for observation in observations {
        Observation::new(observation.raw, observation.correct)?;
        let bin = bin_index(observation.raw);
        counts[bin] += 1;
        confidence_sums[bin] += observation.raw;
        correct_counts[bin] += usize::from(u8::from(observation.correct));
        let outcome = f64::from(u8::from(observation.correct));
        brier_sum += (observation.raw - outcome) * (observation.raw - outcome);
    }

    let total = observations.len() as f64;
    let mut bins = Vec::new();
    let mut ece = 0.0;
    for (index, &count) in counts.iter().enumerate() {
        if count == 0 {
            continue;
        }
        let count = count as f64;
        let mean_confidence = confidence_sums[index] / count;
        let accuracy = correct_counts[index] as f64 / count;
        ece += (count / total) * (accuracy - mean_confidence).abs();
        bins.push(ReliabilityBin {
            lower: index as f64 / BINS as f64,
            upper: (index + 1) as f64 / BINS as f64,
            count: count as usize,
            mean_confidence,
            accuracy,
        });
    }

    Ok(Reliability {
        bins,
        count: observations.len(),
        ece,
        brier: brier_sum / total,
    })
}

/// Renders a text reliability diagram: one row per occupied bin, observed and predicted bars.
pub fn diagram(reliability: &Reliability, width: usize) -> String {
    let mut out = format!(
        "bins {BINS}, samples {}, ECE {:.3}, Brier {:.3}\n",
        reliability.count, reliability.ece, reliability.brier
    );
    for bin in &reliability.bins {
        out.push_str(&format!(
            "{:.1}-{:.1} n={:<4} observed {:.3} {} predicted {:.3} {}\n",
            bin.lower,
            bin.upper,
            bin.count,
            bin.accuracy,
            bar(bin.accuracy, width),
            bin.mean_confidence,
            bar(bin.mean_confidence, width)
        ));
    }
    out
}

fn bin_index(raw: f64) -> usize {
    let scaled = (raw * BINS as f64).floor() as usize;
    scaled.min(BINS - 1)
}

fn bar(value: f64, width: usize) -> String {
    let filled = (value.clamp(0.0, 1.0) * width as f64).round() as usize;
    format!(
        "[{}{}]",
        "#".repeat(filled),
        ".".repeat(width.saturating_sub(filled))
    )
}

#[cfg(test)]
mod tests {
    use super::{bar, bin_index, diagram, reliability, ReliabilityBin, BINS};
    use crate::{CalibrationError, Observation};

    fn observations(samples: &[(f64, bool)]) -> Vec<Observation> {
        samples
            .iter()
            .map(|&(raw, correct)| Observation::new(raw, correct).expect("sample is valid"))
            .collect()
    }

    #[test]
    fn the_errors_match_a_hand_computed_small_sample() {
        let samples = observations(&[(0.95, true), (0.9, true), (0.05, false)]);
        let measured = reliability(&samples).expect("the sample measures");

        assert_eq!(measured.count, 3);
        assert_eq!(measured.bins.len(), 2);
        // 2/3 of the mass sits 0.075 from its outcome, 1/3 sits 0.05 away.
        assert!((measured.ece - 1.0 / 15.0).abs() < 1e-12);
        // (0.05^2 + 0.1^2 + 0.05^2) / 3
        assert!((measured.brier - 0.005).abs() < 1e-12);

        let low: &ReliabilityBin = &measured.bins[0];
        assert_eq!(low.count, 1);
        assert!((low.mean_confidence - 0.05).abs() < 1e-12);
        assert!((low.accuracy - 0.0).abs() < 1e-12);

        let high: &ReliabilityBin = &measured.bins[1];
        assert_eq!(high.count, 2);
        assert!((high.mean_confidence - 0.925).abs() < 1e-12);
        assert!((high.accuracy - 1.0).abs() < 1e-12);
    }

    #[test]
    fn a_perfect_forecast_reports_no_error() {
        let samples = observations(&[(1.0, true), (0.0, false), (1.0, true)]);
        let measured = reliability(&samples).expect("the sample measures");

        assert!(measured.ece < 1e-12);
        assert!(measured.brier < 1e-12);
        assert_eq!(measured.bins.len(), 2);
    }

    #[test]
    fn the_top_edge_lands_in_the_last_bin() {
        assert_eq!(bin_index(0.0), 0);
        assert_eq!(bin_index(0.05), 0);
        assert_eq!(bin_index(0.1), 1);
        assert_eq!(bin_index(0.999), BINS - 1);
        assert_eq!(bin_index(1.0), BINS - 1);
    }

    #[test]
    fn an_empty_sample_is_refused() {
        assert_eq!(reliability(&[]), Err(CalibrationError::Empty));
    }

    #[test]
    fn the_diagram_draws_one_row_per_occupied_bin() {
        let samples = observations(&[(0.95, true), (0.05, false)]);
        let measured = reliability(&samples).expect("the sample measures");
        let drawn = diagram(&measured, 10);

        assert!(drawn.contains("bins 10, samples 2"));
        assert_eq!(drawn.lines().count(), 3);
        assert!(drawn.contains("[##########]"));
        assert!(drawn.contains("[..........]"));
    }

    #[test]
    fn a_bar_never_overflows_or_underflows_its_width() {
        assert_eq!(bar(0.0, 4), "[....]");
        assert_eq!(bar(0.5, 4), "[##..]");
        assert_eq!(bar(1.0, 4), "[####]");
        assert_eq!(bar(0.0, 0), "[]");
    }
}
