use bevy::{
    animation::AnimationTargetId,
    ecs::{schedule::ScheduleCleanupPolicy, system::SystemState},
    prelude::*,
    transform::helper::TransformHelper,
};
use bevy_als::{
    als::{
        character::AlsCharacter,
        foot_ik::{
            AlsDynamicTransitionState, AlsFootLockFootState, AlsFootLockInput,
            AlsFootLockSettings, AlsFootLockState, AlsFootTargetSelection, build_foot_bone_map,
        },
        state::{LocomotionState, Stance},
    },
    character_assembly::AlsCharacterAssembly,
    control_rig_apply::AlsControlRigInput,
    foot_ik_apply::{AlsFootLockTeleportTelemetry, FootIkApplyPlugin, FootIkApplySet},
};
use serde_json::{Value, json};
use std::time::Duration;

fn number(value: &Value) -> f32 {
    value.as_f64().unwrap() as f32
}

fn vector(value: &Value) -> Vec3 {
    Vec3::from_array(std::array::from_fn(|i| number(&value[i])))
}

fn quaternion(value: &Value) -> Quat {
    Quat::from_array(std::array::from_fn(|i| number(&value[i])))
}

fn transform(row: &Value) -> Transform {
    Transform {
        translation: vector(&row["translation"]),
        rotation: quaternion(&row["rotation"]),
        scale: vector(&row["scale"]),
    }
}

fn foot_state(row: &Value) -> AlsFootLockFootState {
    assert_eq!(row["base_present"], false);
    AlsFootLockFootState {
        lock_amount: number(&row["lock_amount"]),
        target_location: vector(&row["target_location"]),
        target_rotation: quaternion(&row["target_rotation"]),
        lock_location: vector(&row["lock_location"]),
        lock_rotation: quaternion(&row["lock_rotation"]),
        lock_rotation_before_foot_limit: quaternion(&row["lock_rotation_before_limit"]),
        final_location: vector(&row["final_location"]),
        final_rotation: quaternion(&row["final_rotation"]),
        ..default()
    }
}

fn foot_value(state: &AlsFootLockFootState) -> Value {
    json!({
        "lock_amount":state.lock_amount,
        "target_location":state.target_location.to_array(),
        "target_rotation":state.target_rotation.to_array(),
        "lock_location":state.lock_location.to_array(),
        "lock_rotation":state.lock_rotation.to_array(),
        "lock_rotation_before_limit":state.lock_rotation_before_foot_limit.to_array(),
        "final_location":state.final_location.to_array(),
        "final_rotation":state.final_rotation.to_array(),
        "base_present":state.movement_base.is_some(),
    })
}

fn rig_value(value: &AlsControlRigInput) -> Value {
    json!({"valid":value.foot_transforms_valid,
        "location":[value.foot_left_location_armature.to_array(),value.foot_right_location_armature.to_array()],
        "rotation":[value.foot_left_rotation_armature.to_array(),value.foot_right_rotation_armature.to_array()]})
}

fn world_value(value: GlobalTransform) -> Value {
    json!({"matrix":value.to_matrix().to_cols_array(),
           "translation":value.translation().to_array(),
           "rotation":value.rotation().to_array()})
}

fn run_case(row: &Value, names: &[&str], parents: &[Option<usize>], corrected: bool) -> Value {
    let observed = &row["observation"];
    let prior = &row["prior"];
    let config = &observed["inputs"];
    let first_valid = observed["became_valid"].as_bool().unwrap();
    let current_chain = &observed["armature"]["chain"];
    let prior_chain = &prior["armature"]["chain"];
    let start_chain = if first_valid { current_chain } else { prior_chain };

    let mut app = App::new();
    app.init_resource::<Time>().add_plugins(FootIkApplyPlugin);
    app.world_mut().schedule_scope(PostUpdate, |world, schedule| {
        assert_eq!(schedule.remove_systems_in_set(FootIkApplySet::Telemetry, world,
            ScheduleCleanupPolicy::RemoveSystemsOnly).unwrap(), 1);
    });
    let mut time = Time::<()>::default();
    time.advance_by(Duration::from_secs_f32(number(&observed["delta"])));
    assert_eq!(time.delta_secs(), number(&observed["delta"]));
    app.insert_resource(time);

    let actor = app.world_mut().spawn((
        AlsCharacter, transform(&start_chain[1]), LocomotionState::Grounded, Stance::Standing,
        AlsFootLockState::default(), AlsDynamicTransitionState::default(), AlsControlRigInput::default(),
        AlsFootTargetSelection { use_ik_bones: true },
        AlsFootLockInput {
            left_ik_amount: number(&config["ik"][0]),
            right_ik_amount: number(&config["ik"][1]),
            left_lock_amount: number(&config["lock"][0]),
            right_lock_amount: number(&config["lock"][1]),
            moving_smooth: config["moving_smooth"].as_bool().unwrap(),
            ..default()
        },
        AlsFootLockSettings {
            allow_foot_lock: config["allow_foot_lock"].as_bool().unwrap(),
            thigh_angle_limit_degrees: number(&config["thigh_limit"]),
            foot_angle_limit_degrees: number(&config["foot_limit"]),
            ..default()
        },
    )).id();
    let avatar = app.world_mut().spawn((transform(&start_chain[0]), ChildOf(actor))).id();
    app.world_mut().entity_mut(actor).insert(AlsCharacterAssembly { avatar, camera: None });

    let mut mesh_parent = avatar;
    for node in row["scene"].as_array().unwrap().iter().rev() {
        mesh_parent = app.world_mut().spawn((transform(node), ChildOf(mesh_parent))).id();
    }
    let mut entities = Vec::with_capacity(names.len());
    for (i,name) in names.iter().enumerate() {
        let parent = parents[i].map_or(mesh_parent, |index| entities[index]);
        let mut local = transform(&row["locals"][i]);
        if corrected && let Some(rotation) = row["corrections"].get(*name) {
            local.rotation = quaternion(rotation);
        }
        let name = Name::new((*name).to_owned());
        let id = AnimationTargetId::from_name(&name);
        entities.push(app.world_mut().spawn((name, id, local, ChildOf(parent))).id());
    }
    let mut mapping = SystemState::<(Query<&Children>,Query<&Name>,Query<&AnimationTargetId>)>::new(app.world_mut());
    let (children,names_query,targets) = mapping.get(app.world()).unwrap();
    let bones = build_foot_bone_map(avatar,&children,&names_query,&targets).unwrap();
    assert_eq!(bones.target(0,true),entities[names.iter().position(|name| *name=="ik_foot_l").unwrap()]);
    assert_eq!(bones.target(1,true),entities[names.iter().position(|name| *name=="ik_foot_r").unwrap()]);
    app.world_mut().entity_mut(actor).insert(bones);

    if !first_valid {
        let before = AlsFootLockState {
            valid: true,
            left: foot_state(&prior["states"][0]),
            right: foot_state(&prior["states"][1]),
            ..default()
        };
        app.world_mut().entity_mut(actor).insert(before);
        *app.world_mut().get_mut::<Transform>(actor).unwrap() = transform(&current_chain[1]);
        *app.world_mut().get_mut::<Transform>(avatar).unwrap() = transform(&current_chain[0]);
    }
    app.world_mut().run_schedule(PostUpdate);
    let state = *app.world().get::<AlsFootLockState>(actor).unwrap();
    let rig = *app.world().get::<AlsControlRigInput>(actor).unwrap();
    let teleport = app.world().get::<AlsFootLockTeleportTelemetry>(actor).unwrap();
    assert_eq!(teleport.detected_count,0);
    let mut helper = SystemState::<TransformHelper>::new(app.world_mut());
    let transforms = helper.get(app.world()).unwrap();
    let pelvis = app.world().get::<ChildOf>(bones.left_thigh).unwrap().parent();
    let compute = |entity|world_value(transforms.compute_global_transform(entity).unwrap());
    json!({"valid":state.valid,"became_valid":state.became_valid,"states":[foot_value(&state.left),foot_value(&state.right)],
        "rig":rig_value(&rig),"targets":[compute(bones.target(0,true)),compute(bones.target(1,true))],
        "armature":compute(avatar),"pelvis":compute(pelvis)})
}

fn main() {
    let root = std::path::Path::new("docs/audit/bf144/484");
    let label = std::env::args().nth(1).unwrap();
    let inputs: Value = serde_json::from_slice(&std::fs::read(root.join("inputs.json")).unwrap()).unwrap();
    let names = inputs["names"].as_array().unwrap().iter().map(|v|v.as_str().unwrap()).collect::<Vec<_>>();
    let parents = inputs["parents"].as_array().unwrap().iter().map(|v|v.as_u64().map(|i|i as usize)).collect::<Vec<_>>();
    let mut output = Vec::new();
    let mut failures = Vec::new();
    for row in inputs["rows"].as_array().unwrap() {
        let query = row["query"].as_u64().unwrap();
        if query%32 == 1 {
            println!("Replaying foot feedback frame {query}");
        }
        let baseline = run_case(row,&names,&parents,false);
        let observed = &row["observation"];
        for field in ["valid","became_valid","states","rig"] {
            if baseline[field] != observed[field] {
                failures.push(json!({"query":query,"field":field,"model":baseline[field],"observed":observed[field]}));
            }
        }
        for field in ["armature","pelvis","targets"] {
            let equal = if field=="targets" {
                (0..2).all(|side| baseline[field][side]["matrix"]==observed[field][side]["matrix"])
            } else {
                baseline[field]["matrix"]==observed[field]["matrix"]
            };
            if !equal {
                failures.push(json!({"query":query,"field":field,"model":baseline[field],"observed":observed[field]}));
            }
        }
        let corrected = run_case(row,&names,&parents,true);
        output.push(json!({"query":query,"prior_root":row["prior_root"],"baseline":baseline,"corrected":corrected}));
    }
    std::fs::write(root.join(format!("replay-unit-{label}.json")),
        serde_json::to_vec(&json!({"failures":failures,"rows":output})).unwrap()).unwrap();
    assert!(failures.is_empty(),"Public foot feedback must reproduce actual frame values");
    assert!(output.iter().any(|row| row["baseline"]["rig"] != row["corrected"]["rig"]), "The held-state D276 control must reach RigInput");
    println!("Qualified {} actual foot feedback frames and fixed-history D276 controls",output.len());
}
