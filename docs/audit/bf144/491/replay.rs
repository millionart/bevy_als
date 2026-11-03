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

fn main() {
    let label = std::env::args().nth(1).unwrap();
    let root = Path::new("docs/audit/bf144/491");
    let input: Vec<Value> = serde_json::from_slice(
        &std::fs::read(format!("tools/tmp/bf144/491/socket-{label}.json")).unwrap(),
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
    assert_eq!(rows.len(), 32);
    std::fs::write(
        root.join(format!("socket-{label}.json")),
        serde_json::to_vec(&rows).unwrap(),
    )
    .unwrap();
    println!("Qualified 32 native socket phase and marker control rows");
}
