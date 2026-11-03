use bevy::{
    animation::AnimatedBy,
    ecs::schedule::ScheduleCleanupPolicy,
    prelude::*,
};
use bevy_als::{
    animation_context_projection::AlsRootAnimationContext,
    extensions::{
        animation_context::AnimationContextState,
        post_physics_animation::{AnimationPoseUpdateGate, PostPhysicsAnimationPlugin, PostPhysicsAnimationSet},
        source_pose_nodes::{blend_animation_contexts, blend_source_poses},
    },
};
use bevy_animation_graph::core::{
    animation_clip::EntityPath,
    animation_graph_player::{AnimationGraphPlayer, AnimationSource},
    id::BoneId,
    physics_systems_avian::{read_back_poses_avian, update_relative_kinematic_position_based_body_velocities},
    plugin::AnimationGraphSet,
    pose::{BonePose, Pose},
    systems::{animation_player, apply_animation_to_targets},
};
use serde_json::{Value, json};

mod native {
    include!("../471/replay.rs");
    pub fn normalized(q: [f64; 4]) -> [f64; 4] {
        normalize(q)
    }
}

fn quat64(value: &Value) -> [f64; 4] {
    std::array::from_fn(|index| value[index].as_f64().unwrap())
}

fn transform(value: &Value) -> Transform {
    let vector = |key: &str| Vec3::from_array(std::array::from_fn(|i| value[key][i].as_f64().unwrap() as f32));
    Transform {
        rotation: Quat::from_array(quat64(&value["rotation"]).map(|v| v as f32)),
        translation: vector("translation"),
        scale: vector("scale"),
    }
}

fn bone_id(value: &Value) -> BoneId {
    EntityPath {
        parts: value["path"].as_array().unwrap().iter()
            .map(|part| Name::new(part.as_str().unwrap().to_owned())).collect(),
    }.id()
}

fn pose(value: &Value) -> Pose {
    let mut result = Pose {
        timestamp: value["timestamp"].as_f64().unwrap() as f32,
        root_motion: serde_json::from_value(value["root_motion"].clone()).unwrap(),
        ..default()
    };
    for bone in value["bones"].as_array().unwrap() {
        let channel: BonePose = serde_json::from_value(bone.clone()).unwrap();
        let id = bone_id(bone);
        assert!(result.get_bone(id).is_none());
        result.add_bone(channel, id);
    }
    result
}

fn writer_app() -> App {
    let mut app = App::new();
    app.add_systems(PostUpdate, (
        animation_player, apply_animation_to_targets, read_back_poses_avian,
        update_relative_kinematic_position_based_body_velocities,
    ));
    PostPhysicsAnimationPlugin.finish(&mut app);
    app.world_mut().schedule_scope(PostUpdate, |world, schedule| {
        assert_eq!(schedule.remove_systems_in_set(AnimationGraphSet::PrePhysics, world,
            ScheduleCleanupPolicy::RemoveSystemsOnly).unwrap(), 1);
        assert_eq!(schedule.remove_systems_in_set(PostPhysicsAnimationSet::Readback, world,
            ScheduleCleanupPolicy::RemoveSystemsOnly).unwrap(), 3);
    });
    app
}

fn runtime_normalized(value: Quat) -> Quat {
    let [x,y,z,w] = value.to_array().map(f64::from);
    let [x,y,z,w] = native::normalized([-x,-z,-y,w]);
    Quat::from_xyzw(-x as f32, -z as f32, -y as f32, w as f32)
}

fn main() {
    let root = std::path::Path::new("docs/audit/bf144/483");
    let label = std::env::args().nth(1).unwrap();
    let inputs: Value = serde_json::from_slice(&std::fs::read(root.join("inputs.json")).unwrap()).unwrap();
    let source = inputs["source"].as_array().unwrap();
    let target = inputs["target"].as_array().unwrap();
    assert!(!source.is_empty() && !target.is_empty());
    let mut failures = Vec::new();
    let mut source_rows = Vec::new();
    for frame in source {
        let query = frame["query"].as_u64().unwrap();
        if query == 0 {
            continue;
        }
        let mut changed = 0;
        let mut f32_changed = 0;
        for (bone, root_bone) in frame["pose"].as_object().unwrap() {
            let mesh = &frame["mesh"][bone];
            let q = quat64(&root_bone["rotation"]);
            let normalized = native::normalized(q);
            if normalized != quat64(&mesh["rotation"]) || root_bone["translation"] != mesh["translation"] || root_bone["scale"] != mesh["scale"] {
                failures.push(json!({"stage":"source_mesh","query":query,"bone":bone,"expected":mesh,"normalized":normalized}));
            }
            changed += usize::from(q != normalized);
            f32_changed += usize::from(q.map(|v| v as f32) != normalized.map(|v| v as f32));
        }
        source_rows.push(json!({"query":query,"changed":changed,"f32_changed":f32_changed}));
    }

    let mut app = writer_app();
    let player = app.world_mut().spawn((AnimationGraphPlayer::default(), AnimationPoseUpdateGate {
        update_graph: false, refresh_bones: false, pending_update: false,
    })).id();
    let mut entities = std::collections::BTreeMap::new();
    for bone in target[0]["root"]["bones"].as_array().unwrap() {
        let id = bone_id(bone);
        let entity = app.world_mut().spawn((Transform::IDENTITY, id.animation_target_id(), AnimatedBy(player))).id();
        entities.insert(bone["name"].as_str().unwrap(), (id, entity));
    }
    let mut control = Vec::new();
    let mut target_rows = Vec::new();
    let mut previous = None;
    for frame in target {
        let query = frame["query"].as_u64().unwrap();
        let mut rig = pose(&frame["input"]);
        let mut mapped = 0;
        for bone in frame["input"]["bones"].as_array().unwrap() {
            let name = bone["name"].as_str().unwrap();
            if let Some(value) = frame["solved"].get(name) {
                let id = bone_id(bone);
                let index = rig.paths[&id];
                let mut output = BonePose::from_transform(transform(value));
                output.weights = rig.bones[index].weights.clone();
                rig.bones[index] = output;
                mapped += 1;
            }
        }
        assert_eq!(mapped, 79);
        if rig != pose(&frame["rig"]) {
            failures.push(json!({"stage":"rig_output","query":query}));
        }
        let weight = frame["ragdoll_weight"].as_f64().unwrap() as f32;
        let model = blend_source_poses(&[(rig, weight)]);
        let expected = pose(&frame["root"]);
        if model.timestamp != expected.timestamp
            || model.root_motion != expected.root_motion
            || model.paths.len() != expected.paths.len()
            || expected.paths.keys().any(|id| model.get_bone(*id) != expected.get_bone(*id))
        {
            failures.push(json!({"stage":"target_root","query":query}));
        }
        let mut context = AnimationContextState::default();
        context.curves = frame["curves"].as_object().unwrap().iter().map(|(key,value)|
            (key.as_str().into(), value.as_f64().unwrap() as f32)).collect();
        let projected = AlsRootAnimationContext { curves: blend_animation_contexts([(context, weight)]).curves.into_iter().collect() };
        let feedback = &frame["feedback"];
        let current: Option<&AlsRootAnimationContext> = previous.as_ref();
        let count = current.map(|state| state.curves.len());
        let standing = current.and_then(|state| state.f32("PoseStanding"));
        let grounded = current.and_then(|state| state.f32("PoseGrounded"));
        if json!(count) != feedback["projected_curve_count"]
            || json!(standing) != feedback["projected_standing"]
            || json!(grounded) != feedback["projected_grounded"]
            || standing.unwrap_or(0.0) != feedback["global_standing"].as_f64().unwrap() as f32
            || grounded.unwrap_or(0.0) != feedback["global_grounded"].as_f64().unwrap() as f32
        {
            failures.push(json!({"stage":"target_prior_curves","query":query,"predicted":[json!(count),json!(standing),json!(grounded)],"observed":feedback}));
        }
        previous = Some(projected);

        app.world_mut().get_mut::<AnimationGraphPlayer>(player).unwrap().set_animation(AnimationSource::Pose(model.clone()));
        if target_rows.is_empty() {
            app.world_mut().run_schedule(PostUpdate);
            assert!(entities.values().all(|(_, e)| app.world().get::<Transform>(*e).unwrap() == &Transform::IDENTITY));
            app.world_mut().get_mut::<AnimationPoseUpdateGate>(player).unwrap().refresh_bones = true;
        }
        app.world_mut().run_schedule(PostUpdate);
        let mut differences = 0;
        for (bone, (id, entity)) in &entities {
            let expected = model.get_bone(*id).unwrap().to_transform();
            let written = *app.world().get::<Transform>(*entity).unwrap();
            if written != expected {
                failures.push(json!({"stage":"public_writer","query":query,"bone":bone}));
            }
            if *bone == "SKM_Als" {
                continue;
            }
            let normalized = runtime_normalized(expected.rotation);
            if written.rotation != normalized {
                differences += 1;
                control.push(json!({"query":query,"bone":bone,"root_f32":expected.rotation.to_array(),"published":written.rotation.to_array(),"native_end_normalization":normalized.to_array()}));
            }
        }
        target_rows.push(json!({"query":query,"published_bones":entities.len(),"changed_with_native_final_normalization":differences,"prior_count":count,"prior_standing":standing,"prior_grounded":grounded}));
    }
    let output = json!({"failures":failures,"source":source_rows,"target":target_rows,"held_root_counterexamples":control});
    std::fs::write(root.join(format!("replay-unit-{label}.json")), serde_json::to_vec(&output).unwrap()).unwrap();
    assert!(failures.is_empty(), "Rig transport, native mesh, public writer or prior curve contract failed");
    assert!(!control.is_empty(), "Held Root input must distinguish the omitted normalization stage");
    println!("Qualified native final mesh and {} public Root publications with {} held-input differences", target_rows.len(), control.len());
}
