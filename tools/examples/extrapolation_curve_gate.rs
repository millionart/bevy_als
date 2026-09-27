use bevy_als::extensions::{AlsExtrapolation, AlsFloatCurve, AlsKeyInterp};

fn curve(extrapolation: AlsExtrapolation) -> AlsFloatCurve {
    AlsFloatCurve {
        times: vec![0.0, 1.0],
        values: vec![0.0, 1.0],
        interps: vec![AlsKeyInterp::Linear, AlsKeyInterp::Linear],
        pre_infinity: extrapolation,
        post_infinity: extrapolation,
        ..Default::default()
    }
}

fn main() {
    let cycle = curve(AlsExtrapolation::Cycle);
    let offset = curve(AlsExtrapolation::CycleWithOffset);
    let oscillate = curve(AlsExtrapolation::Oscillate);
    let cycle_samples = [cycle.sample(1.25), cycle.sample(2.25)];
    let offset_samples = [
        offset.sample(1.25),
        offset.sample(2.25),
        offset.sample(-0.25),
        offset.sample(-1.25),
    ];
    let oscillate_samples = [
        oscillate.sample(1.25),
        oscillate.sample(2.25),
        oscillate.sample(-0.25),
        oscillate.sample(-1.25),
    ];
    let passed = cycle_samples == [0.25, 0.25]
        && offset_samples == [1.25, 2.25, -0.25, -1.25]
        && oscillate_samples == [0.75, 0.25, 0.25, 0.75];
    let verdict = if passed { "PASS" } else { "FAIL" };
    println!(
        "[BF102.28][VERDICT] cycle={cycle_samples:?} offset={offset_samples:?} oscillate={oscillate_samples:?} -> {verdict}"
    );
    if !passed {
        std::process::exit(28);
    }
}
