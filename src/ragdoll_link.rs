use als_core::character::{
    ALS_CHARACTER_CAPSULE_HALF_HEIGHT, ALS_CHARACTER_CAPSULE_RADIUS, ALS_CHARACTER_FLOOR_CLEARANCE,
    AlsCharacter, AlsCollisionLayerOwner, AlsCollisionLayerOwnership, AlsMovementParamsHandle,
    set_collision_layer_owner,
};
use als_core::mantle::scaled_mantle_capsule_dimensions;
use als_core::motion::AlsRagdollAirExit;
use als_core::params::AlsMovementParams;
use als_core::state::{AlsNotifyLocomotionAction, LocomotionState, Stance};
use als_core::transition::{Dimension, StateTransitionEvent, StateValue};
use als_ext::ragdoll_angular_drive::{
    RagdollAngularDrive, RagdollAngularDriveMetrics, RagdollAngularDriveSet,
};
use als_ext::{RagdollActiveMode, RagdollPhysicsHandoffSet, RagdollPhysicsProfileApplied};
use avian3d::prelude::{
    AngularVelocity, Collider, CollisionLayers, LinearVelocity, PhysicsSystems, Position,
    RigidBody, Rotation, ShapeCastConfig, SpatialQuery, SpatialQueryFilter,
};
use bevy::ecs::schedule::ApplyDeferred;
use bevy::prelude::*;
use bevy::transform::helper::TransformHelper;
use bevy_animation_graph::core::animated_scene::{
    AnimatedScene, AnimatedSceneHandle, AnimatedSceneInstance,
};
use bevy_animation_graph::core::animation_graph_player::AnimationGraphPlayer;
use bevy_animation_graph::core::edge_data::DataValue;
use bevy_animation_graph::core::plugin::AnimationGraphSet;
use bevy_animation_graph::core::pose::{BonePose, Pose};
use bevy_animation_graph::core::ragdoll::spawning::{RagdollColliderSpawned, RagdollSpawned, SpawnedRagdoll};
use bevy_animation_graph::core::ragdoll::{
    bone_mapping::RagdollBoneMap,
    definition::{BodyLabel, BodyMode},
    relative_kinematic_body::RelativeKinematicBody,
};
use bevy_animation_graph::core::skeleton::Skeleton;
use bevy_tnua::TnuaToggle;

use crate::character_assembly::AlsCharacterAvatar;

const PELVIS_LABEL: &str = "pelvis";
pub const RAGDOLL_REFERENCE_SPEED_METERS_PER_SECOND: f32 = 10.0;
pub const RAGDOLL_ANGULAR_STIFFNESS: f32 = 25_000.0;

#[derive(Component, Debug, Default, Clone, Copy)]
pub struct AlsRagdollLink {
    pub prev_ragdoll: bool,
}

#[derive(Component, Debug, Clone, Copy)]
struct AlsRagdollColliderLayers(CollisionLayers);

#[derive(Component, Debug, Default, Clone, Copy)]
struct AlsRagdollPhysicsActive;

#[derive(Component, Debug, Clone, Copy)]
struct AlsRagdollCharacterPhysics {
    rigid_body: RigidBody,
    tnua_toggle: Option<TnuaToggle>,
}

#[derive(Component, Debug, Clone, Copy)]
pub(crate) struct AlsRagdollDetachedPlayer {
    parent: Entity,
    local_transform: Transform,
}

#[derive(Component, Debug, Clone, Copy)]
struct AlsRagdollInitialSpeedLimit {
    remaining_updates: u8,
    speed: f32,
}

impl AlsRagdollInitialSpeedLimit {
    fn new(entry_speed: f32) -> Self {
        Self {
            remaining_updates: crate::get_up::RAGDOLL_SETTINGS.initial_speed_limit_refresh_frames
                + 1,
            speed: entry_speed.max(crate::get_up::RAGDOLL_SETTINGS.minimum_initial_speed_limit_mps),
        }
    }

    fn next_limit(&mut self) -> Option<f32> {
        if self.remaining_updates == 0 {
            return None;
        }
        self.remaining_updates -= 1;
        Some(self.speed)
    }
}

#[derive(Component, Debug, Clone, Copy, Default)]
pub struct AlsRagdollGroundFollow {
    pub pelvis: Vec3,
    pub target: Vec3,
    pub velocity: Vec3,
    pub grounded: bool,
}

#[derive(Component, Debug, Clone, Copy, Default, PartialEq)]
pub struct AlsRagdollJointDrive {
    pub speed_amount: f32,
    pub angular_stiffness: f32,
    pub angular_damping: f32,
    pub angular_force_limit: f32,
    pub target_joint_count: u32,
    pub driven_joint_count: u32,
}

pub fn ragdoll_joint_drive_for_speed(speed_meters_per_second: f32) -> AlsRagdollJointDrive {
    let speed_amount = (speed_meters_per_second
        / crate::get_up::RAGDOLL_SETTINGS.reference_speed_mps)
        .clamp(0.0, 1.0);
    AlsRagdollJointDrive {
        speed_amount,
        angular_stiffness: speed_amount * crate::get_up::RAGDOLL_SETTINGS.angular_stiffness,
        angular_damping: 0.0,
        angular_force_limit: 0.0,
        target_joint_count: 0,
        driven_joint_count: 0,
    }
}

pub fn ragdoll_exit_route(grounded: bool, ragdoll_velocity: Vec3) -> (LocomotionState, Vec3) {
    if grounded {
        (LocomotionState::Grounded, Vec3::ZERO)
    } else {
        (LocomotionState::Air, ragdoll_velocity)
    }
}

pub struct AlsRagdollLinkPlugin;

impl Plugin for AlsRagdollLinkPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<StateTransitionEvent>()
            .add_observer(disable_spawned_ragdoll_collider)
            .add_observer(register_ragdoll_camera_trace_owner);
        app.add_systems(
            Update,
            (
                ensure_ragdoll_link,
                ragdoll_exit_route_system
                    .after(als_core::input_request::input_state_request_system)
                    .before(als_core::transition::state_transition_system),
                capture_ragdoll_player_world_transform
                    .after(als_core::transition::state_transition_system),
                ragdoll_character_physics_system
                    .after(capture_ragdoll_player_world_transform)
                    .after(ensure_ragdoll_link),
                restore_ragdoll_player_parent.after(ragdoll_character_physics_system),
                ragdoll_link_system.after(ragdoll_character_physics_system),
            ),
        )
        .add_systems(
            PostUpdate,
            (
                capture_ragdoll_player_world_transform,
                ragdoll_character_physics_system,
                ragdoll_link_system,
            )
                .chain()
                .after(crate::mantle_drive::AlsMantleDriveSet)
                .before(crate::animation_events::AlsAnimationEventSet::Refresh)
                .before(RagdollPhysicsHandoffSet)
                .run_if(
                    |characters: Query<
                        Option<&AlsNotifyLocomotionAction>,
                        (With<AlsCharacter>, Without<AlsRagdollCharacterPhysics>),
                    >| {
                        characters
                            .iter()
                            .any(|action| action == Some(&AlsNotifyLocomotionAction::Ragdolling))
                    },
                ),
        )
        .add_systems(
            PostUpdate,
            (
                sync_ragdoll_body_modes,
                limit_initial_ragdoll_speed,
                ragdoll_joint_drive_adapter_system,
                ApplyDeferred,
            )
                .chain()
                .in_set(RagdollPhysicsHandoffSet)
                .before(RagdollAngularDriveSet),
        )
        .add_systems(
            PostUpdate,
            (
                release_ragdoll_air_exit_on_ground.after(PhysicsSystems::Last),
                ragdoll_ground_follow.after(PhysicsSystems::Last),
                ApplyDeferred,
                apply_ragdoll_ground_follow,
            )
                .chain()
                .before(AnimationGraphSet::PostPhysics),
        )
        .add_systems(
            PostUpdate,
            sync_animated_ragdoll_bodies
                .after(crate::control_rig_apply::AlsControlRigPropagationSet),
        );
    }
}

fn register_ragdoll_camera_trace_owner(
    event: On<RagdollSpawned>,
    avatars: Query<(&AlsCharacterAvatar, &AnimatedSceneInstance)>,
    mut commands: Commands,
) {
    if let Some((avatar, _)) = avatars.iter().find(|(_, scene)| scene.player_entity() == event.animation_player) {
        commands.entity(event.ragdoll).insert(als_camera::AlsCameraTraceOwner(avatar.character));
    }
}

fn disable_spawned_ragdoll_collider(
    event: On<RagdollColliderSpawned>,
    layers: Query<&CollisionLayers>,
    mut commands: Commands,
) {
    let collider = event.event().collider;
    let Ok(layer) = layers.get(collider) else {
        return;
    };
    commands
        .entity(collider)
        .insert((AlsRagdollColliderLayers(*layer), CollisionLayers::NONE));
}

fn ensure_ragdoll_link(
    mut commands: Commands,
    q: Query<
        Entity,
        (
            With<als_core::character::AlsCharacter>,
            Without<AlsRagdollLink>,
        ),
    >,
) {
    for entity in &q {
        commands.entity(entity).insert(AlsRagdollLink::default());
    }
}

fn find_body_by_label(
    spawned: &SpawnedRagdoll,
    labels: &Query<&BodyLabel>,
    label: &str,
) -> Option<Entity> {
    for (_body_id, entity) in spawned.bodies.iter() {
        if let Ok(bl) = labels.get(*entity)
            && bl.0 == label
        {
            return Some(*entity);
        }
    }
    None
}

fn animated_player_for_character(
    character: Entity,
    avatars: &Query<(&AlsCharacterAvatar, &AnimatedSceneInstance)>,
) -> Option<Entity> {
    avatars.iter().find_map(|(avatar, instance)| {
        (avatar.character == character).then_some(instance.player_entity())
    })
}

fn refresh_player_subtree_globals(
    transform_helper: &TransformHelper,
    player: Entity,
    children: &Query<&Children>,
    globals: &mut Query<&mut GlobalTransform>,
) -> Option<Transform> {
    let mut player_world = None;
    let mut pending = vec![player];
    while let Some(entity) = pending.pop() {
        let global = transform_helper.compute_global_transform(entity).ok()?;
        if entity == player {
            player_world = Some(global.compute_transform());
        }
        *globals.get_mut(entity).ok()? = global;
        if let Ok(children) = children.get(entity) {
            pending.extend(children.iter());
        }
    }
    player_world
}

fn capture_ragdoll_player_world_transform(
    mut commands: Commands,
    transform_helper: TransformHelper,
    characters: Query<
        (Entity, Option<&AlsNotifyLocomotionAction>),
        (With<AlsCharacter>, Without<AlsRagdollCharacterPhysics>),
    >,
    avatars: Query<(&AlsCharacterAvatar, &AnimatedSceneInstance)>,
    player_parents: Query<
        (&ChildOf, &Transform),
        (With<AnimationGraphPlayer>, Without<AlsCharacter>),
    >,
    detached_players: Query<(), With<AlsRagdollDetachedPlayer>>,
    children: Query<&Children>,
    mut globals: Query<&mut GlobalTransform>,
) {
    for (character, action) in &characters {
        if action != Some(&AlsNotifyLocomotionAction::Ragdolling) {
            continue;
        }
        let Some(player) = animated_player_for_character(character, &avatars) else {
            continue;
        };
        if detached_players.get(player).is_ok() {
            continue;
        }
        let Ok((parent, local_transform)) = player_parents.get(player) else {
            continue;
        };
        let Some(world_transform) =
            refresh_player_subtree_globals(&transform_helper, player, &children, &mut globals)
        else {
            continue;
        };
        commands.entity(player).remove::<ChildOf>().insert((
            world_transform,
            AlsRagdollDetachedPlayer {
                parent: parent.parent(),
                local_transform: *local_transform,
            },
        ));
    }
}

fn sync_ragdoll_body_modes(
    mut commands: Commands,
    characters: Query<(Entity, Option<&AlsNotifyLocomotionAction>), With<als_core::character::AlsCharacter>>,
    avatars: Query<(&AlsCharacterAvatar, &AnimatedSceneInstance)>,
    mut players: Query<(
        &AnimationGraphPlayer,
        Option<&mut RagdollPhysicsProfileApplied>,
    )>,
    mut bodies: Query<(
        Entity,
        &RigidBody,
        Option<&RagdollActiveMode>,
        &mut LinearVelocity,
        &mut AngularVelocity,
        &mut RelativeKinematicBody,
        Has<AlsRagdollPhysicsActive>,
    )>,
) {
    for (character, action) in &characters {
        let Some(instance) = avatars
            .iter()
            .find_map(|(avatar, instance)| (avatar.character == character).then_some(instance))
        else {
            continue;
        };
        let Ok((player, profile)) = players.get_mut(instance.player_entity()) else {
            continue;
        };
        let Some(spawned) = player.get_spawned_ragdoll() else {
            continue;
        };
        let ragdolling = action == Some(&AlsNotifyLocomotionAction::Ragdolling);
        let mut handoff = false;
        for body_entity in spawned.bodies.values() {
            let Ok((
                entity,
                mode,
                active_mode,
                mut linear_velocity,
                mut angular_velocity,
                mut relative_body,
                activated,
            )) = bodies.get_mut(*body_entity)
            else {
                continue;
            };
            if ragdolling {
                if !activated {
                    commands.entity(entity).insert(AlsRagdollPhysicsActive);
                    handoff = true;
                }
                let target_mode = match active_mode.map(|mode| mode.0) {
                    Some(BodyMode::Kinematic) => RigidBody::Kinematic,
                    Some(BodyMode::Dynamic) | None => RigidBody::Dynamic,
                };
                if *mode != target_mode {
                    commands.entity(entity).insert(target_mode);
                }
                continue;
            }
            linear_velocity.0 = Vec3::ZERO;
            angular_velocity.0 = Vec3::ZERO;
            relative_body.kinematic_linear_velocity = Vec3::ZERO;
            relative_body.kinematic_angular_velocity = Vec3::ZERO;
            if activated {
                commands.entity(entity).remove::<AlsRagdollPhysicsActive>();
            }
            if *mode != RigidBody::Kinematic {
                commands.entity(entity).insert(RigidBody::Kinematic);
            }
        }
        if handoff && let Some(mut profile) = profile {
            profile.handoff_locked_joint_anchor_pending = true;
        }
    }
}

fn sync_animated_ragdoll_bodies(
    time: Res<Time>,
    characters: Query<Option<&AlsNotifyLocomotionAction>, With<AlsCharacter>>,
    avatars: Query<(
        &AlsCharacterAvatar,
        &AnimatedSceneInstance,
        &AnimatedSceneHandle,
    )>,
    scenes: Res<Assets<AnimatedScene>>,
    maps: Res<Assets<RagdollBoneMap>>,
    players: Query<&AnimationGraphPlayer>,
    bones: Query<&GlobalTransform>,
    mut bodies: Query<(
        &mut Position,
        &mut Rotation,
        &mut LinearVelocity,
        &mut AngularVelocity,
    )>,
) {
    for (avatar, instance, handle) in &avatars {
        if characters
            .get(avatar.character)
            .is_ok_and(|state| state == Some(&AlsNotifyLocomotionAction::Ragdolling))
        {
            continue;
        }
        let Some(mapping) = scenes
            .get(&handle.handle)
            .and_then(|scene| scene.ragdoll_bone_map.as_ref())
            .and_then(|mapping| maps.get(mapping))
        else {
            continue;
        };
        let Ok(player) = players.get(instance.player_entity()) else {
            continue;
        };
        let Some(spawned) = player.get_spawned_ragdoll() else {
            continue;
        };
        for (id, entity) in &spawned.bodies {
            let Some(mapping) = mapping.bodies_from_bones.get(id) else {
                continue;
            };
            let Some(bone) = player.bone_entity(mapping.bone.bone.id()) else {
                continue;
            };
            let Ok(global) = bones.get(bone) else {
                continue;
            };
            let Ok((mut position, mut rotation, mut linear_velocity, mut angular_velocity)) =
                bodies.get_mut(*entity)
            else {
                continue;
            };
            let target = global.to_isometry() * mapping.bone.offset;
            let delta = time.delta_secs();
            if !position.is_added() && delta > 0.0 {
                linear_velocity.0 = (Vec3::from(target.translation) - position.0) / delta;
                let mut difference = (target.rotation * rotation.0.conjugate()).normalize();
                if difference.w < 0.0 {
                    difference = -difference;
                }
                angular_velocity.0 = difference.to_scaled_axis() / delta;
            } else {
                linear_velocity.0 = Vec3::ZERO;
                angular_velocity.0 = Vec3::ZERO;
            }
            position.0 = target.translation.into();
            rotation.0 = target.rotation;
        }
    }
}

fn ragdoll_pelvis_velocity(
    spawned: &SpawnedRagdoll,
    labels: &Query<&BodyLabel>,
    velocities: &Query<&LinearVelocity>,
) -> Option<Vec3> {
    let pelvis = find_body_by_label(spawned, labels, PELVIS_LABEL)?;
    velocities.get(pelvis).ok().map(|velocity| velocity.0)
}

fn ragdoll_joint_drive_adapter_system(
    mut commands: Commands,
    mut characters: Query<
        (Entity, Option<&AlsNotifyLocomotionAction>, Option<&mut AlsRagdollJointDrive>),
        With<AlsCharacter>,
    >,
    avatars: Query<(&AlsCharacterAvatar, &AnimatedSceneInstance)>,
    players: Query<(
        Entity,
        &AnimationGraphPlayer,
        Option<&RagdollAngularDriveMetrics>,
    )>,
    mut angular_drives: Query<&mut RagdollAngularDrive>,
    labels: Query<&BodyLabel>,
    velocities: Query<&LinearVelocity>,
) {
    for (character, action, current_drive) in &mut characters {
        let mut drive = AlsRagdollJointDrive::default();
        let player = avatars
            .iter()
            .find_map(|(avatar, instance)| (avatar.character == character).then_some(instance))
            .and_then(|instance| players.get(instance.player_entity()).ok());
        if let Some((player_entity, player, metrics)) = player {
            if action == Some(&AlsNotifyLocomotionAction::Ragdolling) {
                let speed = player
                    .get_spawned_ragdoll()
                    .and_then(|spawned| ragdoll_pelvis_velocity(spawned, &labels, &velocities))
                    .map(Vec3::length)
                    .unwrap_or_default();
                drive = ragdoll_joint_drive_for_speed(speed);
                if let Some(metrics) = metrics {
                    drive.target_joint_count = metrics.target_joint_count;
                    drive.driven_joint_count = metrics.driven_joint_count;
                }
            }
            let stiffness = drive.angular_stiffness;
            let damping = 0.0;
            if let Ok(mut angular_drive) = angular_drives.get_mut(player_entity) {
                angular_drive.stiffness = stiffness;
                angular_drive.damping = damping;
            } else {
                commands.entity(player_entity).insert((
                    RagdollAngularDrive { stiffness, damping },
                    RagdollAngularDriveMetrics::default(),
                ));
            }
        }
        if let Some(mut current_drive) = current_drive {
            *current_drive = drive;
        } else {
            commands.entity(character).insert(drive);
        }
    }
}

fn ragdoll_capsule_dimensions(
    stance: Stance,
    params_handle: &AlsMovementParamsHandle,
    params_assets: &Assets<AlsMovementParams>,
    world_scale: Vec3,
) -> (f32, f32) {
    let (half_height, radius) = if let Some(params) = params_assets.get(params_handle.0.id()) {
        let half_height = if stance == Stance::Crouching { params.crouch.crouched_capsule_half_height } else { params.crouch.standing_capsule_half_height };
        (half_height, params.crouch.standing_capsule_radius)
    } else {
        (
            ALS_CHARACTER_CAPSULE_HALF_HEIGHT,
            ALS_CHARACTER_CAPSULE_RADIUS,
        )
    };
    let (half_height, radius, _) =
        scaled_mantle_capsule_dimensions(half_height, radius, world_scale);
    (half_height, radius)
}

pub fn ragdoll_exit_route_system(
    mut commands: Commands,
    mut transitions: MessageWriter<StateTransitionEvent>,
    mut characters: Query<
        (
            Entity,
            Option<&mut AlsNotifyLocomotionAction>,
            &mut LocomotionState,
            &mut als_core::transition::AlsStateRequest,
            Option<&AlsRagdollGroundFollow>,
            Option<&mut LinearVelocity>,
        ),
        With<AlsCharacter>,
    >,
) {
    for (entity, action, mut locomotion, mut request, follow, velocity) in &mut characters {
        if !request.ragdoll_exit {
            continue;
        }
        if action.as_deref() != Some(&AlsNotifyLocomotionAction::Ragdolling) {
            request.ragdoll_exit = false;
            continue;
        }
        let Some(follow) = follow else {
            continue;
        };
        let (state, exit_velocity) = ragdoll_exit_route(follow.grounded, follow.velocity);
        *action.unwrap() = AlsNotifyLocomotionAction::None;
        request.locomotion_state = None;
        if *locomotion != state {
            transitions.write(StateTransitionEvent {
                entity,
                dimension: Dimension::LocomotionState,
                from: StateValue::LocomotionState(*locomotion),
                to: StateValue::LocomotionState(state),
            });
            *locomotion = state;
        }
        if state == LocomotionState::Air {
            commands.entity(entity).insert(AlsRagdollAirExit);
        } else {
            commands.entity(entity).remove::<AlsRagdollAirExit>();
        }
        if let Some(mut velocity) = velocity {
            velocity.0 = exit_velocity;
        }
        info!(
            "[RAGLINK] exit route grounded={} state={state:?} velocity=({:.3},{:.3},{:.3})",
            follow.grounded, exit_velocity.x, exit_velocity.y, exit_velocity.z
        );
        request.ragdoll_exit = false;
    }
}

fn release_ragdoll_air_exit_on_ground(
    mut commands: Commands,
    spatial: SpatialQuery,
    transform_helper: TransformHelper,
    params_assets: Res<Assets<AlsMovementParams>>,
    characters: Query<
        (Entity, &Position, &Stance, &AlsMovementParamsHandle),
        (With<AlsCharacter>, With<AlsRagdollAirExit>),
    >,
) {
    for (entity, position, stance, params_handle) in &characters {
        let Ok(world) = transform_helper.compute_global_transform(entity) else {
            continue;
        };
        let (world_scale, _, _) = world.to_scale_rotation_translation();
        let (half_height, radius) =
            ragdoll_capsule_dimensions(*stance, params_handle, &params_assets, world_scale);
        let trace_start = position.0 + Vec3::Y * (2.0 * radius);
        let trace_distance = half_height + radius;
        let shape = Collider::sphere(radius);
        let config = ShapeCastConfig::from_max_distance(trace_distance);
        let filter = SpatialQueryFilter::from_excluded_entities([entity]);
        if spatial
            .cast_shape(
                &shape,
                trace_start,
                Quat::IDENTITY,
                Dir3::NEG_Y,
                &config,
                &filter,
            )
            .is_some()
        {
            commands.entity(entity).remove::<AlsRagdollAirExit>();
            info!("[RAGLINK] air exit hold released on ground");
        }
    }
}

#[allow(clippy::type_complexity)]
fn ragdoll_character_physics_system(
    mut commands: Commands,
    rolls: Query<&als_core::roll::AlsRollState>,
    mut characters: Query<
        (
            Entity,
            &mut Transform,
            Option<&mut Position>,
            Option<&mut LinearVelocity>,
            Option<&AlsNotifyLocomotionAction>,
            &RigidBody,
            Option<&CollisionLayers>,
            &mut AlsCollisionLayerOwnership,
            Option<&TnuaToggle>,
            Option<&AlsRagdollCharacterPhysics>,
            Option<&AlsRagdollGroundFollow>,
        ),
        With<AlsCharacter>,
    >,
) {
    for (
        entity,
        mut transform,
        mut position,
        mut velocity,
        action,
        rigid_body,
        collision_layers,
        mut collision_ownership,
        tnua_toggle,
        previous_physics,
        ground_follow,
    ) in &mut characters
    {
        let is_ragdoll = action == Some(&AlsNotifyLocomotionAction::Ragdolling);
        if is_ragdoll {
            if previous_physics.is_none() {
                if let Ok(roll) = rolls.get(entity) {
                    if roll.active {
                        commands
                            .entity(entity)
                            .insert(crate::roll_drive::AlsRollBlendOut::new(roll, crate::get_up::RAGDOLL_SETTINGS.stop_montage_blend_seconds));
                    }
                }
                commands.entity(entity).insert(AlsRagdollCharacterPhysics {
                    rigid_body: *rigid_body,
                    tnua_toggle: tnua_toggle.copied(),
                });
                if crate::get_up::RAGDOLL_SETTINGS.limit_initial_speed {
                    let entry_speed = velocity
                        .as_ref()
                        .map_or(0.0, |velocity| velocity.0.length());
                    commands
                        .entity(entity)
                        .insert(AlsRagdollInitialSpeedLimit::new(entry_speed));
                }
            }
            if *rigid_body != RigidBody::Kinematic {
                commands.entity(entity).insert(RigidBody::Kinematic);
            }
            set_collision_layer_owner(
                &mut commands,
                entity,
                &mut collision_ownership,
                collision_layers.copied(),
                AlsCollisionLayerOwner::Ragdoll,
                true,
            );
            if tnua_toggle.is_none_or(|toggle| *toggle != TnuaToggle::Disabled) {
                commands.entity(entity).insert(TnuaToggle::Disabled);
            }
            continue;
        }
        set_collision_layer_owner(
            &mut commands,
            entity,
            &mut collision_ownership,
            collision_layers.copied(),
            AlsCollisionLayerOwner::Ragdoll,
            false,
        );
        let Some(previous_physics) = previous_physics else {
            continue;
        };
        if let Some(follow) = ground_follow {
            let (exit_state, exit_velocity) = ragdoll_exit_route(follow.grounded, follow.velocity);
            transform.translation = follow.target;
            if let Some(position) = position.as_deref_mut() {
                position.0 = follow.target;
            }
            if let Some(velocity) = velocity.as_deref_mut() {
                velocity.0 = exit_velocity;
            }
            info!(
                "[RAGLINK] exit realign grounded={} state={exit_state:?} target=({:.2},{:.2},{:.2}) velocity=({:.3},{:.3},{:.3})",
                follow.grounded,
                follow.target.x,
                follow.target.y,
                follow.target.z,
                exit_velocity.x,
                exit_velocity.y,
                exit_velocity.z
            );
        }
        commands.entity(entity).insert(previous_physics.rigid_body);
        match previous_physics.tnua_toggle {
            Some(toggle) => {
                commands.entity(entity).insert(toggle);
            }
            None => {
                commands.entity(entity).remove::<TnuaToggle>();
            }
        }
        commands
            .entity(entity)
            .remove::<AlsRagdollCharacterPhysics>();
        commands.entity(entity).remove::<AlsRagdollGroundFollow>();
        commands
            .entity(entity)
            .remove::<AlsRagdollInitialSpeedLimit>();
    }
}

fn limit_initial_ragdoll_speed(
    mut characters: Query<
        (Entity, Option<&AlsNotifyLocomotionAction>, &mut AlsRagdollInitialSpeedLimit),
        With<AlsCharacter>,
    >,
    avatars: Query<(&AlsCharacterAvatar, &AnimatedSceneInstance)>,
    players: Query<&AnimationGraphPlayer>,
    mut velocities: Query<&mut LinearVelocity>,
) {
    for (character, action, mut limit) in &mut characters {
        if action != Some(&AlsNotifyLocomotionAction::Ragdolling) {
            continue;
        }
        let Some(player) = animated_player_for_character(character, &avatars)
            .and_then(|entity| players.get(entity).ok())
        else {
            continue;
        };
        let Some(spawned) = player.get_spawned_ragdoll() else {
            continue;
        };
        let Some(speed) = limit.next_limit() else {
            continue;
        };
        for entity in spawned.bodies.values() {
            if let Ok(mut velocity) = velocities.get_mut(*entity) {
                velocity.0 = velocity.0.clamp_length_max(speed);
            }
        }
    }
}

pub(crate) fn ragdoll_recovery_rotation(pelvis: Quat) -> (Quat, bool) {
    let forward = pelvis * Vec3::X;
    let (yaw, facing_up) = if forward.y.abs() > 0.999999 {
        let canonical = Quat::from_rotation_y(-std::f32::consts::FRAC_PI_2) * pelvis;
        let yaw = 2.0 * (-canonical.x).atan2(canonical.w) * -forward.y.signum();
        (yaw, true)
    } else {
        let roll = (-(pelvis * Vec3::Z).y).atan2((pelvis * Vec3::Y).y);
        (
            als_core::orientation::world_forward_to_yaw(forward),
            roll <= 0.0,
        )
    };
    (
        Quat::from_rotation_y(yaw + if facing_up { std::f32::consts::PI } else { 0.0 }),
        facing_up,
    )
}

pub(crate) fn restore_ragdoll_player_parent(
    mut commands: Commands,
    transform_helper: TransformHelper,
    avatars: Query<(&AlsCharacterAvatar, &AnimatedSceneInstance)>,
    characters: Query<(Option<&AlsNotifyLocomotionAction>, &Transform, &LocomotionState), With<AlsCharacter>>,
    detached_players: Query<&AlsRagdollDetachedPlayer>,
    skeletons: Res<Assets<Skeleton>>,
    bones: Query<(&Name, &Transform, &GlobalTransform, Option<&ChildOf>)>,
    globals: Query<&GlobalTransform>,
    mut players: Query<&mut AnimationGraphPlayer>,
) {
    for (avatar, instance) in &avatars {
        let Ok((action, actor_transform, locomotion)) = characters.get(avatar.character) else {
            continue;
        };
        if action == Some(&AlsNotifyLocomotionAction::Ragdolling) {
            continue;
        }
        let player_entity = instance.player_entity();
        let Ok(detached) = detached_players.get(player_entity) else {
            continue;
        };
        let Ok(mut player) = players.get_mut(player_entity) else {
            continue;
        };
        let Some(skeleton) = skeletons.get(player.skeleton()) else {
            continue;
        };
        let Ok(old_player_world) = globals.get(player_entity) else {
            continue;
        };
        let Ok(parent_world) = transform_helper.compute_global_transform(detached.parent) else {
            continue;
        };
        let Some(pelvis_world) = skeleton.iter_bones().find_map(|bone| {
            player
                .bone_entity(bone)
                .and_then(|entity| bones.get(entity).ok())
                .filter(|(name, _, _, _)| name.as_str() == PELVIS_LABEL)
                .map(|(_, _, world, _)| *world)
        }) else {
            continue;
        };
        let (rotation, facing_up) = ragdoll_recovery_rotation(pelvis_world.rotation());
        let restored_actor = Transform {
            rotation,
            ..*actor_transform
        };
        let parent_in_actor = parent_world.reparented_to(&GlobalTransform::from(*actor_transform));
        let restored_world =
            GlobalTransform::from(restored_actor * parent_in_actor * detached.local_transform);
        commands
            .entity(avatar.character)
            .insert((restored_actor, Rotation(rotation)));
        let mut snapshot = Pose {
            skeleton: player.skeleton().clone(),
            timestamp: player
                .get_default_output_pose()
                .map_or(0.0, |pose| pose.timestamp),
            ..default()
        };
        for bone in skeleton
            .iter_bones()
            .filter(|bone| *bone != skeleton.root())
        {
            let Some(entity) = player.bone_entity(bone) else {
                continue;
            };
            let Ok((name, local, world, parent)) = bones.get(entity) else {
                continue;
            };
            let mut local = *local;
            if name.as_str() == PELVIS_LABEL {
                let parent_world = parent
                    .and_then(|parent| globals.get(parent.parent()).ok())
                    .copied()
                    .unwrap_or(*old_player_world);
                let parent_in_player = parent_world.reparented_to(old_player_world);
                local = world.reparented_to(&(restored_world * parent_in_player));
            }
            snapshot.add_bone(BonePose::from_transform(local), bone);
        }
        player.set_global_input_data(
            crate::ragdolling_layer::RAGDOLLING_FINAL_POSE_KEY,
            DataValue::Pose(snapshot),
        );
        commands
            .entity(player_entity)
            .insert((
                ChildOf(detached.parent),
                detached.local_transform,
                restored_world,
            ))
            .remove::<AlsRagdollDetachedPlayer>();
        if *locomotion == LocomotionState::Grounded {
            commands
                .entity(avatar.character)
                .insert(crate::get_up::AlsGetUpRequest(facing_up));
        }
    }
}

fn ragdoll_ground_filter(
    excluded: Vec<Entity>,
    ownership: &AlsCollisionLayerOwnership,
) -> SpatialQueryFilter {
    SpatialQueryFilter::from_excluded_entities(excluded).with_mask(ownership.original_filters())
}

#[allow(clippy::type_complexity)]
fn ragdoll_ground_follow(
    mut commands: Commands,
    spatial: SpatialQuery,
    transform_helper: TransformHelper,
    params_assets: Res<Assets<AlsMovementParams>>,
    characters: Query<
        (
            Entity,
            &Position,
            Option<&AlsNotifyLocomotionAction>,
            &Stance,
            &AlsMovementParamsHandle,
            &AlsCollisionLayerOwnership,
        ),
        With<AlsCharacter>,
    >,
    avatars: Query<(&AlsCharacterAvatar, &AnimatedSceneInstance)>,
    players: Query<&AnimationGraphPlayer>,
    labels: Query<&BodyLabel>,
    body_states: Query<(&Position, &LinearVelocity), Without<AlsCharacter>>,
) {
    for (entity, _position, action, stance, params_handle, collision_ownership) in &characters {
        if action != Some(&AlsNotifyLocomotionAction::Ragdolling) {
            continue;
        }
        let Some(instance) = avatars
            .iter()
            .find_map(|(avatar, instance)| (avatar.character == entity).then_some(instance))
        else {
            continue;
        };
        let Ok(player) = players.get(instance.player_entity()) else {
            continue;
        };
        let Some(spawned) = player.get_spawned_ragdoll() else {
            continue;
        };
        let Some(pelvis_entity) = find_body_by_label(spawned, &labels, PELVIS_LABEL) else {
            continue;
        };
        let Ok((pelvis_position, pelvis_velocity)) = body_states.get(pelvis_entity) else {
            continue;
        };
        let pelvis = pelvis_position.0;
        let Ok(world) = transform_helper.compute_global_transform(entity) else {
            continue;
        };
        let (world_scale, _, _) = world.to_scale_rotation_translation();
        let (half_height, radius) =
            ragdoll_capsule_dimensions(*stance, params_handle, &params_assets, world_scale);
        let trace_start = pelvis + Vec3::Y * (2.0 * radius);
        let trace_distance = half_height + radius;
        let mut excluded = Vec::with_capacity(1 + spawned.bodies.len() + spawned.colliders.len());
        excluded.push(entity);
        excluded.extend(spawned.bodies.values().copied());
        excluded.extend(spawned.colliders.values().copied());
        let filter = ragdoll_ground_filter(excluded, collision_ownership);
        let shape = Collider::sphere(radius);
        let config = ShapeCastConfig::from_max_distance(trace_distance);
        let hit = spatial.cast_shape(
            &shape,
            trace_start,
            Quat::IDENTITY,
            Dir3::NEG_Y,
            &config,
            &filter,
        );
        let grounded = hit.is_some();
        let target = if let Some(hit) = hit {
            let center = trace_start - Vec3::Y * hit.distance;
            Vec3::new(
                pelvis.x,
                center.y + half_height - radius + ALS_CHARACTER_FLOOR_CLEARANCE,
                pelvis.z,
            )
        } else {
            pelvis
        };
        commands.entity(entity).insert(AlsRagdollGroundFollow {
            pelvis,
            target,
            velocity: pelvis_velocity.0,
            grounded,
        });
    }
}

fn apply_ragdoll_ground_follow(
    mut characters: Query<
        (
            &mut Position,
            &mut Transform,
            &mut LinearVelocity,
            Option<&AlsNotifyLocomotionAction>,
            &AlsRagdollGroundFollow,
        ),
        With<AlsCharacter>,
    >,
) {
    for (mut position, mut transform, mut velocity, action, ground_follow) in &mut characters {
        if action != Some(&AlsNotifyLocomotionAction::Ragdolling) {
            continue;
        }
        position.0 = ground_follow.target;
        transform.translation = ground_follow.target;
        velocity.0 = Vec3::ZERO;
    }
}

#[allow(clippy::type_complexity)]
fn ragdoll_link_system(
    mut commands: Commands,
    mut q: Query<(Entity, Option<&AlsNotifyLocomotionAction>, &mut AlsRagdollLink), With<AlsCharacter>>,
    avatars: Query<(&AlsCharacterAvatar, &AnimatedSceneInstance)>,
    mut players: Query<&mut AnimationGraphPlayer>,
    layers: Query<(Option<&AlsRagdollColliderLayers>, &CollisionLayers)>,
) {
    for (character, action, mut link) in &mut q {
        let is_ragdoll = action == Some(&AlsNotifyLocomotionAction::Ragdolling);
        let Some(instance) = avatars
            .iter()
            .find_map(|(avatar, instance)| (avatar.character == character).then_some(instance))
        else {
            link.prev_ragdoll = is_ragdoll;
            continue;
        };
        let Ok(mut player) = players.get_mut(instance.player_entity()) else {
            link.prev_ragdoll = is_ragdoll;
            continue;
        };

        player.set_input_data("ragdoll_dynamic", DataValue::from(is_ragdoll));
        player.set_input_data("ragdoll_readback", DataValue::from(is_ragdoll));

        if let Some(spawned) = player.get_spawned_ragdoll() {
            for collider in spawned.colliders.values() {
                let Ok((stored, current)) = layers.get(*collider) else {
                    continue;
                };
                if is_ragdoll {
                    if let Some(stored) = stored
                        && *current != stored.0
                    {
                        commands.entity(*collider).insert(stored.0);
                    }
                } else {
                    if stored.is_none() {
                        commands
                            .entity(*collider)
                            .insert(AlsRagdollColliderLayers(*current));
                    }
                    if *current != CollisionLayers::NONE {
                        commands.entity(*collider).insert(CollisionLayers::NONE);
                    }
                }
            }
        }

        link.prev_ragdoll = is_ragdoll;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ragdoll_ground_query_ignores_visibility_only_with_original_pawn_response() {
        use avian3d::prelude::{LayerMask, PhysicsPlugins};
        use bevy::ecs::system::RunSystemOnce;
        use bevy::transform::TransformPlugin;
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            bevy::mesh::MeshPlugin,
            TransformPlugin,
            PhysicsPlugins::new(FixedPostUpdate),
        ));
        app.finish();
        app.cleanup();
        let original = CollisionLayers::new(
            als_core::character::AlsCollisionLayer::Default,
            als_core::character::AlsCollisionLayer::Default,
        );
        let mut ownership = AlsCollisionLayerOwnership::default();
        assert_eq!(
            ownership.set_owner(AlsCollisionLayerOwner::Ragdoll, Some(original), true),
            Some(Some(CollisionLayers::NONE))
        );
        let visibility = app
            .world_mut()
            .spawn((
                RigidBody::Static,
                Collider::cuboid(2.0, 0.1, 2.0),
                CollisionLayers::new(als_core::character::AlsCollisionLayer::Visibility, LayerMask::ALL),
                Transform::from_xyz(0.0, 1.2, 0.0),
            ))
            .id();
        let ground = app
            .world_mut()
            .spawn((
                RigidBody::Static,
                Collider::cuboid(2.0, 0.2, 2.0),
                CollisionLayers::new(als_core::character::AlsCollisionLayer::Default, LayerMask::ALL),
                Transform::IDENTITY,
            ))
            .id();
        app.update();
        app.world_mut().run_schedule(FixedPostUpdate);
        let hit = app.world_mut().run_system_once(move |spatial: SpatialQuery| {
            let filter = ragdoll_ground_filter(Vec::new(), &ownership);
            spatial
                .cast_shape(
                    &Collider::sphere(0.3),
                    Vec3::Y * 1.6,
                    Quat::IDENTITY,
                    Dir3::NEG_Y,
                    &ShapeCastConfig::from_max_distance(1.5),
                    &filter,
                )
                .map(|hit| hit.entity)
        })
        .unwrap();
        assert_ne!(hit, Some(visibility));
        assert_eq!(hit, Some(ground));
    }

    #[test]
    fn ragdoll_trace_dimensions_use_world_horizontal_and_vertical_scale() {
        let mut assets = Assets::<AlsMovementParams>::default();
        let params = AlsMovementParams::default();
        let standing_half = params.crouch.standing_capsule_half_height;
        let crouched_half = params.crouch.crouched_capsule_half_height;
        let radius = params.crouch.standing_capsule_radius;
        let handle = AlsMovementParamsHandle(assets.add(params));
        for scale in [Vec3::new(1.5, 2.0, 0.5), Vec3::new(0.5, 1.25, 1.5)] {
            for (stance, base_half) in [
                (Stance::Standing, standing_half),
                (Stance::Crouching, crouched_half),
            ] {
                let (half_height, scaled_radius) =
                    ragdoll_capsule_dimensions(stance, &handle, &assets, scale);
                assert!((half_height - base_half * scale.y).abs() < 1e-6);
                assert!((scaled_radius - radius * scale.x.min(scale.z)).abs() < 1e-6);
            }
        }
        let missing = AlsMovementParamsHandle(Handle::default());
        let (half_height, scaled_radius) = ragdoll_capsule_dimensions(
            Stance::Standing,
            &missing,
            &assets,
            Vec3::new(1.5, 2.0, 0.5),
        );
        assert!((half_height - ALS_CHARACTER_CAPSULE_HALF_HEIGHT * 2.0).abs() < 1e-6);
        assert!((scaled_radius - ALS_CHARACTER_CAPSULE_RADIUS * 0.5).abs() < 1e-6);

        use avian3d::prelude::PhysicsPlugins;
        use bevy::ecs::system::RunSystemOnce;
        use bevy::transform::TransformPlugin;
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            bevy::mesh::MeshPlugin,
            TransformPlugin,
            PhysicsPlugins::new(FixedPostUpdate),
        ));
        app.finish();
        app.cleanup();
        app.world_mut().spawn((
            RigidBody::Static,
            Collider::cuboid(20.0, 0.2, 20.0),
            Transform::IDENTITY,
        ));
        app.update();
        app.world_mut().run_schedule(FixedPostUpdate);
        let (half_height, radius) = ragdoll_capsule_dimensions(
            Stance::Standing,
            &handle,
            &assets,
            Vec3::new(1.5, 2.0, 0.5),
        );
        let trace_start = Vec3::Y * (0.8 + 2.0 * radius);
        let hit = app
            .world_mut()
            .run_system_once(move |spatial: SpatialQuery| {
                spatial.cast_shape(
                    &Collider::sphere(radius),
                    trace_start,
                    Quat::IDENTITY,
                    Dir3::NEG_Y,
                    &ShapeCastConfig::from_max_distance(half_height + radius),
                    &SpatialQueryFilter::default(),
                )
            })
            .unwrap()
            .unwrap();
        let target_y = trace_start.y - hit.distance + half_height - radius
            + ALS_CHARACTER_FLOOR_CLEARANCE;
        let source_y = 0.1 + standing_half * 2.0 + ALS_CHARACTER_FLOOR_CLEARANCE;
        assert!((target_y - source_y).abs() < 1e-4);
    }

    #[test]
    fn initial_speed_limit_covers_entry_and_eight_refreshes() {
        let mut limit = AlsRagdollInitialSpeedLimit::new(0.5);
        for _ in 0..9 {
            let speed = limit.next_limit().unwrap();
            assert_eq!(
                Vec3::new(6.0, 0.0, 8.0).clamp_length_max(speed).length(),
                2.0
            );
        }
        assert!(limit.next_limit().is_none());
        assert_eq!(
            AlsRagdollInitialSpeedLimit::new(7.0).next_limit(),
            Some(7.0)
        );
    }
    use als_core::state::{Gait, OverlayState, RotationMode};
    use als_core::transition::{AlsStateRequest, AlsStateTransitionPlugin};
    use bevy::{asset::Assets, ecs::system::In, platform::collections::HashMap};
    use bevy_animation_graph::core::{
        animation_clip::{EntityPath, GraphClip},
        animation_graph::AnimationGraph,
        context::{
            pose_fallback::{PoseFallbackContext, RootOffsetResult},
            system_resources::SystemResources,
        },
        pose::Pose,
        ragdoll::{
            bone_mapping::{BodyMapping, BodyWeight, BoneMapping, BoneWeight, RagdollBoneMap},
            configuration::RagdollConfig,
            definition::{Body, Ragdoll},
            read_pose_avian::read_pose,
            spawning::SpawnedRagdoll,
            write_pose::write_pose_to_ragdoll,
        },
        skeleton::Skeleton,
        space_conversion::SpaceConversionContext,
        state_machine::high_level::StateMachine,
    };

    #[derive(Clone, Copy)]
    struct RagdollCoordinateRoundTripInput {
        root: Entity,
        bone: Entity,
        body: Entity,
    }

    struct RagdollCoordinateRoundTrip {
        expected_global: Transform,
        readback_local: Transform,
    }

    fn compose_transform(parent: Transform, child: Transform) -> Transform {
        Transform::from_matrix(parent.to_matrix() * child.to_matrix())
    }

    fn transforms_match(left: Transform, right: Transform) -> bool {
        left.translation.distance(right.translation) <= 1e-4
            && left.rotation.dot(right.rotation).abs() >= 1.0 - 1e-4
            && left.scale.distance(right.scale) <= 1e-4
    }

    fn spawn_coordinate_chain(
        world: &mut World,
        actor_transform: Transform,
        avatar_transform: Transform,
        avatar_is_child: bool,
        player_transform: Transform,
        root_transform: Transform,
        bone_transform: Transform,
    ) -> (Entity, Entity, Entity, Entity, Entity) {
        let actor = world
            .spawn((actor_transform, GlobalTransform::from(actor_transform)))
            .id();
        let avatar_global = if avatar_is_child {
            compose_transform(actor_transform, avatar_transform)
        } else {
            avatar_transform
        };
        let avatar = world
            .spawn((avatar_transform, GlobalTransform::from(avatar_global)))
            .id();
        if avatar_is_child {
            world.entity_mut(actor).add_child(avatar);
        }
        let player_global = compose_transform(avatar_global, player_transform);
        let player = world
            .spawn((player_transform, GlobalTransform::from(player_global)))
            .id();
        world.entity_mut(avatar).add_child(player);
        let root_global = compose_transform(player_global, root_transform);
        let root = world
            .spawn((root_transform, GlobalTransform::from(root_global)))
            .id();
        world.entity_mut(player).add_child(root);
        let bone_global = compose_transform(root_global, bone_transform);
        let bone = world
            .spawn((bone_transform, GlobalTransform::from(bone_global)))
            .id();
        world.entity_mut(root).add_child(bone);
        (actor, avatar, player, root, bone)
    }

    fn spawn_ragdoll_body(world: &mut World, transform: Transform) -> Entity {
        world
            .spawn((
                Position(transform.translation),
                Rotation(transform.rotation),
            ))
            .id()
    }

    fn ragdoll_coordinate_round_trip(
        In(input): In<RagdollCoordinateRoundTripInput>,
        resources: SystemResources,
        bodies: Query<(&Position, &Rotation)>,
    ) -> RagdollCoordinateRoundTrip {
        let root_path = EntityPath::from(vec!["armature".to_owned()]);
        let bone_path = root_path.child("pelvis");
        let mut skeleton = Skeleton::default();
        skeleton.add_bone(root_path.clone(), Transform::IDENTITY, Transform::IDENTITY);
        skeleton.add_bone(bone_path.clone(), Transform::IDENTITY, Transform::IDENTITY);
        skeleton.set_root(root_path.id());

        let body = Body::new();
        let body_id = body.id;
        let mut ragdoll = Ragdoll::default();
        ragdoll.add_body(body);
        let mapping = RagdollBoneMap {
            bones_from_bodies: HashMap::from([(
                bone_path.clone(),
                BoneMapping {
                    bone_id: bone_path.clone(),
                    bodies: vec![BodyWeight {
                        body: body_id,
                        weight: 1.0,
                        ..default()
                    }],
                    created_from: None,
                },
            )]),
            bodies_from_bones: HashMap::from([(
                body_id,
                BodyMapping {
                    body_id,
                    bone: BoneWeight {
                        bone: bone_path.clone(),
                        ..default()
                    },
                    created_from: None,
                },
            )]),
            skeleton: default(),
            ragdoll: default(),
            skeleton_symmetry: default(),
        };
        let entity_map =
            HashMap::from([(root_path.id(), input.root), (bone_path.id(), input.bone)]);
        let pose = Pose::default();
        let fallback = PoseFallbackContext {
            entity_map: &entity_map,
            resources: &resources,
            fallback_to_identity: false,
        };
        let conversions = SpaceConversionContext {
            pose_fallback: fallback,
        };
        let target = write_pose_to_ragdoll(&pose, &skeleton, &ragdoll, &mapping, fallback)
            .bodies
            .pop()
            .expect("fixture body target must exist");
        let root_offset = fallback.compute_root_global_transform_to_rigidbody(&skeleton);
        let expected_global = match root_offset {
            RootOffsetResult::FromRoot(transform) => {
                Transform::from_isometry(transform.to_isometry() * target.character_space_isometry)
            }
            RootOffsetResult::FromRigidbody(_, _) | RootOffsetResult::Failed => {
                panic!("fixture root must be a non-physics hierarchy")
            }
        };
        let expected_from_space =
            conversions.global_transform_of_bone(&pose, &skeleton, bone_path.id());
        assert!(transforms_match(expected_global, expected_from_space));

        let mut spawned = SpawnedRagdoll::new(Entity::PLACEHOLDER);
        spawned.bodies.insert(body_id, input.body);
        let readback = read_pose(
            &spawned,
            &mapping,
            &skeleton,
            &bodies,
            fallback,
            &pose,
            &RagdollConfig::default(),
        );
        let readback_local = readback
            .paths
            .get(&bone_path.id())
            .map(|index| readback.bones[*index].to_transform())
            .expect("fixture readback bone must exist");
        RagdollCoordinateRoundTrip {
            expected_global,
            readback_local,
        }
    }

    #[test]
    fn nonorigin_rotated_ragdoll_contract_requires_detach_then_restores_attachment() {
        let mut world = World::new();
        world.insert_resource(Assets::<GraphClip>::default());
        world.insert_resource(Assets::<AnimationGraph>::default());
        world.insert_resource(Assets::<StateMachine>::default());
        world.insert_resource(Assets::<Skeleton>::default());

        let actor_before =
            Transform::from_xyz(4.0, 1.5, -3.0).with_rotation(Quat::from_rotation_y(0.6));
        let actor_after =
            Transform::from_xyz(-2.0, 2.0, 5.0).with_rotation(Quat::from_rotation_y(-0.85));
        let avatar_local = Transform::from_xyz(0.2, -0.92, -0.35)
            .with_rotation(Quat::from_rotation_y(std::f32::consts::PI));
        let player_local =
            Transform::from_xyz(0.1, 0.25, 0.3).with_rotation(Quat::from_rotation_x(0.2));
        let root_local =
            Transform::from_xyz(0.0, 0.15, 0.0).with_rotation(Quat::from_rotation_z(-0.1));
        let bone_local =
            Transform::from_xyz(0.35, 0.8, -0.2).with_rotation(Quat::from_rotation_x(0.35));
        let avatar_before = compose_transform(actor_before, avatar_local);
        let pre_root =
            compose_transform(compose_transform(avatar_before, player_local), root_local);
        let pre_body_transform = compose_transform(pre_root, bone_local);
        let exit_root = compose_transform(
            compose_transform(compose_transform(actor_after, avatar_local), player_local),
            root_local,
        );
        let exit_body_transform = compose_transform(exit_root, bone_local);

        let (_, _, _, pre_root_entity, pre_bone_entity) = spawn_coordinate_chain(
            &mut world,
            actor_before,
            avatar_local,
            true,
            player_local,
            root_local,
            bone_local,
        );
        let pre_body = spawn_ragdoll_body(&mut world, pre_body_transform);
        let (_, _, _, attached_root, attached_bone) = spawn_coordinate_chain(
            &mut world,
            actor_after,
            avatar_local,
            true,
            player_local,
            root_local,
            bone_local,
        );
        let attached_body = spawn_ragdoll_body(&mut world, pre_body_transform);
        let (detached_actor, detached_avatar, detached_player, detached_root, detached_bone) =
            spawn_coordinate_chain(
                &mut world,
                actor_before,
                avatar_local,
                true,
                player_local,
                root_local,
                bone_local,
            );
        world.entity_mut(detached_player).remove_parent_in_place();
        *world.get_mut::<Transform>(detached_actor).unwrap() = actor_after;
        *world.get_mut::<GlobalTransform>(detached_actor).unwrap() =
            GlobalTransform::from(actor_after);
        *world.get_mut::<GlobalTransform>(detached_avatar).unwrap() =
            GlobalTransform::from(compose_transform(actor_after, avatar_local));
        let detached_body = spawn_ragdoll_body(&mut world, pre_body_transform);
        let (exit_actor, exit_avatar, exit_player, exit_root_entity, exit_bone_entity) =
            spawn_coordinate_chain(
                &mut world,
                actor_after,
                avatar_local,
                true,
                player_local,
                root_local,
                bone_local,
            );
        let exit_body = spawn_ragdoll_body(&mut world, exit_body_transform);

        let pre = world
            .run_system_cached_with(
                ragdoll_coordinate_round_trip,
                RagdollCoordinateRoundTripInput {
                    root: pre_root_entity,
                    bone: pre_bone_entity,
                    body: pre_body,
                },
            )
            .unwrap();
        let attached = world
            .run_system_cached_with(
                ragdoll_coordinate_round_trip,
                RagdollCoordinateRoundTripInput {
                    root: attached_root,
                    bone: attached_bone,
                    body: attached_body,
                },
            )
            .unwrap();
        let detached = world
            .run_system_cached_with(
                ragdoll_coordinate_round_trip,
                RagdollCoordinateRoundTripInput {
                    root: detached_root,
                    bone: detached_bone,
                    body: detached_body,
                },
            )
            .unwrap();
        let exit = world
            .run_system_cached_with(
                ragdoll_coordinate_round_trip,
                RagdollCoordinateRoundTripInput {
                    root: exit_root_entity,
                    bone: exit_bone_entity,
                    body: exit_body,
                },
            )
            .unwrap();

        assert!(transforms_match(pre.expected_global, pre_body_transform));
        assert!(transforms_match(pre.readback_local, bone_local));
        assert!(
            !transforms_match(attached.readback_local, bone_local),
            "following the actor while the avatar remains parented must expose the source lifecycle mismatch"
        );
        assert!(transforms_match(
            detached.expected_global,
            pre_body_transform
        ));
        assert!(transforms_match(detached.readback_local, bone_local));
        assert_eq!(
            world.get::<ChildOf>(detached_avatar).unwrap().parent(),
            detached_actor
        );
        assert!(world.get::<ChildOf>(detached_player).is_none());
        assert!(transforms_match(
            *world.get::<Transform>(detached_player).unwrap(),
            compose_transform(avatar_before, player_local)
        ));
        assert!(transforms_match(
            *world.get::<Transform>(exit_avatar).unwrap(),
            avatar_local
        ));
        assert_eq!(
            world.get::<ChildOf>(exit_avatar).unwrap().parent(),
            exit_actor
        );
        assert_eq!(
            world.get::<ChildOf>(exit_player).unwrap().parent(),
            exit_avatar
        );
        assert!(transforms_match(
            *world.get::<Transform>(exit_player).unwrap(),
            player_local
        ));
        assert!(transforms_match(exit.expected_global, exit_body_transform));
        assert!(transforms_match(exit.readback_local, bone_local));
    }

    #[test]
    fn ragdoll_lifecycle_player_query_is_schedule_safe() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_systems(Update, ragdoll_character_physics_system);
        app.update();
    }

    #[test]
    fn ragdoll_entry_preserves_normal_parent_and_detaches_only_on_entry() {
        use bevy::asset::AssetApp;
        use bevy::world_serialization::{WorldAsset, WorldInstanceReady, WorldInstanceSpawner};
        use bevy_animation_graph::core::animated_scene::AnimatedSceneHandle;

        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            TransformPlugin,
            bevy_animation_graph::AnimationGraphPlugin::default(),
        ))
        .init_asset::<WorldAsset>()
        .add_systems(Update, capture_ragdoll_player_world_transform);
        let actor = app
            .world_mut()
            .spawn((
                AlsCharacter,
                LocomotionState::Grounded,
                Transform::from_xyz(4.0, 1.0, -3.0),
            ))
            .id();
        let avatar = app
            .world_mut()
            .spawn((
                AlsCharacterAvatar { character: actor },
                AnimatedSceneHandle::default(),
                ChildOf(actor),
            ))
            .id();
        let player = app
            .world_mut()
            .spawn((
                AnimationGraphPlayer::new(default()),
                Transform::IDENTITY,
                ChildOf(avatar),
            ))
            .id();
        let instance_id = WorldInstanceSpawner::default().spawn(Handle::<WorldAsset>::default());
        app.world_mut().trigger(WorldInstanceReady {
            entity: avatar,
            instance_id,
        });
        app.world_mut().flush();
        assert_eq!(
            app.world()
                .get::<AnimatedSceneInstance>(avatar)
                .unwrap()
                .player_entity(),
            player
        );

        app.world_mut()
            .run_system_cached(capture_ragdoll_player_world_transform)
            .unwrap();
        assert_eq!(
            app.world().get::<ChildOf>(player).map(ChildOf::parent),
            Some(avatar)
        );
        assert!(
            app.world()
                .get::<AlsRagdollDetachedPlayer>(player)
                .is_none()
        );

        app.world_mut().entity_mut(actor).insert((LocomotionState::Grounded, AlsNotifyLocomotionAction::Ragdolling));
        app.world_mut()
            .run_system_cached(capture_ragdoll_player_world_transform)
            .unwrap();
        assert!(app.world().get::<ChildOf>(player).is_none());
        assert_eq!(
            app.world()
                .get::<AlsRagdollDetachedPlayer>(player)
                .unwrap()
                .parent,
            avatar
        );
        assert!(
            app.world()
                .get::<Transform>(player)
                .unwrap()
                .translation
                .abs_diff_eq(Vec3::new(4.0, 1.0, -3.0), 1e-5)
        );
    }

    #[test]
    fn animated_ragdoll_body_is_in_the_current_bone_world_space_before_simulation() {
        use avian3d::prelude::{Gravity, PhysicsPlugins};
        use bevy::animation::{AnimatedBy, AnimationTargetId};
        use bevy::time::TimeUpdateStrategy;
        use bevy::world_serialization::{WorldAsset, WorldSerializationPlugin};
        use bevy_animation_graph::core::{
            animated_scene::{AnimatedScene, AnimatedSceneHandle},
            animation_graph::{GraphInputPin, SourcePin, TargetPin},
            animation_node::{NodeLike, ReflectNodeLike},
            context::{new_context::NodeContext, spec_context::SpecContext},
            edge_data::DataSpec,
            errors::GraphError,
            pose::BonePose,
            ragdoll::definition::{Collider as RagdollCollider, ColliderMassMode},
        };

        #[derive(Reflect, Clone, Debug)]
        #[reflect(NodeLike)]
        struct GraphRigModifier;
        impl NodeLike for GraphRigModifier {
            fn display_name(&self) -> String { "Graph rig modifier".into() }
            fn spec(&self, mut ctx: SpecContext) -> Result<(), GraphError> {
                ctx.add_input_data("pose", DataSpec::Pose).add_output_data("Pose", DataSpec::Pose).add_output_time();
                Ok(())
            }
            fn update(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
                let mut pose = ctx.data_back("pose")?.into_pose()?;
                for (name, offset) in [("scene/root/pelvis", Vec3::X * 0.013), ("scene/root/marker", Vec3::Y * 0.07)] {
                    let index = pose.paths[&EntityPath::from_slashed_string(name.into()).id()];
                    let bone = &mut pose.bones[index];
                    bone.translation = bone.translation.map(|value| value + offset);
                    if name.ends_with("pelvis") { bone.rotation = bone.rotation.map(|value| value * Quat::from_rotation_z(0.18)); }
                }
                ctx.set_time(pose.timestamp);
                ctx.set_data_fwd("Pose", pose);
                Ok(())
            }
        }
        fn corrupt_unmapped_ecs_pose(characters: Query<&AlsNotifyLocomotionAction>, mut bones: Query<(&Name, &mut Transform)>) {
            if !characters.iter().any(|state| *state == AlsNotifyLocomotionAction::Ragdolling) { return; }
            for (name, mut transform) in &mut bones {
                if name.as_str() == "marker" { transform.translation.y = 42.0; }
            }
        }

        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            TransformPlugin,
            WorldSerializationPlugin,
            bevy::mesh::MeshPlugin,
            bevy::gizmos::GizmoPlugin,
            PhysicsPlugins::new(PostUpdate),
            bevy_animation_graph::AnimationGraphPlugin::from_physics_schedule(PostUpdate),
        ))
        .register_type::<Name>()
        .register_type::<Transform>()
        .register_type::<GlobalTransform>()
        .register_type::<ChildOf>()
        .register_type::<Children>()
        .register_type::<bevy::animation::AnimationPlayer>()
        .register_type::<AnimationTargetId>()
        .register_type::<AnimatedBy>()
        .insert_resource(Gravity(Vec3::ZERO))
        .insert_resource(TimeUpdateStrategy::ManualDuration(
            std::time::Duration::from_secs_f32(1.0 / 60.0),
        ))
        .add_plugins(crate::ragdolling_layer::RagdollingLayerPlugin)
        .add_plugins(crate::ab_als_root::AlsRootGraphPlugin)
        .add_plugins(als_ext::RagdollPhysicsProfilePlugin)
        .init_asset::<AlsMovementParams>()
        .add_plugins(AlsRagdollLinkPlugin)
        .add_plugins(als_ext::post_physics_animation::PostPhysicsAnimationPlugin)
        .configure_sets(
            PostUpdate,
            crate::ragdolling_layer::AlsRagdollingLayerSet.before(AnimationGraphSet::PrePhysics),
        )
        .add_systems(PostUpdate, corrupt_unmapped_ecs_pose.after(TransformSystems::Propagate))
        .configure_sets(PostUpdate, als_ext::post_physics_animation::PostPhysicsAnimationSet::Readback.after(corrupt_unmapped_ecs_pose))
        .add_systems(
            PostUpdate,
            (
                bevy::transform::systems::mark_dirty_trees,
                bevy::transform::systems::propagate_parent_transforms,
                bevy::transform::systems::sync_simple_transforms,
            )
                .chain()
                .after(als_ext::post_physics_animation::PostPhysicsAnimationSet::Readback)
                .in_set(crate::control_rig_apply::AlsControlRigPropagationSet),
        );
        app.finish();
        app.cleanup();
        app.update();

        let root_path = EntityPath::from_slashed_string("scene".to_owned());
        let armature_path = root_path.child("root");
        let armature_local =
            Transform::from_xyz(0.2, 0.15, -0.1).with_rotation(Quat::from_rotation_y(0.23));
        let joint_path = armature_path.child("pelvis");
        let joint_local =
            Transform::from_xyz(0.1, 0.9, 0.2).with_rotation(Quat::from_rotation_x(0.35));
        let marker_path = armature_path.child("marker");
        let marker_local = Transform::from_xyz(0.0, 0.2, 0.1);
        let mut skeleton = Skeleton::default();
        skeleton.set_root(root_path.id());
        skeleton.add_bone(root_path.clone(), Transform::IDENTITY, Transform::IDENTITY);
        skeleton.add_bone(armature_path.clone(), armature_local, armature_local);
        skeleton.add_bone(
            joint_path.clone(),
            joint_local,
            armature_local * joint_local,
        );
        skeleton.add_bone(marker_path.clone(), marker_local, armature_local * marker_local);
        let skeleton_handle = app
            .world_mut()
            .resource_mut::<Assets<Skeleton>>()
            .add(skeleton);
        let mut pose = Pose {
            skeleton: skeleton_handle.clone(),
            ..default()
        };
        pose.add_bone(BonePose::from_transform(joint_local), joint_path.id());
        pose.add_bone(BonePose::from_transform(armature_local), armature_path.id());
        pose.add_bone(BonePose::from_transform(marker_local), marker_path.id());

        let mut graph = AnimationGraph::new();
        for (output, key, tag) in [
            ("grounded", "locomotion_mode", "Als.LocomotionMode.Grounded"),
            ("ragdolling", "locomotion_action", "Als.LocomotionAction.Ragdolling"),
        ] {
            let node = bevy_animation_graph::core::animation_node::AnimationNode::new(output,
                als_ext::SourceGameplayTagInputNode { key: key.into(), output: "value".into(),
                    expected_tag: tag.into(), ..default() });
            graph.add_node(node.clone());
            graph.add_output_data(output.into(), DataSpec::Bool);
            graph.add_output_data_edge(node.id, "value", output);
        }
        for (name, spec) in [
            ("pose", DataSpec::Pose),
            ("ragdoll_config", DataSpec::RagdollConfig),
        ] {
            graph.add_input_data(GraphInputPin::Passthrough(name.to_owned()), spec.clone());
            graph.add_output_data(name.to_owned(), spec);
            graph.add_edge(
                SourcePin::InputData(GraphInputPin::Passthrough(name.to_owned())),
                TargetPin::OutputData(name.to_owned()),
            );
        }
        let rig = bevy_animation_graph::core::animation_node::AnimationNode::new("rig", GraphRigModifier);
        graph.add_node(rig.clone());
        graph.add_edge(SourcePin::InputData(GraphInputPin::Passthrough("pose".into())), TargetPin::NodeData(rig.id, "pose".into()));
        graph.add_output_data_edge(rig.id, "Pose", "pose");
        graph.set_default_data(
            GraphInputPin::Passthrough("pose".to_owned()),
            DataValue::Pose(pose.clone()),
        );
        graph.set_default_data(
            GraphInputPin::Passthrough("ragdoll_config".to_owned()),
            DataValue::from(RagdollConfig {
                default_mode: Some(BodyMode::Kinematic),
                default_readback: Some(false),
                ..default()
            }),
        );
        let graph_handle = app
            .world_mut()
            .resource_mut::<Assets<AnimationGraph>>()
            .add(graph);
        let mut body = Body::new();
        body.offset = (armature_local * joint_local).translation;
        body.default_mode = BodyMode::Dynamic;
        let body_id = body.id;
        let mut collider = RagdollCollider::new();
        collider.mass_mode = ColliderMassMode::Override(1.0);
        body.colliders.push(collider.id);
        let mut ragdoll = Ragdoll::default();
        ragdoll.add_body(body);
        ragdoll.colliders.insert(collider.id, collider);
        let ragdoll_handle = app
            .world_mut()
            .resource_mut::<Assets<Ragdoll>>()
            .add(ragdoll);
        let mapping = RagdollBoneMap {
            bones_from_bodies: HashMap::from([(
                joint_path.clone(),
                BoneMapping {
                    bone_id: joint_path.clone(),
                    bodies: vec![BodyWeight {
                        body: body_id,
                        weight: 1.0,
                        ..default()
                    }],
                    created_from: None,
                },
            )]),
            bodies_from_bones: HashMap::from([(
                body_id,
                BodyMapping {
                    body_id,
                    bone: BoneWeight {
                        bone: joint_path.clone(),
                        ..default()
                    },
                    created_from: None,
                },
            )]),
            skeleton: skeleton_handle.clone(),
            ragdoll: ragdoll_handle.clone(),
            skeleton_symmetry: default(),
        };
        let mapping_handle = app
            .world_mut()
            .resource_mut::<Assets<RagdollBoneMap>>()
            .add(mapping);
        let actor_world =
            Transform::from_xyz(4.0, 1.5, -3.0).with_rotation(Quat::from_rotation_y(0.6));
        let avatar_local = Transform::from_xyz(0.0, -0.92, 0.0)
            .with_rotation(Quat::from_rotation_y(std::f32::consts::PI));
        let player_world = actor_world * avatar_local;
        let actor = app
            .world_mut()
            .spawn((
                AlsCharacter,
                LocomotionState::Grounded,
                actor_world,
                Stance::Standing,
                Gait::Walking,
                RotationMode::ViewDirection,
                OverlayState::Default,
                crate::actual_motion::AlsActualMotion::default(),
                GlobalTransform::from(actor_world),
                Position(actor_world.translation),
                Rotation(actor_world.rotation),
                RigidBody::Kinematic,
            ))
            .id();
        let mut scene_world = World::new();
        let scene_player = scene_world
            .spawn((
                Name::new("scene"),
                Transform::IDENTITY,
                bevy::animation::AnimationPlayer::default(),
            ))
            .id();
        scene_world.entity_mut(scene_player).insert((
            AnimationTargetId(root_path.id().id()),
            AnimatedBy(scene_player),
        ));
        let armature = scene_world
            .spawn((
                Name::new("root"),
                armature_local,
                ChildOf(scene_player),
                AnimationTargetId(armature_path.id().id()),
                AnimatedBy(scene_player),
            ))
            .id();
        scene_world.spawn((
            Name::new("pelvis"),
            joint_local,
            ChildOf(armature),
            AnimationTargetId(joint_path.id().id()),
            AnimatedBy(scene_player),
        ));
        scene_world.spawn((Name::new("marker"), marker_local, ChildOf(armature),
            AnimationTargetId(marker_path.id().id()), AnimatedBy(scene_player)));
        let source = app
            .world_mut()
            .resource_mut::<Assets<WorldAsset>>()
            .add(WorldAsset::new(scene_world));
        let scene = app
            .world_mut()
            .resource_mut::<Assets<AnimatedScene>>()
            .add(AnimatedScene {
                source,
                processed_scene: None,
                animation_graph: graph_handle,
                retargeting: None,
                skeleton: skeleton_handle,
                ragdoll: Some(ragdoll_handle),
                ragdoll_bone_map: Some(mapping_handle),
            });
        let avatar = app
            .world_mut()
            .spawn((
                AlsCharacterAvatar { character: actor },
                AnimatedSceneHandle::new(scene),
                avatar_local,
                GlobalTransform::from(player_world),
                ChildOf(actor),
            ))
            .id();
        for _ in 0..5 {
            app.update();
        }
        let player_entity = app
            .world()
            .get::<AnimatedSceneInstance>(avatar)
            .expect("animated scene must spawn")
            .player_entity();
        let body_entity = app
            .world()
            .get::<AnimationGraphPlayer>(player_entity)
            .unwrap()
            .get_spawned_ragdoll()
            .unwrap()
            .bodies[&body_id];
        app.world_mut().entity_mut(player_entity).insert((
            als_ext::RagdollPhysicsProfile {
                bodies: vec![],
                colliders: vec![],
                joints: vec![],
                collision_disabled_pairs: std::sync::Arc::from([]),
            },
            RagdollPhysicsProfileApplied {
                body_count: 1,
                ..default()
            },
        ));
        let rigged_joint_local = Transform {
            translation: joint_local.translation + Vec3::X * 0.013,
            rotation: joint_local.rotation * Quat::from_rotation_z(0.18),
            ..joint_local
        };
        let expected = player_world * armature_local * rigged_joint_local;
        let actual =
            Transform::from_translation(app.world().get::<Position>(body_entity).unwrap().0)
                .with_rotation(app.world().get::<Rotation>(body_entity).unwrap().0);
        assert!(
            transforms_match(actual, expected),
            "body world pose {actual:?} must follow animated bone {expected:?}"
        );

        let moved_actor =
            Transform::from_xyz(6.0, 2.0, -2.0).with_rotation(Quat::from_rotation_y(0.9));
        app.world_mut().entity_mut(actor).insert((
            moved_actor,
            Position(moved_actor.translation),
            Rotation(moved_actor.rotation),
        ));
        for _ in 0..3 {
            app.update();
        }
        let expected = moved_actor * avatar_local * armature_local * rigged_joint_local;
        let actual =
            Transform::from_translation(app.world().get::<Position>(body_entity).unwrap().0)
                .with_rotation(app.world().get::<Rotation>(body_entity).unwrap().0);
        assert!(transforms_match(actual, expected));

        app.world_mut().entity_mut(actor).insert((LocomotionState::Grounded, AlsNotifyLocomotionAction::Ragdolling));
        let index = pose.paths[&joint_path.id()];
        pose.bones[index].translation = Some(joint_local.translation + Vec3::X * 0.09);
        app.world_mut().get_mut::<AnimationGraphPlayer>(player_entity).unwrap().set_input_data("pose", DataValue::Pose(pose));
        app.world_mut()
            .get_mut::<AnimationGraphPlayer>(player_entity)
            .unwrap()
            .set_input_data(
                "ragdoll_config",
                DataValue::from(RagdollConfig {
                    default_mode: None,
                    default_readback: Some(true),
                    ..default()
                }),
            );
        app.update();
        assert!(app.world().get::<ChildOf>(player_entity).is_none());
        let player = app.world().get::<AnimationGraphPlayer>(player_entity).unwrap();
        assert_eq!(player.get_outputs()["grounded"].as_bool().unwrap(), true);
        assert_eq!(player.get_outputs()["ragdolling"].as_bool().unwrap(), true);
        assert_eq!(app.world().get::<als_ext::SourceAnimationInputs>(player_entity).unwrap().gameplay_tag_input("locomotion_action"), Some(AlsNotifyLocomotionAction::Ragdolling.tag()));
        let handoff = app
            .world()
            .get::<RagdollPhysicsProfileApplied>(player_entity)
            .unwrap();
        assert_eq!(handoff.handoff_animated_body_count, 1);
        assert!(handoff.handoff_animated_body_position_error_m < 1e-4);
        assert!(handoff.handoff_animated_body_rotation_error_radians < 1e-4);
        assert_eq!(
            *app.world().get::<RigidBody>(body_entity).unwrap(),
            RigidBody::Dynamic
        );
        let joint_entity = app.world().get::<AnimationGraphPlayer>(player_entity).unwrap().bone_entity(joint_path.id()).unwrap();
        let physical_bone = app.world().get::<GlobalTransform>(joint_entity).unwrap().compute_transform();
        assert!(transforms_match(physical_bone, expected), "physics readback must follow the rig pose writes");
        let marker_entity = app.world().get::<AnimationGraphPlayer>(player_entity).unwrap().bone_entity(marker_path.id()).unwrap();
        assert!(app.world().get::<Transform>(marker_entity).unwrap().translation.abs_diff_eq(
            marker_local.translation + Vec3::Y * 0.07, 1e-6), "unmapped bones must keep the rig result");
        let followed_actor =
            Transform::from_xyz(8.0, 1.7, -1.0).with_rotation(Quat::from_rotation_y(-0.4));
        app.world_mut().get_mut::<AnimationGraphPlayer>(player_entity).unwrap().set_input_data(
            "ragdoll_config", DataValue::from(RagdollConfig {
                default_readback: Some(false), readback_overrides: [(joint_path.id(), true)].into(), ..default()
            }));
        app.world_mut().entity_mut(actor).insert((
            followed_actor,
            LocomotionState::None,
            Position(followed_actor.translation),
            Rotation(followed_actor.rotation),
        ));
        app.update();
        let player = app.world().get::<AnimationGraphPlayer>(player_entity).unwrap();
        assert_eq!(player.get_outputs()["grounded"].as_bool().unwrap(), false);
        assert_eq!(player.get_outputs()["ragdolling"].as_bool().unwrap(), true);
        let actual =
            Transform::from_translation(app.world().get::<Position>(body_entity).unwrap().0)
                .with_rotation(app.world().get::<Rotation>(body_entity).unwrap().0);
        assert!(
            transforms_match(actual, expected),
            "actor follow must not move the simulated body"
        );
        assert!(transforms_match(app.world().get::<GlobalTransform>(joint_entity).unwrap().compute_transform(), expected));
        assert!(app.world().get::<Transform>(marker_entity).unwrap().translation.abs_diff_eq(marker_local.translation + Vec3::Y * 0.07, 1e-6));

        app.world_mut().entity_mut(actor).insert((LocomotionState::Air, AlsNotifyLocomotionAction::None));
        app.world_mut()
            .get_mut::<AnimationGraphPlayer>(player_entity)
            .unwrap()
            .set_input_data(
                "ragdoll_config",
                DataValue::from(RagdollConfig {
                    default_mode: Some(BodyMode::Kinematic),
                    default_readback: Some(false),
                    ..default()
                }),
            );
        app.update();
        assert_eq!(
            app.world().get::<ChildOf>(player_entity).unwrap().parent(),
            avatar
        );
        let player = app
            .world()
            .get::<AnimationGraphPlayer>(player_entity)
            .unwrap();
        let snapshot = player
            .get_global_input_data("ragdolling_state.final_ragdoll_pose")
            .unwrap()
            .as_pose()
            .unwrap();
        assert_eq!(player.get_outputs()["ragdolling"].as_bool().unwrap(), false);
        assert_eq!(app.world().get::<als_ext::SourceAnimationInputs>(player_entity).unwrap().gameplay_tag_input("locomotion_action"), Some(AlsNotifyLocomotionAction::None.tag()));
        let pelvis = snapshot.bones[snapshot.paths[&joint_path.id()]].to_transform();
        let restored_world = app
            .world()
            .get::<GlobalTransform>(player_entity)
            .unwrap()
            .compute_transform();
        assert!(
            transforms_match(restored_world * armature_local * pelvis, expected),
            "exit snapshot must preserve the last simulated pelvis world pose"
        );
        let expected_actor_rotation = ragdoll_recovery_rotation(expected.rotation).0;
        assert!(
            app.world()
                .get::<Transform>(actor)
                .unwrap()
                .rotation
                .dot(expected_actor_rotation)
                .abs()
                > 0.99999,
            "recovery actor yaw must follow the source pelvis yaw and facing branch"
        );
    }

    #[test]
    fn recovery_rotation_matches_source_roll_branch_and_poles() {
        for yaw in [-2.0_f32, 0.0, 0.75] {
            for roll in [-0.7_f32, 0.7] {
                for pitch in [
                    -std::f32::consts::FRAC_PI_2,
                    -0.3,
                    0.3,
                    std::f32::consts::FRAC_PI_2,
                ] {
                    let source = Quat::from_euler(EulerRot::ZYX, yaw, -pitch, -roll);
                    let pelvis = Quat::from_rotation_y(std::f32::consts::FRAC_PI_2)
                        * Quat::from_xyzw(-source.x, -source.z, -source.y, source.w);
                    let pole = pitch.abs() == std::f32::consts::FRAC_PI_2;
                    let expected_facing = pole || roll <= 0.0;
                    let source_yaw = if pole {
                        yaw - pitch.signum() * roll
                    } else {
                        yaw
                    };
                    let expected_yaw = -source_yaw
                        + if expected_facing {
                            std::f32::consts::PI
                        } else {
                            0.0
                        };
                    let (rotation, facing) = ragdoll_recovery_rotation(pelvis);
                    assert_eq!(facing, expected_facing);
                    assert!(rotation.dot(Quat::from_rotation_y(expected_yaw)).abs() > 0.99999);
                }
            }
        }
    }

    #[test]
    fn ragdoll_exit_routes_airborne_velocity_and_grounded_recovery() {
        for initial_mode in [
            LocomotionState::None,
            LocomotionState::Grounded,
            LocomotionState::Custom,
        ] {
            let mut app = App::new();
            app.add_plugins((MinimalPlugins, AlsStateTransitionPlugin))
                .add_systems(
                    Update,
                    ragdoll_exit_route_system.before(als_core::transition::state_transition_system),
                );
            let air_velocity = Vec3::new(2.5, -4.0, -1.5);
            let airborne = app
                .world_mut()
                .spawn((
                    AlsCharacter,
                    LocomotionState::None,
                    AlsNotifyLocomotionAction::Ragdolling,
                    Stance::Standing,
                    Gait::Running,
                    RotationMode::VelocityDirection,
                    OverlayState::Default,
                    AlsStateRequest::default().with_locomotion_state(LocomotionState::Grounded),
                    LinearVelocity(Vec3::ZERO),
                    AlsRagdollGroundFollow {
                        pelvis: Vec3::ZERO,
                        target: Vec3::new(0.0, 4.0, 0.0),
                        velocity: air_velocity,
                        grounded: false,
                    },
                ))
                .id();
            let grounded = app
                .world_mut()
                .spawn((
                    AlsCharacter,
                    initial_mode,
                    AlsNotifyLocomotionAction::Ragdolling,
                    Stance::Standing,
                    Gait::Running,
                    RotationMode::VelocityDirection,
                    OverlayState::Default,
                    AlsStateRequest::default().with_locomotion_state(LocomotionState::Grounded),
                    LinearVelocity(Vec3::ONE),
                    AlsRagdollGroundFollow {
                        pelvis: Vec3::ZERO,
                        target: Vec3::Y,
                        velocity: air_velocity,
                        grounded: true,
                    },
                ))
                .id();
            app.world_mut()
                .get_mut::<AlsStateRequest>(airborne)
                .unwrap()
                .ragdoll_exit = true;
            app.world_mut()
                .get_mut::<AlsStateRequest>(grounded)
                .unwrap()
                .ragdoll_exit = true;
            app.update();
            let air_state = *app.world().get::<LocomotionState>(airborne).unwrap();
            for entity in [airborne, grounded] {
                assert_eq!(
                    app.world().get::<AlsNotifyLocomotionAction>(entity),
                    Some(&AlsNotifyLocomotionAction::None)
                );
                assert!(
                    !app.world()
                        .get::<AlsStateRequest>(entity)
                        .unwrap()
                        .ragdoll_exit
                );
            }
            let events: Vec<_> = app
                .world_mut()
                .resource_mut::<Messages<StateTransitionEvent>>()
                .drain()
                .collect();
            assert_eq!(
                events.len(),
                1 + usize::from(initial_mode != LocomotionState::Grounded)
            );
            if let Some(event) = events.iter().find(|event| event.entity == grounded) {
                assert_eq!(event.from, StateValue::LocomotionState(initial_mode));
                assert_eq!(
                    event.to,
                    StateValue::LocomotionState(LocomotionState::Grounded)
                );
            }
            let air_after = app.world().get::<LinearVelocity>(airborne).unwrap().0;
            let air_hold = app.world().get::<AlsRagdollAirExit>(airborne).is_some();
            let ground_state = *app.world().get::<LocomotionState>(grounded).unwrap();
            let ground_after = app.world().get::<LinearVelocity>(grounded).unwrap().0;
            let passed = air_state == LocomotionState::Air
                && air_after.distance(air_velocity) <= 1e-6
                && air_hold
                && ground_state == LocomotionState::Grounded
                && ground_after.length_squared() <= 1e-6;
            let verdict = if passed { "PASS" } else { "FAIL" };
            println!(
                "[BF102.50][VERDICT] air_state={air_state:?} air_hold={air_hold} air_velocity=({:.3},{:.3},{:.3}) ground_state={ground_state:?} ground_velocity={:.3} -> {verdict}",
                air_after.x,
                air_after.y,
                air_after.z,
                ground_after.length(),
            );
            assert!(passed);
        }
    }

    #[test]
    fn joint_drive_uses_source_constraint_drive_values() {
        let idle = ragdoll_joint_drive_for_speed(0.0);
        let half = ragdoll_joint_drive_for_speed(5.0);
        let full = ragdoll_joint_drive_for_speed(10.0);
        let angular_stiffness = full.angular_stiffness;
        let passed = idle.speed_amount <= 1e-6
            && (half.speed_amount - 0.5).abs() <= 1e-6
            && (full.speed_amount - 1.0).abs() <= 1e-6
            && idle.angular_stiffness.abs() <= 1e-6
            && (half.angular_stiffness - 12_500.0).abs() <= 1e-6
            && (full.angular_stiffness - 25_000.0).abs() <= 1e-6
            && full.angular_damping.abs() <= 1e-6
            && full.angular_force_limit <= 1e-6
            && (angular_stiffness - 25_000.0).abs() <= 1e-6;
        let verdict = if passed { "PASS" } else { "FAIL" };
        println!(
            "[BF102.51][VERDICT] idle={:.3} half={:.3}/{:.1} full={:.3}/{:.1} angular_stiffness={angular_stiffness:.3} -> {verdict}",
            idle.speed_amount,
            half.speed_amount,
            half.angular_stiffness,
            full.speed_amount,
            full.angular_stiffness,
        );
        assert!(passed);
    }
}
