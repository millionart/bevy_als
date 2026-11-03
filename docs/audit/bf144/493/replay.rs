use bevy::math::{DQuat, DVec3};
use bevy_als::extensions::control_rig_hierarchy::RigTransform;
use serde_json::{Value, json};
use std::path::Path;

#[allow(dead_code)]
mod fixture {
    include!("../486/replay.rs");
    pub fn sampled(chain: &[Value], world: &Value) -> RigTransform {
        tqs(
            &chain.iter().map(local).collect::<Vec<_>>(),
            local(world),
            true,
        )
    }
}

mod target {
    include!("../../../../tools/tmp/bf144/493/target.rs");
}

fn root_blend_control(label: &str) {
    use bevy::prelude::*;
    use bevy_als::extensions::source_pose_nodes::blend_source_poses;
    use bevy_animation_graph::core::{
        animation_clip::EntityPath,
        pose::{BonePose, Pose},
    };
    let root = Path::new("docs/audit/bf144/493");
    let data: Value = serde_json::from_slice(
        &std::fs::read(root.join(format!("runtime-{label}/parameters.json"))).unwrap(),
    )
    .unwrap();
    let names = if label == "virtual" {
        ["VB foot_root", "VB foot_l", "VB foot_r"]
    } else {
        ["ik_foot_root", "ik_foot_l", "ik_foot_r"]
    };
    let find = |pose: &Value, name: &str| {
        pose["pose"]["bones"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["name"] == name)
            .unwrap()
            .clone()
    };
    let mut rows = Vec::new();
    for (index, row) in data["animation_after"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
    {
        let poses = &row["rig_snapshot"]["poses"];
        let mut input = Pose::default();
        for name in names {
            let t: RigTransform = serde_json::from_value(find(&poses["rig_output"], name)).unwrap();
            let id = EntityPath::from(vec![name.to_owned()]).id();
            input.add_bone(
                BonePose::from_transform(Transform {
                    translation: t.translation.as_vec3(),
                    rotation: t.rotation.as_quat(),
                    scale: t.scale.as_vec3(),
                }),
                id,
            );
        }
        let output = blend_source_poses(&[(input.clone(), 1.0)]);
        for name in names {
            let id = EntityPath::from(vec![name.to_owned()]).id();
            let expected = find(&poses["root_result"], name);
            let bone = output.get_bone(id).unwrap();
            assert_eq!(
                json!(bone.translation.unwrap().to_array()),
                expected["translation"]
            );
            assert_eq!(
                json!(bone.rotation.unwrap().to_array()),
                expected["rotation"]
            );
            assert_eq!(json!(bone.scale.unwrap().to_array()), expected["scale"]);
            if input.get_bone(id).unwrap().rotation != bone.rotation {
                rows.push(
                    json!({"query":index,"bone":name,"rig_q":input.get_bone(id).unwrap().rotation,
                                 "root_q":bone.rotation}),
                );
            }
        }
        if index % 32 == 0 {
            println!("Replayed single full-weight Root blend at {index}");
        }
    }
    std::fs::write(
        root.join(format!("root-blend-{label}.json")),
        serde_json::to_vec(&rows).unwrap(),
    )
    .unwrap();
}
fn main() {
    let label = std::env::args().nth(1).unwrap();
    if let Some(mode) = label.strip_prefix("root-blend-") {
        root_blend_control(mode);
        return;
    }
    if label.starts_with("target-") {
        target::run(&label);
        return;
    }
    let root = Path::new("docs/audit/bf144/493");
    let input: Vec<Value> = serde_json::from_slice(
        &std::fs::read(format!("tools/tmp/bf144/493/socket-{label}.json")).unwrap(),
    )
    .unwrap();
    let mut rows = Vec::new();
    for row in input {
        let position = DVec3::from_array(std::array::from_fn(|i| {
            row["expected"][0][i].as_f64().unwrap()
        }));
        let rotation = DQuat::from_array(std::array::from_fn(|i| {
            row["expected"][1][i].as_f64().unwrap()
        }));
        let mut controls = Vec::new();
        for (name, chains) in row["controls"].as_object().unwrap() {
            for phase in ["prior", "current"] {
                let socket = fixture::sampled(chains[phase].as_array().unwrap(), &row["world"]);
                controls.push(json!({"bone":name,"phase":phase,"socket":socket,
                                     "t_exact":socket.translation==position,"q_exact":socket.rotation==rotation}));
            }
        }
        rows.push(json!({"case":row["case"],"frame":row["frame"],"side":row["side"],"valid":row["valid"],
                         "requested":row["requested"],"expected":row["expected"],"controls":controls}));
    }
    assert_eq!(
        rows.len(),
        if label == "virtual-motion" { 316 } else { 160 }
    );
    std::fs::write(
        root.join(format!("socket-{label}.json")),
        serde_json::to_vec(&rows).unwrap(),
    )
    .unwrap();
    println!(
        "Qualified {} native socket phase and marker control rows",
        rows.len()
    );
}
