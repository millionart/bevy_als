use std::time::{Duration, Instant};

use als_core::character::AlsCharacter;
use als_core::control_scheme::AlsControlSchemeConfig;
use als_core::motion::{AlsInputOverride, AlsViewYaw};
use als_core::params::AlsMovementParams;
use als_core::state::{Gait, OverlayState, RotationMode};
use als_core::transition::AlsStateRequest;
use avian3d::prelude::*;
use bevy::animation::AnimationTargetId;
use bevy::ecs::message::MessageWriter;
use bevy::prelude::*;
use bevy_animation_graph::core::animated_scene::AnimatedSceneInstance;
use bevy_animation_graph::core::animation_graph_player::AnimationGraphPlayer;

use bevy_als::animation_context_projection::AlsRootAnimationContext;
use bevy_als::control_rig_apply::{
    AlsControlRigExecution, AlsControlRigInput, AlsControlRigPropagationSet,
    AlsControlRigState,
};
use bevy_als::foot_ik_apply::AlsFootIkSpaceState;
use bevy_als::playable_character::{
    AlsCharacterAvatar, AlsCharacterBlueprint, AlsPlayableCharacterPlugin, spawn_als_character,
};

const DRIVE_VIEW_YAW: f32 = 60.0_f32.to_radians();

#[derive(Resource)]
struct Deadline(Instant);

#[derive(Resource, Default)]
struct GateState {
    character: Option<Entity>,
    baseline_spine: Option<Quat>,
    baseline_foot_scale: Option<Vec3>,
    driven: bool,
    complete: bool,
    observations: u32,
    max_spine_yaw: f32,
    stage_complete: bool,
    input_ready: bool,
    foot_targets_ready: bool,
}

fn main() {
    let mut app = App::new();
    app.add_plugins((
        DefaultPlugins.set(AssetPlugin {
            file_path: "assets".into(),
            ..default()
        }),
        AlsPlayableCharacterPlugin,
    ))
    .init_resource::<GateState>()
    .insert_resource(bevy_als::foot_ik_apply::AlsFootIkTelemetryEnabled(true))
    .insert_resource(Deadline(Instant::now() + Duration::from_secs(12)))
    .add_systems(Startup, setup)
    .add_systems(Update, timeout)
    .add_systems(
        PostUpdate,
        (
            drive_canonical_character
                .after(avian3d::prelude::PhysicsSystems::Last)
                .before(als_ext::post_physics_animation::PostPhysicsAnimationSet::Inputs),
            observe.after(AlsControlRigPropagationSet),
        ),
    );
    app.run();
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut params_assets: ResMut<Assets<AlsMovementParams>>,
    mut config_assets: ResMut<Assets<AlsControlSchemeConfig>>,
    assets: Res<AssetServer>,
    mut state: ResMut<GateState>,
) {
    let ground_material = materials.add(Color::srgb(0.24, 0.27, 0.24));
    commands.spawn((
        RigidBody::Static,
        Collider::cuboid(60.0, 0.3, 60.0),
        Transform::from_xyz(0.0, -0.15, 0.0),
        Mesh3d(meshes.add(Mesh::from(Cuboid::new(60.0, 0.3, 60.0)))),
        MeshMaterial3d(ground_material),
    ));
    let character = spawn_als_character(
        &mut commands,
        &assets,
        &mut params_assets,
        &mut config_assets,
        AlsCharacterBlueprint::new(
            Transform::from_xyz(0.0, 0.95, 0.0)
                .with_rotation(Quat::from_rotation_y(std::f32::consts::PI)),
        ),
    );
    commands
        .entity(character)
        .insert(AlsInputOverride::default());
    state.character = Some(character);
    info!("[CONTROL-RIG-CONTRACT] canonical AB_Als character spawned");
}

fn drive_canonical_character(
    mut state: ResMut<GateState>,
    mut characters: Query<
        (
            Entity,
            &mut AlsInputOverride,
            &mut AlsStateRequest,
            &mut AlsViewYaw,
            &Transform,
        ),
        With<AlsCharacter>,
    >,
) {
    let Some(character) = state.character else {
        return;
    };
    let Ok((entity, mut input, mut request, mut view_yaw, transform)) =
        characters.get_mut(character)
    else {
        return;
    };
    if entity != character || state.baseline_spine.is_none() {
        return;
    }
    input.0.move_dir = Vec2::new(1.0, -1.0).normalize();
    input.0.sprint_held = false;
    input.0.aim_held = true;
    input.0.look_delta = Vec2::ZERO;
    request.gait = Some(Gait::Running);
    request.rotation_mode = Some(RotationMode::Aiming);
    request.overlay_state = Some(OverlayState::Rifle);
    let forward = transform.rotation * Vec3::NEG_Z;
    view_yaw.0 = f32::atan2(forward.x, -forward.z) + DRIVE_VIEW_YAW;
    state.driven = true;
}

fn output_f32(context: Option<&AlsRootAnimationContext>, name: &str) -> f32 {
    context
        .and_then(|context| context.f32(name))
        .unwrap_or_default()
}

fn named_bone_transform(
    root: Entity,
    name: &str,
    children_q: &Query<&Children>,
    names: &Query<&Name>,
    targets: &Query<(), With<AnimationTargetId>>,
    transforms: &Query<&Transform>,
) -> Option<Transform> {
    let mut stack = vec![root];
    while let Some(entity) = stack.pop() {
        let Ok(children) = children_q.get(entity) else {
            continue;
        };
        for child in children.iter() {
            if targets.get(child).is_ok()
                && names
                    .get(child)
                    .is_ok_and(|candidate| candidate.as_str() == name)
            {
                return transforms.get(child).ok().cloned();
            }
            stack.push(child);
        }
    }
    None
}

#[allow(clippy::type_complexity)]
fn observe(
    mut state: ResMut<GateState>,
    avatars: Query<(&ChildOf, &AnimatedSceneInstance), With<AlsCharacterAvatar>>,
    players: Query<(&AnimationGraphPlayer, Option<&AlsRootAnimationContext>)>,
    rigs: Query<&AlsControlRigState, With<AlsCharacter>>,
    constructions: Query<&AlsControlRigExecution, With<AlsCharacter>>,
    rig_inputs: Query<&AlsControlRigInput, With<AlsCharacter>>,
    foot_spaces: Query<&AlsFootIkSpaceState, With<AlsCharacter>>,
    children_q: Query<&Children>,
    names: Query<&Name>,
    targets: Query<(), With<AnimationTargetId>>,
    transforms: Query<&Transform>,
    mut exit: MessageWriter<AppExit>,
) {
    if state.complete {
        return;
    }
    let Some(character) = state.character else {
        return;
    };
    let Some((_, instance)) = avatars
        .iter()
        .find(|(parent, _)| parent.parent() == character)
    else {
        return;
    };
    let Ok((_, context)) = players.get(instance.player_entity()) else {
        return;
    };
    let Some(spine) = named_bone_transform(
        character,
        "spine_03",
        &children_q,
        &names,
        &targets,
        &transforms,
    ) else {
        return;
    };
    let Some(foot_root) = named_bone_transform(
        character,
        "ik_foot_root",
        &children_q,
        &names,
        &targets,
        &transforms,
    ) else {
        return;
    };
    if state.baseline_spine.is_none() {
        state.baseline_spine = Some(spine.rotation);
        state.baseline_foot_scale = Some(foot_root.scale);
        info!("[CONTROL-RIG-CONTRACT] baseline canonical skeleton ready");
        return;
    }
    if !state.driven {
        return;
    }
    let Ok(rig) = rigs.get(character) else {
        return;
    };
    let stage = constructions.get(character).ok();
    let construction = constructions.get(character).ok();
    let rig_input = rig_inputs.get(character).ok();
    let foot_space = foot_spaces.get(character).ok();
    let pose_moving = output_f32(context, "PoseMoving");
    let pose_aiming = output_f32(context, "PoseAiming");
    let hand_left_weight = output_f32(context, "LayerArmLeft").clamp(0.0, 1.0)
        * output_f32(context, "HandLeftIk").clamp(0.0, 1.0);
    let hand_right_weight = output_f32(context, "LayerArmRight").clamp(0.0, 1.0)
        * output_f32(context, "HandRightIk").clamp(0.0, 1.0);
    let baseline_spine = state.baseline_spine.unwrap_or(Quat::IDENTITY);
    let baseline_foot_scale = state.baseline_foot_scale.unwrap_or(Vec3::ONE);
    let spine_changed = baseline_spine.angle_between(spine.rotation) > 0.01;
    let diagonal_scaled = foot_root.scale.x > baseline_foot_scale.x + 0.01;
    let curves_ready = pose_moving > 0.01 && pose_aiming > 0.01;
    state.max_spine_yaw = state.max_spine_yaw.max(rig.final_yaw_angle.abs());
    let state_ready = rig.spine_rotation_allowed && state.max_spine_yaw > 1.0;
    state.stage_complete |= stage.is_some_and(|stage| {
        stage.initialization_count == 1
            && stage.executed_steps > 0
            && stage.frame > 0
    });
    let construction_ready = construction.is_some_and(|construction| {
        construction.foot_height_m.is_finite()
            && construction.foot_height_m > 0.0
            && construction.leg_length_m.is_finite()
            && construction.leg_length_m > 0.0
    });
    state.input_ready |= rig_input.is_some_and(|input| {
        input.use_hand_ik_bones
            && input.use_foot_ik_bones
            && input.velocity_blend_forward_amount.is_finite()
            && input.velocity_blend_backward_amount.is_finite()
            && input.spine_yaw_angle_degrees.is_finite()
            && input.pelvis_offset_amount.is_finite()
            && input.foot_left_location_armature.is_finite()
            && input.foot_right_location_armature.is_finite()
            && input.foot_left_rotation_armature.is_finite()
            && input.foot_right_rotation_armature.is_finite()
    });
    state.foot_targets_ready |= foot_space.is_some_and(|space| {
        space.left.target_armature_local.is_finite()
            && space.right.target_armature_local.is_finite()
            && space.left.normal_armature_local.is_finite()
            && space.right.normal_armature_local.is_finite()
    });
    state.observations += 1;
    if state.observations % 60 == 0 {
        info!(
            "[CONTROL-RIG-CONTRACT][PROBE] pose_moving={pose_moving:.3} pose_aiming={pose_aiming:.3} spine_allowed={} spine_yaw={:.3} spine_peak={:.3} spine_changed={spine_changed} diagonal_scaled={diagonal_scaled} foot_scale=({:.3},{:.3},{:.3}) hand_left={hand_left_weight:.3} hand_right={hand_right_weight:.3} construction_complete={} construction_count={} vm_steps={} graph_frame={} construction_ready={} input_ready={} foot_targets_ready={}",
            rig.spine_rotation_allowed,
            rig.final_yaw_angle,
            state.max_spine_yaw,
            foot_root.scale.x,
            foot_root.scale.y,
            foot_root.scale.z,
            stage.is_some_and(|stage| stage.initialization_count > 0),
            stage.map_or(0, |stage| stage.initialization_count),
            stage.map_or(0, |stage| stage.executed_steps),
            stage.map_or(0, |stage| stage.frame),
            construction_ready,
            state.input_ready,
            state.foot_targets_ready,
        );
    }
    if !(curves_ready
        && state_ready
        && spine_changed
        && diagonal_scaled
        && state.stage_complete
        && construction_ready
        && state.input_ready
        && state.foot_targets_ready)
    {
        return;
    }
    let finite = spine.rotation.is_finite()
        && foot_root.scale.is_finite()
        && rig.final_yaw_angle.is_finite();
    let pass = finite
        && hand_left_weight.is_finite()
        && hand_right_weight.is_finite()
        && state.stage_complete
        && construction_ready
        && state.input_ready
        && state.foot_targets_ready;
    info!(
        "[CONTROL-RIG-CONTRACT] pose_moving={pose_moving:.3} pose_aiming={pose_aiming:.3} spine_yaw={:.3} spine_peak={:.3} spine_changed={spine_changed} diagonal_scaled={diagonal_scaled} hand_left={hand_left_weight:.3} hand_right={hand_right_weight:.3} stage_complete={} construction_ready={} input_ready={} foot_targets_ready={} finite={finite}",
        rig.final_yaw_angle,
        state.max_spine_yaw,
        state.stage_complete,
        construction_ready,
        state.input_ready,
        state.foot_targets_ready,
    );
    info!(
        "[CONTROL-RIG-CONTRACT][VERDICT] linked={} -> {}",
        pass,
        if pass { "PASS" } else { "FAIL" }
    );
    state.complete = true;
    if pass {
        exit.write(AppExit::Success);
    } else {
        exit.write(AppExit::Error(std::num::NonZeroU8::new(8).unwrap()));
    }
}

fn timeout(state: Res<GateState>, deadline: Res<Deadline>, mut exit: MessageWriter<AppExit>) {
    if !state.complete && Instant::now() >= deadline.0 {
        warn!("[CONTROL-RIG-CONTRACT][VERDICT] timeout -> FAIL");
        exit.write(AppExit::Error(std::num::NonZeroU8::new(9).unwrap()));
    }
}
