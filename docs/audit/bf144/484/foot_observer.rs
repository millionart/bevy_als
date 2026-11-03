use bevy::ecs::system::SystemParam;
use bevy_als::als::foot_ik::{
    AlsFootBones, AlsFootLockFootState, AlsFootLockInput, AlsFootLockSettings, AlsFootLockState,
    AlsFootTargetSelection,
};
use bevy_als::control_rig_apply::AlsControlRigInput;
use bevy_als::foot_ik_apply::AlsFootLockTeleportTelemetry;

#[derive(SystemParam)]
struct FootObservation<'w, 's> {
    feet: Query<'w, 's, (
        &'static AlsFootBones,
        Option<&'static AlsFootTargetSelection>,
        &'static AlsFootLockState,
        &'static AlsFootLockInput,
        &'static AlsFootLockSettings,
        Option<&'static AlsControlRigInput>,
        Option<&'static AlsFootLockTeleportTelemetry>,
        Option<&'static bevy_als::als_camera::AlsCameraCustomTimeDilation>,
    )>,
    nodes: Query<'w, 's, (&'static Transform, Option<&'static ChildOf>, Option<&'static Name>)>,
}

fn foot_state_value(state: &AlsFootLockFootState) -> serde_json::Value {
    serde_json::json!({
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

fn observe_foot_feedback(
    observation: &FootObservation,
    before: bool,
    actor: Entity,
    avatar: Entity,
    player: &bevy_animation_graph::core::animation_graph_player::AnimationGraphPlayer,
    skeletons: &Assets<bevy_animation_graph::core::skeleton::Skeleton>,
    transforms: &bevy::transform::helper::TransformHelper,
    dt: f32,
) -> serde_json::Value {
    let Ok((bones, selection, state, input, settings, rig, teleport, dilation)) = observation.feet.get(actor) else {
        return serde_json::Value::Null;
    };
    let states = [foot_state_value(&state.left), foot_state_value(&state.right)];
    if !before {
        return serde_json::json!({"states":states,"valid":state.valid});
    }
    let mut mapped = std::collections::HashMap::new();
    if let Some(pose) = player.get_default_output_pose()
        && let Some(skeleton) = skeletons.get(&pose.skeleton)
    {
        for id in pose.paths.keys() {
            if let Some(entity) = player.bone_entity(*id)
                && let Some(path) = skeleton.id_to_path(*id)
            {
                mapped.insert(entity, path.parts.last().unwrap().as_str().to_owned());
            }
        }
    }
    let chain = |mut entity: Entity| {
        let global = transforms.compute_global_transform(entity).unwrap();
        let mut rows = Vec::new();
        loop {
            let (local, parent, name) = observation.nodes.get(entity).unwrap();
            rows.push(serde_json::json!({
                "bone":mapped.get(&entity),
                "name":name.map(Name::as_str),
                "translation":local.translation.to_array(),
                "rotation":local.rotation.to_array(),
                "scale":local.scale.to_array(),
            }));
            let Some(parent) = parent else {break;};
            entity = parent.parent();
        }
        serde_json::json!({"chain":rows,"translation":global.translation().to_array(),
                           "rotation":global.rotation().to_array(),
                           "matrix":global.to_matrix().to_cols_array()})
    };
    let use_ik = selection.map_or(true, |selection| selection.use_ik_bones);
    let pelvis = observation.nodes.get(bones.left_thigh).unwrap().1.unwrap().parent();
    serde_json::json!({
        "states":states,"valid":state.valid,"became_valid":state.became_valid,
        "use_ik":use_ik,"delta":dt*dilation.map_or(1.0,|value|value.0),
        "targets":[chain(bones.target(0,use_ik)),chain(bones.target(1,use_ik))],
        "armature":chain(avatar),"pelvis":chain(pelvis),
        "inputs":{"ik":[input.left_ik_amount,input.right_ik_amount],"lock":[input.left_lock_amount,input.right_lock_amount],
                  "moving_smooth":input.moving_smooth,"allow_foot_lock":settings.allow_foot_lock,
                  "thigh_limit":settings.thigh_angle_limit_degrees,"foot_limit":settings.foot_angle_limit_degrees},
        "teleport_active":teleport.is_some_and(|value|value.remaining_seconds>0.0),
        "rig":rig.map(|value|serde_json::json!({"valid":value.foot_transforms_valid,
            "location":[value.foot_left_location_armature.to_array(),value.foot_right_location_armature.to_array()],
            "rotation":[value.foot_left_rotation_armature.to_array(),value.foot_right_rotation_armature.to_array()]})),
    })
}
