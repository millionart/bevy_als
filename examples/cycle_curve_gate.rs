use als_ext::{AlsExtrapolation, AlsFloatCurve, AlsKeyInterp};

fn main() {
    let curve = AlsFloatCurve {
        times: vec![0.0, 1.0],
        values: vec![0.0, 1.0],
        interps: vec![AlsKeyInterp::Linear, AlsKeyInterp::Linear],
        pre_infinity: AlsExtrapolation::Cycle,
        post_infinity: AlsExtrapolation::Cycle,
    };
    let values = [
        curve.sample(1.25),
        curve.sample(2.25),
        curve.sample(-0.25),
        curve.sample(-1.25),
    ];
    let expected = [0.25, 0.25, 0.75, 0.75];
    let passed = values
        .iter()
        .zip(expected)
        .all(|(value, expected)| (*value - expected).abs() < 1e-6);
    let verdict = if passed { "PASS" } else { "FAIL" };
    println!(
        "[BF102.26][VERDICT] samples={:?} expected={:?} -> {verdict}",
        values, expected,
    );
    if !passed {
        std::process::exit(26);
    }
}
