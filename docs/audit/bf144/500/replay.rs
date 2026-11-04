use bevy::{
    math::{DQuat, DVec3},
    prelude::*,
};
use bevy_als::als::animation::{inv_exp_approx, velocity_blend_direction};
use serde_json::{Value, json};
use std::path::Path;

include!("../../../../tools/tmp/bf144/500/math.rs");

const ENABLED_FACTORS: u8 = 7;

#[derive(Clone, Copy)]
struct Input {
    native_velocity: DVec3,
    target_velocity: Vec3,
    native_rotation: DQuat,
    target_rotation: Quat,
    previous: [f32; 4],
}
fn project_axes(v: DVec3) -> DVec3 {
    DVec3::new(v.x, v.z, v.y)
}
fn project_quaternion(q: DQuat) -> DQuat {
    DQuat::from_xyzw(-q.x, -q.z, -q.y, q.w)
}
fn body_offset() -> DQuat {
    DQuat::from_xyzw(
        0.0,
        -std::f64::consts::FRAC_1_SQRT_2,
        0.0,
        std::f64::consts::FRAC_1_SQRT_2,
    )
}
fn runtime_rotation(source: DQuat) -> DQuat {
    project_quaternion(source) * body_offset()
}
fn source_rotation(runtime: DQuat) -> DQuat {
    project_quaternion(runtime * body_offset().conjugate())
}

fn verify_basis(source: DQuat) {
    let runtime = runtime_rotation(source);
    for (source_axis, target_axis) in [
        (DVec3::X, DVec3::NEG_Z),
        (DVec3::Y, DVec3::X),
        (DVec3::Z, DVec3::Y),
    ] {
        assert!(
            (project_axes(source.normalize() * source_axis) - runtime.normalize() * target_axis)
                .length()
                < 1e-12
        );
    }
    assert!(
        source
            .normalize()
            .dot(source_rotation(runtime).normalize())
            .abs()
            > 1.0 - 1e-14
    );
}

fn input(source: &Value, target: &Value) -> [Input; 2] {
    let source_v = DVec3::from_array(a64(&source["velocity"]));
    let target_v = Vec3::from_array(a32(&target["velocity"]));
    let source_q = DQuat::from_array(a64(&source["rotation"]));
    let target_q = Quat::from_array(a32(&target["rotation"]));
    verify_basis(source_q);
    [
        Input {
            native_velocity: source_v,
            target_velocity: (project_axes(source_v) * 0.01).as_vec3(),
            native_rotation: source_q,
            target_rotation: runtime_rotation(source_q).as_quat(),
            previous: a32(&source["previous"]["amounts"]),
        },
        Input {
            native_velocity: project_axes(target_v.as_dvec3()) * 100.0,
            target_velocity: target_v,
            native_rotation: source_rotation(target_q.as_dquat()),
            target_rotation: target_q,
            previous: a32(&target["previous"]),
        },
    ]
}

fn source_step(
    inputs: [Input; 2],
    mask: u8,
    previous: [f32; 4],
    delta: f32,
    mode: [u8; 4],
) -> ([f32; 4], [f32; 4]) {
    let v = inputs[usize::from(mask & 1 != 0)].native_velocity;
    let q = inputs[usize::from(mask & 2 != 0)].native_rotation;
    let goal = native_direction(native_local(v, q, mode[0]), true, mode[1]);
    (
        goal,
        native_lerp(previous, goal, native_alpha(delta, mode[2]), mode[3] != 0),
    )
}
fn target_step(
    inputs: [Input; 2],
    mask: u8,
    previous: [f32; 4],
    delta: f32,
) -> ([f32; 4], [f32; 4]) {
    let v = inputs[usize::from(mask & 1 != 0)].target_velocity;
    let q = inputs[usize::from(mask & 2 != 0)].target_rotation;
    let local = velocity_blend_direction(v, q);
    let goal = [
        (-local.z).max(0.0),
        local.z.max(0.0),
        (-local.x).max(0.0),
        local.x.max(0.0),
    ];
    (goal, target_lerp(previous, goal, target_alpha(delta)))
}

fn main() {
    let label = std::env::args().nth(1).unwrap();
    let output = Path::new("docs/audit/bf144/500");
    let rows: Vec<Value> =
        serde_json::from_slice(&std::fs::read("docs/audit/bf144/499/evidence-input.json").unwrap())
            .unwrap();
    let modes: Vec<[u8; 4]> =
        serde_json::from_slice(&std::fs::read("tools/tmp/bf144/500/modes.json").unwrap()).unwrap();
    let first = input(&rows[0]["source"], &rows[0]["target"]);
    let seed = std::array::from_fn::<_, 8, _>(|mask| {
        first[usize::from(mask & usize::from(ENABLED_FACTORS) & 4 != 0)].previous
    });
    let mut target_history = seed;
    let mut native_histories = vec![seed; modes.len()];
    let mut reports = Vec::new();
    let mut actual_mismatches = 0;
    for row in rows {
        let inputs = input(&row["source"], &row["target"]);
        let delta = row["source"]["delta"].as_f64().unwrap() as f32;
        assert_eq!(row["source"]["delta"], row["target"]["delta"]);
        let native_actual: [f32; 4] = a32(&row["source"]["actual"]["amounts"]);
        let target_actual: [f32; 4] = a32(&row["target"]["actual"]);
        let mut grid = Vec::new();
        for mask in 0..8 {
            let enabled = mask as u8 & ENABLED_FACTORS;
            let previous = inputs[usize::from(enabled & 4 != 0)].previous;
            let (target_goal, target_fixed) = target_step(inputs, enabled, previous, delta);
            let (_, target_continuous) = target_step(inputs, enabled, target_history[mask], delta);
            target_history[mask] = target_continuous;
            let source = modes
                .iter()
                .enumerate()
                .map(|(index, &mode)| {
                    let (goal, fixed) = source_step(inputs, enabled, previous, delta, mode);
                    let (_, continuous) =
                        source_step(inputs, enabled, native_histories[index][mask], delta, mode);
                    native_histories[index][mask] = continuous;
                    if mask == 0 {
                        actual_mismatches += usize::from(fixed != native_actual);
                        actual_mismatches += usize::from(continuous != native_actual);
                    }
                    (goal, fixed, continuous)
                })
                .collect::<Vec<_>>();
            if mask == 7 {
                actual_mismatches += usize::from(target_fixed != target_actual);
                actual_mismatches += usize::from(target_continuous != target_actual);
            }
            let limits = |values: Vec<[f32; 4]>| {
                let minimum = std::array::from_fn::<_, 4, _>(|i| {
                    values.iter().map(|v| v[i]).fold(f32::INFINITY, f32::min)
                });
                let maximum = std::array::from_fn::<_, 4, _>(|i| {
                    values
                        .iter()
                        .map(|v| v[i])
                        .fold(f32::NEG_INFINITY, f32::max)
                });
                json!({"minimum":minimum,"maximum":maximum})
            };
            grid.push(json!({"mask":mask,"source":{"goal":source[0].0,"fixed":source[0].1,"continuous":source[0].2,
                "fixed_limits":limits(source.iter().map(|v|v.1).collect()),
                "continuous_limits":limits(source.iter().map(|v|v.2).collect())},
                "target":{"goal":target_goal,"fixed":target_fixed,"continuous":target_continuous}}));
        }
        reports.push(json!({"query":row["query"],"grid":grid}));
        println!(
            "Replayed actual and crossed VelocityBlend history at {}",
            row["query"]
        );
    }
    std::fs::write(output.join(format!("replay-{label}.json")),serde_json::to_vec(
        &json!({"rows":reports,"actual_mismatches":actual_mismatches,"source_models":modes.len()})).unwrap()).unwrap();
    assert_eq!(
        actual_mismatches, 0,
        "The fixed and continuous actual endpoints must match both observed histories"
    );
}
