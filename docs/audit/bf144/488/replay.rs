use bevy::{
    math::{DQuat, DVec3},
    prelude::*,
};
use bevy_als::extensions::control_rig_hierarchy::RigTransform;
use serde_json::{Value, json};
use std::path::Path;

#[allow(dead_code)]
mod existing {
    include!("../486/replay.rs");

    pub fn native(value: &Value) -> RigTransform {
        local(value)
    }

    pub fn native_socket(chain: &[RigTransform], world: RigTransform) -> RigTransform {
        tqs(chain, world, true)
    }

    pub fn sampled(chain: &[Value]) -> GlobalTransform {
        public_affine(&chain.iter().collect::<Vec<_>>())
    }
}

fn read(path: &Path) -> Value {
    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
}

fn controlled_scale(scale: DVec3, vertical: usize) -> DVec3 {
    assert!(scale.min_element() > 0.0);
    DVec3::splat(scale[vertical])
}

fn matrix_shear(transform: &GlobalTransform) -> f64 {
    let affine = transform.affine();
    let [a, b, c] = [
        affine.matrix3.x_axis,
        affine.matrix3.y_axis,
        affine.matrix3.z_axis,
    ]
    .map(|axis| Vec3::from(axis).as_dvec3().normalize());
    a.dot(b).abs().max(a.dot(c).abs()).max(b.dot(c).abs())
}

fn rotation_delta_deg(a: Quat, b: Quat) -> f64 {
    let a = a.as_dquat().normalize();
    let b = b.as_dquat().normalize();
    let delta = a.conjugate() * b;
    2.0 * DVec3::new(delta.x, delta.y, delta.z)
        .length()
        .atan2(delta.w.abs())
        .to_degrees()
}

fn quaternion(value: &Value) -> DQuat {
    DQuat::from_array(std::array::from_fn(|i| value[i].as_f64().unwrap()))
}

fn main() {
    let temp = Path::new("tools/tmp/bf144/488");
    let output = Path::new("docs/audit/bf144/488");
    let label = std::env::args().nth(1).unwrap();
    let source = read(&temp.join("source.json"));
    let target = read(&temp.join("target.json"));
    let source_names = source["names"].as_array().unwrap();
    let names = |name: &str| source_names.iter().position(|v| v == name).unwrap();
    let root = names("root");
    let parent = names("ik_foot_root");
    assert_eq!(source["parents"][parent], root);
    let feet = ["ik_foot_l", "ik_foot_r"].map(names);
    assert!(feet.iter().all(|&i| source["parents"][i] == parent));

    let mut native_rows = Vec::new();
    let mut native_translation_changes = 0;
    for row in source["rows"].as_array().unwrap().iter().skip(1) {
        assert_eq!(row["valid"], true);
        for (side, foot) in feet.into_iter().enumerate() {
            let actual = [root, parent, foot].map(|i| existing::native(&row["prior"][i]));
            let mut uniform = actual;
            uniform[1].scale = controlled_scale(actual[1].scale, 2);
            let world = existing::native(&row["world"]);
            let before = existing::native_socket(&actual, world);
            let after = existing::native_socket(&uniform, world);
            let expected = &row["expected"][side];
            assert_eq!(json!(before.translation.to_array()), expected[0]);
            assert_eq!(before.rotation, quaternion(&expected[1]));
            assert_eq!(before.rotation, after.rotation);
            native_translation_changes += usize::from(before.translation != after.translation);
            native_rows.push(json!({"frame":row["frame"],"side":side,"scale":actual[1].scale,
                                    "control_scale":uniform[1].scale,"actual":before,"uniform":after}));
        }
        if row["frame"].as_u64().unwrap() % 64 == 0 {
            println!("Qualified native scale control at {}", row["frame"]);
        }
    }

    let mut target_rows = Vec::new();
    let mut target_rotation_changes = 0;
    for row in target["rows"].as_array().unwrap() {
        let observed = &row["observation"];
        for (side, expected) in observed["targets"].as_array().unwrap().iter().enumerate() {
            let chain = expected["chain"].as_array().unwrap();
            assert_eq!(chain[0]["bone"], ["ik_foot_l", "ik_foot_r"][side]);
            assert_eq!(chain[1]["bone"], "ik_foot_root");
            assert_eq!(chain[2]["bone"], "root");
            let parent = existing::native(&chain[1]);
            assert!(
                chain
                    .iter()
                    .all(|node| existing::native(node).scale.min_element() > 0.0)
            );
            let mut uniform = chain.clone();
            let scale = controlled_scale(parent.scale, 1);
            uniform[1]["scale"] = json!(scale);
            let before = existing::sampled(chain);
            let after = existing::sampled(&uniform);
            let actual_q = before.rotation();
            let uniform_q = after.rotation();
            assert_eq!(
                json!(before.to_matrix().to_cols_array()),
                expected["matrix"]
            );
            assert_eq!(json!(actual_q.to_array()), expected["rotation"]);
            assert_eq!(
                json!(actual_q.to_array()),
                observed["states"][side]["target_rotation"]
            );
            assert_eq!(
                json!(before.translation().to_array()),
                observed["states"][side]["target_location"]
            );
            if parent.scale.x == parent.scale.y && parent.scale.z == parent.scale.y {
                assert_eq!(before, after);
            }
            target_rotation_changes += usize::from(actual_q != uniform_q);
            target_rows.push(json!({
                "query":row["query"],"side":side,"scale":parent.scale,"control_scale":scale,
                "actual_q":actual_q,"uniform_q":uniform_q,
                "actual_t":before.translation(),"uniform_t":after.translation(),
                "actual_norm_sq":actual_q.as_dquat().length_squared(),"uniform_norm_sq":uniform_q.as_dquat().length_squared(),
                "actual_shear":matrix_shear(&before),"uniform_shear":matrix_shear(&after),
                "normalized_orientation_delta_deg":rotation_delta_deg(actual_q,uniform_q)
            }));
        }
        if row["query"].as_u64().unwrap() % 64 == 1 {
            println!("Qualified target scale control at {}", row["query"]);
        }
    }
    assert_eq!((native_rows.len(), target_rows.len()), (634, 636));
    let report = json!({"native":native_rows,"target":target_rows,
                        "native_translation_changes":native_translation_changes,
                        "target_rotation_changes":target_rotation_changes});
    std::fs::write(
        output.join(format!("replay-{label}.json")),
        serde_json::to_vec(&report).unwrap(),
    )
    .unwrap();
    assert!(
        native_translation_changes > 0 && target_rotation_changes > 0,
        "The isolated scale toggle must be active"
    );
    println!(
        "Verified 634 actual native sockets and 636 actual affine samples; {target_rotation_changes} target Q changes"
    );
}
