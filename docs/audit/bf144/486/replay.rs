use bevy::{
    ecs::system::SystemState,
    math::{DQuat, DVec3},
    prelude::*,
    transform::helper::TransformHelper,
};
use bevy_als::extensions::control_rig_hierarchy::RigTransform;
use serde_json::{Value, json};
use std::path::Path;

#[allow(dead_code)]
mod socket {
    include!("../485/native.rs");

    pub fn component(parent: RigTransform, child: RigTransform, normalize: bool) -> RigTransform {
        let mut output = compose(parent, child);
        if normalize {
            output.rotation = DQuat::from_array(arithmetic::normalized(output.rotation.to_array()));
        }
        output
    }

    pub fn project(parent: RigTransform, child: RigTransform) -> RigTransform {
        compose(parent, child)
    }
}

fn dv(value: &Value) -> DVec3 {
    DVec3::from_array(std::array::from_fn(|i| value[i].as_f64().unwrap()))
}

fn quat(value: &Value) -> DQuat {
    DQuat::from_array(std::array::from_fn(|i| value[i].as_f64().unwrap()))
}

fn local(value: &Value) -> RigTransform {
    RigTransform {
        translation: dv(&value["translation"]),
        rotation: quat(&value["rotation"]),
        scale: dv(&value["scale"]),
    }
}

fn as_runtime(value: RigTransform) -> Transform {
    Transform {
        translation: value.translation.as_vec3(),
        rotation: value.rotation.as_quat(),
        scale: value.scale.as_vec3(),
    }
}

fn source_basis(translation: DVec3, rotation: DQuat, scale: DVec3) -> RigTransform {
    RigTransform {
        translation: DVec3::new(translation.x, translation.z, translation.y) * 100.0,
        rotation: DQuat::from_xyzw(-rotation.x, -rotation.z, -rotation.y, rotation.w),
        scale: DVec3::new(scale.x, scale.z, scale.y),
    }
}

fn output_rotation(q: DQuat) -> Quat {
    Quat::from_xyzw(-q.x as f32, -q.z as f32, -q.y as f32, q.w as f32)
}

fn tqs(chain: &[RigTransform], world: RigTransform, normalize: bool) -> RigTransform {
    let (&root, children) = chain.split_first().unwrap();
    let component = children.iter().fold(root, |parent, &child| {
        socket::component(parent, child, normalize)
    });
    socket::project(world, component)
}

fn socket_grid(
    world: RigTransform,
    published: &[RigTransform],
    comparison: &[RigTransform],
) -> [RigTransform; 4] {
    [
        tqs(published, world, true),
        tqs(published, world, false),
        tqs(comparison, world, true),
        tqs(comparison, world, false),
    ]
}

fn grid_record(grid: [RigTransform; 4]) -> Value {
    let exact = [(0, 1), (2, 3), (0, 2), (1, 3)].map(|(a, b)| {
        assert!(grid[a].rotation.dot(grid[b].rotation) > 0.0);
        let (a, b) = (grid[a], grid[b]);
        json!({
            "t64":a.translation!=b.translation,
            "q64":a.rotation!=b.rotation,
            "t32":(a.translation*0.01).as_vec3()!=(b.translation*0.01).as_vec3(),
            "q32":output_rotation(a.rotation)!=output_rotation(b.rotation)
        })
    });
    json!({"tqs":grid,"differing_fields":exact})
}

fn public_affine(chain: &[&Value]) -> GlobalTransform {
    let mut world = World::new();
    let mut entity = world.spawn(as_runtime(local(chain.last().unwrap()))).id();
    for row in chain.iter().rev().skip(1) {
        entity = world.spawn((as_runtime(local(row)), ChildOf(entity))).id();
    }
    let mut state = SystemState::<TransformHelper>::new(&mut world);
    state
        .get(&world)
        .unwrap()
        .compute_global_transform(entity)
        .unwrap()
}

fn bone_path(mut index: usize, parents: &[Option<usize>]) -> Vec<usize> {
    let mut chain = vec![index];
    while let Some(parent) = parents[index] {
        assert!(parent < index);
        index = parent;
        chain.push(index);
    }
    chain.reverse();
    chain
}

fn read(path: &Path) -> Value {
    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
}

fn main() {
    let temporary = Path::new("tools/tmp/bf144/486");
    let root = Path::new("docs/audit/bf144/486");
    let label = std::env::args().nth(1).unwrap();
    let source = read(&temporary.join("source.json"));
    let target = read(&temporary.join("target.json"));
    let chains = |data: &Value| {
        let names = data["names"].as_array().unwrap();
        let parents: Vec<_> = data["parents"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_u64().map(|i| i as usize))
            .collect();
        ["ik_foot_l", "ik_foot_r"].map(|name| {
            let index = names.iter().position(|v| v == name).unwrap();
            let path = bone_path(index, &parents);
            assert_eq!(
                path.iter()
                    .map(|&i| names[i].as_str().unwrap())
                    .collect::<Vec<_>>(),
                ["root", "ik_foot_root", name]
            );
            path
        })
    };
    let source_chains = chains(&source);
    let target_chains = chains(&target);
    let names = target["names"].as_array().unwrap();

    let mut source_rows = Vec::new();
    let mut source_misses = Vec::new();
    for row in source["rows"].as_array().unwrap().iter().skip(1) {
        assert_eq!(row["valid"], true);
        let world = local(&row["world"]);
        for (side, path) in source_chains.iter().enumerate() {
            let published = path
                .iter()
                .map(|&i| local(&row["prior"][i]))
                .collect::<Vec<_>>();
            let raw = path
                .iter()
                .map(|&i| local(&row["raw"][i]))
                .collect::<Vec<_>>();
            let grid = socket_grid(world, &published, &raw);
            let expected = &row["expected"][side];
            if grid[0].translation != dv(&expected[0]) || grid[0].rotation != quat(&expected[1]) {
                source_misses.push(json!([row["frame"], side]));
            }
            source_rows.push(json!({"frame":row["frame"],"side":side,"grid":grid_record(grid)}));
        }
        if row["frame"].as_u64().unwrap() % 32 == 0 {
            println!("Replayed source mesh publication grid at {}", row["frame"]);
        }
    }

    let mut target_rows = Vec::new();
    for row in target["rows"].as_array().unwrap() {
        let observed = &row["observation"];
        for (side, path) in target_chains.iter().enumerate() {
            let expected = &observed["targets"][side];
            let leaf_chain = expected["chain"]
                .as_array()
                .unwrap()
                .iter()
                .collect::<Vec<_>>();
            assert_eq!(leaf_chain.len(), 7);
            let affine = public_affine(&leaf_chain);
            assert_eq!(
                json!(affine.to_matrix().to_cols_array()),
                expected["matrix"]
            );
            assert_eq!(json!(affine.rotation().to_array()), expected["rotation"]);
            assert_eq!(
                json!(affine.rotation().to_array()),
                observed["states"][side]["target_rotation"]
            );
            assert_eq!(
                json!(affine.translation().to_array()),
                observed["states"][side]["target_location"]
            );
            for external in &leaf_chain[3..5] {
                assert_eq!(as_runtime(local(external)), Transform::IDENTITY);
            }
            let world = public_affine(&leaf_chain[3..]);
            assert_eq!(
                json!(world.to_matrix().to_cols_array()),
                observed["armature"]["matrix"]
            );
            let (scale, rotation, translation) = world.to_scale_rotation_translation();
            let source_world = source_basis(
                translation.as_dvec3(),
                rotation.as_dquat(),
                scale.as_dvec3(),
            );
            let mut current = Vec::new();
            let mut normalized_publication = Vec::new();
            for (&index, node) in path.iter().zip(leaf_chain[..3].iter().rev()) {
                let original = local(&row["locals"][index]);
                assert_eq!(node["bone"], names[index]);
                assert_eq!(local(node), original);
                let mut corrected = original;
                if let Some(rotation) = row["corrections"].get(names[index].as_str().unwrap()) {
                    corrected.rotation = quat(rotation);
                }
                current.push(source_basis(
                    original.translation,
                    original.rotation,
                    original.scale,
                ));
                normalized_publication.push(source_basis(
                    corrected.translation,
                    corrected.rotation,
                    corrected.scale,
                ));
            }
            let grid = socket_grid(source_world, &current, &normalized_publication);
            target_rows.push(json!({"query":row["query"],"side":side,"actual_q":affine.rotation().to_array(),
                                    "actual_t":affine.translation().to_array(),"grid":grid_record(grid)}));
        }
        if row["query"].as_u64().unwrap() % 32 == 1 {
            println!(
                "Replayed actual affine and paired TQS grid at {}",
                row["query"]
            );
        }
    }
    assert_eq!((source_rows.len(), target_rows.len()), (634, 636));
    let report = json!({"source_native_misses":source_misses,"source":source_rows,"target":target_rows,
                        "grid_modes":["published_with_component_normalization","published_without_component_normalization",
                                      "other_publication_with_component_normalization","other_publication_without_component_normalization"]});
    std::fs::write(
        root.join(format!("replay-{label}.json")),
        serde_json::to_vec(&report).unwrap(),
    )
    .unwrap();
    assert!(
        source_misses.is_empty(),
        "Native socket baseline requires per-bone component normalization"
    );
    assert!(
        target_rows
            .iter()
            .any(|r| r["grid"]["differing_fields"][2]["q32"] == true)
    );
    println!("Verified 634 source sockets and 636 actual target affine samples");
}
