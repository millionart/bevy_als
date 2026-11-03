use super::{DQuat, RigTransform, Value, dv, json, local, quat};
use std::path::Path;

#[allow(dead_code)]
mod arithmetic {
    include!("../479/replay.rs");

    pub fn normalized(v: Q) -> Q {
        normalize(v, 8)
    }
    pub fn rotated(q: Q, v: DVec3) -> DVec3 {
        rotate(q, v, 4)
    }
}

fn multiply(a: DQuat, b: DQuat) -> DQuat {
    let a = a.to_array();
    let b = b.to_array();
    let x = [b[3], -b[2], b[1], -b[0]];
    let y = [b[2], b[3], -b[0], -b[1]];
    let z = [-b[1], b[0], b[3], -b[2]];
    DQuat::from_array(std::array::from_fn(|i| {
        ((a[3] * b[i] + a[0] * x[i]) + a[1] * y[i]) + a[2] * z[i]
    }))
}

fn compose(parent: RigTransform, child: RigTransform) -> RigTransform {
    assert!(parent.scale.min_element() > 0.0 && child.scale.min_element() > 0.0);
    RigTransform {
        translation: arithmetic::rotated(
            parent.rotation.to_array(),
            parent.scale * child.translation,
        ) + parent.translation,
        rotation: multiply(parent.rotation, child.rotation),
        scale: parent.scale * child.scale,
    }
}

pub fn run(root: &Path) {
    let input: Value =
        serde_json::from_slice(&std::fs::read(root.join("native-input.json")).unwrap()).unwrap();
    let names = input["names"].as_array().unwrap();
    let markers =
        ["ik_foot_l", "ik_foot_r"].map(|name| names.iter().position(|v| v == name).unwrap());
    let parents = input["parents"].as_array().unwrap();
    let mut counters = [[0; 2]; 4];
    let mut outputs = Vec::new();
    for (index, row) in input["rows"].as_array().unwrap().iter().enumerate() {
        let world = local(&row["world"]);
        let modes = ["prior","current","raw","prior"].into_iter().enumerate().map(|(mode,key)|{
            let mut globals = Vec::new();
            for (bone, value) in row[key].as_array().unwrap().iter().enumerate() {
                let mut value = local(value);
                let parent = parents[bone].as_i64().unwrap();
                if parent >= 0 {
                    value = compose(globals[parent as usize], value);
                    if mode != 3 {
                        value.rotation = DQuat::from_array(arithmetic::normalized(value.rotation.to_array()));
                    }
                }
                globals.push(value);
            }
            markers.iter().enumerate().map(|(side,&marker)| {
                let value = compose(world, globals[marker]);
                let target = &row["expected"][side];
                let translation_exact = value.translation == dv(&target[0]);
                let rotation_exact = value.rotation == quat(&target[1]);
                counters[mode][0] += usize::from(translation_exact);
                counters[mode][1] += usize::from(rotation_exact);
                json!({"transform":value,"translation_exact":translation_exact,"rotation_exact":rotation_exact})
            }).collect::<Vec<_>>()
        }).collect::<Vec<_>>();
        outputs.push(json!({"frame":row["frame"],"valid":row["valid"],"expected":row["expected"],"modes":modes}));
        if index % 32 == 0 {
            println!("Replayed native socket sample {index}");
        }
    }
    let report = json!({"modes":["prior_published","current_published","prior_raw_root_after_initialization","prior_without_component_normalization"],
        "exact_t_q":counters,"rows":outputs});
    std::fs::write(
        root.join("native.json"),
        serde_json::to_vec(&report).unwrap(),
    )
    .unwrap();
    println!("Exact native socket T/Q rows: {counters:?}");
    assert!(!outputs[0]["valid"].as_bool().unwrap());
    assert!(outputs[1..].iter().all(|row| row["valid"] == true));
    for (index, row) in outputs.iter().enumerate() {
        let selected = usize::from(index == 0);
        assert!(
            row["modes"][selected]
                .as_array()
                .unwrap()
                .iter()
                .all(|side| side["translation_exact"] == true && side["rotation_exact"] == true)
        );
    }
    assert!(counters[1][1] < 634 && counters[2][1] < 634 && counters[3][1] < 634);
}
