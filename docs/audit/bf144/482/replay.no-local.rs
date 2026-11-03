use serde_json::{Value, json};

mod hierarchy {
    include!("../481/replay.rs");

    mod interpolation {
        include!("../473/replay.rs");
        pub fn blend(a: [f64; 4], b: [f64; 4], weight: f32) -> [f64; 4] {
            slerp(a, b, weight, 3)
        }
    }

    impl Tree {
        fn final_pose(&mut self) -> (Vec<RigTransform>, Vec<RigTransform>) {
            let locals = (0..self.nodes.len()).map(|i| self.final_local(i)).collect();
            let globals = (0..self.nodes.len()).map(|i| self.get(i, 2)).collect();
            (locals, globals)
        }

        fn final_local(&mut self, index: usize) -> RigTransform {
            self.nodes[index].local
        }
    }

    pub fn evaluate(row: &Value, parents: &[Option<usize>], source: bool, tolerance: f64) -> Value {
        let mut tree = Tree {
            nodes: row["before"].as_array().unwrap().iter().map(node).collect(),
            parents: parents.to_vec(), source, tolerance, writes: 0, global_evaluations: 0, marks: 0,
        };
        let foot = row["foot"].as_u64().unwrap() as usize;
        let before = tree.get(foot, 2);
        let weight = row["weight"].as_f64().unwrap() as f32;
        let decision = if weight < 1e-8 {
            "zero"
        } else {
            let requested: [f64; 4] = serde_json::from_value(row["rotation"].clone()).unwrap();
            let mut value = before;
            value.rotation = if weight < 1.0 {
                if source {
                    DQuat::from_array(interpolation::blend(before.rotation.to_array(), requested, weight))
                } else {
                    bevy_als::extensions::control_rig_hierarchy::rig_slerp(before.rotation, DQuat::from_array(requested), weight as f64)
                }
            } else {
                DQuat::from_array(requested)
            };
            if rig_quat_near(before.rotation, value.rotation, 1e-4_f32 as f64) {
                "equals"
            } else {
                tree.set(foot, value);
                "write"
            }
        };
        let foot_exact = tree.nodes[foot].global == transform(&row["expected_foot"]);
        let raw_exact = source || tree.nodes == row["expected"].as_array().unwrap().iter().map(node).collect::<Vec<_>>();
        let mut differences = Vec::new();
        let mut locals = Vec::new();
        let mut globals = Vec::new();
        let mut controls = Vec::new();
        if let Some(expected_local) = row["local"].as_array() {
            if source {
                for unit in [1.0, 0.01] {
                    let mut copy = Tree {
                        nodes: tree.nodes.iter().map(|node| Node {
                            local: runtime_axes(node.local, unit),
                            global: runtime_axes(node.global, unit),
                            dirty: node.dirty,
                        }).collect(),
                        parents: parents.to_vec(), source: false, tolerance,
                        writes: 0, global_evaluations: 0, marks: 0,
                    };
                    let (local, global) = copy.final_pose();
                    controls.push(json!({"unit":unit,"local":local,"global":global}));
                }
            }
            (locals, globals) = tree.final_pose();
            for (field, values, expected) in [("local", &locals, expected_local), ("global", &globals, row["globals"].as_array().unwrap())] {
                for (i, (value, native)) in values.iter().zip(expected).enumerate() {
                    let expected = transform(native);
                    if *value != expected {
                        differences.push(json!({"space":field,"index":i,"model":value,"observed":expected}));
                    }
                }
            }
        }
        json!({"decision":decision,"foot_exact":foot_exact,"public_raw_exact":(!source).then_some(raw_exact),"differences":differences,"local":locals,"global":globals,"controls":controls})
    }

    fn runtime_axes(value: RigTransform, unit: f64) -> RigTransform {
        let [x, y, z, w] = value.rotation.to_array();
        let vector = |v: bevy::math::DVec3| bevy::math::DVec3::new(v.x, v.z, v.y);
        RigTransform {
            rotation: DQuat::from_xyzw(-x, -z, -y, w),
            translation: vector(value.translation) * unit,
            scale: vector(value.scale),
        }
    }
}

fn main() {
    let label = std::env::args().nth(1).unwrap();
    let root = std::path::Path::new("docs/audit/bf144/482");
    let inputs: Value = serde_json::from_slice(&std::fs::read(root.join("inputs.json")).unwrap()).unwrap();
    let parents: Vec<_> = inputs["parents"].as_array().unwrap().iter().map(|v| v.as_u64().map(|i| i as usize)).collect();
    assert_eq!(parents.len(), 79);
    let mut failures = 0;
    let mut output = Vec::new();
    for domain in ["source", "current"] {
        let source = domain == "source";
        let tolerance = if source { 1e-4_f32 as f64 } else { inputs["tolerance"].as_f64().unwrap() };
        for row in inputs[domain].as_array().unwrap() {
            let result = hierarchy::evaluate(row, &parents, source, tolerance);
            if result["foot_exact"] != true || result["public_raw_exact"] == false || !result["differences"].as_array().unwrap().is_empty() {
                failures += 1;
            }
            output.push(json!({"domain":domain,"dataset":row["dataset"],"case":row["case"],"mode":row["mode"],"query":row["query"],"pc":row["pc"],"result":result}));
        }
    }
    std::fs::write(root.join(format!("replay-unit-{label}.json")), serde_json::to_vec(&json!({"failures":failures,"rows":output})).unwrap()).unwrap();
    assert_eq!(failures, 0, "Every setter and final pose consumer matches");
    println!("Qualified {} setter calls and complete final pose consumers", output.len());
}
