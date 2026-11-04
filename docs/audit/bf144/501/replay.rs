use bevy::{
    math::{DQuat, DVec3},
    prelude::*,
};
use bevy_als::als::animation::{inv_exp_approx, velocity_blend_direction};
use serde_json::{Value, json};
use std::path::Path;

include!("../../../../tools/tmp/bf144/501/math.rs");
include!("../../../../tools/tmp/bf144/501/basis.rs");

fn direction_control(v: Vec3, q: Quat, euclidean: bool, reciprocal: bool) -> [f32; 4] {
    let local = q.inverse() * v;
    let unit = if euclidean {
        local * local.length_squared().sqrt().recip()
    } else {
        local
    };
    let sum = unit.abs().element_sum();
    let u = if reciprocal {
        unit * sum.recip()
    } else {
        unit / sum
    };
    [(-u.z).max(0.0), u.z.max(0.0), (-u.x).max(0.0), u.x.max(0.0)]
}

fn main() {
    let label = std::env::args().nth(1).unwrap();
    let root = Path::new("docs/audit/bf144/501");
    let data: Value =
        serde_json::from_slice(&std::fs::read(root.join("evidence-input.json")).unwrap()).unwrap();
    let source = &data["source"];
    let target = &data["target"];
    let observed = input(source, target);
    let native: [f32; 4] = a32(&source["actual"]["amounts"]);
    let runtime: [f32; 4] = a32(&target["actual"]);
    let delta = target["simulation_delta"].as_f64().unwrap() as f32;
    assert_eq!(target["simulation_delta"], target["delta"]);
    let world_source =
        DVec3::from_array(a64(&source["end"])) - DVec3::from_array(a64(&source["start"]));
    assert_eq!(world_source / delta as f64, observed[0].native_velocity);
    let start = Vec3::from_array(a32(&target["start"]));
    let end = Vec3::from_array(a32(&target["end"]));
    let planned = Vec3::from_array(a32(&target["plan"]));
    assert_eq!(planned, observed[0].target_velocity);
    let planar = |v: Vec3| Vec3::new(v.x, 0.0, v.z);
    assert_eq!(planar((end - start) / delta), observed[1].target_velocity);
    let delta_planned = Vec3::from_array(a32(&target["plan_delta"]));
    let endpoint = start.as_dvec3() + delta_planned.as_dvec3();
    assert_eq!(
        planar(((endpoint - start.as_dvec3()) / delta as f64).as_vec3()),
        planned
    );
    let modes: Vec<[u8; 4]> =
        serde_json::from_slice(&std::fs::read("tools/tmp/bf144/501/modes.json").unwrap()).unwrap();
    let native_modes = modes
        .iter()
        .filter(|&&mode| source_step(observed, 0, [0.0; 4], delta, mode).1 == native)
        .count();
    assert!(native_modes > 0);
    assert_eq!(target_step(observed, 3, [0.0; 4], delta).1, runtime);
    let plan_control = planned;
    let mut controls = Vec::new();
    for (use_plan, source_rotation) in [(false, false), (true, false), (false, true), (true, true)]
    {
        let mut inputs = observed;
        if use_plan {
            inputs[1].target_velocity = plan_control;
        }
        let mask = 1 | if source_rotation { 0 } else { 2 };
        let (goal, seed) = target_step(inputs, mask, [0.0; 4], delta);
        let q = inputs[usize::from(!source_rotation)].target_rotation;
        let normalized = target_lerp(
            [0.0; 4],
            direction_control(inputs[1].target_velocity, q, true, false),
            target_alpha(delta),
        );
        let reciprocal = target_lerp(
            [0.0; 4],
            direction_control(inputs[1].target_velocity, q, true, true),
            target_alpha(delta),
        );
        let reciprocal_only = target_lerp(
            [0.0; 4],
            direction_control(inputs[1].target_velocity, q, false, true),
            target_alpha(delta),
        );
        controls.push(
            json!({"use_plan":use_plan,"source_rotation":source_rotation,
                             "goal":goal,"seed":seed,"with_euclidean":normalized,"with_euclidean_reciprocal":reciprocal,"reciprocal_only":reciprocal_only}),
        );
    }
    let native_l1 = modes
        .iter()
        .filter_map(|&mode| {
            if source_step(observed, 0, [0.0; 4], delta, mode).1 != native {
                return None;
            }
            let local = native_local(
                observed[0].native_velocity,
                observed[0].native_rotation,
                mode[0],
            );
            let goal = native_direct_direction(local, true, mode[1]);
            Some(native_lerp(
                [0.0; 4],
                goal,
                native_alpha(delta, mode[2]),
                mode[3] != 0,
            ))
        })
        .collect::<Vec<_>>();
    std::fs::write(root.join(format!("replay-{label}.json")),serde_json::to_vec(
        &json!({"native_modes":native_modes,"native_seed":native,"target_seed":runtime,"controls":controls,"native_direct_l1":native_l1})).unwrap()).unwrap();
    assert_ne!(
        controls[0]["seed"], controls[1]["seed"],
        "Restoring the planned velocity must change the observed seed"
    );
    assert!(
        native_l1.iter().all(|amounts| amounts != &native),
        "Direct component division must lose the native seed"
    );
    println!("Verified first-motion native and target seed and the planned-velocity control");
}
