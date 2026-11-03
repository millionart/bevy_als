use bevy::prelude::*;
use bevy_als::extensions::{
    mesh_space_additive::{apply_local_space_additive, apply_mesh_space_additive},
    mesh_space_difference::make_mesh_space_difference,
    reference_local_difference::make_local_difference,
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
    let root = Path::new("docs/audit/bf144/495");
    let data: Value =
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
    let recorded_reference = vector(&data["gltf"]["stand"]["value"]);
    let raw_reference = vector(&data["native"]["stand"][0]["scale"]);
    let idle = pose(vector(&data["gltf"]["idle"]["value"]));
    let look = pose(vector(&data["gltf"]["look"]["value"]));
    let mut results = Vec::new();
    for row in data["rows"].as_array().unwrap() {
        let mut controls = Vec::new();
        for (raw_idle, raw_look) in [(false, false), (true, false), (false, true), (true, true)] {
            let idle_ref = pose(if raw_idle {
                raw_reference
            } else {
                recorded_reference
            });
            let look_ref = pose(if raw_look {
                raw_reference
            } else {
                recorded_reference
            });
            let idle_delta = make_local_difference(&idle_ref, &idle, &skeleton);
            let look_delta = make_mesh_space_difference(&look_ref, &look, &skeleton);
            let mut overlay = pose(vector(&row["overlay_base"]));
            apply_local_space_additive(
                &mut overlay,
                &idle_delta,
                row["overlay_alpha"].as_f64().unwrap() as f32,
                &skeleton,
            );
            let mut head = if row["use_overlay_as_head_base"] == true {
                overlay.clone()
            } else {
                pose(vector(&row["head_base"]))
            };
            apply_mesh_space_additive(
                &mut head,
                &look_delta,
                row["head_alpha"].as_f64().unwrap() as f32,
                &skeleton,
            );
            if !raw_idle && !raw_look {
                assert_eq!(scale(&idle_delta), vector(&row["idle_scale"]));
                assert_eq!(scale(&look_delta), vector(&row["head_add"]));
                assert_eq!(scale(&overlay), vector(&row["overlay_output"]));
                assert_eq!(scale(&head), vector(&row["head_output"]));
            }
            controls.push(json!({"raw_idle":raw_idle,"raw_look":raw_look,"idle_delta":scale(&idle_delta),
                                  "look_delta":scale(&look_delta),"overlay":scale(&overlay),"head":scale(&head)}));
        }
        results.push(json!({"query":row["query"],"controls":controls}));
        println!(
            "Replayed actual Idle and Head scale at query {}",
            row["query"]
        );
    }
    std::fs::write(
        root.join(format!("replay-{label}.json")),
        serde_json::to_vec(&results).unwrap(),
    )
    .unwrap();
    for index in [2, 3] {
        let controls = &results[index]["controls"];
        let one_ulp = 1.0000001192092896;
        assert_eq!(controls[1]["head"][1], one_ulp);
        assert_eq!(controls[2]["head"][1], one_ulp);
        assert_eq!(
            controls[3]["head"][1], 1.0,
            "Both raw references are required to eliminate the two additive deltas"
        );
    }
}
