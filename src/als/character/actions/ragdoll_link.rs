use crate::als::character::{
    ALS_CHARACTER_CAPSULE_HALF_HEIGHT, ALS_CHARACTER_CAPSULE_RADIUS, ALS_CHARACTER_FLOOR_CLEARANCE,
    AlsCharacter, AlsCollisionLayerOwner, AlsCollisionLayerOwnership, AlsMovementParamsHandle,
    set_collision_layer_owner,
};
use crate::als::mantle::scaled_mantle_capsule_dimensions;
use crate::als::motion::AlsRagdollAirExit;
use crate::als::params::AlsMovementParams;
use crate::als::state::{AlsNotifyLocomotionAction, LocomotionState, Stance};
use crate::als::transition::{Dimension, StateTransitionEvent, StateValue, commit_state};
use crate::extensions::SceneQuery;
use crate::extensions::SourceCollisionIgnorePawn;
use crate::extensions::post_physics_animation::source_kinematic_angular_velocity;
use crate::extensions::ragdoll_angular_drive::{
    RagdollAngularDrive, RagdollAngularDriveMetrics, RagdollAngularDriveSet,
};
use crate::extensions::{
    RagdollActiveMode, RagdollCollisionMember, RagdollPhysicsHandoffSet,
    RagdollPhysicsProfileApplied,
};
use crate::source_physics_timing::source_physics_delta;
use avian3d::prelude::{
    AngularVelocity, Collider, CollisionLayers, LinearVelocity, PhysicsSystems, Position,
    RigidBody, Rotation, ShapeCastConfig, ShapeHitData, SpatialQueryFilter,
};
use bevy::ecs::schedule::ApplyDeferred;
use bevy::math::{DQuat, Isometry3d};
use bevy::prelude::*;
use bevy::reflect::{PartialReflect, ReflectMut};
use bevy::transform::helper::TransformHelper;
use bevy_animation_graph::core::animated_scene::{
    AnimatedScene, AnimatedSceneHandle, AnimatedSceneInstance,
};
use bevy_animation_graph::core::animation_graph_player::AnimationGraphPlayer;
use bevy_animation_graph::core::edge_data::DataValue;
use bevy_animation_graph::core::physics_systems_avian::{
    spawn_missing_ragdolls_avian, update_ragdoll_rigidbodies,
    update_relative_kinematic_body_velocities,
};
use bevy_animation_graph::core::plugin::AnimationGraphSet;
use bevy_animation_graph::core::pose::{BonePose, Pose};
use bevy_animation_graph::core::ragdoll::spawning::{
    RagdollColliderSpawned, RagdollSpawned, SpawnedRagdoll,
};
use bevy_animation_graph::core::ragdoll::{
    bone_mapping::RagdollBoneMap,
    definition::{BodyLabel, BodyMode},
    relative_kinematic_body::RelativeKinematicBody,
};
use bevy_animation_graph::core::skeleton::Skeleton;
use bevy_tnua::TnuaToggle;

use crate::character_assembly::{
    AlsCharacterAssembly, AlsCharacterAvatar, animated_player_for_character,
};

const PELVIS_LABEL: &str = "pelvis";
pub const RAGDOLL_REFERENCE_SPEED_METERS_PER_SECOND: f32 = 10.0;
pub const RAGDOLL_ANGULAR_STIFFNESS: f32 = 25_000.0;

#[derive(Component, Debug, Default, Clone, Copy)]
pub struct AlsRagdollLink {
    pub prev_ragdoll: bool,
}

#[derive(Component, Debug, Clone, Copy)]
struct AlsRagdollColliderLayers(CollisionLayers);

type RagdollColliderLayersQuery<'w, 's> = Query<
    'w,
    's,
    (
        Option<&'static AlsRagdollColliderLayers>,
        &'static CollisionLayers,
        Option<&'static RagdollCollisionMember>,
        Has<SourceCollisionIgnorePawn>,
    ),
>;

#[derive(Component, Debug, Default, Clone, Copy)]
struct AlsRagdollPhysicsActive;

#[derive(Component, Debug, Clone, Copy)]
struct AlsRagdollScaleState {
    root: Entity,
    previous: Vec3,
    stopped: bool,
    ragdolling: bool,
}

impl AlsRagdollScaleState {
    fn uniform_scale_changed(&self, scale: Vec3) -> bool {
        scale.max_element() - scale.min_element() <= 1e-4
            && (scale - self.previous).abs().max_element() > 1e-4
    }

    fn recovering_from_zero(self, scale: Vec3) -> bool {
        if !self.ragdolling || !self.stopped {
            return false;
        }
        let mut next = self;
        next.update(scale, true);
        !next.stopped
    }

    fn update(&mut self, scale: Vec3, ragdolling: bool) {
        if self.uniform_scale_changed(scale) {
            let zero = scale.abs().max_element() <= 1e-4;
            let was_zero = self.previous.abs().max_element() <= 1e-4;
            if zero != was_zero {
                self.stopped = zero;
            }
            self.previous = scale;
        }
        if ragdolling && !self.ragdolling {
            self.stopped = false;
        }
        self.ragdolling = ragdolling;
    }
}

#[derive(Component, Debug, Clone, Copy)]
struct AlsRagdollCharacterPhysics {
    rigid_body: RigidBody,
    tnua_toggle: Option<TnuaToggle>,
}

#[derive(Component, Debug, Clone, Copy)]
#[doc(hidden)]
pub struct AlsRagdollDetachedPlayer {
    pub character: Entity,
    parent: Entity,
    local_transform: Transform,
}

#[derive(Component, Debug, Clone, Copy)]
#[doc(hidden)]
pub struct AlsRagdollInitialSpeedLimit {
    pub remaining_updates: u8,
    speed: f32,
}

impl AlsRagdollInitialSpeedLimit {
    fn new(entry_speed: f32) -> Self {
        Self {
            remaining_updates: crate::get_up::RAGDOLL_SETTINGS.initial_speed_limit_refresh_frames,
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

const MESH_IGNORES_PAWN: bool = include!("../../settings/mesh_pawn_ignored.rs");

impl Plugin for AlsRagdollLinkPlugin {
    fn build(&self, app: &mut App) {
        let outer = crate::AlsPhysicsSchedule::outer(app);
        let prepare = crate::AlsPhysicsSchedule::prepare(app);
        app.add_message::<StateTransitionEvent>()
            .add_observer(disable_spawned_ragdoll_collider)
            .add_observer(register_ragdoll_camera_trace_owner);
        app.add_systems(
            prepare,
            (
                ensure_ragdoll_link,
                (
                    avian3d::collider_tree::update_moved_collider_aabbs::<Collider>.run_if(
                        |characters: Query<(
                            &crate::als::transition::AlsStateRequest,
                            &AlsNotifyLocomotionAction,
                        )>| {
                            characters.iter().any(|(request, action)| {
                                request.ragdoll_exit
                                    && *action == AlsNotifyLocomotionAction::Ragdolling
                            })
                        },
                    ),
                    ragdoll_exit_route_system,
                )
                    .chain()
                    .after(crate::als::input_request::input_state_request_system)
                    .after(crate::als::motion::locomotion_drive_system)
                    .before(crate::als::transition::state_transition_system),
                (
                    capture_ragdoll_player_world_transform,
                    ragdoll_character_physics_system,
                    (restore_ragdoll_player_parent, ragdoll_link_system),
                )
                    .chain()
                    .after(crate::als::transition::state_transition_system)
                    .after(ensure_ragdoll_link),
            ).in_set(crate::playable_character::AlsPhysicsPrepareSet),
        )
        .add_systems(
            outer,
            reset_ragdoll_on_scale_recovery
                .before(spawn_missing_ragdolls_avian)
                .in_set(AnimationGraphSet::PrePhysics),
        )
        .add_systems(
            outer,
            set_simulating_ragdoll_body_modes
                .after(update_ragdoll_rigidbodies)
                .before(update_relative_kinematic_body_velocities)
                .in_set(AnimationGraphSet::PrePhysics),
        )
        .add_systems(
            outer,
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
            outer,
            (
                capture_ragdoll_player_world_transform,
                ragdoll_character_physics_system,
                ragdoll_link_system,
                sync_ragdoll_body_modes,
            )
                .chain()
                .after(crate::als::motion::AlsActualVelocitySet)
                .before(crate::actual_motion::AlsActualMotionSet)
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
            outer,
            (
                sync_ragdoll_body_modes,
                (
                    ragdoll_ground_follow,
                    ApplyDeferred,
                    apply_ragdoll_ground_follow,
                )
                    .chain()
                    .run_if(|time: Res<Time<Virtual>>| !time.is_paused()),
                ragdoll_joint_drive_adapter_system,
                limit_initial_ragdoll_speed,
                ApplyDeferred,
            )
                .chain()
                .in_set(RagdollPhysicsHandoffSet)
                .before(RagdollAngularDriveSet),
        )
        .add_systems(
            outer,
            release_ragdoll_air_exit_on_ground
                .after(PhysicsSystems::Last)
                .before(AnimationGraphSet::PostPhysics),
        )
        .add_systems(
            PostUpdate,
            sync_animated_ragdoll_bodies
                .after(crate::control_rig_apply::AlsControlRigPropagationSet),
        );
    }
}

pub(crate) fn register_ragdoll_camera_trace_owner(
    event: On<RagdollSpawned>,
    avatars: Query<(Entity, &AlsCharacterAvatar, &AnimatedSceneInstance)>,
    assemblies: Query<&AlsCharacterAssembly>,
    owners: Query<&crate::als_camera::AlsCameraTraceOwner>,
    mut commands: Commands,
) {
    if let Some((avatar_entity, avatar, _)) = avatars
        .iter()
        .find(|(_, _, scene)| scene.player_entity() == event.animation_player)
        && assemblies
            .get(avatar.character)
            .is_ok_and(|assembly| assembly.avatar == avatar_entity)
        && !owners
            .get(event.ragdoll)
            .is_ok_and(|owner| owner.0 == avatar.character)
    {
        commands
            .entity(event.ragdoll)
            .insert(crate::als_camera::AlsCameraTraceOwner(avatar.character));
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
            With<crate::als::character::AlsCharacter>,
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

#[derive(bevy::ecs::system::SystemParam)]
pub struct AlsRagdollStartEligibility<'w, 's> {
    assemblies: Query<'w, 's, &'static AlsCharacterAssembly>,
    avatars: Query<'w, 's, (&'static AlsCharacterAvatar, &'static AnimatedSceneInstance)>,
    players: Query<'w, 's, &'static AnimationGraphPlayer>,
    labels: Query<'w, 's, &'static BodyLabel>,
    bodies: Query<'w, 's, (), With<RigidBody>>,
}

impl AlsRagdollStartEligibility<'_, '_> {
    pub(crate) fn allows(&self, character: Entity) -> bool {
        let Some(player) = animated_player_for_character(
            character,
            self.assemblies.get(character).ok(),
            &self.avatars,
        )
        .and_then(|player| self.players.get(player).ok()) else {
            return false;
        };
        let Some(spawned) = player.get_spawned_ragdoll() else {
            return false;
        };
        ["pelvis", "spine_03"].into_iter().all(|label| {
            find_body_by_label(spawned, &self.labels, label)
                .is_some_and(|body| self.bodies.contains(body))
        })
    }
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

fn landed_into_ragdoll(transitions: &Messages<StateTransitionEvent>, character: Entity) -> bool {
    let air = StateValue::LocomotionState(LocomotionState::Air);
    let grounded = StateValue::LocomotionState(LocomotionState::Grounded);
    let none = StateValue::LocomotionState(LocomotionState::None);
    let mut landed = false;
    transitions
        .iter_current_update_messages()
        .filter(|event| event.entity == character && event.dimension == Dimension::LocomotionState)
        .any(|event| {
            let starts_ragdoll = landed && event.from == grounded && event.to == none;
            landed = event.from == air && event.to == grounded;
            starts_ragdoll
        })
}

fn capture_ragdoll_player_world_transform(
    mut commands: Commands,
    transform_helper: TransformHelper,
    transitions: Option<Res<Messages<StateTransitionEvent>>>,
    characters: Query<
        (
            Entity,
            Option<&AlsNotifyLocomotionAction>,
            Option<&AlsCharacterAssembly>,
        ),
        (With<AlsCharacter>, Without<AlsRagdollCharacterPhysics>),
    >,
    avatars: Query<(&AlsCharacterAvatar, &AnimatedSceneInstance)>,
    player_parents: Query<
        (&ChildOf, &Transform),
        (With<AnimationGraphPlayer>, Without<AlsCharacter>),
    >,
    detached_players: Query<(Entity, &AlsRagdollDetachedPlayer)>,
    assemblies: Query<&AlsCharacterAssembly, With<AlsCharacter>>,
    alive: Query<()>,
    children: Query<&Children>,
    mut globals: Query<&mut GlobalTransform>,
) {
    for (player, detached) in &detached_players {
        if !alive.contains(detached.parent)
            || animated_player_for_character(
                detached.character,
                assemblies.get(detached.character).ok(),
                &avatars,
            ) != Some(player)
        {
            commands.entity(player).despawn();
        }
    }
    for (character, action, assembly) in &characters {
        if action != Some(&AlsNotifyLocomotionAction::Ragdolling) {
            continue;
        }
        let Some(player) = animated_player_for_character(character, assembly, &avatars) else {
            continue;
        };
        if detached_players.get(player).is_ok() {
            continue;
        }
        let Ok((parent, local_transform)) = player_parents.get(player) else {
            continue;
        };
        let world_transform = if transitions
            .as_deref()
            .is_some_and(|transitions| landed_into_ragdoll(transitions, character))
        {
            globals
                .get(player)
                .ok()
                .map(GlobalTransform::compute_transform)
        } else {
            refresh_player_subtree_globals(&transform_helper, player, &children, &mut globals)
        };
        let Some(world_transform) = world_transform else {
            continue;
        };
        commands.entity(player).remove::<ChildOf>().insert((
            world_transform,
            AlsRagdollDetachedPlayer {
                character,
                parent: parent.parent(),
                local_transform: *local_transform,
            },
        ));
    }
}

#[doc(hidden)]
pub fn clear_spawned_ragdoll(player: &mut AnimationGraphPlayer) {
    let ReflectMut::Struct(fields) = player.reflect_mut() else {
        panic!("Animation graph player must reflect as a struct")
    };
    let spawned = fields
        .field_mut("spawned_ragdoll")
        .and_then(|field| field.try_downcast_mut::<Option<SpawnedRagdoll>>())
        .expect("Animation graph player must expose its spawned ragdoll");
    *spawned = None;
}

fn reset_ragdoll_on_scale_recovery(
    transform_helper: TransformHelper,
    characters: Query<
        (
            Entity,
            &AlsNotifyLocomotionAction,
            Option<&AlsCharacterAssembly>,
        ),
        With<AlsCharacter>,
    >,
    avatars: Query<(&AlsCharacterAvatar, &AnimatedSceneInstance)>,
    mut players: Query<(
        &mut AnimationGraphPlayer,
        &Transform,
        &AlsRagdollScaleState,
        &RagdollPhysicsProfileApplied,
    )>,
    children: Query<&Children>,
    mut globals: Query<&mut GlobalTransform>,
) {
    for (character, action, assembly) in &characters {
        if *action != AlsNotifyLocomotionAction::Ragdolling {
            continue;
        }
        let Some(entity) = animated_player_for_character(character, assembly, &avatars) else {
            continue;
        };
        let Ok((mut player, transform, state, profile)) = players.get_mut(entity) else {
            continue;
        };
        if !state.recovering_from_zero(transform.scale)
            || profile.spawned_root != Some(state.root)
            || !player
                .get_spawned_ragdoll()
                .is_some_and(|spawned| spawned.root == state.root)
        {
            continue;
        }
        if refresh_player_subtree_globals(&transform_helper, entity, &children, &mut globals)
            .is_some()
        {
            clear_spawned_ragdoll(&mut player);
        }
    }
}

fn set_simulating_ragdoll_body_modes(
    mut commands: Commands,
    characters: Query<
        (
            Entity,
            &AlsNotifyLocomotionAction,
            Option<&AlsCharacterAssembly>,
        ),
        With<AlsCharacter>,
    >,
    avatars: Query<(&AlsCharacterAvatar, &AnimatedSceneInstance)>,
    players: Query<(
        &AnimationGraphPlayer,
        Option<&Transform>,
        Option<&AlsRagdollScaleState>,
    )>,
    bodies: Query<(&RigidBody, &RagdollActiveMode)>,
) {
    for (character, action, assembly) in &characters {
        if *action != AlsNotifyLocomotionAction::Ragdolling {
            continue;
        }
        let Some(player) = animated_player_for_character(character, assembly, &avatars) else {
            continue;
        };
        let Ok((player, transform, scale_state)) = players.get(player) else {
            continue;
        };
        let Some(spawned) = player.get_spawned_ragdoll() else {
            continue;
        };
        if let Some(mut state) = scale_state
            .copied()
            .filter(|state| state.root == spawned.root)
        {
            state.update(
                transform.map_or(Vec3::ONE, |transform| transform.scale),
                true,
            );
            if state.stopped {
                continue;
            }
        }
        for body in spawned.bodies.values().copied() {
            if let Ok((&RigidBody::Kinematic, &RagdollActiveMode(BodyMode::Dynamic))) =
                bodies.get(body)
            {
                commands.entity(body).insert(RigidBody::Dynamic);
            }
        }
    }
}

fn sync_ragdoll_body_modes(
    mut commands: Commands,
    transform_helper: TransformHelper,
    scene_handles: Query<&AnimatedSceneHandle>,
    scene_assets: Option<Res<Assets<AnimatedScene>>>,
    maps: Option<Res<Assets<RagdollBoneMap>>>,
    bones: Query<&GlobalTransform>,
    characters: Query<
        (
            Entity,
            Option<&AlsNotifyLocomotionAction>,
            Option<&AlsCharacterAssembly>,
            Option<&LinearVelocity>,
        ),
        With<AlsCharacter>,
    >,
    avatars: Query<(&AlsCharacterAvatar, &AnimatedSceneInstance)>,
    layers: RagdollColliderLayersQuery,
    mut players: Query<(
        &mut AnimationGraphPlayer,
        Option<&mut RagdollPhysicsProfileApplied>,
        Option<&Transform>,
        Option<&mut AlsRagdollScaleState>,
    )>,
    mut bodies: Query<
        (
            Entity,
            &RigidBody,
            Option<&RagdollActiveMode>,
            Option<&Transform>,
            &mut Position,
            &mut Rotation,
            &mut LinearVelocity,
            &mut AngularVelocity,
            &mut RelativeKinematicBody,
            Has<AlsRagdollPhysicsActive>,
        ),
        Without<AlsCharacter>,
    >,
) {
    for (character, action, assembly, actor_velocity) in &characters {
        let Some(player_entity) = animated_player_for_character(character, assembly, &avatars)
        else {
            continue;
        };
        let Ok((mut player, profile, transform, scale_state)) = players.get_mut(player_entity)
        else {
            continue;
        };
        let Some(root) = player.get_spawned_ragdoll().map(|spawned| spawned.root) else {
            continue;
        };
        let action_ragdolling = action == Some(&AlsNotifyLocomotionAction::Ragdolling);
        let scale = transform.map_or(Vec3::ONE, |transform| transform.scale);
        let recovering = action_ragdolling
            && scale_state
                .as_ref()
                .is_some_and(|state| state.root != root && state.recovering_from_zero(scale));
        let rescaling = action_ragdolling
            && scale_state.as_ref().is_some_and(|state| {
                state.root == root
                    && state.ragdolling
                    && !state.stopped
                    && state.uniform_scale_changed(scale)
            });
        let repositioning = recovering || rescaling;
        let recovered_root_world = if recovering {
            transform_helper.compute_global_transform(root).ok()
        } else {
            None
        };
        let stopped = if let Some(mut state) = scale_state {
            if state.root != root {
                *state = AlsRagdollScaleState {
                    root,
                    previous: scale,
                    stopped: false,
                    ragdolling: action_ragdolling,
                };
            } else {
                state.update(scale, action_ragdolling);
            }
            state.stopped
        } else {
            commands.entity(player_entity).insert(AlsRagdollScaleState {
                root,
                previous: scale,
                stopped: false,
                ragdolling: action_ragdolling,
            });
            false
        };
        let ragdolling = action_ragdolling && !stopped;
        player.set_input_data("ragdoll_dynamic", DataValue::from(ragdolling));
        player.set_input_data("ragdoll_readback", DataValue::from(ragdolling));
        let Some(spawned) = player.get_spawned_ragdoll() else {
            continue;
        };
        if recovering {
            sync_ragdoll_collider_layers(&mut commands, spawned, &layers, true);
        }
        let mut handoff = false;
        let mapping = assembly
            .and_then(|assembly| scene_handles.get(assembly.avatar).ok())
            .and_then(|handle| scene_assets.as_ref()?.get(&handle.handle))
            .and_then(|scene| maps.as_ref()?.get(scene.ragdoll_bone_map.as_ref()?));
        for (id, body_entity) in &spawned.bodies {
            let Ok((
                entity,
                mode,
                active_mode,
                transform,
                mut position,
                mut rotation,
                mut linear_velocity,
                mut angular_velocity,
                mut relative_body,
                activated,
            )) = bodies.get_mut(*body_entity)
            else {
                continue;
            };
            if ragdolling {
                if !activated || repositioning {
                    if let Some(mapping) =
                        mapping.and_then(|mapping| mapping.bodies_from_bones.get(id))
                        && let Some(bone) = player.bone_entity(mapping.bone.bone.id())
                    {
                        let global = if repositioning {
                            transform_helper.compute_global_transform(bone).ok()
                        } else {
                            bones.get(bone).ok().copied()
                        };
                        if let Some(global) = global {
                            let target = global.to_isometry() * mapping.bone.offset;
                            position.0 = target.translation.into();
                            rotation.0 = target.rotation;
                            if let (Some(root_world), Some(transform)) =
                                (recovered_root_world, transform)
                            {
                                let local = GlobalTransform::from(Transform::from_isometry(target))
                                    .reparented_to(&root_world);
                                commands.entity(entity).insert(Transform {
                                    scale: transform.scale,
                                    ..local
                                });
                            }
                        }
                    }
                }
                if !activated {
                    commands.entity(entity).insert(AlsRagdollPhysicsActive);
                    handoff = true;
                }
                let target_mode = match active_mode.map(|mode| mode.0) {
                    Some(BodyMode::Kinematic) => RigidBody::Kinematic,
                    Some(BodyMode::Dynamic) | None => RigidBody::Dynamic,
                };
                if recovering || (!activated && target_mode.is_kinematic()) {
                    linear_velocity.0 = if target_mode.is_dynamic() {
                        actor_velocity.map_or(Vec3::ZERO, |velocity| velocity.0)
                    } else {
                        Vec3::ZERO
                    };
                    angular_velocity.0 = Vec3::ZERO;
                    relative_body.kinematic_linear_velocity = Vec3::ZERO;
                    relative_body.kinematic_angular_velocity = Vec3::ZERO;
                }
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
    schedule: Option<Res<crate::AlsPhysicsSchedule>>,
    physics: Option<Res<Time<avian3d::prelude::Physics>>>,
    mut last_step_time: Local<std::time::Duration>,
    characters: Query<
        (Option<&AlsNotifyLocomotionAction>, &AlsCharacterAssembly),
        With<AlsCharacter>,
    >,
    avatars: Query<(
        Entity,
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
    let delta = if let (Some(crate::AlsPhysicsSchedule::Avian(_)), Some(physics)) =
        (schedule.as_deref(), physics)
    {
        let elapsed = physics.elapsed();
        let delta = elapsed.saturating_sub(*last_step_time);
        *last_step_time = elapsed;
        if delta.is_zero() {
            return;
        }
        delta.as_secs_f32()
    } else {
        source_physics_delta(time.delta()).as_secs_f32()
    };
    for (avatar_entity, avatar, instance, handle) in &avatars {
        let Ok((action, assembly)) = characters.get(avatar.character) else {
            continue;
        };
        if assembly.avatar != avatar_entity
            || action == Some(&AlsNotifyLocomotionAction::Ragdolling)
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
            if !position.is_added() && delta > 0.0 {
                linear_velocity.0 = (Vec3::from(target.translation) - position.0) / delta;
                angular_velocity.0 =
                    source_kinematic_angular_velocity(rotation.0, target.rotation, delta);
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
    time: Res<Time<Virtual>>,
    mut commands: Commands,
    mut characters: Query<
        (
            Entity,
            Option<&AlsNotifyLocomotionAction>,
            Option<&AlsCharacterAssembly>,
            Option<&mut AlsRagdollJointDrive>,
        ),
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
    if time.is_paused() {
        return;
    }
    for (character, action, assembly, current_drive) in &mut characters {
        let mut drive = AlsRagdollJointDrive::default();
        let player = animated_player_for_character(character, assembly, &avatars)
            .and_then(|entity| players.get(entity).ok());
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
        let half_height = if stance == Stance::Crouching {
            params.crouch.actual_crouched_half_height()
        } else {
            params.crouch.standing_capsule_half_height
        };
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
    spatial: SceneQuery,
    candidates: Query<crate::als::motion::CharacterCollisionCandidates>,
    transform_helper: TransformHelper,
    params_assets: Res<Assets<AlsMovementParams>>,
    avatars: Query<(&AlsCharacterAvatar, &AnimatedSceneInstance)>,
    players: Query<&AnimationGraphPlayer>,
    skeletons: Option<Res<Assets<Skeleton>>>,
    bone_transforms: Query<&GlobalTransform>,
    mut characters: Query<
        (
            Entity,
            Option<&mut AlsNotifyLocomotionAction>,
            &mut LocomotionState,
            &mut crate::als::transition::AlsStateRequest,
            Option<&mut AlsRagdollGroundFollow>,
            Option<&mut LinearVelocity>,
            &Position,
            &Stance,
            Option<&crate::als::transition::AlsDesiredStance>,
            Option<&mut crate::als::motion::AlsCrouchTransaction>,
            &AlsMovementParamsHandle,
            &AlsCollisionLayerOwnership,
            Option<&AlsCharacterAssembly>,
        ),
        With<AlsCharacter>,
    >,
) {
    for (
        entity,
        action,
        locomotion,
        mut request,
        follow,
        velocity,
        position,
        stance,
        desired_stance,
        crouch,
        params_handle,
        ownership,
        assembly,
    ) in &mut characters
    {
        if !request.ragdoll_exit {
            continue;
        }
        if action.as_deref() != Some(&AlsNotifyLocomotionAction::Ragdolling) {
            request.ragdoll_exit = false;
            continue;
        }
        let Some(player) = animated_player_for_character(entity, assembly, &avatars)
            .and_then(|player| players.get(player).ok())
        else {
            request.ragdoll_exit = false;
            continue;
        };
        let has_pose = skeletons
            .as_ref()
            .and_then(|skeletons| skeletons.get(player.skeleton()))
            .is_some_and(|skeleton| {
                skeleton.iter_bones().any(|bone| {
                    player
                        .bone_entity(bone)
                        .is_some_and(|bone| bone_transforms.contains(bone))
                })
            });
        if !has_pose {
            request.ragdoll_exit = false;
            continue;
        }
        let Some(mut follow) = follow else {
            continue;
        };
        let Ok(world) = transform_helper.compute_global_transform(entity) else {
            continue;
        };
        let (world_scale, _, _) = world.to_scale_rotation_translation();
        let (half_height, radius) =
            ragdoll_capsule_dimensions(*stance, params_handle, &params_assets, world_scale);
        let location = if follow.pelvis == Vec3::ZERO {
            position.0
        } else {
            follow.pelvis
        };
        let mut excluded = vec![entity];
        if let Some(spawned) = player.get_spawned_ragdoll() {
            excluded.extend(spawned.bodies.values().copied());
            excluded.extend(spawned.colliders.values().copied());
        }
        let start = location + Vec3::Y * (2.0 * radius);
        let hit = ragdoll_ground_hit(
            &spatial,
            &candidates,
            start,
            half_height + radius,
            radius,
            excluded,
            ownership.original_layers(),
        );
        follow.grounded = hit.is_some();
        follow.target = hit.map_or(location, |hit| {
            let center = start - Vec3::Y * hit.distance;
            Vec3::new(
                location.x,
                center.y + half_height - radius + ALS_CHARACTER_FLOOR_CLEARANCE,
                location.z,
            )
        });
        let (state, exit_velocity) = ragdoll_exit_route(follow.grounded, follow.velocity);
        if let Some(mut crouch) = crouch {
            let desired = desired_stance.map_or(*stance, |desired| desired.0);
            crouch.pending_action_stance = match state {
                LocomotionState::Air => Some(Stance::Standing),
                LocomotionState::Grounded
                    if matches!(desired, Stance::Standing | Stance::Crouching) =>
                {
                    Some(desired)
                }
                _ => None,
            };
        }
        *action.unwrap() = AlsNotifyLocomotionAction::None;
        request.locomotion_state = None;
        commit_state(entity, locomotion, state, &mut transitions);
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
    spatial: SceneQuery,
    candidates: Query<crate::als::motion::CharacterCollisionCandidates>,
    transform_helper: TransformHelper,
    params_assets: Res<Assets<AlsMovementParams>>,
    characters: Query<
        (
            Entity,
            &Position,
            &Stance,
            &AlsMovementParamsHandle,
            Option<&CollisionLayers>,
            &AlsCollisionLayerOwnership,
        ),
        (With<AlsCharacter>, With<AlsRagdollAirExit>),
    >,
) {
    for (entity, position, stance, params_handle, current_layers, ownership) in &characters {
        let Ok(world) = transform_helper.compute_global_transform(entity) else {
            continue;
        };
        let (world_scale, _, _) = world.to_scale_rotation_translation();
        let (half_height, radius) =
            ragdoll_capsule_dimensions(*stance, params_handle, &params_assets, world_scale);
        let trace_start = position.0 + Vec3::Y * (2.0 * radius);
        let trace_distance = half_height + radius;
        let layers = current_layers
            .filter(|layers| **layers != CollisionLayers::NONE)
            .copied()
            .unwrap_or_else(|| ownership.original_layers());
        if ragdoll_ground_hit(
            &spatial,
            &candidates,
            trace_start,
            trace_distance,
            radius,
            vec![entity],
            layers,
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
    transform_helper: TransformHelper,
    motion: Query<&crate::actual_motion::AlsActualMotion, With<AlsCharacter>>,
    rolls: Query<&crate::als::roll::AlsRollState>,
    mut transitions: Query<&mut crate::stop_link::AlsTransitionMontageStack>,
    mut turns: Query<(
        &AlsCharacterAvatar,
        &mut crate::turn_layer::AlsTurnMontageStack,
    )>,
    mut characters: Query<
        (
            Entity,
            &Transform,
            Option<&mut Position>,
            Option<&mut LinearVelocity>,
            Option<&AlsNotifyLocomotionAction>,
            &RigidBody,
            Option<&CollisionLayers>,
            &mut AlsCollisionLayerOwnership,
            Option<&TnuaToggle>,
            Option<&AlsRagdollCharacterPhysics>,
            Option<&AlsRagdollGroundFollow>,
            Option<&AlsCharacterAssembly>,
            Option<&ChildOf>,
        ),
        With<AlsCharacter>,
    >,
) {
    for (
        entity,
        transform,
        mut position,
        mut velocity,
        action,
        rigid_body,
        collision_layers,
        mut collision_ownership,
        tnua_toggle,
        previous_physics,
        ground_follow,
        assembly,
        parent,
    ) in &mut characters
    {
        let is_ragdoll = action == Some(&AlsNotifyLocomotionAction::Ragdolling);
        if is_ragdoll {
            if previous_physics.is_none() {
                if let Ok(mut stack) = transitions.get_mut(entity) {
                    stack.stop_with_duration(
                        crate::get_up::RAGDOLL_SETTINGS.stop_montage_blend_seconds,
                    );
                }
                if let Some(assembly) = assembly
                    && let Ok((avatar, mut stack)) = turns.get_mut(assembly.avatar)
                    && avatar.character == entity
                {
                    stack.request_stop();
                }
                if let Ok(roll) = rolls.get(entity) {
                    if roll.active {
                        commands
                            .entity(entity)
                            .insert(crate::roll_drive::AlsRollBlendOut::new(
                                roll,
                                crate::get_up::RAGDOLL_SETTINGS.stop_montage_blend_seconds,
                            ));
                    }
                }
                commands.entity(entity).insert(AlsRagdollCharacterPhysics {
                    rigid_body: *rigid_body,
                    tnua_toggle: tnua_toggle.copied(),
                });
                if crate::get_up::RAGDOLL_SETTINGS.limit_initial_speed {
                    let entry_speed = motion
                        .get(entity)
                        .map_or(0.0, |motion| motion.world_velocity.length());
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
            let mut aligned = *transform;
            aligned.translation = if let Some(parent) = parent {
                let Ok(world) = transform_helper.compute_global_transform(parent.parent()) else {
                    continue;
                };
                world.affine().inverse().transform_point3(follow.target)
            } else {
                follow.target
            };
            commands.entity(entity).insert(aligned);
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
    time: Res<Time<Virtual>>,
    mut characters: Query<
        (
            Entity,
            Option<&AlsNotifyLocomotionAction>,
            Option<&AlsCharacterAssembly>,
            &mut AlsRagdollInitialSpeedLimit,
        ),
        With<AlsCharacter>,
    >,
    avatars: Query<(&AlsCharacterAvatar, &AnimatedSceneInstance)>,
    players: Query<&AnimationGraphPlayer>,
    mut velocities: Query<&mut LinearVelocity>,
) {
    if time.is_paused() {
        return;
    }
    for (character, action, assembly, mut limit) in &mut characters {
        if action != Some(&AlsNotifyLocomotionAction::Ragdolling) {
            continue;
        }
        let Some(player) = animated_player_for_character(character, assembly, &avatars)
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

#[doc(hidden)]
pub fn ragdoll_recovery_rotation(pelvis: Quat) -> (Quat, bool) {
    let half = std::f64::consts::FRAC_1_SQRT_2;
    let canonical = DQuat::from_xyzw(0.0, -half, 0.0, half) * pelvis.as_dquat();
    let (x, y, z, w) = (-canonical.x, -canonical.z, -canonical.y, canonical.w);
    let singularity = z * x - w * y;
    let (yaw, roll) = if singularity.abs() > 0.4999995 {
        (2.0 * x.atan2(w) * singularity.signum(), 0.0)
    } else {
        (
            (2.0 * (w * z + x * y)).atan2(1.0 - 2.0 * (y * y + z * z)),
            (-2.0 * (w * x + y * z)).atan2(1.0 - 2.0 * (x * x + y * y)),
        )
    };
    let facing_up = roll <= 0.0;
    let yaw = if facing_up {
        std::f64::consts::PI - yaw
    } else {
        -yaw
    };
    (DQuat::from_rotation_y(yaw).as_quat(), facing_up)
}

pub(crate) fn restore_ragdoll_player_parent(
    mut commands: Commands,
    transform_helper: TransformHelper,
    schedule: Option<Res<crate::AlsPhysicsSchedule>>,
    scene_handles: Query<&AnimatedSceneHandle>,
    scenes: Option<Res<Assets<AnimatedScene>>>,
    maps: Option<Res<Assets<RagdollBoneMap>>>,
    body_poses: Query<(&Position, &Rotation)>,
    avatars: Query<(Entity, &AlsCharacterAvatar, &AnimatedSceneInstance)>,
    characters: Query<
        (
            Option<&AlsNotifyLocomotionAction>,
            &LocomotionState,
            &AlsCharacterAssembly,
            Option<&ChildOf>,
        ),
        With<AlsCharacter>,
    >,
    detached_players: Query<&AlsRagdollDetachedPlayer>,
    skeletons: Res<Assets<Skeleton>>,
    bones: Query<(&Name, &Transform, &GlobalTransform, Option<&ChildOf>)>,
    globals: Query<&GlobalTransform>,
    mut players: Query<&mut AnimationGraphPlayer>,
) {
    for (avatar_entity, avatar, instance) in &avatars {
        let Ok((action, locomotion, assembly, actor_parent)) = characters.get(avatar.character)
        else {
            continue;
        };
        if assembly.avatar != avatar_entity
            || action == Some(&AlsNotifyLocomotionAction::Ragdolling)
        {
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
        let Some((pelvis, published_pelvis)) = skeleton.iter_bones().find_map(|bone| {
            player
                .bone_entity(bone)
                .and_then(|entity| bones.get(entity).ok())
                .filter(|(name, _, _, _)| name.as_str() == PELVIS_LABEL)
                .map(|(_, _, world, _)| (bone, *world))
        }) else {
            continue;
        };
        // A fixed tick can recover before the renderer has published the latest ragdoll pose.
        let physical_pelvis = || {
            if !matches!(schedule.as_deref(), Some(crate::AlsPhysicsSchedule::Avian(_))) {
                return None;
            }
            let scene = scenes.as_ref()?.get(&scene_handles.get(avatar_entity).ok()?.handle)?;
            let map = maps.as_ref()?.get(scene.ragdoll_bone_map.as_ref()?)?;
            let mapping = map.bones_from_bodies.get(&skeleton.id_to_path(pelvis)?)?;
            let [weight] = mapping.bodies.as_slice() else {
                return None;
            };
            let body = player.get_spawned_ragdoll()?.bodies.get(&weight.body)?;
            let (position, rotation) = body_poses.get(*body).ok()?;
            let pose = Isometry3d::new(position.0, rotation.0) * weight.offset;
            Some(GlobalTransform::from(Transform {
                translation: pose.translation.into(),
                rotation: pose.rotation,
                scale: published_pelvis.scale(),
            }))
        };
        let pelvis_world = physical_pelvis().unwrap_or(published_pelvis);
        let (rotation, facing_up) = ragdoll_recovery_rotation(pelvis_world.rotation());
        let Ok(actor_world) = transform_helper.compute_global_transform(avatar.character) else {
            continue;
        };
        let yaw = rotation.to_euler(EulerRot::YXZ).0;
        let (_, pitch, roll) = actor_world.rotation().to_euler(EulerRot::YXZ);
        let rotation = Quat::from_euler(EulerRot::YXZ, yaw, pitch, roll);
        let restored_actor_world = Transform {
            rotation,
            ..actor_world.compute_transform()
        };
        let restored_actor_global = GlobalTransform::from(restored_actor_world);
        let (restored_actor, restored_actor_global) = if let Some(parent) = actor_parent {
            let Ok(world) = transform_helper.compute_global_transform(parent.parent()) else {
                continue;
            };
            let local = restored_actor_global.reparented_to(&world);
            (local, world.mul_transform(local))
        } else {
            (restored_actor_world, restored_actor_global)
        };
        let parent_in_actor = actor_world.affine().inverse() * parent_world.affine();
        let restored_parent_world =
            GlobalTransform::from(Mat4::from(restored_actor_global.affine() * parent_in_actor));
        let mut restored_mesh = detached.local_transform;
        let actor_offset = parent_in_actor.transform_point3(restored_mesh.translation);
        let mesh_world_location = restored_actor_world.translation + rotation * actor_offset;
        restored_mesh.translation = restored_parent_world
            .affine()
            .inverse()
            .transform_point3(mesh_world_location);
        let Ok(detached_world) = transform_helper.compute_global_transform(player_entity) else {
            continue;
        };
        let parent_scale_in_mesh_axes = restored_parent_world
            .mul_transform(Transform::from_rotation(restored_mesh.rotation))
            .scale();
        restored_mesh.scale = detached_world.scale() / parent_scale_in_mesh_axes;
        let restored_world = restored_parent_world.mul_transform(restored_mesh);
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
                let world = if bone == pelvis { pelvis_world } else { *world };
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
            .insert((ChildOf(detached.parent), restored_mesh, restored_world))
            .remove::<AlsRagdollDetachedPlayer>();
        if *locomotion == LocomotionState::Grounded {
            commands
                .entity(avatar.character)
                .insert(crate::get_up::AlsGetUpRequest(facing_up));
        }
    }
}

fn ragdoll_ground_hit(
    spatial: &SceneQuery,
    candidates: &Query<crate::als::motion::CharacterCollisionCandidates>,
    trace_start: Vec3,
    trace_distance: f32,
    radius: f32,
    excluded: Vec<Entity>,
    layers: CollisionLayers,
) -> Option<ShapeHitData> {
    let filter = SpatialQueryFilter::from_excluded_entities(excluded.iter().copied());
    spatial.cast_shape_predicate(
        &Collider::sphere(radius),
        trace_start,
        Quat::IDENTITY,
        Dir3::NEG_Y,
        &ShapeCastConfig::from_max_distance(trace_distance),
        &filter,
        &|candidate| {
            candidates.get(candidate).is_ok_and(
                |(candidate_layers, sensor, owner, _, ignores_pawn)| {
                    !sensor
                        && !ignores_pawn
                        && !owner.is_some_and(|owner| excluded.contains(&owner.body))
                        && layers.interacts_with(candidate_layers.copied().unwrap_or_default())
                },
            )
        },
    )
}

#[allow(clippy::type_complexity)]
fn ragdoll_ground_follow(
    mut commands: Commands,
    spatial: SceneQuery,
    candidates: Query<crate::als::motion::CharacterCollisionCandidates>,
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
            Option<&AlsCharacterAssembly>,
        ),
        With<AlsCharacter>,
    >,
    avatars: Query<(&AlsCharacterAvatar, &AnimatedSceneInstance)>,
    players: Query<&AnimationGraphPlayer>,
    labels: Query<&BodyLabel>,
    body_states: Query<(&Position, &LinearVelocity), Without<AlsCharacter>>,
) {
    for (entity, position, action, stance, params_handle, collision_ownership, assembly) in
        &characters
    {
        if action != Some(&AlsNotifyLocomotionAction::Ragdolling) {
            continue;
        }
        let Some(player_entity) = animated_player_for_character(entity, assembly, &avatars) else {
            continue;
        };
        let Ok(player) = players.get(player_entity) else {
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
        let ragdoll_location = if pelvis == Vec3::ZERO {
            position.0
        } else {
            pelvis
        };
        let Ok(world) = transform_helper.compute_global_transform(entity) else {
            continue;
        };
        let (world_scale, _, _) = world.to_scale_rotation_translation();
        let (half_height, radius) =
            ragdoll_capsule_dimensions(*stance, params_handle, &params_assets, world_scale);
        let trace_start = ragdoll_location + Vec3::Y * (2.0 * radius);
        let trace_distance = half_height + radius;
        let mut excluded = Vec::with_capacity(1 + spawned.bodies.len() + spawned.colliders.len());
        excluded.push(entity);
        excluded.extend(spawned.bodies.values().copied());
        excluded.extend(spawned.colliders.values().copied());
        let hit = ragdoll_ground_hit(
            &spatial,
            &candidates,
            trace_start,
            trace_distance,
            radius,
            excluded,
            collision_ownership.original_layers(),
        );
        let grounded = hit.is_some();
        let target = if let Some(hit) = hit {
            let center = trace_start - Vec3::Y * hit.distance;
            Vec3::new(
                ragdoll_location.x,
                center.y + half_height - radius + ALS_CHARACTER_FLOOR_CLEARANCE,
                ragdoll_location.z,
            )
        } else {
            ragdoll_location
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
    mut commands: Commands,
    transform_helper: TransformHelper,
    mut characters: Query<
        (
            Entity,
            &mut Position,
            &Transform,
            &mut LinearVelocity,
            Option<&AlsNotifyLocomotionAction>,
            &AlsRagdollGroundFollow,
            Option<&ChildOf>,
        ),
        With<AlsCharacter>,
    >,
) {
    for (entity, mut position, transform, mut velocity, action, ground_follow, parent) in
        &mut characters
    {
        if action != Some(&AlsNotifyLocomotionAction::Ragdolling) {
            continue;
        }
        let mut aligned = *transform;
        aligned.translation = if let Some(parent) = parent {
            let Ok(world) = transform_helper.compute_global_transform(parent.parent()) else {
                continue;
            };
            world
                .affine()
                .inverse()
                .transform_point3(ground_follow.target)
        } else {
            ground_follow.target
        };
        position.0 = ground_follow.target;
        commands.entity(entity).insert(aligned);
        velocity.0 = Vec3::ZERO;
    }
}

#[allow(clippy::type_complexity)]
fn ragdoll_link_system(
    mut commands: Commands,
    mut q: Query<
        (
            Entity,
            Option<&AlsNotifyLocomotionAction>,
            Option<&AlsCharacterAssembly>,
            &mut AlsRagdollLink,
        ),
        With<AlsCharacter>,
    >,
    avatars: Query<(&AlsCharacterAvatar, &AnimatedSceneInstance)>,
    mut players: Query<(&mut AnimationGraphPlayer, Option<&AlsRagdollScaleState>)>,
    layers: RagdollColliderLayersQuery,
) {
    for (character, action, assembly, mut link) in &mut q {
        let is_ragdoll = action == Some(&AlsNotifyLocomotionAction::Ragdolling);
        let Some(player_entity) = animated_player_for_character(character, assembly, &avatars)
        else {
            link.prev_ragdoll = is_ragdoll;
            continue;
        };
        let Ok((mut player, scale_state)) = players.get_mut(player_entity) else {
            link.prev_ragdoll = is_ragdoll;
            continue;
        };

        let simulated = is_ragdoll && !scale_state.is_some_and(|state| state.stopped);
        player.set_input_data("ragdoll_dynamic", DataValue::from(simulated));
        player.set_input_data("ragdoll_readback", DataValue::from(simulated));

        if let Some(spawned) = player.get_spawned_ragdoll() {
            sync_ragdoll_collider_layers(&mut commands, spawned, &layers, is_ragdoll);
        }

        link.prev_ragdoll = is_ragdoll;
    }
}

fn sync_ragdoll_collider_layers(
    commands: &mut Commands,
    spawned: &SpawnedRagdoll,
    layers: &RagdollColliderLayersQuery<'_, '_>,
    is_ragdoll: bool,
) {
    for collider in spawned.colliders.values() {
        let Ok((stored, current, member, ignores_pawn)) = layers.get(*collider) else {
            continue;
        };
        if MESH_IGNORES_PAWN && !ignores_pawn {
            commands.entity(*collider).insert(SourceCollisionIgnorePawn);
        }
        if is_ragdoll {
            if let Some(desired) = member
                .map(|member| member.source_layers)
                .or(stored.map(|stored| stored.0))
                && *current != desired
            {
                commands.entity(*collider).insert(desired);
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

#[cfg(test)]
pub(crate) mod tests {
    #[test]
    fn landing_ragdoll_uses_only_the_current_character_transitions() {
        let mut messages = Messages::default();
        let actor = Entity::from_raw_u32(1).expect("landing fixture actor");
        let other = Entity::from_raw_u32(2).expect("other fixture actor");
        let transition = |entity, from, to| StateTransitionEvent {
            entity,
            dimension: Dimension::LocomotionState,
            from: StateValue::LocomotionState(from),
            to: StateValue::LocomotionState(to),
        };
        messages.write(transition(
            actor,
            LocomotionState::Air,
            LocomotionState::Grounded,
        ));
        messages.write(transition(
            other,
            LocomotionState::Grounded,
            LocomotionState::None,
        ));
        assert!(!landed_into_ragdoll(&messages, actor));
        assert!(!landed_into_ragdoll(&messages, other));
        messages.write(transition(
            actor,
            LocomotionState::Grounded,
            LocomotionState::None,
        ));
        assert!(landed_into_ragdoll(&messages, actor));
        messages.update();
        messages.write(transition(
            actor,
            LocomotionState::Grounded,
            LocomotionState::None,
        ));
        assert!(!landed_into_ragdoll(&messages, actor));
    }

    #[test]
    fn high_landing_keeps_the_published_mesh_and_kinematic_root() {
        use bevy::animation::AnimatedBy;

        let (mut app, character, avatar) = crate::roll_drive::tests::loaded_roll_character();
        let (player, bodies) = loaded_ragdoll_bodies(&mut app, avatar);
        let find_body = |name: &str| {
            bodies
                .iter()
                .find(|(label, _)| label == name)
                .expect("high landing has the mapped physics body")
                .1
        };
        let root_body = find_body("root");
        let pelvis_body = find_body("pelvis");
        let delta = 0.029539665_f32;
        app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
            std::time::Duration::from_secs_f32(delta),
        ));
        let position = Vec3::new(-15.0, 10.82, -15.0);
        let transform = Transform::from_translation(position)
            .with_scale(Vec3::splat(2.0))
            .with_rotation(Quat::from_rotation_y(-std::f32::consts::FRAC_PI_4));
        app.world_mut().entity_mut(character).insert((
            Position(position),
            transform,
            GlobalTransform::from(transform),
            LinearVelocity::ZERO,
        ));
        let mut bones = app
            .world_mut()
            .query::<(&Name, &AnimatedBy, &Transform, &GlobalTransform, &ChildOf)>();
        let mut previous = None;
        let mut simulated_frames = 0;
        for frame in 0..64 {
            app.update();
            let world = app.world();
            let (_, _, local, visible, parent) = bones
                .iter(world)
                .find(|(name, owner, ..)| name.as_str() == "pelvis" && owner.0 == player)
                .expect("loaded player has a visible pelvis");
            let mesh = world.get::<GlobalTransform>(player).unwrap().translation();
            let root = world
                .get::<GlobalTransform>(parent.parent())
                .unwrap()
                .translation();
            let current = (mesh, root, local.translation, visible.translation());
            if world.get::<AlsNotifyLocomotionAction>(character)
                == Some(&AlsNotifyLocomotionAction::Ragdolling)
            {
                let (previous_mesh, previous_root, previous_local, previous_visible) =
                    previous.expect("high landing has a previous visible frame");
                let body_root = world.get::<Position>(root_body).unwrap().0;
                let body_pelvis = world.get::<Position>(pelvis_body).unwrap().0;
                let local_step = local.translation.distance(previous_local);
                let visible_step = visible.translation().distance(previous_visible);
                println!(
                    "[PELVIS-ENTRY] frame={frame} mesh_before={previous_mesh:?} mesh_after={mesh:?} body_root={body_root:?} local_step={local_step} world_step={visible_step}"
                );
                assert_eq!(
                    mesh, previous_mesh,
                    "Landing must detach the published mesh frame"
                );
                assert_eq!(
                    world.get::<RigidBody>(root_body),
                    Some(&RigidBody::Kinematic)
                );
                assert!(
                    body_root.abs_diff_eq(previous_root, 1e-5) && root.abs_diff_eq(body_root, 1e-5),
                    "Kinematic root must hold the published bone frame: before={previous_root:?} body={body_root:?} visible={root:?}"
                );
                assert_eq!(
                    world.get::<LinearVelocity>(root_body),
                    Some(&LinearVelocity::ZERO)
                );
                assert!(
                    visible.translation().abs_diff_eq(body_pelvis, 1e-5),
                    "Visible pelvis must follow its simulated body"
                );
                assert!(
                    (local_step * 2.0 - visible_step).abs() < 1e-5,
                    "Pelvis step must respect the published parent scale: local={local_step} world={visible_step}"
                );
                simulated_frames += 1;
                if simulated_frames == 3 {
                    return;
                }
            }
            previous = Some(current);
        }
        panic!("High landing did not publish three simulated frames");
    }

    #[test]
    fn ragdoll_drive_reads_pelvis_speed_before_startup_limit() {
        use crate::extensions::source_physics_joint::SourcePhysicsJoint;
        use avian3d::prelude::JointDisabled;
        #[derive(Resource)]
        struct SeedVelocity(Vec3);
        #[derive(Resource, Default)]
        struct LimitedSpeed(f32);

        let (mut app, actor, avatar) = crate::roll_drive::tests::loaded_roll_character();
        let (player, bodies) = loaded_ragdoll_bodies(&mut app, avatar);
        app.world_mut()
            .get_mut::<crate::als::transition::AlsStateRequest>(actor)
            .unwrap()
            .ragdoll_start = true;
        for _ in 0..4 {
            app.update();
        }
        assert_eq!(
            app.world().get::<AlsNotifyLocomotionAction>(actor),
            Some(&AlsNotifyLocomotionAction::Ragdolling)
        );
        let pelvis = bodies
            .iter()
            .find(|(label, _)| label == "pelvis")
            .unwrap()
            .1;
        let joints = app
            .world()
            .get::<AnimationGraphPlayer>(player)
            .unwrap()
            .get_spawned_ragdoll()
            .unwrap()
            .joints
            .values()
            .copied()
            .collect::<Vec<_>>();
        app.insert_resource(SeedVelocity(Vec3::ZERO))
            .init_resource::<LimitedSpeed>();
        app.add_systems(
            PostUpdate,
            (
                (move |seed: Res<SeedVelocity>, mut velocities: Query<&mut LinearVelocity>| {
                    velocities.get_mut(pelvis).unwrap().0 = seed.0;
                })
                .before(ragdoll_joint_drive_adapter_system)
                .before(limit_initial_ragdoll_speed),
                (move |velocities: Query<&LinearVelocity>, mut result: ResMut<LimitedSpeed>| {
                    result.0 = velocities.get(pelvis).unwrap().0.length();
                })
                .after(limit_initial_ragdoll_speed)
                .before(RagdollAngularDriveSet),
            ),
        );
        for limiting in [false, true] {
            for velocity in [
                Vec3::ZERO,
                Vec3::X,
                Vec3::new(3.0, 4.0, 0.0),
                Vec3::NEG_Y * 12.0,
            ] {
                app.world_mut().resource_mut::<SeedVelocity>().0 = velocity;
                let mut limit = app
                    .world_mut()
                    .get_mut::<AlsRagdollInitialSpeedLimit>(actor)
                    .unwrap();
                limit.speed = 2.0;
                limit.remaining_updates = u8::from(limiting);
                app.update();
                let expected_speed = if limiting {
                    velocity.length().min(2.0)
                } else {
                    velocity.length()
                };
                assert!((app.world().resource::<LimitedSpeed>().0 - expected_speed).abs() < 1e-5);
                let expected_stiffness = (velocity.length() / 10.0).min(1.0) * 25_000.0;
                assert!(
                    (app.world()
                        .get::<RagdollAngularDrive>(player)
                        .unwrap()
                        .stiffness
                        - expected_stiffness)
                        .abs()
                        < 0.01
                );
                let mut motor_count = 0;
                for joint in &joints {
                    if app.world().get::<JointDisabled>(*joint).is_some() {
                        continue;
                    }
                    let joint = app.world().get::<SourcePhysicsJoint>(*joint).unwrap();
                    assert_eq!(joint.motor_stiffness_scale, 1.5);
                    for motor in &joint.motors {
                        assert!(
                            (motor.spring.stiffness - expected_stiffness * 1.5).abs() < 0.02,
                            "limiting={limiting} velocity={velocity:?} stiffness={}",
                            motor.spring.stiffness
                        );
                        assert_eq!(motor.spring.damping, 0.0);
                        motor_count += 1;
                    }
                }
                assert_eq!(motor_count, 57);
            }
        }
    }

    #[test]
    fn ragdoll_scale_stop_follows_relative_zero_crossings() {
        for (scale, stopped) in [
            (Vec3::ZERO, true),
            (Vec3::splat(0.00005), true),
            (Vec3::splat(0.0001), true),
            (Vec3::splat(0.0002), false),
            (Vec3::new(0.0, 1.0, 1.0), false),
        ] {
            let mut state = AlsRagdollScaleState {
                root: Entity::PLACEHOLDER,
                previous: Vec3::ONE,
                stopped: false,
                ragdolling: true,
            };
            state.update(scale, true);
            assert_eq!(state.stopped, stopped, "scale={scale:?}");
        }
        let mut state = AlsRagdollScaleState {
            root: Entity::PLACEHOLDER,
            previous: Vec3::splat(0.0002),
            stopped: false,
            ragdolling: true,
        };
        state.update(Vec3::splat(0.0001), true);
        assert!(!state.stopped);
        assert_eq!(state.previous, Vec3::splat(0.0002));
        state.update(Vec3::ZERO, true);
        assert!(state.stopped);
        state.update(Vec3::ONE, true);
        assert!(!state.stopped);
    }

    #[test]
    fn ragdoll_entry_preserves_body_velocity_before_physics() {
        use crate::extensions::post_physics_animation::update_relative_kinematic_target_velocities;
        use bevy::math::Isometry3d;
        use bevy::time::TimeUpdateStrategy;
        use bevy_animation_graph::core::physics_systems_avian::update_ragdolls_avian;
        use bevy_animation_graph::core::ragdoll::relative_kinematic_body::RelativeKinematicBodyPositionBased;

        for delta in [1.0_f32 / 60.0, 1.0 / 30.0] {
            let (mut app, actor, avatar) = crate::roll_drive::tests::loaded_roll_character();
            let (player, bodies) = loaded_ragdoll_bodies(&mut app, avatar);
            let pelvis = bodies
                .iter()
                .find(|(name, _)| name == PELVIS_LABEL)
                .unwrap()
                .1;
            let root = bodies.iter().find(|(name, _)| name == "root").unwrap().1;
            assert!(
                app.world()
                    .get::<crate::extensions::post_physics_animation::PostPhysicsAnimation>(player)
                    .is_some()
            );
            assert_eq!(
                app.world().get::<RigidBody>(pelvis),
                Some(&RigidBody::Kinematic)
            );
            app.world_mut()
                .entity_mut(pelvis)
                .insert((LinearVelocity(Vec3::X), AngularVelocity(Vec3::ZERO)));
            app.world_mut()
                .get_mut::<crate::als::transition::AlsStateRequest>(actor)
                .unwrap()
                .ragdoll_start = true;
            app.insert_resource(TimeUpdateStrategy::ManualDuration(
                std::time::Duration::from_secs_f32(delta),
            ));
            app.add_systems(
                PostUpdate,
                (move |mut bodies: Query<(
                    &Position,
                    &Rotation,
                    &mut RelativeKinematicBodyPositionBased,
                )>| {
                    let (position, rotation, mut target) = bodies.get_mut(pelvis).unwrap();
                    target.relative_to = None;
                    target.relative_target = Isometry3d::new(
                        position.0 + Vec3::X * (delta * 0.5),
                        Quat::from_rotation_y(0.15) * rotation.0,
                    );
                })
                .after(update_ragdolls_avian)
                .before(update_relative_kinematic_target_velocities),
            );
            app.add_systems(
                PostUpdate,
                (move |bodies: Query<(&RigidBody, &LinearVelocity, &AngularVelocity)>| {
                    let (mode, linear, angular) = bodies.get(pelvis).unwrap();
                    assert_eq!(*mode, RigidBody::Dynamic);
                    assert_eq!(linear.0, Vec3::X);
                    assert_eq!(angular.0, Vec3::ZERO);
                    assert_eq!(*bodies.get(root).unwrap().0, RigidBody::Kinematic);
                })
                .after(update_relative_kinematic_body_velocities)
                .before(RagdollPhysicsHandoffSet),
            );
            app.update();
            assert_eq!(
                app.world().get::<AlsNotifyLocomotionAction>(actor),
                Some(&AlsNotifyLocomotionAction::Ragdolling)
            );
        }
    }

    #[test]
    fn paused_ragdoll_zero_scale_stops_real_bodies_and_readback() {
        for (scale, active) in [
            (Vec3::ZERO, false),
            (Vec3::splat(0.0001), false),
            (Vec3::splat(0.0002), true),
            (Vec3::new(0.0, 1.0, 1.0), true),
        ] {
            let (mut app, actor, avatar) = crate::roll_drive::tests::loaded_roll_character();
            let (player, bodies) = loaded_ragdoll_bodies(&mut app, avatar);
            app.world_mut()
                .get_mut::<crate::als::transition::AlsStateRequest>(actor)
                .unwrap()
                .ragdoll_start = true;
            for _ in 0..4 {
                app.update();
            }
            assert_eq!(
                app.world().get::<AlsNotifyLocomotionAction>(actor),
                Some(&AlsNotifyLocomotionAction::Ragdolling)
            );
            let dynamic_count = |app: &App| {
                bodies
                    .iter()
                    .filter(|(_, entity)| {
                        app.world().get::<RigidBody>(*entity) == Some(&RigidBody::Dynamic)
                    })
                    .count()
            };
            assert_eq!(dynamic_count(&app), 20);
            app.world_mut().resource_mut::<Time<Virtual>>().pause();
            app.world_mut().get_mut::<Transform>(player).unwrap().scale = scale;
            app.update();
            assert_eq!(
                dynamic_count(&app),
                if active { 20 } else { 0 },
                "scale={scale:?}"
            );
            let graph = app.world().get::<AnimationGraphPlayer>(player).unwrap();
            assert_eq!(
                graph
                    .get_input_data("ragdoll_dynamic".into())
                    .unwrap()
                    .as_bool()
                    .unwrap(),
                active
            );
            assert_eq!(
                graph
                    .get_input_data("ragdoll_readback".into())
                    .unwrap()
                    .as_bool()
                    .unwrap(),
                active
            );
        }
    }

    #[test]
    fn recovering_from_near_zero_scale_recreates_the_ragdoll_once() {
        for (near_zero, recovered_scale, rebuild, owner_velocity) in [
            (0.0, 1.0, true, Vec3::ZERO),
            (0.00005, 0.75, true, Vec3::new(0.15, 0.2, -0.1)),
            (0.0001, 1.5, true, Vec3::ZERO),
            (0.0002, 1.0, false, Vec3::ZERO),
        ] {
            let (mut app, actor, avatar) = crate::roll_drive::tests::loaded_roll_character();
            let (player, bodies) = loaded_ragdoll_bodies(&mut app, avatar);
            let old_pelvis = bodies
                .iter()
                .find(|(name, _)| name == PELVIS_LABEL)
                .unwrap()
                .1;
            app.world_mut()
                .get_mut::<crate::als::transition::AlsStateRequest>(actor)
                .unwrap()
                .ragdoll_start = true;
            for _ in 0..10 {
                app.update();
            }
            let old_root = app
                .world()
                .get::<AnimationGraphPlayer>(player)
                .unwrap()
                .get_spawned_ragdoll()
                .unwrap()
                .root;
            app.world_mut().resource_mut::<Time<Virtual>>().pause();
            app.world_mut().get_mut::<Transform>(player).unwrap().scale = Vec3::splat(near_zero);
            app.update();
            assert_eq!(
                app.world()
                    .get::<AlsRagdollScaleState>(player)
                    .unwrap()
                    .stopped,
                rebuild
            );
            let old_linear = Vec3::new(0.3, 0.2, -0.1);
            let old_angular = Vec3::new(0.2, -0.4, 0.3);
            app.world_mut()
                .entity_mut(old_pelvis)
                .insert((LinearVelocity(old_linear), AngularVelocity(old_angular)));
            app.world_mut().get_mut::<LinearVelocity>(actor).unwrap().0 = owner_velocity;
            app.world_mut().get_mut::<Transform>(player).unwrap().scale =
                Vec3::splat(recovered_scale);
            app.update();

            let world = app.world();
            let spawned = world
                .get::<AnimationGraphPlayer>(player)
                .unwrap()
                .get_spawned_ragdoll()
                .unwrap();
            let root = spawned.root;
            assert_eq!(
                root != old_root,
                rebuild,
                "Scale recovery threshold: {near_zero}"
            );
            if rebuild {
                assert!(world.get_entity(old_root).is_err());
                assert!(world.get_entity(old_pelvis).is_err());
                assert_eq!(
                    (
                        spawned.bodies.len(),
                        spawned.colliders.len(),
                        spawned.joints.len()
                    ),
                    (21, 22, 20)
                );
                assert_eq!(
                    world
                        .get::<RagdollPhysicsProfileApplied>(player)
                        .unwrap()
                        .spawned_root,
                    Some(root)
                );
                let mut dynamic = 0;
                for body in spawned.bodies.values() {
                    let mode = world.get::<RigidBody>(*body).unwrap();
                    let expected = if mode.is_dynamic() {
                        dynamic += 1;
                        owner_velocity
                    } else {
                        Vec3::ZERO
                    };
                    assert_eq!(world.get::<LinearVelocity>(*body).unwrap().0, expected);
                    assert_eq!(world.get::<AngularVelocity>(*body).unwrap().0, Vec3::ZERO);
                }
                assert_eq!(dynamic, 20);
                for collider in spawned.colliders.values() {
                    let member = world.get::<RagdollCollisionMember>(*collider).unwrap();
                    assert_eq!(member.group, root);
                    assert_eq!(
                        world.get::<CollisionLayers>(*collider),
                        Some(&member.source_layers)
                    );
                }
            } else {
                assert_eq!(
                    world.get::<LinearVelocity>(old_pelvis).unwrap().0,
                    old_linear
                );
                assert_eq!(
                    world.get::<AngularVelocity>(old_pelvis).unwrap().0,
                    old_angular
                );
            }
            app.update();
            assert_eq!(
                app.world()
                    .get::<AnimationGraphPlayer>(player)
                    .unwrap()
                    .get_spawned_ragdoll()
                    .unwrap()
                    .root,
                root
            );
        }
    }

    #[test]
    fn rescaled_ragdoll_uses_the_live_bone_pose_at_a_translated_mesh_origin() {
        use bevy::ecs::system::RunSystemOnce;

        let (mut app, actor, avatar) = crate::roll_drive::tests::loaded_roll_character();
        let (player, _) = loaded_ragdoll_bodies(&mut app, avatar);
        app.world_mut()
            .get_mut::<crate::als::transition::AlsStateRequest>(actor)
            .unwrap()
            .ragdoll_start = true;
        for _ in 0..10 {
            app.update();
        }
        assert!(
            app.world()
                .get::<AlsRagdollDetachedPlayer>(player)
                .is_some()
        );
        let pelvis_bone = {
            let graph = app.world().get::<AnimationGraphPlayer>(player).unwrap();
            let skeleton = app
                .world()
                .resource::<Assets<Skeleton>>()
                .get(graph.skeleton())
                .unwrap();
            skeleton
                .iter_bones()
                .find_map(|bone| {
                    graph.bone_entity(bone).filter(|entity| {
                        app.world()
                            .get::<Name>(*entity)
                            .is_some_and(|name| name.as_str() == PELVIS_LABEL)
                    })
                })
                .unwrap()
        };
        app.world_mut().resource_mut::<Time<Virtual>>().pause();
        {
            let mut mesh = app.world_mut().get_mut::<Transform>(player).unwrap();
            mesh.translation = Vec3::new(3.0, 1.0, -2.0);
            mesh.rotation = Quat::from_rotation_y(0.4) * mesh.rotation;
        }
        for (scale, from_zero) in [(0.75, false), (1.5, false), (0.75, true)] {
            if from_zero {
                app.world_mut().get_mut::<Transform>(player).unwrap().scale = Vec3::ZERO;
                app.update();
            }
            let old_root = app
                .world()
                .get::<AnimationGraphPlayer>(player)
                .unwrap()
                .get_spawned_ragdoll()
                .unwrap()
                .root;
            app.world_mut().get_mut::<Transform>(player).unwrap().scale = Vec3::splat(scale);
            let expected = app
                .world_mut()
                .run_system_once(move |helper: TransformHelper| {
                    helper.compute_global_transform(pelvis_bone).unwrap()
                })
                .unwrap();
            app.update();

            let world = app.world();
            let spawned = world
                .get::<AnimationGraphPlayer>(player)
                .unwrap()
                .get_spawned_ragdoll()
                .unwrap();
            assert_eq!(spawned.root != old_root, from_zero);
            let pelvis = *spawned
                .bodies
                .values()
                .find(|entity| {
                    world
                        .get::<BodyLabel>(**entity)
                        .is_some_and(|label| label.0 == PELVIS_LABEL)
                })
                .unwrap();
            let actual = world.get::<Position>(pelvis).unwrap().0;
            let rotation = world.get::<Rotation>(pelvis).unwrap().0;
            assert!(
                actual.distance(expected.translation()) < 1e-5,
                "Rescaled pelvis: scale={scale} from_zero={from_zero} actual={actual:?} expected={:?}",
                expected.translation()
            );
            assert!(rotation.dot(expected.rotation()).abs() > 1.0 - 1e-6);
        }
    }

    pub(crate) fn loaded_ragdoll_bodies(
        app: &mut App,
        avatar: Entity,
    ) -> (Entity, Vec<(String, Entity)>) {
        let player = app
            .world()
            .get::<AnimatedSceneInstance>(avatar)
            .unwrap()
            .player_entity();
        let mut bodies = None;
        for _ in 0..600 {
            bodies = app
                .world()
                .get::<AnimationGraphPlayer>(player)
                .and_then(|player| player.get_spawned_ragdoll())
                .map(|spawned| {
                    spawned
                        .bodies
                        .values()
                        .filter_map(|entity| {
                            app.world()
                                .get::<BodyLabel>(*entity)
                                .map(|label| (label.0.clone(), *entity))
                        })
                        .collect::<Vec<_>>()
                });
            if bodies.as_ref().is_some_and(|bodies| {
                ["pelvis", "spine_03", "foot_l"]
                    .into_iter()
                    .all(|label| bodies.iter().any(|(name, _)| name == label))
            }) {
                break;
            }
            app.update();
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let bodies = bodies.expect("Ragdoll bodies did not load within 600 frames");
        assert!(
            ["pelvis", "spine_03", "foot_l"]
                .into_iter()
                .all(|label| bodies.iter().any(|(name, _)| name == label)),
            "Required fixture bodies are missing"
        );
        (player, bodies)
    }

    pub(crate) fn remove_ragdoll_body(app: &mut App, bodies: &[(String, Entity)], label: &str) {
        let body = bodies.iter().find(|(name, _)| name == label).unwrap().1;
        app.world_mut().entity_mut(body).remove::<RigidBody>();
    }

    #[test]
    fn character_queries_and_contacts_ignore_another_actors_ragdoll() {
        use crate::character_assembly::{AlsCharacterBlueprint, spawn_als_character};
        use avian3d::prelude::CollisionHooks;
        use bevy::ecs::system::RunSystemOnce;
        let (mut app, actor, avatar) = crate::roll_drive::tests::loaded_roll_character();
        let (player, bodies) = loaded_ragdoll_bodies(&mut app, avatar);
        let other = app
            .world_mut()
            .run_system_once(
                |mut commands: Commands,
                 assets: Res<AssetServer>,
                 mut params: ResMut<Assets<AlsMovementParams>>,
                 mut configs: ResMut<
                    Assets<crate::als::control_scheme::AlsControlSchemeConfig>,
                >| {
                    spawn_als_character(
                        &mut commands,
                        &assets,
                        &mut params,
                        &mut configs,
                        AlsCharacterBlueprint::new(Transform::from_xyz(4.0, 1.5, 0.0)),
                    )
                },
            )
            .unwrap();
        let floor = app
            .world_mut()
            .spawn((
                RigidBody::Static,
                Collider::cuboid(1.0, 0.1, 1.0),
                Transform::from_xyz(20.0, 0.0, 0.0),
                CollisionLayers::default(),
            ))
            .id();
        app.world_mut()
            .get_mut::<crate::als::transition::AlsStateRequest>(actor)
            .unwrap()
            .ragdoll_start = true;
        for _ in 0..90 {
            app.update();
        }
        assert_eq!(
            app.world().get::<AlsNotifyLocomotionAction>(actor),
            Some(&AlsNotifyLocomotionAction::Ragdolling)
        );
        let pelvis = bodies
            .iter()
            .find(|(label, _)| label == "pelvis")
            .unwrap()
            .1;
        let origin = app.world().get::<Position>(pelvis).unwrap().0;
        let rig = app
            .world()
            .get::<AnimationGraphPlayer>(player)
            .unwrap()
            .get_spawned_ragdoll()
            .unwrap()
            .colliders
            .values()
            .copied()
            .collect::<Vec<_>>();
        assert!(
            !rig.is_empty()
                && rig.iter().all(|entity| app
                    .world()
                    .get::<SourceCollisionIgnorePawn>(*entity)
                    .is_some())
        );
        let queried_rig = rig.clone();
        let (raw_hits, blocking_hits, floor_blocks, ground_hit, ground_is_rig) = app.world_mut().run_system_once(move |spatial: SceneQuery,
            colliders: Query<&Collider>, candidates: Query<crate::als::motion::CharacterCollisionCandidates>| {
            let hits = spatial.shape_intersections(colliders.get(other).unwrap(), origin, Quat::IDENTITY,
                &SpatialQueryFilter::from_excluded_entities([other]));
            let raw = hits.iter().filter(|entity| queried_rig.contains(entity)).count();
            let blocking = hits.into_iter().filter(|entity| queried_rig.contains(entity)
                && crate::als::motion::character_blocks_collider(other, *entity, &candidates)).count();
            let ground = ragdoll_ground_hit(&spatial, &candidates, origin + Vec3::Y * 1.5, 3.0, 0.3,
                vec![other], CollisionLayers::default()).map(|hit| hit.entity);
            (raw, blocking, crate::als::motion::character_blocks_collider(other, floor, &candidates), ground.is_some(),
                ground.is_some_and(|entity| queried_rig.contains(&entity)))
        }).unwrap();
        assert!(
            raw_hits > 0,
            "The other character's capsule must overlap the active ragdoll"
        );
        assert_eq!(blocking_hits, 0);
        assert!(floor_blocks);
        assert!(ground_hit);
        assert!(!ground_is_rig);
        let allowed = app
            .world_mut()
            .run_system_once(
                move |hooks: crate::extensions::RagdollCollisionHooks, mut commands: Commands| {
                    rig.iter()
                        .filter(|entity| hooks.filter_pairs(other, **entity, &mut commands))
                        .count()
                },
            )
            .unwrap();
        assert_eq!(allowed, 0);
    }

    #[test]
    fn ragdoll_start_requires_both_source_body_instances() {
        for missing in [None, Some("pelvis"), Some("spine_03"), Some("foot_l")] {
            let (mut app, actor, avatar) = crate::roll_drive::tests::loaded_roll_character();
            let (player, bodies) = loaded_ragdoll_bodies(&mut app, avatar);
            if let Some(missing) = missing {
                remove_ragdoll_body(&mut app, &bodies, missing);
            }
            let initial_layers = *app.world().get::<CollisionLayers>(actor).unwrap();
            app.world_mut()
                .get_mut::<crate::als::transition::AlsStateRequest>(actor)
                .unwrap()
                .ragdoll_start = true;
            app.update();
            let accepted = missing.is_none_or(|label| label == "foot_l");
            assert_eq!(
                app.world().get::<AlsNotifyLocomotionAction>(actor)
                    == Some(&AlsNotifyLocomotionAction::Ragdolling),
                accepted,
                "missing={missing:?}"
            );
            if !accepted {
                assert_eq!(
                    *app.world().get::<LocomotionState>(actor).unwrap(),
                    LocomotionState::Grounded
                );
                assert_eq!(
                    *app.world().get::<RigidBody>(actor).unwrap(),
                    RigidBody::Dynamic
                );
                assert_eq!(
                    *app.world().get::<CollisionLayers>(actor).unwrap(),
                    initial_layers
                );
            }
            assert!(
                !app.world()
                    .get::<crate::als::transition::AlsStateRequest>(actor)
                    .unwrap()
                    .ragdoll_start
            );
            assert!(
                app.world()
                    .get::<AnimationGraphPlayer>(player)
                    .unwrap()
                    .get_error()
                    .is_none()
            );
        }
    }

    #[test]
    fn ragdoll_exit_requires_a_live_player_and_nonempty_bone_pose() {
        use bevy::ecs::system::RunSystemOnce;
        for fault in ["none", "missing_player", "empty_pose"] {
            let (mut app, actor, avatar) = crate::roll_drive::tests::loaded_roll_character();
            let (player_entity, _) = loaded_ragdoll_bodies(&mut app, avatar);
            app.world_mut()
                .get_mut::<crate::als::transition::AlsStateRequest>(actor)
                .unwrap()
                .ragdoll_start = true;
            let mut ready = false;
            for _ in 0..600 {
                app.update();
                if app.world().get::<AlsNotifyLocomotionAction>(actor)
                    == Some(&AlsNotifyLocomotionAction::Ragdolling)
                    && app
                        .world()
                        .get::<AlsRagdollGroundFollow>(actor)
                        .is_some_and(|follow| follow.grounded)
                {
                    ready = true;
                    break;
                }
            }
            assert!(
                ready,
                "Ragdoll did not reach its grounded exit fixture within 600 frames"
            );
            let bones = {
                let player = app
                    .world()
                    .get::<AnimationGraphPlayer>(player_entity)
                    .unwrap();
                let skeleton = app
                    .world()
                    .resource::<Assets<Skeleton>>()
                    .get(player.skeleton())
                    .unwrap();
                skeleton
                    .iter_bones()
                    .filter_map(|bone| player.bone_entity(bone))
                    .filter_map(|entity| {
                        app.world()
                            .get::<GlobalTransform>(entity)
                            .copied()
                            .map(|transform| (entity, transform))
                    })
                    .collect::<Vec<_>>()
            };
            assert!(!bones.is_empty());
            let saved_player = if fault == "missing_player" {
                app.world_mut()
                    .entity_mut(player_entity)
                    .take::<AnimationGraphPlayer>()
            } else {
                None
            };
            if fault == "empty_pose" {
                for (bone, _) in &bones {
                    app.world_mut()
                        .entity_mut(*bone)
                        .remove::<GlobalTransform>();
                }
            }
            let layers = *app.world().get::<CollisionLayers>(actor).unwrap();
            app.world_mut()
                .get_mut::<crate::als::transition::AlsStateRequest>(actor)
                .unwrap()
                .ragdoll_exit = true;
            app.world_mut()
                .run_system_once(ragdoll_exit_route_system)
                .unwrap();
            if fault != "none" {
                assert_eq!(
                    app.world().get::<AlsNotifyLocomotionAction>(actor),
                    Some(&AlsNotifyLocomotionAction::Ragdolling),
                    "fault={fault}"
                );
                assert_eq!(
                    *app.world().get::<RigidBody>(actor).unwrap(),
                    RigidBody::Kinematic
                );
                assert_eq!(*app.world().get::<CollisionLayers>(actor).unwrap(), layers);
                assert!(
                    !app.world()
                        .get::<crate::als::transition::AlsStateRequest>(actor)
                        .unwrap()
                        .ragdoll_exit
                );
                if let Some(player) = saved_player {
                    app.world_mut().entity_mut(player_entity).insert(player);
                }
                if fault == "empty_pose" {
                    for (bone, transform) in &bones {
                        app.world_mut().entity_mut(*bone).insert(*transform);
                    }
                }
                app.world_mut()
                    .run_system_once(ragdoll_exit_route_system)
                    .unwrap();
                assert_eq!(
                    app.world().get::<AlsNotifyLocomotionAction>(actor),
                    Some(&AlsNotifyLocomotionAction::Ragdolling)
                );
                app.world_mut()
                    .get_mut::<crate::als::transition::AlsStateRequest>(actor)
                    .unwrap()
                    .ragdoll_exit = true;
                app.world_mut()
                    .run_system_once(ragdoll_exit_route_system)
                    .unwrap();
            }
            assert_eq!(
                app.world().get::<AlsNotifyLocomotionAction>(actor),
                Some(&AlsNotifyLocomotionAction::None),
                "fault={fault}"
            );
            assert_eq!(
                *app.world().get::<LocomotionState>(actor).unwrap(),
                LocomotionState::Grounded
            );
        }
    }

    #[test]
    fn ragdoll_exit_is_scheduled_after_jump_request_production() {
        use bevy::ecs::schedule::{NodeId, graph::Direction};
        let (mut app, _, _) = crate::roll_drive::tests::loaded_roll_character();
        app.update();
        app.world_mut().schedule_scope(Update, |_, schedule| {
            let graph = schedule.graph();
            let dependencies = graph.dependency().graph();
            let hierarchy = graph.hierarchy().graph();
            let find = |system_type, label| {
                schedule
                    .systems()
                    .unwrap()
                    .find(|(_, system)| system.system_type() == system_type)
                    .map(|(key, _)| NodeId::System(key))
                    .unwrap_or_else(|| panic!("Missing system: {label}"))
            };
            let source = find(
                IntoSystem::into_system(crate::als::motion::locomotion_drive_system).system_type(),
                "locomotion_drive_system",
            );
            let target = find(
                IntoSystem::into_system(ragdoll_exit_route_system).system_type(),
                "ragdoll_exit_route_system",
            );
            let containing_sets = |system| {
                let mut nodes = vec![system];
                let mut index = 0;
                while index < nodes.len() {
                    for parent in hierarchy.neighbors_directed(nodes[index], Direction::Incoming) {
                        if !nodes.contains(&parent) {
                            nodes.push(parent);
                        }
                    }
                    index += 1;
                }
                nodes
            };
            let mut pending = containing_sets(source);
            let target_nodes = containing_sets(target);
            let mut visited = std::collections::HashSet::new();
            let mut reached = false;
            while let Some(node) = pending.pop() {
                if !visited.insert(node) {
                    continue;
                }
                for next in dependencies.neighbors(node) {
                    reached |= target_nodes.contains(&next);
                    pending.push(next);
                }
            }
            assert!(
                reached,
                "Jump request production must precede ragdoll exit routing"
            );
        });
    }

    #[test]
    fn ragdoll_exit_inputs_start_get_up_in_the_same_frame() {
        use crate::als::transition::Dimension;
        for input in ["request", "ragdoll", "jump"] {
            let (mut app, actor, avatar) = crate::roll_drive::tests::loaded_roll_character();
            let (player, _) = loaded_ragdoll_bodies(&mut app, avatar);
            let clips = [false, true].map(|facing| {
                app.world()
                    .resource::<AssetServer>()
                    .load::<bevy_animation_graph::core::animation_clip::GraphClip>(
                    &crate::get_up::get_up_settings(facing).clip,
                )
            });
            app.world_mut()
                .get_mut::<crate::als::transition::AlsStateRequest>(actor)
                .unwrap()
                .ragdoll_start = true;
            let mut ready = false;
            for _ in 0..600 {
                app.update();
                if app.world().get::<AlsNotifyLocomotionAction>(actor) == Some(&AlsNotifyLocomotionAction::Ragdolling)
                    && app.world().get::<AlsRagdollGroundFollow>(actor).is_some_and(|follow| follow.grounded)
                    && clips.iter().all(|clip| app.world().resource::<Assets<bevy_animation_graph::core::animation_clip::GraphClip>>().contains(clip)) {
                    ready = true;
                    break;
                }
            }
            assert!(
                ready,
                "Ragdoll did not reach its grounded exit fixture within 600 frames"
            );
            app.world_mut()
                .entity_mut(actor)
                .insert(crate::als::motion::AlsJumpPrev(false));
            if input == "request" {
                app.world_mut()
                    .get_mut::<crate::als::transition::AlsStateRequest>(actor)
                    .unwrap()
                    .ragdoll_exit = true;
            } else {
                let mut command = app
                    .world_mut()
                    .get_mut::<crate::als::input::AlsInputCommand>(actor)
                    .unwrap();
                command.ragdoll_pressed = input == "ragdoll";
                command.jump_pressed = input == "jump";
            }
            app.world_mut()
                .resource_mut::<Messages<StateTransitionEvent>>()
                .clear();
            app.update();
            assert_eq!(
                app.world().get::<AlsNotifyLocomotionAction>(actor),
                Some(&AlsNotifyLocomotionAction::GettingUp),
                "input={input}"
            );
            let transitions: Vec<_> = app
                .world_mut()
                .resource_mut::<Messages<StateTransitionEvent>>()
                .drain()
                .filter(|event| {
                    event.entity == actor && event.dimension == Dimension::LocomotionState
                })
                .map(|event| (event.from, event.to))
                .collect();
            assert_eq!(
                transitions,
                [(
                    LocomotionState::None.into(),
                    LocomotionState::Grounded.into()
                )]
            );
            assert!(
                app.world()
                    .get::<crate::als::motion::AlsMovementInputBlocked>(actor)
                    .is_some(),
                "input={input}"
            );
            assert!(
                !app.world()
                    .get::<crate::als::transition::AlsStateRequest>(actor)
                    .unwrap()
                    .ragdoll_exit
            );
            assert!(
                app.world()
                    .get::<AnimationGraphPlayer>(player)
                    .unwrap()
                    .get_error()
                    .is_none()
            );
        }
    }

    #[test]
    fn ragdoll_exit_preserves_desired_stance_when_get_up_starts_first() {
        use crate::als::transition::AlsDesiredStance;
        use bevy::ecs::system::RunSystemOnce;
        for initial in [Stance::Standing, Stance::Crouching] {
            for desired in [Stance::Standing, Stance::Crouching] {
                let (mut app, actor, avatar) = crate::roll_drive::tests::loaded_roll_character();
                let (player, _) = loaded_ragdoll_bodies(&mut app, avatar);
                app.world_mut()
                    .get_mut::<AlsDesiredStance>(actor)
                    .unwrap()
                    .0 = initial;
                for _ in 0..6 {
                    app.update();
                }
                assert_eq!(*app.world().get::<Stance>(actor).unwrap(), initial);
                app.world_mut()
                    .get_mut::<crate::als::transition::AlsStateRequest>(actor)
                    .unwrap()
                    .ragdoll_start = true;
                for _ in 0..90 {
                    app.update();
                }
                assert_eq!(
                    app.world().get::<AlsNotifyLocomotionAction>(actor),
                    Some(&AlsNotifyLocomotionAction::Ragdolling)
                );
                assert!(
                    app.world()
                        .get::<AlsRagdollGroundFollow>(actor)
                        .unwrap()
                        .grounded
                );
                app.world_mut()
                    .get_mut::<AlsDesiredStance>(actor)
                    .unwrap()
                    .0 = desired;
                let colliders = app
                    .world()
                    .get::<AnimationGraphPlayer>(player)
                    .unwrap()
                    .get_spawned_ragdoll()
                    .unwrap()
                    .colliders
                    .values()
                    .copied()
                    .collect::<Vec<_>>();
                for collider in colliders {
                    app.world_mut()
                        .entity_mut(collider)
                        .insert(CollisionLayers::NONE);
                }
                app.world_mut()
                    .get_mut::<crate::als::transition::AlsStateRequest>(actor)
                    .unwrap()
                    .ragdoll_exit = true;
                app.world_mut()
                    .run_system_once(ragdoll_exit_route_system)
                    .unwrap();
                assert_eq!(
                    app.world()
                        .get::<crate::als::motion::AlsCrouchTransaction>(actor)
                        .unwrap()
                        .pending_action_stance,
                    Some(desired)
                );
                app.world_mut()
                    .entity_mut(actor)
                    .insert(AlsNotifyLocomotionAction::GettingUp);
                app.world_mut()
                    .run_system_once(crate::als::motion::capsule_reshape_system)
                    .unwrap();
                assert_eq!(
                    *app.world().get::<Stance>(actor).unwrap(),
                    desired,
                    "initial={initial:?} desired={desired:?}"
                );
                let transaction = app
                    .world()
                    .get::<crate::als::motion::AlsCrouchTransaction>(actor)
                    .unwrap();
                let handle = app.world().get::<AlsMovementParamsHandle>(actor).unwrap();
                let params = app
                    .world()
                    .resource::<Assets<AlsMovementParams>>()
                    .get(handle.id())
                    .unwrap();
                let expected_half = if desired == Stance::Crouching {
                    params.crouch.actual_crouched_half_height()
                } else {
                    params.crouch.standing_capsule_half_height
                };
                assert!((transaction.collider_half_height - expected_half).abs() < 1e-5);
            }
        }
    }

    #[test]
    fn source_ragdoll_exit_loaded_support_lifecycle() {
        use bevy::ecs::system::RunSystemOnce;
        for change in [
            "unchanged",
            "remove",
            "move_away",
            "move_in",
            "add",
            "raise",
            "sensor",
            "ignore",
            "self_query",
        ] {
            let (mut app, actor, avatar) = crate::roll_drive::tests::loaded_roll_character();
            loaded_ragdoll_bodies(&mut app, avatar);
            app.world_mut()
                .get_mut::<crate::als::transition::AlsStateRequest>(actor)
                .unwrap()
                .ragdoll_start = true;
            for _ in 0..90 {
                app.update();
            }
            assert!(
                app.world()
                    .get::<AlsRagdollGroundFollow>(actor)
                    .unwrap()
                    .grounded
            );
            let supports: Vec<_> = app
                .world_mut()
                .query::<(Entity, &RigidBody)>()
                .iter(app.world())
                .filter_map(|(entity, body)| (*body == RigidBody::Static).then_some(entity))
                .collect();
            assert_eq!(supports.len(), 1);
            let support = supports[0];
            let original = *app.world().get::<Transform>(support).unwrap();
            let away = original.with_translation(original.translation + Vec3::X * 1000.0);
            if matches!(change, "move_in" | "add" | "sensor" | "ignore") {
                app.world_mut()
                    .entity_mut(support)
                    .insert((away, Position(away.translation)));
                app.update();
                assert!(
                    !app.world()
                        .get::<AlsRagdollGroundFollow>(actor)
                        .unwrap()
                        .grounded
                );
            }
            match change {
                "remove" | "self_query" => {
                    app.world_mut().despawn(support);
                }
                "move_away" => {
                    app.world_mut()
                        .entity_mut(support)
                        .insert((away, Position(away.translation)));
                }
                "move_in" => {
                    app.world_mut()
                        .entity_mut(support)
                        .insert((original, Position(original.translation)));
                }
                "raise" => {
                    let raised = original.with_translation(original.translation + Vec3::Y * 0.1);
                    app.world_mut()
                        .entity_mut(support)
                        .insert((raised, Position(raised.translation)));
                }
                "add" | "sensor" | "ignore" => {
                    let collider = app.world().get::<Collider>(support).unwrap().clone();
                    let layers = *app.world().get::<CollisionLayers>(support).unwrap();
                    let mut replacement =
                        app.world_mut()
                            .spawn((RigidBody::Static, collider, original, layers));
                    if change == "sensor" {
                        replacement.insert(Sensor);
                    }
                    if change == "ignore" {
                        replacement.insert(CollisionLayers::NONE);
                    }
                }
                _ => {}
            }
            if change == "self_query" {
                let player = app
                    .world()
                    .get::<AnimatedSceneInstance>(avatar)
                    .unwrap()
                    .player_entity();
                let colliders = app
                    .world()
                    .get::<AnimationGraphPlayer>(player)
                    .unwrap()
                    .get_spawned_ragdoll()
                    .unwrap()
                    .colliders
                    .values()
                    .copied()
                    .collect::<Vec<_>>();
                assert_eq!(colliders.len(), 22);
                for collider in colliders {
                    app.world_mut().entity_mut(collider).insert((
                        CollisionLayers::ALL,
                        crate::extensions::ShapeCollisionMode::QueryAndPhysics,
                    ));
                }
            }
            let saved_velocity = Vec3::new(2.5, -4.0, -1.5);
            app.world_mut()
                .get_mut::<AlsRagdollGroundFollow>(actor)
                .unwrap()
                .velocity = saved_velocity;
            app.world_mut()
                .get_mut::<crate::als::transition::AlsStateRequest>(actor)
                .unwrap()
                .ragdoll_exit = true;
            app.world_mut()
                .run_system_once(avian3d::collider_tree::update_moved_collider_aabbs::<Collider>)
                .unwrap();
            app.world_mut()
                .run_system_once(ragdoll_exit_route_system)
                .unwrap();
            let grounded = matches!(change, "unchanged" | "move_in" | "add" | "raise");
            assert_eq!(
                app.world().get::<LinearVelocity>(actor).unwrap().0,
                if grounded { Vec3::ZERO } else { saved_velocity },
                "change={change}"
            );
            app.update();
            let expected_action = if grounded {
                AlsNotifyLocomotionAction::GettingUp
            } else {
                AlsNotifyLocomotionAction::None
            };
            assert_eq!(
                app.world().get::<AlsNotifyLocomotionAction>(actor),
                Some(&expected_action),
                "Source loaded exit must branch on current support: {change}"
            );
            assert_eq!(
                app.world()
                    .get::<crate::get_up::AlsGetUpState>(actor)
                    .is_some(),
                grounded
            );
            assert_eq!(
                app.world()
                    .get::<crate::als::motion::AlsMovementInputBlocked>(actor)
                    .is_some(),
                grounded
            );
            assert!(
                app.world()
                    .get::<AlsRagdollCharacterPhysics>(actor)
                    .is_none()
            );
            assert!(
                !app.world()
                    .get::<crate::als::transition::AlsStateRequest>(actor)
                    .unwrap()
                    .ragdoll_exit
            );
            println!(
                "Source loaded exit support={change} grounded={grounded} action={expected_action:?}"
            );
        }
    }

    #[test]
    fn loaded_scaled_ragdoll_get_up_keeps_walking_on_the_exit_frame() {
        use bevy::ecs::system::RunSystemOnce;

        let (mut app, actor, avatar) = crate::roll_drive::tests::loaded_roll_character();
        let (player, _) = loaded_ragdoll_bodies(&mut app, avatar);
        let start = Vec3::Y * 9.9;
        app.world_mut().entity_mut(actor).insert((
            Position(start),
            Transform::from_translation(start).with_scale(Vec3::splat(2.0)),
            LinearVelocity::ZERO,
        ));
        for _ in 0..120 {
            app.update();
        }
        assert!(
            app.world()
                .get::<AlsRagdollGroundFollow>(actor)
                .is_some_and(|follow| follow.grounded)
        );
        app.world_mut()
            .get_mut::<crate::als::transition::AlsStateRequest>(actor)
            .unwrap()
            .ragdoll_exit = true;
        for frame in 0..6 {
            app.update();
            let world = app.world();
            let mode = *world.get::<LocomotionState>(actor).unwrap();
            let position = world.get::<Position>(actor).unwrap().0;
            let velocity = world.get::<LinearVelocity>(actor).unwrap().0;
            let action = world.get::<AlsNotifyLocomotionAction>(actor);
            assert_eq!(
                action,
                Some(&AlsNotifyLocomotionAction::GettingUp),
                "frame={frame}"
            );
            assert_eq!(
                mode,
                LocomotionState::Grounded,
                "frame={frame} position={position:?} velocity={velocity:?}"
            );
        }
        let (mesh, mount) = app
            .world_mut()
            .run_system_once(move |transforms: TransformHelper| {
                (
                    transforms.compute_global_transform(player).unwrap(),
                    transforms.compute_global_transform(avatar).unwrap(),
                )
            })
            .unwrap();
        assert!((mesh.translation().y - mount.translation().y - 0.92).abs() < 1e-5);
        let world = app.world();
        let graph = world.get::<AnimationGraphPlayer>(player).unwrap();
        let (_, _, rig) = crate::extensions::root_graph_boundaries::current_control_rig(
            graph,
            world.resource::<Assets<bevy_animation_graph::core::animation_graph::AnimationGraph>>(),
        )
        .unwrap();
        assert!(
            rig.to_world
                .translation
                .as_vec3()
                .abs_diff_eq(mesh.translation(), 1e-5),
            "Rig queried from {:?}, while the restored mesh is at {:?}",
            rig.to_world.translation,
            mesh.translation()
        );
        let supports: Vec<_> = app
            .world_mut()
            .query::<(Entity, &RigidBody)>()
            .iter(app.world())
            .filter_map(|(entity, body)| (*body == RigidBody::Static).then_some(entity))
            .collect();
        assert_eq!(supports.len(), 1);
        app.world_mut().despawn(supports[0]);
        for _ in 0..3 {
            app.update();
        }
        assert_eq!(
            app.world().get::<LocomotionState>(actor),
            Some(&LocomotionState::Air)
        );
    }

    #[test]
    fn browser_review_parent_ragdoll_follow_and_exit_write_world_positions() {
        use bevy::ecs::system::RunSystemOnce;
        for exiting in [false, true] {
            let mut app = App::new();
            app.add_plugins(MinimalPlugins);
            let parent = app
                .world_mut()
                .spawn((
                    Transform::from_xyz(10.0, 1.0, -2.0)
                        .with_rotation(Quat::from_rotation_y(0.6))
                        .with_scale(Vec3::splat(1.5)),
                    GlobalTransform::IDENTITY,
                ))
                .id();
            let target = Vec3::new(2.0, 3.0, -4.0);
            let actor = app
                .world_mut()
                .spawn((
                    AlsCharacter,
                    Transform::from_xyz(0.5, 1.0, 0.2),
                    ChildOf(parent),
                    Position(Vec3::ZERO),
                    LinearVelocity(Vec3::ONE),
                    RigidBody::Kinematic,
                    AlsCollisionLayerOwnership::default(),
                    if exiting {
                        AlsNotifyLocomotionAction::None
                    } else {
                        AlsNotifyLocomotionAction::Ragdolling
                    },
                    AlsRagdollGroundFollow {
                        pelvis: target,
                        target,
                        velocity: Vec3::ZERO,
                        grounded: true,
                    },
                ))
                .id();
            if exiting {
                app.world_mut()
                    .entity_mut(actor)
                    .insert(AlsRagdollCharacterPhysics {
                        rigid_body: RigidBody::Dynamic,
                        tnua_toggle: None,
                    });
                app.world_mut()
                    .run_system_once(ragdoll_character_physics_system)
                    .unwrap();
            } else {
                app.world_mut()
                    .run_system_once(apply_ragdoll_ground_follow)
                    .unwrap();
            }
            let world = app
                .world_mut()
                .run_system_once(move |helper: TransformHelper| {
                    helper.compute_global_transform(actor).unwrap()
                })
                .unwrap();
            assert!(
                world.translation().distance(target) < 1e-5,
                "Parented ragdoll actor changed the world target: exiting={exiting}, actual={:?}, target={target:?}",
                world.translation()
            );
            assert_eq!(app.world().get::<Position>(actor).unwrap().0, target);
            assert_eq!(
                app.world().get::<LinearVelocity>(actor).unwrap().0,
                Vec3::ZERO
            );
        }
    }

    #[test]
    fn loaded_ragdoll_exit_preserves_current_actor_pitch_and_roll() {
        for (pitch, roll) in [(0.0_f32, 0.0_f32), (0.2, -0.1), (-0.15, 0.25)] {
            let (mut app, actor, _) = crate::roll_drive::tests::loaded_roll_character();
            app.world_mut()
                .entity_mut(actor)
                .insert(AlsNotifyLocomotionAction::Ragdolling);
            for _ in 0..90 {
                app.update();
            }
            let rotation = Quat::from_euler(EulerRot::YXZ, 0.3, pitch, roll);
            app.world_mut()
                .get_mut::<Transform>(actor)
                .unwrap()
                .rotation = rotation;
            app.world_mut().get_mut::<Rotation>(actor).unwrap().0 = rotation;
            app.world_mut()
                .get_mut::<AlsStateRequest>(actor)
                .unwrap()
                .ragdoll_exit = true;
            app.update();
            let (_, actual_pitch, actual_roll) = app
                .world()
                .get::<Rotation>(actor)
                .unwrap()
                .0
                .to_euler(EulerRot::YXZ);
            assert_ne!(
                app.world().get::<AlsNotifyLocomotionAction>(actor),
                Some(&AlsNotifyLocomotionAction::Ragdolling)
            );
            assert!(
                (actual_pitch - pitch).abs() < 1e-4 && (actual_roll - roll).abs() < 1e-4,
                "Source StopRagdolling replaces only current actor yaw: pitch={pitch} roll={roll} actual_pitch={actual_pitch} actual_roll={actual_roll}"
            );
        }
    }

    #[test]
    fn browser_review_parent_ragdoll_recovery_keeps_actor_and_pelvis_world_space() {
        use bevy::ecs::system::RunSystemOnce;
        let (mut app, actor, avatar) = crate::roll_drive::tests::loaded_roll_character();
        let actor_world = app
            .world_mut()
            .run_system_once(move |helper: TransformHelper| {
                helper.compute_global_transform(actor).unwrap()
            })
            .unwrap();
        let parent_transform = Transform::from_xyz(10.0, 2.0, -3.0)
            .with_rotation(Quat::from_euler(EulerRot::YXZ, 0.7, 0.15, -0.2))
            .with_scale(Vec3::splat(1.5));
        let parent = app.world_mut().spawn(parent_transform).id();
        app.world_mut().entity_mut(actor).insert((
            ChildOf(parent),
            actor_world.reparented_to(&GlobalTransform::from(parent_transform)),
            AlsNotifyLocomotionAction::Ragdolling,
        ));
        let player_entity = app
            .world()
            .get::<AnimatedSceneInstance>(avatar)
            .unwrap()
            .player_entity();
        app.world_mut()
            .run_system_once(capture_ragdoll_player_world_transform)
            .unwrap();
        assert!(
            app.world()
                .get::<AlsRagdollDetachedPlayer>(player_entity)
                .is_some()
        );
        let (pelvis, old_player_world) = {
            let player = app
                .world()
                .get::<AnimationGraphPlayer>(player_entity)
                .unwrap();
            let skeleton = app
                .world()
                .resource::<Assets<Skeleton>>()
                .get(player.skeleton())
                .unwrap();
            let pelvis = skeleton
                .iter_bones()
                .find_map(|bone| {
                    player.bone_entity(bone).filter(|entity| {
                        app.world()
                            .get::<Name>(*entity)
                            .is_some_and(|name| name.as_str() == PELVIS_LABEL)
                    })
                })
                .unwrap();
            (
                pelvis,
                *app.world().get::<GlobalTransform>(player_entity).unwrap(),
            )
        };
        let pelvis_world = *app.world().get::<GlobalTransform>(pelvis).unwrap();
        let bone_parent = app.world().get::<ChildOf>(pelvis).unwrap().parent();
        let parent_in_player = app
            .world()
            .get::<GlobalTransform>(bone_parent)
            .unwrap()
            .reparented_to(&old_player_world);
        let moved_parent = parent_transform
            .with_translation(Vec3::new(12.0, 2.5, -2.0))
            .with_rotation(Quat::from_euler(EulerRot::YXZ, 1.1, -0.15, 0.3));
        *app.world_mut().get_mut::<Transform>(parent).unwrap() = moved_parent;
        let target = actor_world.translation() + Vec3::new(1.0, 0.2, -0.5);
        app.world_mut()
            .get_mut::<Transform>(actor)
            .unwrap()
            .translation = GlobalTransform::from(moved_parent)
            .affine()
            .inverse()
            .transform_point3(target);
        app.world_mut()
            .get_mut::<Transform>(actor)
            .unwrap()
            .rotation =
            moved_parent.rotation.inverse() * Quat::from_euler(EulerRot::YXZ, 0.3, 0.2, -0.1);
        app.world_mut().entity_mut(actor).insert((
            Position(target),
            AlsNotifyLocomotionAction::None,
            LocomotionState::Grounded,
        ));
        let expected_yaw = ragdoll_recovery_rotation(pelvis_world.rotation())
            .0
            .to_euler(EulerRot::YXZ)
            .0;
        let expected_rotation = Quat::from_euler(EulerRot::YXZ, expected_yaw, 0.2, -0.1);
        app.world_mut()
            .run_system_once(restore_ragdoll_player_parent)
            .unwrap();
        let actor_after = app
            .world_mut()
            .run_system_once(move |helper: TransformHelper| {
                helper.compute_global_transform(actor).unwrap()
            })
            .unwrap();
        assert!(
            actor_after.translation().distance(target) < 1e-5,
            "Recovery treated a local actor transform as world: {:?} != {target:?}",
            actor_after.translation()
        );
        assert!(actor_after.rotation().dot(expected_rotation).abs() > 1.0 - 1e-5);
        assert!(
            app.world()
                .get::<Rotation>(actor)
                .unwrap()
                .0
                .dot(expected_rotation)
                .abs()
                > 1.0 - 1e-5
        );
        let player_world = app
            .world_mut()
            .run_system_once(move |helper: TransformHelper| {
                helper.compute_global_transform(player_entity).unwrap()
            })
            .unwrap();
        let player = app
            .world()
            .get::<AnimationGraphPlayer>(player_entity)
            .unwrap();
        let skeleton = app
            .world()
            .resource::<Assets<Skeleton>>()
            .get(player.skeleton())
            .unwrap();
        let bone = skeleton
            .iter_bones()
            .find(|bone| player.bone_entity(*bone) == Some(pelvis))
            .unwrap();
        let pose = player
            .get_global_input_data(crate::ragdolling_layer::RAGDOLLING_FINAL_POSE_KEY)
            .unwrap()
            .as_pose()
            .unwrap();
        let local = pose.get_bone(bone).unwrap().to_transform();
        let restored_pelvis = player_world * parent_in_player * local;
        assert!(
            restored_pelvis
                .translation()
                .distance(pelvis_world.translation())
                < 1e-4
        );
        assert!(
            restored_pelvis
                .rotation()
                .dot(pelvis_world.rotation())
                .abs()
                > 1.0 - 1e-5,
            "Source recovery snapshot must preserve the last simulated pelvis world rotation"
        );
    }

    #[test]
    fn ragdoll_exit_restores_the_mesh_offset_and_live_world_scale() {
        use bevy::ecs::system::RunSystemOnce;

        for (scale, parented, stance, offset) in [
            (Vec3::splat(0.75), false, Stance::Standing, -0.92),
            (Vec3::new(1.4, 0.8, 0.6), false, Stance::Crouching, -0.58),
            (Vec3::splat(0.5), true, Stance::Standing, -0.92),
        ] {
            let (mut app, actor, avatar) = crate::roll_drive::tests::loaded_roll_character();
            if parented {
                let parent = app
                    .world_mut()
                    .spawn(
                        Transform::from_xyz(3.0, 2.0, 1.0)
                            .with_rotation(Quat::from_euler(EulerRot::YXZ, 0.6, 0.1, -0.2))
                            .with_scale(Vec3::splat(1.5)),
                    )
                    .id();
                app.world_mut().entity_mut(actor).insert(ChildOf(parent));
            }
            app.world_mut().get_mut::<Transform>(actor).unwrap().scale = scale;
            app.world_mut()
                .entity_mut(actor)
                .insert((stance, AlsNotifyLocomotionAction::Ragdolling));
            app.world_mut()
                .run_system_once(crate::character_assembly::sync_avatar_stance_offset)
                .unwrap();
            let player = app
                .world()
                .get::<AnimatedSceneInstance>(avatar)
                .unwrap()
                .player_entity();
            app.world_mut()
                .run_system_once(capture_ragdoll_player_world_transform)
                .unwrap();
            assert!(
                app.world()
                    .get::<AlsRagdollDetachedPlayer>(player)
                    .is_some()
            );
            app.world_mut().get_mut::<Transform>(actor).unwrap().scale *= 1.25;
            let detached_scale = {
                let mut mesh = app.world_mut().get_mut::<Transform>(player).unwrap();
                if stance == Stance::Crouching {
                    mesh.scale *= Vec3::new(1.1, 0.9, 1.2);
                }
                mesh.scale
            };
            app.world_mut()
                .entity_mut(actor)
                .insert((AlsNotifyLocomotionAction::None, LocomotionState::Grounded));
            app.world_mut()
                .run_system_once(restore_ragdoll_player_parent)
                .unwrap();
            let (actor_world, mesh_world) = app
                .world_mut()
                .run_system_once(move |helper: TransformHelper| {
                    (
                        helper.compute_global_transform(actor).unwrap(),
                        helper.compute_global_transform(player).unwrap(),
                    )
                })
                .unwrap();
            let expected = actor_world.translation() + actor_world.rotation() * (Vec3::Y * offset);
            assert!(
                mesh_world.translation().distance(expected) < 1e-5,
                "Restored mesh offset: scale={scale:?} parented={parented} stance={stance:?} expected={expected:?} actual={:?}",
                mesh_world.translation()
            );
            assert!(
                mesh_world.scale().abs_diff_eq(detached_scale, 1e-5),
                "Restored mesh scale: parented={parented} stance={stance:?} expected={detached_scale:?} actual={:?}",
                mesh_world.scale()
            );
        }
    }

    use super::*;
    use avian3d::prelude::Sensor;

    #[test]
    fn ragdoll_entry_stops_current_turn_and_transition_montages() {
        use crate::stop_link::{AlsTransitionMontageStack, AlsTransitionPlayback};
        use crate::turn_layer::{AlsTurnDrive, AlsTurnMontageStack, SourceTurnRequest};
        use bevy::ecs::system::RunSystemOnce;

        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        let mut transition = AlsTransitionPlayback::standing_dynamic(0.0);
        transition.weight = 0.75;
        transition.blend_out = 0.45;
        let mut second_transition = AlsTransitionPlayback::crouching_dynamic(1.0);
        second_transition.weight = 0.5;
        second_transition.blend_out = 0.6;
        let character = app
            .world_mut()
            .spawn((
                AlsCharacter,
                Transform::IDENTITY,
                RigidBody::Kinematic,
                AlsCollisionLayerOwnership::default(),
                AlsNotifyLocomotionAction::Ragdolling,
                AlsTransitionMontageStack {
                    entries: vec![transition, second_transition],
                    ..default()
                },
            ))
            .id();
        let mut turn = AlsTurnMontageStack::default();
        SourceTurnRequest {
            variant: crate::als::turn_in_place::TurnVariant::Left90,
            yaw: 90.0,
            crouching: false,
        }
        .play(&mut turn);
        let mut old_turn = AlsTurnMontageStack::default();
        SourceTurnRequest {
            variant: crate::als::turn_in_place::TurnVariant::Right90,
            yaw: 90.0,
            crouching: false,
        }
        .play(&mut old_turn);
        let old_avatar = app
            .world_mut()
            .spawn((
                AlsCharacterAvatar { character },
                ChildOf(character),
                old_turn,
            ))
            .id();
        let avatar = app
            .world_mut()
            .spawn((
                AlsCharacterAvatar { character },
                ChildOf(character),
                AlsTurnDrive::default(),
                turn,
            ))
            .id();
        app.world_mut()
            .entity_mut(character)
            .insert(AlsCharacterAssembly {
                avatar,
                camera: None,
            });

        app.world_mut()
            .run_system_once(ragdoll_character_physics_system)
            .unwrap();
        app.world_mut().flush();
        let turn_stopped = app
            .world()
            .get::<AlsTurnMontageStack>(avatar)
            .unwrap()
            .entries
            .is_empty();
        let transition_stopped = app
            .world()
            .get::<AlsTransitionMontageStack>(character)
            .unwrap()
            .entries
            .iter()
            .all(|entry| {
                entry.stop_elapsed == Some(0.0) && (entry.stop_duration - 0.2).abs() < 1e-6
            });
        assert!(
            turn_stopped && transition_stopped,
            "turn_stopped={turn_stopped} transition_stopped={transition_stopped}"
        );
        assert_eq!(
            app.world()
                .get::<AlsTurnMontageStack>(old_avatar)
                .unwrap()
                .entries
                .len(),
            1
        );
        app.world_mut()
            .run_system_once(ragdoll_character_physics_system)
            .unwrap();
        assert!(
            app.world()
                .get::<AlsTransitionMontageStack>(character)
                .unwrap()
                .entries
                .iter()
                .all(|entry| entry.stop_elapsed == Some(0.0))
        );
    }

    #[test]
    fn native_ragdoll_spawner_reports_completion_once_after_all_joints() {
        use bevy::ecs::system::RunSystemOnce;
        use bevy_animation_graph::core::ragdoll::{
            definition::Ragdoll, spawning::spawn_ragdoll_avian,
        };
        #[derive(Resource, Default)]
        struct Completions(Vec<(Entity, usize)>);
        let source: Ragdoll = ron::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/assets/ALS/Character/PA_Als.rag.ron"
        )))
        .unwrap();
        for joint_count in [0, 1, source.joints.len()] {
            let mut app = App::new();
            app.add_plugins((
                MinimalPlugins,
                AssetPlugin::default(),
                TransformPlugin,
                avian3d::prelude::PhysicsPlugins::default(),
            ));
            app.init_resource::<Completions>().add_observer(
                |event: On<RagdollSpawned>,
                 joints: Query<&bevy_animation_graph::core::ragdoll::definition::JointLabel>,
                 mut completed: ResMut<Completions>| {
                    completed.0.push((event.ragdoll, joints.iter().count()));
                },
            );
            app.finish();
            app.cleanup();
            let player = app.world_mut().spawn_empty().id();
            let mut asset = source.clone();
            let selected = asset
                .joints
                .keys()
                .copied()
                .take(joint_count)
                .collect::<std::collections::HashSet<_>>();
            asset.joints.retain(|id, _| selected.contains(id));
            if joint_count == 0 {
                asset.bodies.retain(|_, body| body.label == PELVIS_LABEL);
                let colliders = asset
                    .bodies
                    .values()
                    .flat_map(|body| body.colliders.iter().copied())
                    .collect::<std::collections::HashSet<_>>();
                asset.colliders.retain(|id, _| colliders.contains(id));
                assert_eq!(asset.bodies.len(), 1);
                assert!(!asset.colliders.is_empty());
            }
            let spawned = app
                .world_mut()
                .run_system_once(move |mut commands: Commands| {
                    spawn_ragdoll_avian(
                        player,
                        &asset,
                        bevy::math::Isometry3d::IDENTITY,
                        None,
                        &mut commands,
                    )
                })
                .unwrap();
            app.world_mut().flush();
            let completed = &app.world().resource::<Completions>().0;
            assert_eq!(
                completed.len(),
                1,
                "Each accepted ragdoll needs one completion, joints={joint_count}"
            );
            assert_eq!(
                completed[0],
                (spawned.root, joint_count),
                "Completion must follow all joint creation"
            );
        }
    }

    #[test]
    fn ragdoll_player_follows_current_assembly_avatar_with_old_player_alive() {
        use bevy::ecs::system::RunSystemOnce;
        use bevy::world_serialization::{WorldAsset, WorldInstanceReady, WorldInstanceSpawner};
        use bevy_animation_graph::AnimationGraphPlugin;
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            AnimationGraphPlugin::default(),
        ));
        app.add_observer(register_ragdoll_camera_trace_owner);
        let character = app.world_mut().spawn(AlsCharacter).id();
        let spawn_avatar = |app: &mut App| {
            let avatar = app
                .world_mut()
                .spawn((
                    AlsCharacterAvatar { character },
                    AnimatedSceneHandle::default(),
                    ChildOf(character),
                ))
                .id();
            let player = app
                .world_mut()
                .spawn((
                    AnimationGraphPlayer::new(Handle::default()),
                    ChildOf(avatar),
                ))
                .id();
            app.world_mut().trigger(WorldInstanceReady {
                entity: avatar,
                instance_id: WorldInstanceSpawner::default().spawn(Handle::<WorldAsset>::default()),
            });
            app.world_mut().flush();
            (avatar, player)
        };
        let (old_avatar, old_player) = spawn_avatar(&mut app);
        let (new_avatar, new_player) = spawn_avatar(&mut app);
        app.world_mut().entity_mut(character).insert(
            crate::character_assembly::AlsCharacterAssembly {
                avatar: new_avatar,
                camera: None,
            },
        );
        let selected = app
            .world_mut()
            .run_system_once(
                move |avatars: Query<(&AlsCharacterAvatar, &AnimatedSceneInstance)>,
                      assemblies: Query<&AlsCharacterAssembly>| {
                    animated_player_for_character(
                        character,
                        assemblies.get(character).ok(),
                        &avatars,
                    )
                },
            )
            .unwrap();
        assert!(app.world().entities().contains(old_avatar));
        assert!(app.world().entities().contains(old_player));
        assert_eq!(selected, Some(new_player));
        let old_ragdoll = app.world_mut().spawn_empty().id();
        let new_ragdoll = app.world_mut().spawn_empty().id();
        app.world_mut().trigger(RagdollSpawned {
            ragdoll: old_ragdoll,
            animation_player: old_player,
        });
        app.world_mut().trigger(RagdollSpawned {
            ragdoll: new_ragdoll,
            animation_player: new_player,
        });
        app.world_mut().flush();
        assert!(
            app.world()
                .get::<crate::als_camera::AlsCameraTraceOwner>(old_ragdoll)
                .is_none()
        );
        assert_eq!(
            app.world()
                .get::<crate::als_camera::AlsCameraTraceOwner>(new_ragdoll)
                .unwrap()
                .0,
            character
        );
    }

    #[test]
    fn ragdoll_activation_prefers_profile_collision_layers_over_saved_birth_layers() {
        use bevy::ecs::system::RunSystemOnce;
        use bevy::reflect::{PartialReflect, ReflectMut};
        use bevy::world_serialization::{WorldAsset, WorldInstanceReady, WorldInstanceSpawner};
        use bevy_animation_graph::AnimationGraphPlugin;
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            AnimationGraphPlugin::default(),
        ));
        let character = app
            .world_mut()
            .spawn((
                AlsCharacter,
                AlsRagdollLink::default(),
                AlsNotifyLocomotionAction::Ragdolling,
            ))
            .id();
        let avatar = app
            .world_mut()
            .spawn((
                AlsCharacterAvatar { character },
                AnimatedSceneHandle::default(),
                ChildOf(character),
            ))
            .id();
        let player = app
            .world_mut()
            .spawn((
                AnimationGraphPlayer::new(Handle::default()),
                ChildOf(avatar),
            ))
            .id();
        app.world_mut().trigger(WorldInstanceReady {
            entity: avatar,
            instance_id: WorldInstanceSpawner::default().spawn(Handle::<WorldAsset>::default()),
        });
        app.world_mut().flush();
        app.world_mut()
            .entity_mut(character)
            .insert(AlsCharacterAssembly {
                avatar,
                camera: None,
            });
        let root = app.world_mut().spawn_empty().id();
        let collider = app
            .world_mut()
            .spawn((
                CollisionLayers::NONE,
                AlsRagdollColliderLayers(CollisionLayers::from_bits(1, 1)),
                RagdollCollisionMember {
                    group: root,
                    body_index: 18,
                    source_layers: CollisionLayers::NONE,
                    disabled_pairs: std::sync::Arc::from([]),
                },
            ))
            .id();
        let mut spawned = SpawnedRagdoll::new(root);
        spawned.colliders.insert(
            ron::from_str("(uuid: \"30000000-0000-4000-8000-000000000001\")").unwrap(),
            collider,
        );
        let mut component = app
            .world_mut()
            .get_mut::<AnimationGraphPlayer>(player)
            .unwrap();
        let ReflectMut::Struct(fields) = component.reflect_mut() else {
            panic!("player must reflect as a struct")
        };
        *fields
            .field_mut("spawned_ragdoll")
            .unwrap()
            .try_downcast_mut::<Option<SpawnedRagdoll>>()
            .unwrap() = Some(spawned);
        drop(component);
        app.world_mut()
            .run_system_once(ragdoll_link_system)
            .unwrap();
        assert_eq!(
            app.world().get::<CollisionLayers>(collider),
            Some(&CollisionLayers::NONE)
        );
    }

    #[test]
    fn ragdoll_ground_query_ignores_visibility_only_with_original_pawn_response() {
        use avian3d::prelude::{LayerMask, PhysicsPlugins, Sensor};
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
            crate::als::character::AlsCollisionLayer::Default,
            crate::als::character::AlsCollisionLayer::Default,
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
                CollisionLayers::new(
                    crate::als::character::AlsCollisionLayer::Visibility,
                    LayerMask::ALL,
                ),
                Transform::from_xyz(0.0, 1.2, 0.0),
            ))
            .id();
        let reverse_reject = app
            .world_mut()
            .spawn((
                RigidBody::Static,
                Collider::cuboid(2.0, 0.1, 2.0),
                CollisionLayers::new(
                    crate::als::character::AlsCollisionLayer::Default,
                    crate::als::character::AlsCollisionLayer::Visibility,
                ),
                Transform::from_xyz(0.0, 0.8, 0.0),
            ))
            .id();
        let sensor = app
            .world_mut()
            .spawn((
                RigidBody::Static,
                Collider::cuboid(2.0, 0.1, 2.0),
                CollisionLayers::new(
                    crate::als::character::AlsCollisionLayer::Default,
                    LayerMask::ALL,
                ),
                Sensor,
                Transform::from_xyz(0.0, 0.4, 0.0),
            ))
            .id();
        let ground = app
            .world_mut()
            .spawn((
                RigidBody::Static,
                Collider::cuboid(2.0, 0.2, 2.0),
                CollisionLayers::new(
                    crate::als::character::AlsCollisionLayer::Default,
                    LayerMask::ALL,
                ),
                Transform::IDENTITY,
            ))
            .id();
        app.update();
        app.world_mut().run_schedule(FixedPostUpdate);
        let hit = app.world_mut().run_system_once(move |spatial: SceneQuery,
            candidates: Query<crate::als::motion::CharacterCollisionCandidates>| {
            ragdoll_ground_hit(&spatial, &candidates, Vec3::Y * 1.6, 1.5, 0.3,
                Vec::new(), ownership.original_layers())
                .map(|hit| hit.entity)
        })
        .unwrap();
        assert_ne!(hit, Some(visibility));
        assert_ne!(hit, Some(reverse_reject));
        assert_ne!(hit, Some(sensor));
        assert_eq!(hit, Some(ground));
    }

    #[test]
    fn ragdoll_air_exit_waits_for_a_blocking_ground() {
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
        ))
        .init_asset::<AlsMovementParams>();
        app.finish();
        app.cleanup();
        let params = app
            .world_mut()
            .resource_mut::<Assets<AlsMovementParams>>()
            .add(AlsMovementParams::default());
        let character = app
            .world_mut()
            .spawn((
                AlsCharacter,
                AlsRagdollAirExit,
                Position(Vec3::Y),
                Stance::Standing,
                AlsMovementParamsHandle(params),
                CollisionLayers::new(
                    crate::als::character::AlsCollisionLayer::Default,
                    crate::als::character::AlsCollisionLayer::Default,
                ),
                Transform::from_xyz(0.0, 1.0, 0.0),
            ))
            .id();
        let reverse_reject = app
            .world_mut()
            .spawn((
                RigidBody::Static,
                Collider::cuboid(2.0, 0.1, 2.0),
                CollisionLayers::new(
                    crate::als::character::AlsCollisionLayer::Default,
                    crate::als::character::AlsCollisionLayer::Visibility,
                ),
                Transform::from_xyz(0.0, 0.4, 0.0),
            ))
            .id();
        app.world_mut().spawn((
            RigidBody::Static,
            Collider::cuboid(2.0, 0.1, 2.0),
            CollisionLayers::new(
                crate::als::character::AlsCollisionLayer::Default,
                LayerMask::ALL,
            ),
            Sensor,
            Transform::from_xyz(0.0, 0.8, 0.0),
        ));
        app.update();
        app.world_mut().run_schedule(FixedPostUpdate);
        app.world_mut()
            .run_system_once(release_ragdoll_air_exit_on_ground)
            .unwrap();
        app.world_mut().flush();
        assert!(app.world().get::<AlsRagdollAirExit>(character).is_some());
        app.world_mut()
            .entity_mut(reverse_reject)
            .insert(CollisionLayers::new(
                crate::als::character::AlsCollisionLayer::Default,
                crate::als::character::AlsCollisionLayer::Default,
            ));
        app.update();
        app.world_mut().run_schedule(FixedPostUpdate);
        app.world_mut()
            .run_system_once(release_ragdoll_air_exit_on_ground)
            .unwrap();
        app.world_mut().flush();
        assert!(app.world().get::<AlsRagdollAirExit>(character).is_none());
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
            .run_system_once(move |spatial: SceneQuery| {
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
        let target_y =
            trace_start.y - hit.distance + half_height - radius + ALS_CHARACTER_FLOOR_CLEARANCE;
        let source_y = 0.1 + standing_half * 2.0 + ALS_CHARACTER_FLOOR_CLEARANCE;
        assert!((target_y - source_y).abs() < 1e-4);
    }

    #[test]
    fn paused_ragdoll_owner_preserves_the_initial_body_speed_limit_budget() {
        use bevy::ecs::system::RunSystemOnce;
        use bevy::reflect::{PartialReflect, ReflectMut};
        use bevy::world_serialization::{WorldAsset, WorldInstanceReady, WorldInstanceSpawner};
        use bevy_animation_graph::AnimationGraphPlugin;
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            AnimationGraphPlugin::default(),
        ));
        let character = app
            .world_mut()
            .spawn((
                AlsCharacter,
                AlsRagdollInitialSpeedLimit::new(0.5),
                AlsNotifyLocomotionAction::Ragdolling,
            ))
            .id();
        let avatar = app
            .world_mut()
            .spawn((
                AlsCharacterAvatar { character },
                AnimatedSceneHandle::default(),
                ChildOf(character),
            ))
            .id();
        let player = app
            .world_mut()
            .spawn((
                AnimationGraphPlayer::new(Handle::default()),
                ChildOf(avatar),
            ))
            .id();
        app.world_mut().trigger(WorldInstanceReady {
            entity: avatar,
            instance_id: WorldInstanceSpawner::default().spawn(Handle::<WorldAsset>::default()),
        });
        app.world_mut().flush();
        app.world_mut()
            .entity_mut(character)
            .insert(AlsCharacterAssembly {
                avatar,
                camera: None,
            });
        let root = app.world_mut().spawn_empty().id();
        let body = app.world_mut().spawn(LinearVelocity(Vec3::X * 10.0)).id();
        let mut spawned = SpawnedRagdoll::new(root);
        spawned.bodies.insert(Body::new().id, body);
        let mut component = app
            .world_mut()
            .get_mut::<AnimationGraphPlayer>(player)
            .unwrap();
        let ReflectMut::Struct(fields) = component.reflect_mut() else {
            panic!("Player must reflect as a struct")
        };
        *fields
            .field_mut("spawned_ragdoll")
            .unwrap()
            .try_downcast_mut::<Option<SpawnedRagdoll>>()
            .unwrap() = Some(spawned);
        drop(component);
        app.world_mut()
            .run_system_once(limit_initial_ragdoll_speed)
            .unwrap();
        assert_eq!(
            app.world().get::<LinearVelocity>(body).unwrap().0,
            Vec3::X * 2.0
        );
        app.world_mut().resource_mut::<Time<Virtual>>().pause();
        for _ in 0..12 {
            app.world_mut()
                .run_system_once(limit_initial_ragdoll_speed)
                .unwrap();
        }
        app.world_mut().resource_mut::<Time<Virtual>>().unpause();
        for _ in 0..7 {
            app.world_mut().get_mut::<LinearVelocity>(body).unwrap().0 = Vec3::X * 10.0;
            app.world_mut()
                .run_system_once(limit_initial_ragdoll_speed)
                .unwrap();
            assert_eq!(
                app.world().get::<LinearVelocity>(body).unwrap().0,
                Vec3::X * 2.0,
                "Source pause must preserve the seven refreshes remaining after entry"
            );
        }
        app.world_mut().get_mut::<LinearVelocity>(body).unwrap().0 = Vec3::X * 10.0;
        app.world_mut()
            .run_system_once(limit_initial_ragdoll_speed)
            .unwrap();
        assert_eq!(
            app.world().get::<LinearVelocity>(body).unwrap().0,
            Vec3::X * 10.0
        );
    }

    #[test]
    fn actual_high_landing_speed_limit_preserves_body_speed_below_saved_entry_speed() {
        use bevy::ecs::system::RunSystemOnce;
        let (mut app, character, avatar) = crate::roll_drive::tests::loaded_roll_character();
        let position = Vec3::new(0.0, 9.824, 0.0);
        let transform = Transform::from_translation(position);
        app.world_mut().entity_mut(character).insert((
            Position(position),
            transform,
            GlobalTransform::from(transform),
            LinearVelocity::ZERO,
        ));
        let mut saved_entry_speed = 0.0;
        let mut entered = false;
        for _ in 0..150 {
            let saved = app
                .world()
                .get::<crate::actual_motion::AlsActualMotion>(character)
                .unwrap()
                .world_velocity
                .length();
            app.update();
            if app.world().get::<AlsNotifyLocomotionAction>(character)
                == Some(&AlsNotifyLocomotionAction::Ragdolling)
            {
                saved_entry_speed = saved;
                entered = true;
                break;
            }
        }
        assert!(
            entered && saved_entry_speed >= 10.0,
            "Actual high landing must enter ragdoll with source saved velocity: {saved_entry_speed}"
        );
        let player = app
            .world()
            .get::<AnimatedSceneInstance>(avatar)
            .unwrap()
            .player_entity();
        let spawned = app
            .world()
            .get::<AnimationGraphPlayer>(player)
            .unwrap()
            .get_spawned_ragdoll()
            .unwrap();
        let body = *spawned
            .bodies
            .values()
            .find(|entity| app.world().get::<RigidBody>(**entity) == Some(&RigidBody::Dynamic))
            .unwrap();
        for speed in [5.0_f32, 20.0] {
            let expected_velocity = Vec3::X * speed.min(saved_entry_speed.max(2.0));
            app.world_mut().get_mut::<LinearVelocity>(body).unwrap().0 = Vec3::X * speed;
            app.world_mut()
                .run_system_once(limit_initial_ragdoll_speed)
                .unwrap();
            assert!(
                app.world()
                    .get::<LinearVelocity>(body)
                    .unwrap()
                    .0
                    .abs_diff_eq(expected_velocity, 1e-5),
                "Source initial ragdoll limit must preserve a body speed below saved entry speed: saved={saved_entry_speed} actual={:?}",
                app.world().get::<LinearVelocity>(body).unwrap().0
            );
        }
    }

    #[test]
    fn loaded_active_ragdoll_pause_preserves_follow_body_and_joint_drive_outputs() {
        let (mut app, character, avatar) = crate::roll_drive::tests::loaded_roll_character();
        app.world_mut()
            .get_mut::<crate::als::transition::AlsStateRequest>(character)
            .unwrap()
            .ragdoll_start = true;
        for _ in 0..4 {
            app.update();
        }
        assert_eq!(
            app.world().get::<AlsNotifyLocomotionAction>(character),
            Some(&AlsNotifyLocomotionAction::Ragdolling)
        );
        let player = app
            .world()
            .get::<AnimatedSceneInstance>(avatar)
            .unwrap()
            .player_entity();
        let spawned = app
            .world()
            .get::<AnimationGraphPlayer>(player)
            .unwrap()
            .get_spawned_ragdoll()
            .unwrap();
        let mut bodies = spawned
            .bodies
            .values()
            .map(|entity| {
                (
                    *entity,
                    app.world().get::<Position>(*entity).unwrap().0,
                    app.world().get::<Rotation>(*entity).unwrap().0,
                    app.world().get::<LinearVelocity>(*entity).unwrap().0,
                    app.world().get::<AngularVelocity>(*entity).unwrap().0,
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(bodies.len(), 21);
        let actor = *app.world().get::<Transform>(character).unwrap();
        let follow = *app
            .world()
            .get::<AlsRagdollGroundFollow>(character)
            .unwrap();
        let drive = *app.world().get::<RagdollAngularDrive>(player).unwrap();
        let pelvis = *spawned
            .bodies
            .values()
            .find(|entity| {
                app.world()
                    .get::<BodyLabel>(**entity)
                    .is_some_and(|label| label.0 == PELVIS_LABEL)
            })
            .unwrap();
        app.world_mut().resource_mut::<Time<Virtual>>().pause();
        for _ in 0..4 {
            app.update();
            assert_eq!(
                *app.world().get::<Transform>(character).unwrap(),
                actor,
                "Source paused ragdoll owner preserves its actor transform"
            );
            let current = app
                .world()
                .get::<AlsRagdollGroundFollow>(character)
                .unwrap();
            assert_eq!(
                (
                    current.pelvis,
                    current.target,
                    current.velocity,
                    current.grounded
                ),
                (
                    follow.pelvis,
                    follow.target,
                    follow.velocity,
                    follow.grounded
                )
            );
            assert_eq!(
                *app.world().get::<RagdollAngularDrive>(player).unwrap(),
                drive,
                "Source paused ragdoll owner preserves its actual joint drive parameters"
            );
            for (entity, position, rotation, velocity, angular) in &bodies {
                assert_eq!(app.world().get::<Position>(*entity).unwrap().0, *position);
                assert_eq!(app.world().get::<Rotation>(*entity).unwrap().0, *rotation);
                assert_eq!(
                    app.world().get::<LinearVelocity>(*entity).unwrap().0,
                    *velocity
                );
                assert_eq!(
                    app.world().get::<AngularVelocity>(*entity).unwrap().0,
                    *angular
                );
            }
        }
        let moved = app.world().get::<Position>(pelvis).unwrap().0 + Vec3::X * 0.1;
        app.world_mut().get_mut::<Position>(pelvis).unwrap().0 = moved;
        app.update();
        assert_eq!(app.world().get::<Position>(pelvis).unwrap().0, moved);
        assert_eq!(*app.world().get::<Transform>(character).unwrap(), actor);
        let current = app
            .world()
            .get::<AlsRagdollGroundFollow>(character)
            .unwrap();
        assert_eq!(
            (current.pelvis, current.target),
            (follow.pelvis, follow.target)
        );
        bodies
            .iter_mut()
            .find(|(entity, _, _, _, _)| *entity == pelvis)
            .unwrap()
            .1 = moved;

        app.world_mut().resource_mut::<Time<Virtual>>().unpause();
        app.update();
        assert_eq!(
            app.world()
                .get::<AlsRagdollGroundFollow>(character)
                .unwrap()
                .pelvis,
            moved
        );
        assert!(
            bodies.iter().any(|(entity, position, _, _, _)| app
                .world()
                .get::<Position>(*entity)
                .unwrap()
                .0
                .distance(*position)
                > 1e-6),
            "Source resumed ragdoll physics continues moving its active bodies"
        );
    }

    #[test]
    fn ragdoll_follow_samples_the_completed_physics_step() {
        use avian3d::prelude::{
            ColliderDisabled, GravityScale, JointDisabled, LinearDamping, SleepingDisabled,
        };
        use bevy::time::TimeUpdateStrategy;

        for delta in [1.0_f32 / 60.0, 1.0 / 30.0] {
            let (mut app, actor, avatar) = crate::roll_drive::tests::loaded_roll_character();
            let (player, bodies) = loaded_ragdoll_bodies(&mut app, avatar);
            let pelvis = bodies
                .iter()
                .find(|(name, _)| name == PELVIS_LABEL)
                .unwrap()
                .1;
            app.world_mut()
                .get_mut::<crate::als::transition::AlsStateRequest>(actor)
                .unwrap()
                .ragdoll_start = true;
            for _ in 0..12 {
                app.update();
            }
            assert_eq!(
                app.world().get::<AlsNotifyLocomotionAction>(actor),
                Some(&AlsNotifyLocomotionAction::Ragdolling)
            );
            let spawned = app
                .world()
                .get::<AnimationGraphPlayer>(player)
                .unwrap()
                .get_spawned_ragdoll()
                .unwrap();
            let colliders: Vec<_> = spawned.colliders.values().copied().collect();
            let joints: Vec<_> = spawned.joints.values().copied().collect();
            for collider in colliders {
                app.world_mut()
                    .entity_mut(collider)
                    .insert(ColliderDisabled);
            }
            for joint in joints {
                app.world_mut().entity_mut(joint).insert(JointDisabled);
            }
            app.world_mut().entity_mut(pelvis).insert((
                Position(Vec3::Y * 10.0),
                LinearVelocity(Vec3::X),
                AngularVelocity::ZERO,
                GravityScale(0.0),
                LinearDamping(0.0),
                SleepingDisabled,
            ));
            app.insert_resource(TimeUpdateStrategy::ManualDuration(
                std::time::Duration::from_secs_f32(delta),
            ));

            for _ in 0..2 {
                let before = app.world().get::<Position>(pelvis).unwrap().0;
                app.update();
                assert!(app.world().get::<Position>(pelvis).unwrap().x > before.x);
                let follow = app.world().get::<AlsRagdollGroundFollow>(actor).unwrap();
                assert_eq!(follow.pelvis, before);
                assert!(!follow.grounded);
                assert!((app.world().get::<Position>(actor).unwrap().x - before.x).abs() < 1e-5);
            }
        }
    }

    #[test]
    fn ragdoll_zero_target_uses_actor_location_for_the_ground_trace() {
        use bevy::ecs::system::RunSystemOnce;
        let (mut app, character, avatar) = crate::roll_drive::tests::loaded_roll_character();
        app.world_mut()
            .get_mut::<crate::als::transition::AlsStateRequest>(character)
            .unwrap()
            .ragdoll_start = true;
        for _ in 0..4 {
            app.update();
        }
        assert_eq!(
            app.world().get::<AlsNotifyLocomotionAction>(character),
            Some(&AlsNotifyLocomotionAction::Ragdolling)
        );
        let player = app
            .world()
            .get::<AnimatedSceneInstance>(avatar)
            .unwrap()
            .player_entity();
        let bodies = app
            .world()
            .get::<AnimationGraphPlayer>(player)
            .unwrap()
            .get_spawned_ragdoll()
            .unwrap()
            .bodies
            .values()
            .copied()
            .collect::<Vec<_>>();
        let pelvis = bodies
            .into_iter()
            .find(|entity| {
                app.world()
                    .get::<BodyLabel>(*entity)
                    .is_some_and(|label| label.0 == PELVIS_LABEL)
            })
            .unwrap();
        let actor = Vec3::new(4.0, 3.0, 6.0);
        app.world_mut().entity_mut(character).insert((
            Position(actor),
            Transform::from_translation(actor),
            GlobalTransform::from_translation(actor),
        ));
        app.world_mut()
            .entity_mut(pelvis)
            .insert((Position(Vec3::ZERO), LinearVelocity::ZERO));
        app.world_mut()
            .run_system_once(ragdoll_ground_follow)
            .unwrap();
        app.world_mut().flush();
        let follow = *app
            .world()
            .get::<AlsRagdollGroundFollow>(character)
            .unwrap();
        assert_eq!(follow.pelvis, Vec3::ZERO);
        assert_eq!(
            follow.target.xz(),
            actor.xz(),
            "Source zero RagdollTargetLocation uses the actual actor location for ground tracing: {follow:?}"
        );
        app.world_mut()
            .run_system_once(apply_ragdoll_ground_follow)
            .unwrap();
        assert_eq!(
            app.world().get::<Position>(character).unwrap().0.xz(),
            actor.xz(),
            "Source zero target must not teleport the actual character horizontally to the world origin"
        );
    }

    #[test]
    fn initial_speed_limit_covers_eight_update_frames() {
        let mut limit = AlsRagdollInitialSpeedLimit::new(0.5);
        for _ in 0..8 {
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

    #[test]
    fn local_ragdoll_input_consumes_the_first_speed_limit_frame() {
        use crate::als::input::AlsInputCommand;
        use bevy::time::TimeUpdateStrategy;

        #[derive(Resource, Default)]
        struct Speeds(Vec<f32>);

        for delta in [1.0_f32 / 60.0, 1.0 / 30.0] {
            let (mut app, actor, avatar) = crate::roll_drive::tests::loaded_roll_character();
            let (_, bodies) = loaded_ragdoll_bodies(&mut app, avatar);
            let pelvis = bodies
                .iter()
                .find(|(name, _)| name == PELVIS_LABEL)
                .unwrap()
                .1;
            app.init_resource::<Speeds>();
            app.insert_resource(TimeUpdateStrategy::ManualDuration(
                std::time::Duration::from_secs_f32(delta),
            ));
            app.add_systems(
                PostUpdate,
                (
                    (move |mut velocities: Query<&mut LinearVelocity>| {
                        velocities.get_mut(pelvis).unwrap().0 = Vec3::X * 3.0;
                    })
                    .after(ragdoll_joint_drive_adapter_system)
                    .before(limit_initial_ragdoll_speed),
                    (move |velocities: Query<&LinearVelocity>, mut speeds: ResMut<Speeds>| {
                        speeds.0.push(velocities.get(pelvis).unwrap().0.x);
                    })
                    .after(limit_initial_ragdoll_speed)
                    .before(RagdollAngularDriveSet),
                )
                    .in_set(RagdollPhysicsHandoffSet),
            );

            for frame in 0..9 {
                app.world_mut()
                    .get_mut::<AlsInputCommand>(actor)
                    .unwrap()
                    .ragdoll_pressed = frame == 0;
                app.update();
                assert_eq!(
                    app.world().get::<AlsNotifyLocomotionAction>(actor),
                    Some(&AlsNotifyLocomotionAction::Ragdolling)
                );
            }
            assert_eq!(
                app.world().resource::<Speeds>().0,
                [2.0, 2.0, 2.0, 2.0, 2.0, 2.0, 2.0, 2.0, 3.0]
            );
        }
    }
    use crate::als::state::{Gait, OverlayState, RotationMode};
    use crate::als::transition::{AlsStateRequest, AlsStateTransitionPlugin};
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
    fn detached_visible_ragdoll_remains_debug_cycle_candidate() {
        use bevy::time::TimeUpdateStrategy;

        let mut app = App::new();
        app.add_plugins((MinimalPlugins, crate::debug_target::AlsDebugTargetPlugin))
            .init_resource::<ButtonInput<KeyCode>>()
            .insert_resource(TimeUpdateStrategy::ManualDuration(
                std::time::Duration::from_secs_f64(0.05),
            ))
            .insert_resource(crate::show_debug_hud::AlsShowDebugHudState {
                visible: true,
                ..default()
            });
        let master = app
            .world_mut()
            .spawn((
                AlsCharacter,
                crate::character_assembly::AlsPlayerCharacter,
                crate::debug_target::AlsLastDebugRender(0.0),
            ))
            .id();
        let actor = app.world_mut().spawn(AlsCharacter).id();
        let avatar = app
            .world_mut()
            .spawn(AlsCharacterAvatar { character: actor })
            .id();
        let original_parent = app.world_mut().spawn(ChildOf(avatar)).id();
        let detached = app
            .world_mut()
            .spawn((
                AlsRagdollDetachedPlayer {
                    character: actor,
                    parent: original_parent,
                    local_transform: Transform::IDENTITY,
                },
                Transform::IDENTITY,
            ))
            .id();
        app.world_mut().spawn((
            bevy::mesh::skinning::SkinnedMesh {
                inverse_bindposes: default(),
                joints: Vec::new(),
            },
            ViewVisibility::VISIBLE,
            ChildOf(detached),
        ));

        app.update();
        let stamped = app
            .world()
            .get::<crate::debug_target::AlsLastDebugRender>(actor)
            .is_some();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::PageUp);
        app.update();
        let selected = app
            .world()
            .get::<crate::debug_target::AlsDebugTarget>(actor)
            .is_some();
        assert!(
            stamped && selected,
            "stamped={stamped} selected={selected} master={master:?}"
        );
    }

    #[test]
    fn detached_ragdoll_cleanup_follows_current_owner_and_normal_despawn() {
        use bevy::reflect::{PartialReflect, ReflectMut};
        use bevy::world_serialization::{WorldAsset, WorldInstanceReady, WorldInstanceSpawner};
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            TransformPlugin,
            bevy_animation_graph::AnimationGraphPlugin::default(),
        ));
        let spawn = |app: &mut App| {
            let actor = app
                .world_mut()
                .spawn((
                    AlsCharacter,
                    Transform::IDENTITY,
                    AlsNotifyLocomotionAction::Ragdolling,
                ))
                .id();
            let avatar = app
                .world_mut()
                .spawn((
                    AlsCharacterAvatar { character: actor },
                    AnimatedSceneHandle::default(),
                    Transform::IDENTITY,
                    ChildOf(actor),
                ))
                .id();
            let parent = app
                .world_mut()
                .spawn((Transform::IDENTITY, ChildOf(avatar)))
                .id();
            let player = app
                .world_mut()
                .spawn((
                    AnimationGraphPlayer::new(default()),
                    Transform::IDENTITY,
                    ChildOf(parent),
                ))
                .id();
            app.world_mut().trigger(WorldInstanceReady {
                entity: avatar,
                instance_id: WorldInstanceSpawner::default().spawn(Handle::<WorldAsset>::default()),
            });
            app.world_mut().flush();
            app.world_mut()
                .entity_mut(actor)
                .insert(AlsCharacterAssembly {
                    avatar,
                    camera: None,
                });
            let root = app.world_mut().spawn_empty().id();
            let body = app
                .world_mut()
                .spawn((RigidBody::Dynamic, ChildOf(root)))
                .id();
            let collider = app
                .world_mut()
                .spawn((Collider::sphere(0.1), ChildOf(body)))
                .id();
            let joint = app.world_mut().spawn(ChildOf(root)).id();
            let spawned = SpawnedRagdoll::new(root);
            let mut component = app
                .world_mut()
                .get_mut::<AnimationGraphPlayer>(player)
                .unwrap();
            let ReflectMut::Struct(fields) = component.reflect_mut() else {
                panic!("player must reflect as a struct")
            };
            *fields
                .field_mut("spawned_ragdoll")
                .unwrap()
                .try_downcast_mut::<Option<SpawnedRagdoll>>()
                .unwrap() = Some(spawned);
            drop(component);
            app.world_mut()
                .run_system_cached(capture_ragdoll_player_world_transform)
                .unwrap();
            assert!(app.world().get::<ChildOf>(player).is_none());
            (actor, avatar, parent, player, [root, body, collider, joint])
        };
        for order in 0..8 {
            let (other_actor, other_avatar, _, other_player, other_members) = spawn(&mut app);
            let (actor, avatar, parent, player, members) = spawn(&mut app);
            match order {
                0 => {
                    app.world_mut().despawn(actor);
                }
                1 => {
                    app.world_mut().despawn(avatar);
                }
                2 => {
                    app.world_mut().despawn(parent);
                }
                3 => {
                    app.world_mut().despawn(avatar);
                    app.world_mut().despawn(actor);
                }
                4 => {
                    app.world_mut().despawn(player);
                    app.world_mut().despawn(avatar);
                }
                5 => {
                    app.world_mut().despawn(avatar);
                    app.world_mut().despawn(player);
                }
                6 | 7 => {
                    let replacement = if order == 7 {
                        avatar
                    } else {
                        app.world_mut()
                            .spawn((
                                AlsCharacterAvatar { character: actor },
                                AnimatedSceneHandle::default(),
                                Transform::IDENTITY,
                                ChildOf(actor),
                            ))
                            .id()
                    };
                    let replacement_player = app
                        .world_mut()
                        .spawn((
                            AnimationGraphPlayer::new(default()),
                            Transform::IDENTITY,
                            ChildOf(replacement),
                        ))
                        .id();
                    app.world_mut().trigger(WorldInstanceReady {
                        entity: replacement,
                        instance_id: WorldInstanceSpawner::default()
                            .spawn(Handle::<WorldAsset>::default()),
                    });
                    app.world_mut().flush();
                    app.world_mut()
                        .get_mut::<AlsCharacterAssembly>(actor)
                        .unwrap()
                        .avatar = replacement;
                    assert!(app.world().entities().contains(replacement_player));
                }
                _ => unreachable!(),
            }
            app.world_mut()
                .run_system_cached(capture_ragdoll_player_world_transform)
                .unwrap();
            app.world_mut().flush();
            assert!(
                !app.world().entities().contains(player),
                "Detached player survives owner removal, order {order}"
            );
            assert!(
                members
                    .iter()
                    .all(|entity| !app.world().entities().contains(*entity)),
                "The player discard hook must clean its ragdoll hierarchy, order {order}"
            );
            assert!(
                app.world().entities().contains(other_actor)
                    && app.world().entities().contains(other_avatar)
            );
            assert!(app.world().entities().contains(other_player));
            assert!(
                other_members
                    .iter()
                    .all(|entity| app.world().entities().contains(*entity))
            );
            app.world_mut().despawn(other_actor);
            if app.world().entities().contains(actor) {
                app.world_mut().despawn(actor);
            }
            app.world_mut()
                .run_system_cached(capture_ragdoll_player_world_transform)
                .unwrap();
            app.world_mut().flush();
        }
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
                Transform::from_xyz(4.0, 1.0, -3.0).with_scale(Vec3::splat(2.0)),
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
        app.world_mut()
            .entity_mut(actor)
            .insert(AlsCharacterAssembly {
                avatar,
                camera: None,
            });
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

        app.world_mut().entity_mut(actor).insert((
            LocomotionState::Grounded,
            AlsNotifyLocomotionAction::Ragdolling,
        ));
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
        assert_eq!(
            app.world().get::<Transform>(player).unwrap().scale,
            Vec3::splat(2.0)
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
            fn display_name(&self) -> String {
                "Graph rig modifier".into()
            }
            fn spec(&self, mut ctx: SpecContext) -> Result<(), GraphError> {
                ctx.add_input_data("pose", DataSpec::Pose)
                    .add_output_data("Pose", DataSpec::Pose)
                    .add_output_time();
                Ok(())
            }
            fn update(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
                let mut pose = ctx.data_back("pose")?.into_pose()?;
                for (name, offset) in [
                    ("scene/root/pelvis", Vec3::X * 0.013),
                    ("scene/root/marker", Vec3::Y * 0.07),
                ] {
                    let index = pose.paths[&EntityPath::from_slashed_string(name.into()).id()];
                    let bone = &mut pose.bones[index];
                    bone.translation = bone.translation.map(|value| value + offset);
                    if name.ends_with("pelvis") {
                        bone.rotation = bone
                            .rotation
                            .map(|value| value * Quat::from_rotation_z(0.18));
                    }
                }
                ctx.set_time(pose.timestamp);
                ctx.set_data_fwd("Pose", pose);
                Ok(())
            }
        }
        fn corrupt_unmapped_ecs_pose(
            characters: Query<&AlsNotifyLocomotionAction>,
            mut bones: Query<(&Name, &mut Transform)>,
        ) {
            if !characters
                .iter()
                .any(|state| *state == AlsNotifyLocomotionAction::Ragdolling)
            {
                return;
            }
            for (name, mut transform) in &mut bones {
                if name.as_str() == "marker" {
                    transform.translation.y = 42.0;
                }
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
        .add_plugins(crate::extensions::RagdollPhysicsProfilePlugin)
        .init_asset::<AlsMovementParams>()
        .add_plugins(AlsRagdollLinkPlugin)
        .add_plugins(crate::extensions::post_physics_animation::PostPhysicsAnimationPlugin)
        .configure_sets(
            PostUpdate,
            crate::ragdolling_layer::AlsRagdollingLayerSet.before(AnimationGraphSet::PrePhysics),
        )
        .configure_sets(
            PostUpdate,
            crate::ab_als_root::AlsRootGraphSet
                .in_set(crate::extensions::post_physics_animation::PostPhysicsAnimationSet::Inputs),
        )
        .add_systems(
            PostUpdate,
            corrupt_unmapped_ecs_pose.after(TransformSystems::Propagate),
        )
        .configure_sets(
            PostUpdate,
            crate::extensions::post_physics_animation::PostPhysicsAnimationSet::Readback
                .after(corrupt_unmapped_ecs_pose),
        )
        .add_systems(
            PostUpdate,
            (
                bevy::transform::systems::mark_dirty_trees,
                bevy::transform::systems::propagate_parent_transforms,
                bevy::transform::systems::sync_simple_transforms,
            )
                .chain()
                .after(crate::extensions::post_physics_animation::PostPhysicsAnimationSet::Readback)
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
        skeleton.add_bone(
            marker_path.clone(),
            marker_local,
            armature_local * marker_local,
        );
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
            (
                "ragdolling",
                "locomotion_action",
                "Als.LocomotionAction.Ragdolling",
            ),
        ] {
            let node = bevy_animation_graph::core::animation_node::AnimationNode::new(
                output,
                crate::extensions::SourceGameplayTagInputNode {
                    key: key.into(),
                    output: "value".into(),
                    expected_tag: tag.into(),
                    ..default()
                },
            );
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
        let rig =
            bevy_animation_graph::core::animation_node::AnimationNode::new("rig", GraphRigModifier);
        graph.add_node(rig.clone());
        graph.add_edge(
            SourcePin::InputData(GraphInputPin::Passthrough("pose".into())),
            TargetPin::NodeData(rig.id, "pose".into()),
        );
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
        scene_world.spawn((
            Name::new("marker"),
            marker_local,
            ChildOf(armature),
            AnimationTargetId(marker_path.id().id()),
            AnimatedBy(scene_player),
        ));
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
        app.world_mut()
            .entity_mut(actor)
            .insert(AlsCharacterAssembly {
                avatar,
                camera: None,
            });
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
            crate::extensions::post_physics_animation::PostPhysicsAnimation,
            crate::extensions::RagdollPhysicsProfile {
                bodies: vec![],
                colliders: vec![],
                joints: vec![],
                collision_disabled_pairs: std::sync::Arc::from([]),
                solver_settings: crate::pa_als::parse_pa_als_profile().solver_settings,
                inertia_conditioning_settings: crate::pa_als::parse_pa_als_profile()
                    .inertia_conditioning_settings,
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

        app.world_mut()
            .entity_mut(actor)
            .insert((LocomotionState::None, AlsNotifyLocomotionAction::Ragdolling));
        let index = pose.paths[&joint_path.id()];
        pose.bones[index].translation = Some(joint_local.translation + Vec3::X * 0.09);
        app.world_mut()
            .get_mut::<AnimationGraphPlayer>(player_entity)
            .unwrap()
            .set_input_data("pose", DataValue::Pose(pose));
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
        let player = app
            .world()
            .get::<AnimationGraphPlayer>(player_entity)
            .unwrap();
        assert_eq!(player.get_outputs()["grounded"].as_bool().unwrap(), false);
        assert_eq!(player.get_outputs()["ragdolling"].as_bool().unwrap(), true);
        assert_eq!(
            app.world()
                .get::<crate::extensions::SourceAnimationInputs>(player_entity)
                .unwrap()
                .gameplay_tag_input("locomotion_action"),
            Some(AlsNotifyLocomotionAction::Ragdolling.tag())
        );
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
        let joint_entity = app
            .world()
            .get::<AnimationGraphPlayer>(player_entity)
            .unwrap()
            .bone_entity(joint_path.id())
            .unwrap();
        let physical_bone = app
            .world()
            .get::<GlobalTransform>(joint_entity)
            .unwrap()
            .compute_transform();
        assert!(
            transforms_match(physical_bone, expected),
            "physics readback must follow the rig pose writes"
        );
        let marker_entity = app
            .world()
            .get::<AnimationGraphPlayer>(player_entity)
            .unwrap()
            .bone_entity(marker_path.id())
            .unwrap();
        assert!(
            app.world()
                .get::<Transform>(marker_entity)
                .unwrap()
                .translation
                .abs_diff_eq(marker_local.translation + Vec3::Y * 0.07, 1e-6),
            "unmapped bones must keep the rig result"
        );
        let followed_actor =
            Transform::from_xyz(8.0, 1.7, -1.0).with_rotation(Quat::from_rotation_y(-0.4));
        app.world_mut()
            .get_mut::<AnimationGraphPlayer>(player_entity)
            .unwrap()
            .set_input_data(
                "ragdoll_config",
                DataValue::from(RagdollConfig {
                    default_readback: Some(false),
                    readback_overrides: [(joint_path.id(), true)].into(),
                    ..default()
                }),
            );
        app.world_mut().entity_mut(actor).insert((
            followed_actor,
            LocomotionState::None,
            Position(followed_actor.translation),
            Rotation(followed_actor.rotation),
        ));
        app.update();
        let player = app
            .world()
            .get::<AnimationGraphPlayer>(player_entity)
            .unwrap();
        assert_eq!(player.get_outputs()["grounded"].as_bool().unwrap(), false);
        assert_eq!(player.get_outputs()["ragdolling"].as_bool().unwrap(), true);
        let actual =
            Transform::from_translation(app.world().get::<Position>(body_entity).unwrap().0)
                .with_rotation(app.world().get::<Rotation>(body_entity).unwrap().0);
        assert!(
            transforms_match(actual, expected),
            "actor follow must not move the simulated body"
        );
        assert!(transforms_match(
            app.world()
                .get::<GlobalTransform>(joint_entity)
                .unwrap()
                .compute_transform(),
            expected
        ));
        assert!(
            app.world()
                .get::<Transform>(marker_entity)
                .unwrap()
                .translation
                .abs_diff_eq(marker_local.translation + Vec3::Y * 0.07, 1e-6)
        );

        app.world_mut()
            .entity_mut(actor)
            .insert((LocomotionState::Air, AlsNotifyLocomotionAction::None));
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
        assert_eq!(
            app.world()
                .get::<crate::extensions::SourceAnimationInputs>(player_entity)
                .unwrap()
                .gameplay_tag_input("locomotion_action"),
            Some(AlsNotifyLocomotionAction::None.tag())
        );
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
    fn ragdoll_recovery_matches_native_rotator_boundaries() {
        #[derive(serde::Deserialize)]
        struct RecoveryCase {
            name: String,
            target_pelvis_xyzw: [f32; 4],
            facing_up: bool,
            actor_yaw_degrees: f64,
        }

        let cases: Vec<RecoveryCase> = serde_json::from_str(include_str!(
            "../../../tests/fixtures/actions/recovery_orientation.json"
        ))
        .unwrap();
        assert_eq!(cases.len(), 7);
        for case in cases {
            let pelvis = Quat::from_array(case.target_pelvis_xyzw);
            let (rotation, facing_up) = ragdoll_recovery_rotation(pelvis);
            let expected = DQuat::from_rotation_y(case.actor_yaw_degrees.to_radians()).as_quat();
            let aligned = if rotation.dot(expected) < 0.0 {
                -rotation
            } else {
                rotation
            };
            assert_eq!(facing_up, case.facing_up, "{}", case.name);
            assert!(
                aligned.abs_diff_eq(expected, 1e-6),
                "{}: {aligned:?} {expected:?}",
                case.name
            );
        }
    }

    #[test]
    fn ragdoll_exit_routes_airborne_velocity_and_grounded_recovery() {
        use bevy::ecs::system::RunSystemOnce;
        for initial_mode in [
            LocomotionState::None,
            LocomotionState::Grounded,
            LocomotionState::Custom,
        ] {
            for grounded in [false, true] {
                let (mut app, actor, avatar) = crate::roll_drive::tests::loaded_roll_character();
                loaded_ragdoll_bodies(&mut app, avatar);
                app.world_mut()
                    .get_mut::<crate::als::transition::AlsStateRequest>(actor)
                    .unwrap()
                    .ragdoll_start = true;
                for _ in 0..8 {
                    app.update();
                }
                assert_eq!(
                    app.world().get::<AlsNotifyLocomotionAction>(actor),
                    Some(&AlsNotifyLocomotionAction::Ragdolling)
                );
                let saved_velocity = Vec3::new(2.5, -4.0, -1.5);
                {
                    let mut follow = app
                        .world_mut()
                        .get_mut::<AlsRagdollGroundFollow>(actor)
                        .unwrap();
                    follow.pelvis = if grounded {
                        Vec3::Y * 0.2
                    } else {
                        Vec3::Y * 10.0
                    };
                    follow.velocity = saved_velocity;
                }
                app.world_mut()
                    .entity_mut(actor)
                    .insert((initial_mode, LinearVelocity(Vec3::ONE)));
                app.world_mut()
                    .get_mut::<crate::als::transition::AlsDesiredStance>(actor)
                    .unwrap()
                    .0 = Stance::Crouching;
                {
                    let mut request = app
                        .world_mut()
                        .get_mut::<crate::als::transition::AlsStateRequest>(actor)
                        .unwrap();
                    request.ragdoll_exit = true;
                    request.locomotion_state = Some(LocomotionState::Grounded);
                }
                app.world_mut()
                    .run_system_once(ragdoll_exit_route_system)
                    .unwrap();
                assert_eq!(
                    *app.world().get::<LocomotionState>(actor).unwrap(),
                    if grounded {
                        LocomotionState::Grounded
                    } else {
                        LocomotionState::Air
                    }
                );
                assert_eq!(
                    *app.world().get::<LinearVelocity>(actor).unwrap(),
                    LinearVelocity(if grounded { Vec3::ZERO } else { saved_velocity })
                );
                assert_eq!(
                    app.world().get::<AlsRagdollAirExit>(actor).is_some(),
                    !grounded
                );
                assert_eq!(
                    app.world()
                        .get::<crate::als::motion::AlsCrouchTransaction>(actor)
                        .unwrap()
                        .pending_action_stance,
                    Some(if grounded {
                        Stance::Crouching
                    } else {
                        Stance::Standing
                    })
                );
                let request = app
                    .world()
                    .get::<crate::als::transition::AlsStateRequest>(actor)
                    .unwrap();
                assert!(!request.ragdoll_exit && request.locomotion_state.is_none());
            }
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
