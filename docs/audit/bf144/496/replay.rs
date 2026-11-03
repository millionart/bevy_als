use bevy::{math::DVec3, prelude::*};
use bevy_als::extensions::{
    mesh_space_additive::{apply_local_space_additive, apply_mesh_space_additive},
    mesh_space_difference::make_mesh_space_difference,
    reference_local_difference::make_local_difference,
    source_pose_nodes::blend_source_poses,
};
use bevy_animation_graph::core::{
    animation_clip::EntityPath,
    pose::{BonePose, Pose},
    skeleton::Skeleton,
};
use serde_json::{Value, json};
use std::path::Path;

fn vector(value: &Value) -> Vec3 {
    Vec3::from_array(std::array::from_fn(|i| value[i].as_f64().unwrap() as f32))
}

fn main() {
    let label = std::env::args().nth(1).unwrap();
    let root = Path::new("docs/audit/bf144/496");
    let input: Value =
        serde_json::from_slice(&std::fs::read(root.join("evidence-input.json")).unwrap()).unwrap();
    let path = EntityPath::from(vec!["VB foot_l".to_owned()]);
    let id = path.id();
    let mut skeleton = Skeleton::default();
    skeleton.add_bone(path, Transform::IDENTITY, Transform::IDENTITY);
    skeleton.set_root(id);
    let pose = |scale| {
        let mut pose = Pose::default();
        pose.add_bone(
            BonePose {
                scale: Some(scale),
                ..default()
            },
            id,
        );
        pose
    };
    let scale = |pose: &Pose| {
        pose.get_bone(id)
            .and_then(|bone| bone.scale)
            .unwrap_or_default()
    };
    let lean_original = vector(&input["tracks"]["lean"]["scale"]);
    let lean_raw = vector(&input["native"]["lean"]["scale"]);
    let mut rows = Vec::new();
    for row in input["rows"].as_array().unwrap() {
        let mut controls = Vec::new();
        for (raw_lean, unit_stand) in [(false, false), (true, false), (false, true), (true, true)] {
            let mut run_delta = None;
            let locomotion = if !row["run_start"].is_null() {
                let run = &row["run_start"];
                let reference = pose(if raw_lean { lean_raw } else { lean_original });
                let desired = run["desired"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_f64().unwrap() as f32)
                    .collect::<Vec<_>>();
                let total: f32 = desired.iter().sum();
                let mut inputs = Vec::new();
                for (name, desired_index, ordinal) in [("forward", 0, 0), ("right", 3, 1)] {
                    let leaf = pose(vector(&input["tracks"][name]["scale"]));
                    let delta = make_local_difference(&reference, &leaf, &skeleton);
                    if !raw_lean {
                        assert_eq!(scale(&delta), vector(&run["child_scales"][ordinal]));
                    }
                    inputs.push((pose(scale(&delta)), desired[desired_index] / total));
                }
                let delta = blend_source_poses(&inputs);
                if !raw_lean {
                    assert_eq!(scale(&delta), vector(&run["delta"]));
                }
                run_delta = Some(scale(&delta));
                let mut base = pose(vector(&run["base"]));
                apply_local_space_additive(&mut base, &delta, 1.0, &skeleton);
                base
            } else {
                pose(vector(&row["locomotion"]))
            };
            let ordinary = if unit_stand {
                Vec3::ONE
            } else {
                vector(&row["ordinary_reference"])
            };
            let dynamic = make_mesh_space_difference(&pose(ordinary), &locomotion, &skeleton);
            let overlay_scale = vector(&row["legs_overlay_base"]);
            let mut overlay = pose(overlay_scale);
            apply_mesh_space_additive(&mut overlay, &dynamic, 1.0, &skeleton);
            let alpha = row["legs_alpha"].as_f64().unwrap() as f32;
            let legs = if alpha == 1.0 {
                overlay.clone()
            } else {
                blend_source_poses(&[(locomotion.clone(), 1.0 - alpha), (overlay.clone(), alpha)])
            };
            let mut head = legs.clone();
            let head_delta = vector(&row["head_delta"]);
            apply_mesh_space_additive(&mut head, &pose(head_delta), 1.0, &skeleton);
            if !raw_lean && !unit_stand {
                assert_eq!(scale(&locomotion), vector(&row["locomotion"]));
                assert_eq!(scale(&dynamic), vector(&row["dynamic_delta"]));
                assert_eq!(scale(&overlay), vector(&row["legs_overlay_applied"]));
                assert_eq!(scale(&legs), vector(&row["legs_output"]));
                assert_eq!(scale(&head), vector(&row["head_output"]));
            }
            let source_locomotion = scale(&locomotion).as_dvec3();
            let source_delta = source_locomotion * ordinary.as_dvec3().recip() - DVec3::ONE;
            let source_overlay = overlay_scale.as_dvec3() * (DVec3::ONE + source_delta);
            let wa = 1.0_f32 - alpha;
            let wb = 1.0_f32 - wa;
            let double_legs =
                source_overlay.mul_add(DVec3::splat(alpha as f64), source_locomotion * wa as f64);
            let source_legs =
                source_overlay.mul_add(DVec3::splat(wb as f64), source_locomotion * wa as f64);
            let source_head = source_legs * (DVec3::ONE + head_delta.as_dvec3());
            controls.push(json!({"raw_lean":raw_lean,"unit_stand":unit_stand,
                "run_delta":run_delta,"locomotion":scale(&locomotion),"dynamic":scale(&dynamic),
                "overlay":scale(&overlay),"legs":scale(&legs),"head":scale(&head),
                "fixed_input_f64":{"dynamic":source_delta,"overlay":source_overlay,"target_weights_legs":double_legs,
                    "source_complement_legs":source_legs,"head":source_head,
                    "f32_dynamic":source_delta.as_vec3(),"f32_overlay":source_overlay.as_vec3(),
                    "f32_legs":source_legs.as_vec3(),"f32_head":source_head.as_vec3(),"weight_a":wa,"source_weight_b":wb}}));
        }
        rows.push(json!({"query":row["query"],"controls":controls}));
        println!(
            "Replayed Run Start, dynamic scale and Legs at query {}",
            row["query"]
        );
    }
    std::fs::write(
        root.join(format!("replay-{label}.json")),
        serde_json::to_vec(&rows).unwrap(),
    )
    .unwrap();
    assert_eq!(
        rows[0]["controls"][1]["run_delta"],
        json!([0.0, 0.0, 0.0]),
        "Raw Lean must remove the additive scale"
    );
    assert_eq!(
        rows[1]["controls"][2]["dynamic"],
        json!([0.0, 0.0, 0.0]),
        "Neutral ordinary reference must remove the dynamic delta"
    );
}
