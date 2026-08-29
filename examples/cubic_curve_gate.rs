use als_ext::{AlsFloatCurve, AlsKeyInterp};

fn main() {
    let curve = AlsFloatCurve {
        times: vec![0.0, 1.0],
        values: vec![0.0, 1.0],
        interps: vec![AlsKeyInterp::Cubic, AlsKeyInterp::Cubic],
        arrive_tangents: vec![0.0, 2.0],
        leave_tangents: vec![2.0, 0.0],
        ..Default::default()
    };
    let quarter = curve.sample(0.25);
    let midpoint = curve.sample(0.5);
    let three_quarters = curve.sample(0.75);
    let weighted_curve = AlsFloatCurve {
        times: vec![0.0, 1.0],
        values: vec![0.0, 1.0],
        interps: vec![AlsKeyInterp::Cubic, AlsKeyInterp::Cubic],
        weight_modes: vec![2, 1],
        arrive_tangents: vec![0.0, 0.0],
        leave_tangents: vec![1.0, 0.0],
        arrive_weights: vec![0.0, 0.75],
        leave_weights: vec![1.0606602, 0.0],
        ..Default::default()
    };
    let weighted_midpoint = weighted_curve.sample(0.5);
    let passed = (quarter - 0.34375).abs() < 1e-6
        && (midpoint - 0.5).abs() < 1e-6
        && (three_quarters - 0.65625).abs() < 1e-6
        && (weighted_midpoint - 0.78125).abs() < 1e-5;
    let verdict = if passed { "PASS" } else { "FAIL" };
    println!(
        "[BF102.27][VERDICT] cubic=({quarter:.5},{midpoint:.5},{three_quarters:.5}) weighted_midpoint={weighted_midpoint:.5} expected=(0.34375,0.50000,0.65625,0.78125) -> {verdict}"
    );
    if !passed {
        std::process::exit(27);
    }
}
