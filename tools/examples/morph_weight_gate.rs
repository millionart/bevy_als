use bevy_animation_graph::core::pose::BonePose;

fn blend(base: &BonePose, overlay: &BonePose, alpha: f32) -> Vec<f32> {
    let mut pose = base.clone();
    pose.linear_blend_mut(overlay, alpha);
    pose.weights.unwrap_or_default()
}

fn error(actual: &[f32], expected: &[f32]) -> f32 {
    if actual.len() != expected.len() {
        return f32::INFINITY;
    }
    actual
        .iter()
        .zip(expected)
        .map(|(actual, expected)| (actual - expected).abs())
        .fold(0.0, f32::max)
}

fn main() {
    let base = BonePose {
        weights: Some(vec![0.6, 0.4]),
        ..BonePose::default()
    };
    let overlay = BonePose {
        weights: Some(vec![0.8, 0.2]),
        ..BonePose::default()
    };
    let start = blend(&base, &overlay, 0.0);
    let middle = blend(&base, &overlay, 0.5);
    let end = blend(&base, &overlay, 1.0);
    let same = blend(&base, &base, 0.5);
    let short_overlay = BonePose {
        weights: Some(vec![0.8]),
        ..BonePose::default()
    };
    let short_middle = blend(&base, &short_overlay, 0.5);
    let passed = error(&start, &[0.6, 0.4]) < 1e-6
        && error(&middle, &[0.7, 0.3]) < 1e-6
        && error(&end, &[0.8, 0.2]) < 1e-6
        && error(&same, &[0.6, 0.4]) < 1e-6
        && error(&short_middle, &[0.7, 0.2]) < 1e-6;
    let verdict = if passed { "PASS" } else { "FAIL" };
    println!(
        "[BF102.30][VERDICT] start={start:?} middle={middle:?} end={end:?} same={same:?} short_middle={short_middle:?} -> {verdict}"
    );
    if !passed {
        std::process::exit(30);
    }
}
