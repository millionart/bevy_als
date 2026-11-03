use bevy::prelude::*;
use bevy_als::{
    als::{
        foot_ik::{
            AlsFootLockFootState, AlsFootLockInput, AlsFootLockMovementBase, AlsFootLockSettings,
            AlsFootLockState, capture_foot_lock_movement_base, constrain_foot_lock_rotation,
            foot_lock_location_constraint_rotation, is_relevant_animation_weight,
            refresh_foot_lock, reset_foot_lock_to_target,
        },
        state::LocomotionState,
    },
    control_rig_apply::AlsControlRigInput,
    extensions::ik_space::{
        ArmaturePoint, ArmatureRotation, WorldPoint, WorldRotation, armature_point_from_world,
        armature_rotation_from_world, world_point_from_armature, world_rotation_from_armature,
    },
};
use serde_json::{Value, json};
use std::path::Path;

mod als {
    pub use bevy_als::als::foot_ik;
}

include!("../../../../tools/tmp/bf144/487/production.rs");
include!("../../../../tools/tmp/bf144/487/foot_types.rs");
mod vm {
    include!("../../../../tools/tmp/bf144/487/vm.rs");
}

fn read(path: &Path) -> Value {
    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
}

fn write(path: &Path, value: Value) {
    std::fs::write(path, serde_json::to_vec(&value).unwrap()).unwrap();
}

fn world(value: &Value) -> GlobalTransform {
    GlobalTransform::from(Mat4::from_cols_array(&std::array::from_fn(|i| {
        number(&value["matrix"][i])
    })))
}

fn project(q: &Value, actual: Quat) -> Quat {
    let q = quaternion(q);
    let runtime = Quat::from_xyzw(-q.x, -q.z, -q.y, q.w);
    assert!(runtime.dot(actual).is_finite());
    if runtime.dot(actual) < 0.0 {
        -runtime
    } else {
        runtime
    }
}

fn foot_case(row: &Value, axes: [Vec3; 2], rotations: [Quat; 2]) -> Value {
    let observed = &row["observation"];
    let config = &observed["inputs"];
    let became_valid = observed["became_valid"].as_bool().unwrap();
    let armature = world(&observed["armature"]);
    let pelvis = world(&observed["pelvis"]);
    let current = armature.compute_transform();
    let prior = world(&row["prior"]["armature"]).compute_transform();
    let settings = AlsFootLockSettings {
        allow_foot_lock: config["allow_foot_lock"].as_bool().unwrap(),
        thigh_angle_limit_degrees: number(&config["thigh_limit"]),
        foot_angle_limit_degrees: number(&config["foot_limit"]),
        ..default()
    };
    let lock_input = AlsFootLockInput {
        moving_smooth: config["moving_smooth"].as_bool().unwrap(),
        ..default()
    };
    let mut sides = [AlsFootLockFootState::default(); 2];
    for (side, state) in sides.iter_mut().enumerate() {
        if !became_valid {
            *state = foot_state(&row["prior"]["states"][side]);
            refresh_previous_foot_lock_transform(state, &prior, &current, false);
        }
        consume_target(
            state,
            vector(&observed["states"][side]["target_location"]),
            rotations[side],
            number(&config["ik"][side]).clamp(0.0, 1.0),
            number(&config["lock"][side]).clamp(0.0, 1.0),
            &settings,
            &lock_input,
            &LocomotionState::Grounded,
            became_valid,
            number(&observed["delta"]),
            None,
        );
        source_constrain_foot_lock(state, &armature, &pelvis, axes[side], &settings);
    }
    let lock = AlsFootLockState {
        valid: true,
        became_valid,
        left: sides[0],
        right: sides[1],
        ..default()
    };
    let mut rig = AlsControlRigInput::default();
    publish_inputs(&mut rig, armature, &lock);
    json!({"states":sides.iter().map(foot_value).collect::<Vec<_>>(),"rig":rig_value(&rig)})
}

fn main() {
    let root = Path::new("docs/audit/bf144/487");
    let temporary = Path::new("tools/tmp/bf144/487");
    let label = std::env::args().nth(1).unwrap();
    let inputs = read(&temporary.join("foot-input.json"));
    let grid = read(&temporary.join("grid.json"));
    let profile = read(Path::new("assets/ALS/Character/CR_Als.runtime.json"));
    let axes = [
        "foot_left_thigh_axis_pelvis_space",
        "foot_right_thigh_axis_pelvis_space",
    ]
    .map(|name| vector(&profile["target"][name]));
    let mut rows = Vec::new();
    let mut replacement_rows = [Vec::new(), Vec::new()];
    let mut rig_rotation_differences = 0;
    for (index, row) in inputs["rows"].as_array().unwrap().iter().enumerate() {
        let observed = &row["observation"];
        let query = row["query"].as_u64().unwrap();
        assert_eq!(query, index as u64 + 1);
        assert_eq!(observed["valid"], true);
        assert_eq!(observed["use_ik"], true);
        assert_eq!(observed["teleport_active"], false);
        let actual = [0, 1].map(|side| quaternion(&observed["states"][side]["target_rotation"]));
        let baseline = foot_case(row, axes, actual);
        assert_eq!(
            baseline["states"], observed["states"],
            "Actual foot state query {query}"
        );
        assert_eq!(
            baseline["rig"], observed["rig"],
            "Actual RigInput query {query}"
        );
        let paired = [1, 0].map(|mode| {
            [0, 1].map(|side| {
                let input = &grid["target"][index * 2 + side];
                assert_eq!(input["query"], query);
                assert_eq!(input["side"], side);
                assert_eq!(quaternion(&input["actual_q"]), actual[side]);
                project(&input["grid"]["tqs"][mode]["rotation"], actual[side])
            })
        });
        let off = foot_case(row, axes, paired[0]);
        let on = foot_case(row, axes, paired[1]);
        for (i, control) in [&off, &on].into_iter().enumerate() {
            assert_eq!(control["rig"]["location"], baseline["rig"]["location"]);
            replacement_rows[i]
                .push(json!({"query":query,"baseline":baseline["rig"],"corrected":control["rig"]}));
        }
        rig_rotation_differences += (0..2)
            .filter(|&side| off["rig"]["rotation"][side] != on["rig"]["rotation"][side])
            .count();
        rows.push(json!({"query":query,"baseline":baseline,"off":off,"on":on}));
        if index % 32 == 0 {
            println!("Replayed actual and paired foot consumers at query {query}");
        }
    }
    assert_eq!(rows.len(), 318);
    write(
        &root.join(format!("foot-{label}.json")),
        json!({"rows":rows,"rig_rotation_differences":rig_rotation_differences}),
    );
    assert!(
        rig_rotation_differences > 0,
        "The paired component normalization control must reach RigInput"
    );
    for (mode, inputs) in ["off", "on"].into_iter().zip(replacement_rows) {
        write(
            &temporary.join(format!("vm-input-{label}-{mode}.json")),
            json!(inputs),
        );
        vm::run(&temporary, &label, mode);
        let name = format!("vm-{label}-{mode}.json");
        std::fs::rename(temporary.join(&name), root.join(name)).unwrap();
    }
}
