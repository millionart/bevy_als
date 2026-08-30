use als_core::character::{
    ALS_CHARACTER_CAPSULE_HALF_HEIGHT, ALS_CHARACTER_CAPSULE_RADIUS, ALS_CHARACTER_FLOOR_CLEARANCE,
    AlsCharacter, AlsCollisionLayerOwner, AlsCollisionLayerOwnership, AlsMovementParamsHandle,
    set_collision_layer_owner,
};
use als_core::motion::AlsRagdollAirExit;
use als_core::params::AlsMovementParams;
use als_core::state::{LocomotionState, Stance};
use als_ext::ragdoll_angular_drive::{
    RagdollAngularDrive, RagdollAngularDriveMetrics, RagdollAngularDriveSet,
};
use avian3d::prelude::{
    AngularVelocity, Collider, CollisionLayers, LinearVelocity, PhysicsSystems, Position,
    RigidBody, Rotation, ShapeCastConfig, SpatialQuery, SpatialQueryFilter,
};
use bevy::ecs::schedule::ApplyDeferred;
use bevy::prelude::*;
use bevy_animation_graph::core::animated_scene::AnimatedSceneInstance;
use bevy_animation_graph::core::animation_graph_player::AnimationGraphPlayer;
use bevy_animation_graph::core::edge_data::DataValue;
use bevy_animation_graph::core::plugin::AnimationGraphSet;
use bevy_animation_graph::core::ragdoll::spawning::{RagdollColliderSpawned, SpawnedRagdoll};
use bevy_animation_graph::core::ragdoll::{
    definition::BodyLabel,
    relative_kinematic_body::{RelativeKinematicBody, RelativeKinematicBodyPositionBased},
};
use bevy_tnua::TnuaToggle;

use crate::character_assembly::AlsCharacterAvatar;

const PELVIS_LABEL: &str = "pelvis";
pub const RAGDOLL_REFERENCE_SPEED_METERS_PER_SECOND: f32 = 10.0;
pub const RAGDOLL_ANGULAR_STIFFNESS: f32 = 25_000.0;
const RAGDOLL_UE_STIFFNESS_TO_AVIAN_TORQUE: f32 = 0.0001;

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
    let speed_amount =
        (speed_meters_per_second / RAGDOLL_REFERENCE_SPEED_METERS_PER_SECOND).clamp(0.0, 1.0);
    AlsRagdollJointDrive {
        speed_amount,
        angular_stiffness: speed_amount * RAGDOLL_ANGULAR_STIFFNESS,
        angular_damping: 0.0,
        angular_force_limit: 0.0,
        target_joint_count: 0,
        driven_joint_count: 0,
    }
}

pub fn ragdoll_torque_stiffness(angular_stiffness: f32) -> f32 {
    angular_stiffness.max(0.0) * RAGDOLL_UE_STIFFNESS_TO_AVIAN_TORQUE
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
        app.add_observer(disable_spawned_ragdoll_collider);
        app.add_systems(
            Update,
            (
                ensure_ragdoll_link,
                ragdoll_exit_route_system
                    .after(als_core::input_request::input_state_request_system)
                    .before(als_core::transition::state_transition_system),
                ragdoll_character_physics_system
                    .after(ensure_ragdoll_link)
                    .after(als_core::transition::state_transition_system),
                ragdoll_link_system.after(ragdoll_character_physics_system),
            ),
        )
        .add_systems(
            FixedPostUpdate,
            (
                sync_ragdoll_body_modes.after(AnimationGraphSet::PrePhysics),
                ragdoll_joint_drive_adapter_system,
                ApplyDeferred,
            )
                .chain()
                .before(RagdollAngularDriveSet),
        )
        .add_systems(
            FixedPostUpdate,
            (
                release_ragdoll_air_exit_on_ground.after(PhysicsSystems::Last),
                ragdoll_ground_follow.after(PhysicsSystems::Last),
                ApplyDeferred,
                apply_ragdoll_ground_follow,
            )
                .chain()
                .before(AnimationGraphSet::PostPhysics),
        );
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

fn sync_ragdoll_body_modes(
    mut commands: Commands,
    characters: Query<(Entity, &LocomotionState), With<als_core::character::AlsCharacter>>,
    avatars: Query<(&AlsCharacterAvatar, &AnimatedSceneInstance)>,
    players: Query<&AnimationGraphPlayer>,
    mut bodies: Query<(
        Entity,
        &RigidBody,
        &mut Position,
        &mut Rotation,
        &mut LinearVelocity,
        &mut AngularVelocity,
        &mut RelativeKinematicBody,
        &RelativeKinematicBodyPositionBased,
        Has<AlsRagdollPhysicsActive>,
    )>,
    bases: Query<
        (&Position, &Rotation, &LinearVelocity),
        Without<RelativeKinematicBodyPositionBased>,
    >,
) {
    for (character, locomotion) in &characters {
        let Some(instance) = avatars
            .iter()
            .find_map(|(avatar, instance)| (avatar.character == character).then_some(instance))
        else {
            continue;
        };
        let Ok(player) = players.get(instance.player_entity()) else {
            continue;
        };
        let Some(spawned) = player.get_spawned_ragdoll() else {
            continue;
        };
        let ragdolling = *locomotion == LocomotionState::Ragdolling;
        for body_entity in spawned.bodies.values() {
            let Ok((
                entity,
                mode,
                mut position,
                mut rotation,
                mut linear_velocity,
                mut angular_velocity,
                mut relative_body,
                target,
                activated,
            )) = bodies.get_mut(*body_entity)
            else {
                continue;
            };
            if ragdolling {
                if !activated {
                    let (world_position, world_rotation, inherited_velocity) = target
                        .relative_to
                        .and_then(|base| bases.get(base).ok())
                        .map(|(base_position, base_rotation, base_velocity)| {
                            (
                                base_position.0
                                    + base_rotation.0
                                        * Vec3::from(target.relative_target.translation),
                                base_rotation.0 * target.relative_target.rotation,
                                base_velocity.0,
                            )
                        })
                        .unwrap_or((
                            Vec3::from(target.relative_target.translation),
                            target.relative_target.rotation,
                            Vec3::ZERO,
                        ));
                    position.0 = world_position;
                    rotation.0 = world_rotation;
                    linear_velocity.0 = inherited_velocity;
                    angular_velocity.0 = Vec3::ZERO;
                    commands.entity(entity).insert(AlsRagdollPhysicsActive);
                }
                if *mode != RigidBody::Dynamic {
                    commands.entity(entity).insert(RigidBody::Dynamic);
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
        (Entity, &LocomotionState, Option<&mut AlsRagdollJointDrive>),
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
    for (character, locomotion, current_drive) in &mut characters {
        let mut drive = AlsRagdollJointDrive::default();
        let player = avatars
            .iter()
            .find_map(|(avatar, instance)| (avatar.character == character).then_some(instance))
            .and_then(|instance| players.get(instance.player_entity()).ok());
        if let Some((player_entity, player, metrics)) = player {
            if *locomotion == LocomotionState::Ragdolling {
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
            let torque_stiffness = ragdoll_torque_stiffness(drive.angular_stiffness);
            if let Ok(mut angular_drive) = angular_drives.get_mut(player_entity) {
                angular_drive.torque_stiffness = torque_stiffness;
            } else {
                commands.entity(player_entity).insert((
                    RagdollAngularDrive { torque_stiffness },
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
) -> (f32, f32) {
    let Some(params) = params_assets.get(params_handle.0.id()) else {
        return (
            ALS_CHARACTER_CAPSULE_HALF_HEIGHT,
            ALS_CHARACTER_CAPSULE_RADIUS,
        );
    };
    let half_height = match stance {
        Stance::Standing => params.crouch.standing_capsule_half_height,
        Stance::Crouching => params.crouch.crouched_capsule_half_height,
    };
    (half_height, params.crouch.standing_capsule_radius)
}

pub fn ragdoll_exit_route_system(
    mut commands: Commands,
    mut characters: Query<
        (
            Entity,
            &LocomotionState,
            &mut als_core::transition::AlsStateRequest,
            Option<&AlsRagdollGroundFollow>,
            Option<&mut LinearVelocity>,
        ),
        With<AlsCharacter>,
    >,
) {
    for (entity, locomotion, mut request, follow, velocity) in &mut characters {
        if !request.ragdoll_exit {
            continue;
        }
        if *locomotion != LocomotionState::Ragdolling {
            request.ragdoll_exit = false;
            continue;
        }
        let Some(follow) = follow else {
            continue;
        };
        let (state, exit_velocity) = ragdoll_exit_route(follow.grounded, follow.velocity);
        request.locomotion_state = Some(state);
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
    params_assets: Res<Assets<AlsMovementParams>>,
    characters: Query<
        (Entity, &Position, &Stance, &AlsMovementParamsHandle),
        (With<AlsCharacter>, With<AlsRagdollAirExit>),
    >,
) {
    for (entity, position, stance, params_handle) in &characters {
        let (half_height, radius) =
            ragdoll_capsule_dimensions(*stance, params_handle, &params_assets);
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
    mut characters: Query<
        (
            Entity,
            &mut Transform,
            Option<&mut Position>,
            Option<&mut LinearVelocity>,
            &LocomotionState,
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
        locomotion,
        rigid_body,
        collision_layers,
        mut collision_ownership,
        tnua_toggle,
        previous_physics,
        ground_follow,
    ) in &mut characters
    {
        let is_ragdoll = *locomotion == LocomotionState::Ragdolling;
        if is_ragdoll {
            if previous_physics.is_none() {
                commands.entity(entity).insert(AlsRagdollCharacterPhysics {
                    rigid_body: *rigid_body,
                    tnua_toggle: tnua_toggle.copied(),
                });
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
    }
}

#[allow(clippy::type_complexity)]
fn ragdoll_ground_follow(
    mut commands: Commands,
    spatial: SpatialQuery,
    params_assets: Res<Assets<AlsMovementParams>>,
    characters: Query<
        (
            Entity,
            &Position,
            &LocomotionState,
            &Stance,
            &AlsMovementParamsHandle,
        ),
        With<AlsCharacter>,
    >,
    avatars: Query<(&AlsCharacterAvatar, &AnimatedSceneInstance)>,
    players: Query<&AnimationGraphPlayer>,
    labels: Query<&BodyLabel>,
    body_states: Query<(&Position, &LinearVelocity), Without<AlsCharacter>>,
) {
    for (entity, _position, locomotion, stance, params_handle) in &characters {
        if *locomotion != LocomotionState::Ragdolling {
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
        let (half_height, radius) =
            ragdoll_capsule_dimensions(*stance, params_handle, &params_assets);
        let trace_start = pelvis + Vec3::Y * (2.0 * radius);
        let trace_distance = half_height + radius;
        let mut excluded = Vec::with_capacity(1 + spawned.bodies.len() + spawned.colliders.len());
        excluded.push(entity);
        excluded.extend(spawned.bodies.values().copied());
        excluded.extend(spawned.colliders.values().copied());
        let filter = SpatialQueryFilter::from_excluded_entities(excluded);
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
            &LocomotionState,
            &AlsRagdollGroundFollow,
        ),
        With<AlsCharacter>,
    >,
) {
    for (mut position, mut transform, mut velocity, locomotion, ground_follow) in &mut characters {
        if *locomotion != LocomotionState::Ragdolling {
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
    mut q: Query<(Entity, &LocomotionState, &mut AlsRagdollLink), With<AlsCharacter>>,
    avatars: Query<(&AlsCharacterAvatar, &AnimatedSceneInstance)>,
    mut players: Query<&mut AnimationGraphPlayer>,
    layers: Query<(Option<&AlsRagdollColliderLayers>, &CollisionLayers)>,
) {
    for (character, loco, mut link) in &mut q {
        let is_ragdoll = *loco == LocomotionState::Ragdolling;
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
    use als_core::state::{Gait, OverlayState, RotationMode};
    use als_core::transition::{AlsStateRequest, AlsStateTransitionPlugin};

    #[test]
    fn ragdoll_exit_routes_airborne_velocity_and_grounded_recovery() {
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
                LocomotionState::Ragdolling,
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
                LocomotionState::Ragdolling,
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

    #[test]
    fn joint_drive_scales_source_stiffness_with_pelvis_speed() {
        let idle = ragdoll_joint_drive_for_speed(0.0);
        let half = ragdoll_joint_drive_for_speed(5.0);
        let full = ragdoll_joint_drive_for_speed(10.0);
        let torque_stiffness = ragdoll_torque_stiffness(full.angular_stiffness);
        let passed = idle.speed_amount <= 1e-6
            && idle.angular_stiffness <= 1e-6
            && (half.speed_amount - 0.5).abs() <= 1e-6
            && (half.angular_stiffness - 12_500.0).abs() <= 1e-3
            && (full.speed_amount - 1.0).abs() <= 1e-6
            && (full.angular_stiffness - RAGDOLL_ANGULAR_STIFFNESS).abs() <= 1e-3
            && full.angular_damping <= 1e-6
            && full.angular_force_limit <= 1e-6
            && (torque_stiffness - 2.5).abs() <= 1e-3;
        let verdict = if passed { "PASS" } else { "FAIL" };
        println!(
            "[BF102.51][VERDICT] idle={:.3} half={:.3}/{:.1} full={:.3}/{:.1} torque_stiffness={torque_stiffness:.3} -> {verdict}",
            idle.speed_amount,
            half.speed_amount,
            half.angular_stiffness,
            full.speed_amount,
            full.angular_stiffness,
        );
        assert!(passed);
    }
}
