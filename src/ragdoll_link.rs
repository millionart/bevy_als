use als_core::character::{
    ALS_CHARACTER_CAPSULE_HALF_HEIGHT, ALS_CHARACTER_CAPSULE_RADIUS, ALS_CHARACTER_FLOOR_CLEARANCE,
    AlsCharacter, AlsMovementParamsHandle,
};
use als_core::params::AlsMovementParams;
use als_core::state::{LocomotionState, Stance};
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

const PELVIS_LABEL: &str = "pelvis";

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
    collision_layers: Option<CollisionLayers>,
    tnua_toggle: Option<TnuaToggle>,
}

#[derive(Component, Debug, Clone, Copy, Default)]
pub struct AlsRagdollGroundFollow {
    pub pelvis: Vec3,
    pub target: Vec3,
    pub grounded: bool,
}

pub struct AlsRagdollLinkPlugin;

impl Plugin for AlsRagdollLinkPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(disable_spawned_ragdoll_collider);
        app.add_systems(
            Update,
            (
                ensure_ragdoll_link,
                ragdoll_character_physics_system.after(ensure_ragdoll_link),
                ragdoll_link_system.after(ragdoll_character_physics_system),
            ),
        )
        .add_systems(
            FixedPostUpdate,
            (
                sync_ragdoll_body_modes.after(AnimationGraphSet::PrePhysics),
                ApplyDeferred,
            )
                .chain()
                .before(PhysicsSystems::First),
        )
        .add_systems(
            FixedPostUpdate,
            (
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
    characters: Query<&LocomotionState, With<als_core::character::AlsCharacter>>,
    instances: Query<&AnimatedSceneInstance>,
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
    let Ok(locomotion) = characters.single() else {
        return;
    };
    let ragdolling = *locomotion == LocomotionState::Ragdolling;
    for instance in &instances {
        let Ok(player) = players.get(instance.player_entity()) else {
            continue;
        };
        let Some(spawned) = player.get_spawned_ragdoll() else {
            continue;
        };
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
            Option<&TnuaToggle>,
            Option<&AlsRagdollCharacterPhysics>,
            Option<&AlsRagdollGroundFollow>,
        ),
        With<AlsCharacter>,
    >,
) {
    let Ok((
        entity,
        mut transform,
        mut position,
        mut velocity,
        locomotion,
        rigid_body,
        collision_layers,
        tnua_toggle,
        previous_physics,
        ground_follow,
    )) = characters.single_mut()
    else {
        return;
    };
    let is_ragdoll = *locomotion == LocomotionState::Ragdolling;
    if is_ragdoll {
        if previous_physics.is_none() {
            commands.entity(entity).insert(AlsRagdollCharacterPhysics {
                rigid_body: *rigid_body,
                collision_layers: collision_layers.copied(),
                tnua_toggle: tnua_toggle.copied(),
            });
        }
        if *rigid_body != RigidBody::Kinematic {
            commands.entity(entity).insert(RigidBody::Kinematic);
        }
        if collision_layers.is_none_or(|layers| *layers != CollisionLayers::NONE) {
            commands.entity(entity).insert(CollisionLayers::NONE);
        }
        if tnua_toggle.is_none_or(|toggle| *toggle != TnuaToggle::Disabled) {
            commands.entity(entity).insert(TnuaToggle::Disabled);
        }
        return;
    }
    let Some(previous_physics) = previous_physics else {
        return;
    };
    if let Some(follow) = ground_follow {
        transform.translation = follow.target;
        if let Some(position) = position.as_deref_mut() {
            position.0 = follow.target;
        }
        if let Some(velocity) = velocity.as_deref_mut() {
            velocity.0 = Vec3::ZERO;
        }
        info!(
            "[RAGLINK] exit realign to ground ({:.2},{:.2},{:.2})",
            follow.target.x, follow.target.y, follow.target.z
        );
    }
    commands.entity(entity).insert(previous_physics.rigid_body);
    match previous_physics.collision_layers {
        Some(layers) => {
            commands.entity(entity).insert(layers);
        }
        None => {
            commands.entity(entity).remove::<CollisionLayers>();
        }
    }
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
    instances: Query<&AnimatedSceneInstance>,
    players: Query<&AnimationGraphPlayer>,
    labels: Query<&BodyLabel>,
    body_positions: Query<&Position, Without<AlsCharacter>>,
) {
    let Ok((entity, _position, locomotion, stance, params_handle)) = characters.single() else {
        return;
    };
    if *locomotion != LocomotionState::Ragdolling {
        return;
    }
    let Ok(instance) = instances.single() else {
        return;
    };
    let Ok(player) = players.get(instance.player_entity()) else {
        return;
    };
    let Some(spawned) = player.get_spawned_ragdoll() else {
        return;
    };
    let Some(pelvis_entity) = find_body_by_label(spawned, &labels, PELVIS_LABEL) else {
        return;
    };
    let Ok(pelvis_position) = body_positions.get(pelvis_entity) else {
        return;
    };
    let pelvis = pelvis_position.0;
    let (half_height, radius) = ragdoll_capsule_dimensions(*stance, params_handle, &params_assets);
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
        grounded,
    });
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
    let Ok((mut position, mut transform, mut velocity, locomotion, ground_follow)) =
        characters.single_mut()
    else {
        return;
    };
    if *locomotion != LocomotionState::Ragdolling {
        return;
    }
    position.0 = ground_follow.target;
    transform.translation = ground_follow.target;
    velocity.0 = Vec3::ZERO;
}

#[allow(clippy::type_complexity)]
fn ragdoll_link_system(
    mut commands: Commands,
    mut q: Query<(&LocomotionState, &mut AlsRagdollLink), With<AlsCharacter>>,
    instances: Query<&AnimatedSceneInstance>,
    mut players: Query<&mut AnimationGraphPlayer>,
    layers: Query<(Option<&AlsRagdollColliderLayers>, &CollisionLayers)>,
) {
    let Ok((loco, mut link)) = q.single_mut() else {
        return;
    };
    let is_ragdoll = *loco == LocomotionState::Ragdolling;

    let Ok(instance) = instances.single() else {
        link.prev_ragdoll = is_ragdoll;
        return;
    };
    let Ok(mut player) = players.get_mut(instance.player_entity()) else {
        link.prev_ragdoll = is_ragdoll;
        return;
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
