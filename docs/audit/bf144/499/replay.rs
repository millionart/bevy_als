use bevy::{
    math::{DQuat, DVec3},
    prelude::*,
};
use bevy_als::als::animation::{inv_exp_approx, velocity_blend_direction};
use serde_json::{Value, json};
use std::path::Path;

fn a32<const N: usize>(v: &Value) -> [f32; N] {
    std::array::from_fn(|i| v[i].as_f64().unwrap() as f32)
}
fn a64<const N: usize>(v: &Value) -> [f64; N] {
    std::array::from_fn(|i| v[i].as_f64().unwrap())
}
fn alpha_x(delta: f32) -> f32 {
    std::f32::consts::LN_2 / (0.1_f32 + 1e-8) * delta
}
fn target_alpha(delta: f32) -> f32 {
    1.0 - inv_exp_approx(alpha_x(delta))
}

fn native_alpha(delta: f32, mode: u8) -> f32 {
    let x = alpha_x(delta);
    let mul_add = |a: f32, b: f32, c: f32, bit| {
        if mode & bit != 0 {
            a.mul_add(b, c)
        } else {
            a * b + c
        }
    };
    let first = mul_add(1.00746054, x, 1.0, 1);
    let second = mul_add(0.45053901 * x, x, first, 2);
    let denominator = mul_add((0.25724632 * x) * x, x, second, 4);
    1.0 - 1.0 / denominator
}

fn native_local(v: DVec3, q: DQuat, mode: u8) -> Vec3 {
    assert_eq!((v.z, q.x, q.y), (0.0, 0.0, 0.0));
    let tx = 2.0 * (q.z * v.y);
    let ty = 2.0 * (-q.z * v.x);
    let mul_add = |a: f64, b: f64, c: f64, bit| {
        if mode & bit != 0 {
            a.mul_add(b, c)
        } else {
            a * b + c
        }
    };
    Vec3::new(
        mul_add(q.z, ty, mul_add(q.w, tx, v.x, 1), 2) as f32,
        mul_add(-q.z, tx, mul_add(q.w, ty, v.y, 1), 2) as f32,
        0.0,
    )
}

fn native_direction(local: Vec3, normalize: bool, mode: u8) -> [f32; 4] {
    let square = match mode {
        0 => local.x * local.x + local.y * local.y,
        1 => local.x.mul_add(local.x, local.y * local.y),
        _ => local.y.mul_add(local.y, local.x * local.x),
    };
    if square <= 1e-8 {
        return [0.0; 4];
    }
    let v = if normalize {
        local * square.sqrt().recip()
    } else {
        local
    };
    let v = v * (v.x.abs() + v.y.abs() + v.z.abs()).recip();
    [v.x.max(0.0), (-v.x).max(0.0), (-v.y).max(0.0), v.y.max(0.0)]
}

fn target_direction(row: &Value) -> [f32; 4] {
    let v = velocity_blend_direction(
        Vec3::from_array(a32(&row["velocity"])),
        Quat::from_array(a32(&row["rotation"])),
    );
    [(-v.z).max(0.0), v.z.max(0.0), (-v.x).max(0.0), v.x.max(0.0)]
}

fn native_lerp(before: [f32; 4], goal: [f32; 4], alpha: f32, fused: bool) -> [f32; 4] {
    std::array::from_fn(|i| {
        let delta = goal[i] - before[i];
        if fused {
            alpha.mul_add(delta, before[i])
        } else {
            before[i] + alpha * delta
        }
    })
}

fn target_lerp(before: [f32; 4], goal: [f32; 4], alpha: f32) -> [f32; 4] {
    std::array::from_fn(|i| before[i].lerp(goal[i], alpha))
}

fn main() {
    let label = std::env::args().nth(1).unwrap();
    let root = Path::new("docs/audit/bf144/499");
    let data: Value =
        serde_json::from_slice(&std::fs::read(root.join("evidence-input.json")).unwrap()).unwrap();
    let mut rows = Vec::new();
    let mut source_modes = vec![0; 4 * 3 * 8 * 2];
    for row in data.as_array().unwrap() {
        let native = &row["source"];
        let target = &row["target"];
        let native_world = DVec3::from_array(a64(&native["velocity"]));
        let native_rotation = DQuat::from_array(a64(&native["rotation"]));
        let native_before = a32(&native["previous"]["amounts"]);
        let native_actual: [f32; 4] = a32(&native["actual"]["amounts"]);
        let target_before = a32(&target["previous"]);
        let target_actual = a32(&target["actual"]);
        let delta = target["delta"].as_f64().unwrap() as f32;
        let target_goal = target_direction(target);
        let ta = target_alpha(delta);
        let local = Quat::from_array(a32(&target["rotation"])).inverse()
            * Vec3::from_array(a32(&target["velocity"]));
        let unit = local * local.length_squared().sqrt().recip();
        let direction = unit / unit.abs().element_sum();
        let target_euclidean = [
            (-direction.z).max(0.0),
            direction.z.max(0.0),
            (-direction.x).max(0.0),
            direction.x.max(0.0),
        ];

        assert_eq!(
            target_lerp(target_before, target_goal, ta),
            target_actual,
            "Actual target at {}",
            row["query"]
        );
        let native_delta = native["delta"].as_f64().unwrap() as f32;
        let mut source = Vec::new();
        for local_mode in 0..4 {
            let local = native_local(native_world, native_rotation, local_mode);
            for square_mode in 0..3 {
                let goal = native_direction(local, true, square_mode);
                let no_euclidean = native_direction(local, false, square_mode);
                for alpha_mode in 0..8 {
                    let alpha = native_alpha(native_delta, alpha_mode);
                    for fused in [false, true] {
                        let result = native_lerp(native_before, goal, alpha, fused);
                        let mode = source.len();
                        source_modes[mode] +=
                            (0..4).filter(|&i| result[i] != native_actual[i]).count();
                        source.push(json!({"mode":[local_mode,square_mode,alpha_mode,u8::from(fused)],"local":local,
                            "goal":goal,"alpha":alpha,"amounts":result,"no_euclidean_goal":no_euclidean,
                            "meter_after_local_goal":native_direction((local.as_dvec3()*0.01).as_vec3(),true,square_mode),
                            "no_euclidean_amounts":native_lerp(native_before,no_euclidean,alpha,fused),
                            "target_lerp_amounts":target_lerp(native_before,goal,alpha)}));
                    }
                }
            }
        }
        rows.push(json!({"query":row["query"],"target":{"goal":target_goal,"alpha":ta,"actual":target_actual,
                 "delta_lerp":native_lerp(target_before,target_goal,ta,false),
                 "explicit_two_terms":std::array::from_fn::<_,4,_>(|i|target_before[i]*(1.0-ta)+target_goal[i]*ta),
                 "with_euclidean_goal":target_euclidean,"with_euclidean_amounts":target_lerp(target_before,target_euclidean,ta),
                 "fused_delta_lerp":native_lerp(target_before,target_goal,ta,true)},"native":source}));
        println!("Replayed actual VelocityBlend inputs at {}", row["query"]);
    }
    let min = *source_modes.iter().min().unwrap();
    let exact = source_modes
        .iter()
        .enumerate()
        .filter_map(|(i, &n)| (n == min).then_some(i))
        .collect::<Vec<_>>();
    std::fs::write(
        root.join(format!("replay-{label}.json")),
        serde_json::to_vec(
            &json!({"rows":rows,"minimum_source_field_mismatches":min,"best_source_models":exact}),
        )
        .unwrap(),
    )
    .unwrap();
    println!(
        "Native minimum mismatches {min}, qualifying arithmetic variants {}",
        exact.len()
    );
    assert_eq!(
        min, 0,
        "The observed native trajectory requires the two-stage direction"
    );
}
