// Written by Kone Tshivhinda & Claude
//! End-to-end checks of the public API on a hand-built overconfident sample.

use timelex_calibration::{
    derive_threshold, diagram, reliability, CalibrationError, Calibrator, Observation,
};

fn sample() -> Vec<Observation> {
    let mut observations = Vec::new();
    for _ in 0..6 {
        observations.push(Observation::new(0.9, true).expect("score is valid"));
    }
    for _ in 0..4 {
        observations.push(Observation::new(0.9, false).expect("score is valid"));
    }
    for _ in 0..2 {
        observations.push(Observation::new(0.6, true).expect("score is valid"));
    }
    for _ in 0..3 {
        observations.push(Observation::new(0.6, false).expect("score is valid"));
    }
    observations
}

fn calibrated(calibrator: &Calibrator, observations: &[Observation]) -> Vec<Observation> {
    observations
        .iter()
        .map(|observation| {
            Observation::new(
                calibrator
                    .calibrate(observation.raw)
                    .expect("a fitted score calibrates"),
                observation.correct,
            )
            .expect("a calibrated probability is in [0, 1]")
        })
        .collect()
}

#[test]
fn fitting_an_overconfident_sample_moves_each_score_to_its_observed_rate() {
    let observations = sample();
    let calibrator = Calibrator::fit(&observations).expect("the sample fits");

    assert!((calibrator.calibrate(0.6).unwrap() - 0.4).abs() < 1e-12);
    assert!((calibrator.calibrate(0.9).unwrap() - 0.6).abs() < 1e-12);
    assert!((calibrator.calibrate(0.5).unwrap() - 0.4).abs() < 1e-12);
}

#[test]
fn calibration_lowers_both_error_measures_on_an_overconfident_sample() {
    let observations = sample();
    let calibrator = Calibrator::fit(&observations).expect("the sample fits");

    let before = reliability(&observations).expect("the raw sample measures");
    let after = reliability(&calibrated(&calibrator, &observations)).expect("the calibrated sample measures");

    assert!(before.ece > 0.2, "the raw sample is overconfident");
    assert!(after.ece < 1e-12, "calibrated scores hit the observed rates");
    assert!(
        after.brier < before.brier,
        "Brier fell from {} to {}",
        before.brier,
        after.brier
    );
}

#[test]
fn the_derived_threshold_admits_every_score_once_the_pooled_precision_holds() {
    let observations = sample();
    let calibrator = Calibrator::fit(&observations).expect("the sample fits");
    let scores = calibrated(&calibrator, &observations);

    // No raw score set reaches 90% precision, so the raw sample has no operating point.
    assert_eq!(derive_threshold(&observations, 0.9).unwrap(), None);

    // The calibrated sample pools to 8 of 15, so a 50% floor admits all of it.
    let chosen = derive_threshold(&scores, 0.5)
        .expect("the floor is valid")
        .expect("a threshold exists");
    assert!((chosen.precision - 8.0 / 15.0).abs() < 1e-12);
    assert_eq!(chosen.selected, 15);
    assert_eq!(chosen.correct, 8);
}

#[test]
fn the_diagram_reports_the_summary_and_one_row_per_occupied_bin() {
    let observations = sample();
    let measured = reliability(&observations).expect("the sample measures");
    let drawn = diagram(&measured, 20);

    assert!(drawn.contains("bins 10, samples 15"));
    assert_eq!(drawn.lines().count(), 3);
}

#[test]
fn every_entry_point_refuses_an_empty_sample() {
    assert_eq!(Calibrator::fit(&[]), Err(CalibrationError::Empty));
    assert_eq!(reliability(&[]), Err(CalibrationError::Empty));
    assert_eq!(derive_threshold(&[], 0.9), Err(CalibrationError::Empty));
}
