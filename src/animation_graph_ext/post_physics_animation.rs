use std::{
    collections::HashSet,
    sync::Arc,
    time::{Duration, Instant},
};

use crate::extensions::{
    acl_animation::AlsAclAnimations,
    control_rig_hierarchy::{RigTransform, rig_normalize},
    pre_rig_sharing::{ActorScope, AlsPreRigSharingSettings, with_actor_scope},
    source_pose::SourceSkeletonLayouts,
    source_sequence_player::{AlsAnimationSharingSettings, SharedClipSamples},
};
use crate::source_physics_timing::source_physics_delta;
use avian3d::prelude::{PhysicsSchedule, PhysicsStepSystems, PhysicsSystems, Position, Rotation};
use bevy::reflect::ReflectRef;
use bevy::{
    animation::{AnimatedBy, AnimationTargetId},
    asset::AssetEvent,
    ecs::{
        batching::BatchingStrategy,
        query::{QueryFilter, QueryItem},
        schedule::InternedScheduleLabel,
        system::SystemParam,
    },
    math::{Affine3A, Isometry3d},
    prelude::*,
    transform::TransformSystems,
};
use bevy_animation_graph::{AnimationGraphPlayback, AnimationPlaybackSystems};
use bevy_animation_graph::core::{
    animated_scene::{AnimatedScene, AnimatedSceneHandle, AnimatedSceneInstance},
    animation_clip::GraphClip,
    animation_graph::{AnimationGraph, DEFAULT_OUTPUT_RAGDOLL_CONFIG},
    animation_graph_player::{AnimationGraphPlayer, AnimationSource},
    context::{pose_fallback::PoseFallbackContext, system_resources::SystemResources},
    id::BoneId,
    plugin::AnimationGraphSet,
    pose::Pose,
    ragdoll::{
        bone_mapping::RagdollBoneMap,
        configuration::RagdollConfig,
        relative_kinematic_body::{RelativeKinematicBody, RelativeKinematicBodyPositionBased},
        spawning::SpawnedRagdoll,
    },
    skeleton::Skeleton,
    space_conversion::SpaceConversionContext,
    state_machine::high_level::StateMachine,
    systems::{
        RootMotionOutput, debug_draw_animation_players, extract_root_motion,
        run_animation_player,
    },
};

#[derive(Component)]
pub struct PostPhysicsAnimation;

/// Evaluate this player on the host physics clock. Use it for a separate graph whose
/// output is consumed by the simulation, such as the root displacement of a roll.
#[derive(Component, Default)]
pub struct PhysicsAnimationPlayer;

/// Dedicated simulation graphs publish root motion here, once per actual Avian step.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PhysicsAnimationSet;

/// Optional cumulative wall time spent evaluating ALS graphs, including parallel workers.
/// Read the difference across a physics or presentation phase to measure that phase.
#[derive(Resource, Default)]
pub struct GraphEvaluationTime(std::time::Duration);

impl GraphEvaluationTime {
    pub fn elapsed(&self) -> std::time::Duration {
        self.0
    }
}

#[derive(Component)]
pub struct ManualAnimationPlayer;

/// Applies one player's pose to a separate visual skeleton with matching bone paths.
///
/// Put this on the entity referenced by the visual bones' `AnimatedBy` components.
/// The leader owns graph time, events and curves. Each visual keeps its own world transform.
#[derive(Component, Clone, Debug)]
pub struct AlsAnimationPoseFollower {
    pub leader: Entity,
    pub skeleton: Handle<Skeleton>,
}

#[derive(Component, Clone, Copy, Debug)]
pub struct InitialAnimation {
    pub ready: bool,
}

#[derive(Component, Clone, Copy, Debug)]
pub struct AnimationPoseUpdateGate {
    pub update_graph: bool,
    pub refresh_bones: bool,
    pub pending_update: bool,
}

#[derive(Component)]
pub(crate) struct PublishedSourceBone {
    pub(crate) local_cm: RigTransform,
    graph_rotation: Quat,
    visible_rotation: Quat,
}

impl PublishedSourceBone {
    pub(crate) fn new(
        local_cm: RigTransform,
        graph_rotation: Quat,
        visible_rotation: Quat,
    ) -> Self {
        Self {
            local_cm,
            graph_rotation,
            visible_rotation,
        }
    }

    pub(crate) fn matches_rotation(&self, rotation: Quat) -> bool {
        rotation == self.graph_rotation || rotation == self.visible_rotation
    }
}

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PostPhysicsAnimationSet {
    Inputs,
    Evaluate,
    Readback,
}

pub struct PostPhysicsAnimationPlugin;

impl Plugin for PostPhysicsAnimationPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<crate::extensions::SourceAnimationInputsPlugin>() {
            app.add_plugins(crate::extensions::SourceAnimationInputsPlugin);
        }
        app.add_plugins(AnimationGraphPlayback {
            physics: install_physics_playback,
            pose: install_pose_playback,
        });
    }
}

fn install_physics_playback(app: &mut App, outer: InternedScheduleLabel) {
    app.add_systems(
        outer,
        update_relative_kinematic_target_velocities
            .in_set(AnimationPlaybackSystems::KinematicTargets)
            .run_if(|time: Res<Time>| !time.delta().is_zero()),
    );
    if crate::AlsPhysicsSchedule::uses_physics_steps(app) {
        app.configure_sets(
            PhysicsSchedule,
            PhysicsAnimationSet
                .before(bevy_tnua::TnuaPipelineSystems::Sensors)
                .before(PhysicsStepSystems::First),
        )
        .add_systems(
            PhysicsSchedule,
            (
                evaluate_players::<With<PhysicsAnimationPlayer>>,
                publish_sampled_motion::<With<PhysicsAnimationPlayer>>,
            )
                .chain()
                .in_set(PhysicsAnimationSet),
        );
    } else {
        app.add_systems(
            outer,
            (
                evaluate_players::<Without<PostPhysicsAnimation>>,
                extract_root_motion,
            )
                .chain()
                .in_set(AnimationPlaybackSystems::Sample),
        );
    }
}

fn install_pose_playback(app: &mut App, pose: InternedScheduleLabel) {
    app.configure_sets(
        pose,
        (
            PostPhysicsAnimationSet::Inputs.after(PhysicsSystems::Last),
            PostPhysicsAnimationSet::Evaluate
                .after(PostPhysicsAnimationSet::Inputs)
                .before(AnimationGraphSet::PostPhysics),
            AnimationGraphSet::Final.before(TransformSystems::Propagate),
            PostPhysicsAnimationSet::Readback
                .after(AnimationGraphSet::Final)
                .after(TransformSystems::Propagate),
        ),
    )
    .add_systems(pose, apply_animation_to_visible_targets.in_set(AnimationPlaybackSystems::WritePose))
    .add_systems(
        pose,
        (read_back_ragdoll_poses, normalize_scaled_readback_pose, apply_readback_pose)
            .chain()
            .in_set(PostPhysicsAnimationSet::Readback),
    );
    if crate::AlsPhysicsSchedule::uses_physics_steps(app) {
        type EarlyFrame = (Without<PostPhysicsAnimation>, Without<PhysicsAnimationPlayer>);
        type LateFrame = (With<PostPhysicsAnimation>, Without<PhysicsAnimationPlayer>);
        app.configure_sets(
            pose,
            (
                AnimationGraphSet::PrePhysics,
                PostPhysicsAnimationSet::Inputs,
                PostPhysicsAnimationSet::Evaluate,
                AnimationGraphSet::PostPhysics,
                AnimationGraphSet::Final,
            ).chain(),
        )
        .add_systems(
            pose,
            (
                evaluate_players::<EarlyFrame>,
                publish_sampled_motion::<EarlyFrame>,
            )
                .chain()
                .in_set(AnimationGraphSet::PrePhysics),
        )
        .add_systems(
            pose,
            (evaluate_players::<LateFrame>, publish_sampled_motion::<LateFrame>)
                .chain()
                .in_set(PostPhysicsAnimationSet::Evaluate),
        );
    } else {
        app.add_systems(
            pose,
            (evaluate_players::<With<PostPhysicsAnimation>>, extract_late_root_motion)
                .chain()
                .in_set(PostPhysicsAnimationSet::Evaluate),
        );
    }
}

pub(crate) fn source_kinematic_angular_velocity(start: Quat, target: Quat, delta: f32) -> Vec3 {
    if delta <= 1e-8 {
        return Vec3::ZERO;
    }
    let target = if start.dot(target) < 0.0 {
        -target
    } else {
        target
    };
    let derivative = (target - start) / delta;
    let angular = (derivative * start.conjugate()) * 2.0;
    Vec3::new(angular.x, angular.y, angular.z)
}

pub(crate) fn update_relative_kinematic_target_velocities(
    time: Res<Time>,
    schedule: Option<Res<crate::AlsPhysicsSchedule>>,
    legacy_clock: Option<Res<crate::source_physics_timing::SourcePhysicsClockRestore>>,
    mut bodies: Query<(
        &RelativeKinematicBodyPositionBased,
        &Position,
        &Rotation,
        &mut RelativeKinematicBody,
        Has<crate::extensions::RagdollActiveMode>,
    )>,
    references: Query<(&Position, &Rotation), Without<RelativeKinematicBodyPositionBased>>,
) {
    let world_delta = time.delta_secs();
    let physics_delta = if schedule.is_none() || legacy_clock.is_some() {
        source_physics_delta(time.delta()).as_secs_f32()
    } else {
        world_delta
    };
    for (target, position, rotation, mut body, use_source) in &mut bodies {
        let world = Isometry3d::new(position.0, rotation.0);
        let current = target
            .relative_to
            .and_then(|entity| references.get(entity).ok())
            .map_or(world, |(position, rotation)| {
                Isometry3d::new(position.0, rotation.0).inverse() * world
            });
        let next = target.relative_target;
        let delta = if use_source {
            physics_delta
        } else {
            world_delta
        };
        body.kinematic_linear_velocity = ((next.translation - current.translation) / delta).into();
        body.kinematic_angular_velocity = if use_source {
            source_kinematic_angular_velocity(current.rotation, next.rotation, delta)
        } else {
            let next_rotation = if current.rotation.dot(next.rotation) < 0.0 {
                -next.rotation
            } else {
                next.rotation
            };
            (next_rotation * current.rotation.conjugate())
                .normalize()
                .to_scaled_axis()
                / delta
        };
        body.relative_to = target.relative_to;
    }
}

fn has_physics_readback(player: &AnimationGraphPlayer) -> bool {
    player.get_spawned_ragdoll().is_some()
        && player
            .get_outputs()
            .get(DEFAULT_OUTPUT_RAGDOLL_CONFIG)
            .and_then(|value| value.as_ragdoll_config().ok())
            .is_none_or(|config| {
                config.default_readback.unwrap_or(true)
                    || config.readback_overrides.values().any(|enabled| *enabled)
            })
}

fn read_back_ragdoll_poses(
    mut players: Query<(Entity, &mut AnimationGraphPlayer)>,
    maps: Res<Assets<RagdollBoneMap>>,
    bodies: Query<(&Position, &Rotation)>,
    resources: SystemResources,
) {
    for (entity, mut player) in &mut players {
        if resources
            .transform_query
            .get(entity)
            .is_ok_and(|(transform, _)| transform.scale.abs().max_element() <= 1e-4)
            || !has_physics_readback(&player)
        {
            continue;
        }
        let ReflectRef::Struct(fields) = player.reflect_ref() else {
            continue;
        };
        let Some(map) = fields
            .field("ragdoll_bone_map")
            .and_then(|field| field.try_downcast_ref::<Option<Handle<RagdollBoneMap>>>())
            .and_then(|handle| handle.as_ref())
            .and_then(|handle| maps.get(handle))
        else {
            continue;
        };
        let Some(spawned) = player.get_spawned_ragdoll() else {
            continue;
        };
        let Some(pose) = player.get_default_output_pose() else {
            continue;
        };
        let Some(skeleton) = resources.skeleton_assets.get(&pose.skeleton) else {
            continue;
        };
        let config = player
            .get_outputs()
            .get(DEFAULT_OUTPUT_RAGDOLL_CONFIG)
            .and_then(|value| value.as_ragdoll_config().ok())
            .cloned()
            .unwrap_or_default();
        let entities = skeleton
            .iter_bones()
            .filter_map(|bone| player.bone_entity(bone).map(|entity| (bone, entity)))
            .collect();
        let fallback = PoseFallbackContext {
            entity_map: &entities,
            resources: &resources,
            fallback_to_identity: true,
        };
        let updated = bevy_animation_graph::core::ragdoll::read_pose_avian::read_pose(
            spawned, map, skeleton, &bodies, fallback, pose, &config,
        );
        player.set_default_output_pose(updated);
    }
}

fn successful_readback_bones(
    map: &RagdollBoneMap,
    spawned: &SpawnedRagdoll,
    config: &RagdollConfig,
    bodies: &Query<(&Position, &Rotation)>,
) -> HashSet<BoneId> {
    map.bones_from_bodies
        .iter()
        .filter_map(|(path, mapping)| {
            if !config.should_readback(path.id()).unwrap_or(true) {
                return None;
            }
            mapping
                .bodies
                .iter()
                .any(|body| {
                    body.weight > 0.0
                        && spawned
                            .bodies
                            .get(&body.body)
                            .is_some_and(|entity| bodies.get(*entity).is_ok())
                })
                .then_some(path.id())
        })
        .collect()
}

fn normalize_ragdoll_pose_scale(
    pose: &mut Pose,
    map: &RagdollBoneMap,
    skeleton: &Skeleton,
    conversions: SpaceConversionContext,
    uniform_scale: Option<f32>,
    readback: &HashSet<BoneId>,
) {
    let mut targets = map
        .bones_from_bodies
        .keys()
        .filter(|path| readback.contains(&path.id()))
        .filter_map(|path| {
            let index = *pose.paths.get(&path.id())?;
            let target = conversions.global_transform_of_bone(pose, skeleton, path.id());
            Some((path, index, target))
        })
        .collect::<Vec<_>>();
    targets.sort_by_key(|(path, _, _)| path.parts.len());
    for (path, index, mut target) in targets {
        let parent = path
            .parent()
            .map(|path| conversions.global_transform_of_bone(pose, skeleton, path.id()))
            .unwrap_or_default();
        let local = if let Some(scale) = uniform_scale {
            target.scale *= scale;
            Transform::from_matrix(
                (parent.compute_affine().inverse() * target.compute_affine()).into(),
            )
        } else {
            let fallback = &conversions.pose_fallback;
            let mut parent_affine = Affine3A::IDENTITY;
            let mut ancestor = skeleton.parent(&path.id());
            while let Some(id) = ancestor {
                if id == skeleton.root() {
                    parent_affine = fallback
                        .root_global_transform(skeleton)
                        .unwrap_or_default()
                        .affine()
                        * parent_affine;
                    break;
                }
                let base = fallback.local_transform(id).unwrap_or_default();
                let transform = pose
                    .get_bone(id)
                    .map_or(base, |bone| bone.to_transform_with_base(base));
                parent_affine = transform.compute_affine() * parent_affine;
                ancestor = skeleton.parent(&id);
            }
            Transform {
                translation: parent_affine.inverse().transform_point3(target.translation),
                rotation: (parent.rotation.conjugate() * target.rotation).normalize(),
                scale: fallback
                    .local_transform(path.id())
                    .unwrap_or_default()
                    .scale,
            }
        };
        let bone = &mut pose.bones[index];
        bone.translation = Some(local.translation);
        bone.rotation = Some(local.rotation);
        bone.scale = Some(local.scale);
    }
}

fn normalize_scaled_readback_pose(
    mut players: Query<(
        Entity,
        &mut AnimationGraphPlayer,
        &GlobalTransform,
        Has<crate::extensions::RagdollPhysicsProfile>,
    )>,
    hosts: Query<(&AnimatedSceneHandle, &AnimatedSceneInstance)>,
    scenes: Option<Res<Assets<AnimatedScene>>>,
    maps: Res<Assets<RagdollBoneMap>>,
    bodies: Query<(&Position, &Rotation)>,
    resources: SystemResources,
) {
    let Some(scenes) = scenes else {
        return;
    };
    for (entity, mut player, world, source_profile) in &mut players {
        if !has_physics_readback(&player) {
            continue;
        }
        let scale = world.to_scale_rotation_translation().0;
        let uniform_scale = (scale.max_element() - scale.min_element() <= 1e-4).then_some(scale.x);
        if !scale.is_finite()
            || scale.min_element() <= 0.0
            || (uniform_scale.is_none() && !source_profile)
            || uniform_scale.is_some_and(|scale| (scale - 1.0).abs() <= 1e-4)
        {
            continue;
        }
        let Some(map) = hosts
            .iter()
            .find(|(_, instance)| instance.player_entity() == entity)
            .and_then(|(handle, _)| scenes.get(&handle.handle))
            .and_then(|scene| scene.ragdoll_bone_map.as_ref())
            .and_then(|handle| maps.get(handle))
        else {
            continue;
        };
        let Some(mut pose) = player.get_default_output_pose().cloned() else {
            continue;
        };
        let Some(skeleton) = resources.skeleton_assets.get(&pose.skeleton) else {
            continue;
        };
        let Some(spawned) = player.get_spawned_ragdoll() else {
            continue;
        };
        let config = player
            .get_outputs()
            .get(DEFAULT_OUTPUT_RAGDOLL_CONFIG)
            .and_then(|value| value.as_ragdoll_config().ok())
            .cloned()
            .unwrap_or_default();
        let readback = successful_readback_bones(map, spawned, &config, &bodies);
        if readback.is_empty() {
            continue;
        }
        let entities = skeleton
            .iter_bones()
            .filter_map(|bone| player.bone_entity(bone).map(|entity| (bone, entity)))
            .collect();
        let conversions = SpaceConversionContext {
            pose_fallback: PoseFallbackContext {
                entity_map: &entities,
                resources: &resources,
                fallback_to_identity: true,
            },
        };
        normalize_ragdoll_pose_scale(
            &mut pose,
            map,
            skeleton,
            conversions,
            uniform_scale,
            &readback,
        );
        player.set_default_output_pose(pose);
    }
}

fn apply_readback_pose(
    mut commands: Commands,
    mut targets: Query<(
        Entity,
        &mut Transform,
        &AnimationTargetId,
        &AnimatedBy,
        Option<&PublishedSourceBone>,
    )>,
    players: Query<(&AnimationGraphPlayer, Option<&AnimationPoseUpdateGate>)>,
) {
    for (entity, mut transform, id, animated_by, precise) in &mut targets {
        let Ok((player, gate)) = players.get(animated_by.0) else {
            continue;
        };
        if gate.is_some_and(|gate| !gate.refresh_bones) || !has_physics_readback(player) {
            continue;
        }
        let Some(pose) = player.get_default_output_pose() else {
            continue;
        };
        let Some(index) = pose.paths.get(&BoneId::from(*id)) else {
            continue;
        };
        let bone = &pose.bones[*index];
        if precise.is_some_and(|precise| {
            bone.translation
                .is_some_and(|value| value != (precise.local_cm.translation * 0.01).as_vec3())
                || bone
                    .rotation
                    .is_some_and(|value| !precise.matches_rotation(value))
                || bone
                    .scale
                    .is_some_and(|value| value != precise.local_cm.scale.as_vec3())
        }) {
            commands.entity(entity).remove::<PublishedSourceBone>();
        }
        if let Some(value) = bone.translation {
            transform.translation = value;
        }
        if let Some(value) = bone.rotation {
            transform.rotation = value;
        }
        if let Some(value) = bone.scale {
            transform.scale = value;
        }
    }
}

#[derive(SystemParam)]
#[doc(hidden)]
pub struct SharingAssetEvents<'w, 's> {
    graphs: Option<MessageReader<'w, 's, AssetEvent<AnimationGraph>>>,
    clips: Option<MessageReader<'w, 's, AssetEvent<GraphClip>>>,
    states: Option<MessageReader<'w, 's, AssetEvent<StateMachine>>>,
    skeletons: Option<MessageReader<'w, 's, AssetEvent<Skeleton>>>,
    gltfs: Option<MessageReader<'w, 's, AssetEvent<bevy::gltf::Gltf>>>,
    gltf_assets: Option<Res<'w, Assets<bevy::gltf::Gltf>>>,
}

impl SharingAssetEvents<'_, '_> {
    fn has_revision(&mut self, resources: &SystemResources) -> bool {
        fn revised<A: Asset>(reader: &mut Option<MessageReader<AssetEvent<A>>>) -> bool {
            let Some(reader) = reader else { return false };
            let mut changed = false;
            for event in reader.read() {
                changed |= matches!(event, AssetEvent::Modified { .. } | AssetEvent::Removed { .. });
            }
            changed
        }

        let published_revision = revised(&mut self.graphs)
            | revised(&mut self.clips)
            | revised(&mut self.states)
            | revised(&mut self.skeletons)
            | revised(&mut self.gltfs);

        // Camera and UI systems can publish asset events after pose sampling.
        published_revision
            || resources.animation_graph_assets.is_changed()
            || resources.graph_clip_assets.is_changed()
            || resources.state_machine_assets.is_changed()
            || resources.skeleton_assets.is_changed()
            || self
                .gltf_assets
                .as_ref()
                .is_some_and(|assets| assets.is_changed())
    }
}

fn batch_size_within_step(
    requested: usize,
    (previous_width, elapsed): (usize, Duration),
    step: Duration,
) -> usize {
    let capacity = (previous_width as u128).saturating_mul(step.as_nanos())
        / elapsed.as_nanos().max(1);
    capacity.clamp(1, requested as u128) as usize
}

type PlayerEvaluation = (
    Entity,
    &'static mut AnimationGraphPlayer,
    &'static mut crate::extensions::SourceAnimationInputs,
    Option<&'static mut AnimationPoseUpdateGate>,
    Option<&'static InitialAnimation>,
    Option<&'static crate::AlsAnimationLod>,
    Option<&'static mut crate::AlsAnimationRate>,
);

pub fn evaluate_players<F: QueryFilter>(
    time: Res<Time>,
    fixed_time: Option<Res<Time<Fixed>>>,
    mut players: Query<PlayerEvaluation, (F, Without<ManualAnimationPlayer>)>,
    resources: SystemResources,
    timing: Option<ResMut<GraphEvaluationTime>>,
    mut skeleton_layouts: Local<Arc<SourceSkeletonLayouts>>,
    sharing: Option<Res<AlsAnimationSharingSettings>>,
    pre_rig_sharing: Option<Res<AlsPreRigSharingSettings>>,
    clip_samples: Local<Arc<SharedClipSamples>>,
    acl_animations: Option<Res<AlsAclAnimations>>,
    mut asset_events: SharingAssetEvents,
    mut previous_batch: Local<Option<(usize, Duration)>>,
) {
    let timing_started = timing.as_ref().map(|_| bevy::platform::time::Instant::now());
    if resources.skeleton_assets.is_changed() {
        *skeleton_layouts = Arc::new(SourceSkeletonLayouts::new(&resources.skeleton_assets));
    }
    let skeleton_layouts = &*skeleton_layouts;
    let pre_rig_enabled = sharing.as_ref().is_some_and(|settings| settings.enabled)
        && pre_rig_sharing.as_ref().is_some_and(|settings| settings.enabled)
        && players.iter().nth(1).is_some();
    let inputs_changed = pre_rig_enabled
        && (asset_events.has_revision(&resources)
            || acl_animations.as_ref().is_some_and(|assets| assets.is_changed()));
    let requested_batch = clip_samples
        .pre_rig_sharing
        .begin_frame(pre_rig_enabled, inputs_changed);
    let batch_size = previous_batch.map_or(requested_batch, |previous| {
        let step = fixed_time.as_ref().map_or_else(|| time.delta(), |time| time.timestep());
        // Estimate how much shared work fits into one simulation step.
        batch_size_within_step(requested_batch, previous, step)
    });
    let acl_animations = acl_animations.as_deref().filter(|animations| !animations.is_empty());
    let clip_samples = sharing.filter(|sharing| sharing.enabled).map(|_| {
        clip_samples.clear();
        &*clip_samples
    });
    let started = pre_rig_enabled.then(Instant::now);
    let evaluate =
        |(entity, mut player, mut inputs, mut gate, initial, lod, mut rate):
             QueryItem<'_, '_, PlayerEvaluation>| {
            let actor_scope = pre_rig_enabled
                .then(|| ActorScope::for_player(entity, &player, lod))
                .flatten();
            let initialize = initial.is_some()
                || gate.as_ref().is_some_and(|gate| gate.refresh_bones)
                    && player.get_animation_source().is_graph()
                    && player
                        .get_context_arena()
                        .is_none_or(|arena| arena.frame_index() == 0);
            let update_graph = gate.as_ref().is_none_or(|gate| gate.update_graph);
            if !update_graph && !initialize
                && let Some(rate) = rate.as_mut()
            {
                rate.cancel_inactive();
            }
            if (initialize || update_graph)
                && initial.is_none_or(|initial| {
                    initial.ready && player.get_default_output_pose().is_none()
                })
            {
                let playing = !player.is_paused();
                player.set_global_input_data(
                    "animation.evaluate_pose",
                    initialize || gate.as_ref().is_none_or(|gate| gate.refresh_bones),
                );
                if initialize {
                    let delta = player
                        .get_global_input_data("animation.delta_seconds")
                        .cloned();
                    if delta.is_some() {
                        player.set_global_input_data("animation.delta_seconds", 0.0_f32);
                    }
                    inputs.evaluate_with_skeleton_layouts(entity, skeleton_layouts, lod, clip_samples, acl_animations, || {
                        with_actor_scope(actor_scope.clone(), || {
                            run_animation_player(
                                entity,
                                player.reborrow(),
                                &Time::default(),
                                &resources,
                            )
                        })
                    });
                    if let Some(delta) = delta {
                        player.set_global_input_data("animation.delta_seconds", delta);
                    }
                    if player.get_error().is_none() && let Some(rate) = rate.as_mut() {
                        rate.publish(None, None);
                    }
                }
                if update_graph
                    && (!initialize
                        || (initial.is_none()
                            && playing
                            && !player.is_paused()
                            && player.get_error().is_none()
                            && player.get_default_output_pose().is_some()))
                {
                    player.set_global_input_data(
                        "animation.evaluate_pose",
                        gate.as_ref().is_none_or(|gate| gate.refresh_bones),
                    );
                    let original = player.get_global_input_data("animation.delta_seconds").cloned();
                    let explicit = original.as_ref().and_then(|value| value.as_f32().ok());
                    let delta = explicit.unwrap_or_else(|| time.delta_secs());
                    let elapsed = rate.as_mut().map_or(Some(delta), |rate| rate.step(delta, initialize));
                    if let Some(elapsed) = elapsed {
                        let override_delta = explicit.is_some() && elapsed != delta;
                        if override_delta {
                            player.set_global_input_data("animation.delta_seconds", elapsed);
                        }
                        let previous_pose = rate.as_ref().filter(|rate| rate.needs_previous_pose())
                            .and_then(|_| player.get_default_output_pose().cloned());
                        let started = rate.as_ref().and_then(|rate| rate.measure_cost.then(std::time::Instant::now));
                        let mut animation_time = Time::default();
                        animation_time.advance_by(std::time::Duration::from_secs_f32(elapsed.max(0.0)));
                        inputs.evaluate_with_skeleton_layouts(entity, skeleton_layouts, lod, clip_samples, acl_animations, || {
                            with_actor_scope(actor_scope, || {
                                run_animation_player(entity, player.reborrow(), &animation_time, &resources)
                            })
                        });
                        let work = started.map(|started| started.elapsed());
                        if override_delta && let Some(original) = original {
                            player.set_global_input_data("animation.delta_seconds", original);
                        }
                        if player.get_error().is_none() && let Some(rate) = rate.as_mut() {
                            rate.publish(previous_pose, work);
                        }
                    }
                }
                if update_graph
                    && initial.is_none()
                    && playing
                    && player.get_error().is_none()
                    && player.get_default_output_pose().is_some()
                    && rate.as_ref().is_none_or(|rate| rate.updated_this_frame)
                    && let Some(gate) = gate.as_mut()
                {
                    gate.pending_update = false;
                }
            }
            if let Some(lod) = lod
                && let Some(compact) = player.get_default_output_pose().and_then(|pose| lod.compact_pose(pose))
            {
                player.set_default_output_pose(compact);
            }
            debug_draw_animation_players(player, &resources);
            inputs.set_rig_query_context(None);
        };
    // A single batch has no work for another worker to take.
    if players
        .iter()
        .size_hint()
        .1
        .is_some_and(|count| count <= batch_size)
    {
        players.iter_mut().for_each(evaluate);
    } else {
        players
            .par_iter_mut()
            .batching_strategy(BatchingStrategy::new().min_batch_size(batch_size))
            .for_each(evaluate);
    }
    *previous_batch = started.map(|started| (batch_size, started.elapsed()));
    if let (Some(mut timing), Some(started)) = (timing, timing_started) {
        timing.0 += started.elapsed();
    }
}

fn apply_animation_to_visible_targets(
    mut commands: Commands,
    graphs: Option<Res<Assets<AnimationGraph>>>,
    skeletons: Option<Res<Assets<Skeleton>>>,
    mut targets: Query<(
        Entity,
        &mut Transform,
        Option<&mut bevy::mesh::morph::MorphWeights>,
        &AnimationTargetId,
        &AnimatedBy,
        Option<&mut PublishedSourceBone>,
    )>,
    players: Query<(
        &AnimationGraphPlayer,
        Option<&AnimationPoseUpdateGate>,
        Option<&crate::AlsAnimationLod>,
    )>,
    mut rates: Query<&mut crate::AlsAnimationRate>,
    followers: Query<&AlsAnimationPoseFollower, Without<AnimationGraphPlayer>>,
) {
    let mut source_poses = bevy::platform::collections::HashMap::new();
    let mut rig_outputs = bevy::platform::collections::HashMap::new();
    let mut rig_hierarchies = bevy::platform::collections::HashMap::new();
    for (entity, mut transform, morphs, id, animated_by, published) in &mut targets {
        let follower = followers.get(animated_by.0).ok();
        let source = follower.map_or(animated_by.0, |follower| follower.leader);
        let Ok((player, gate, lod)) = players.get(source) else {
            continue;
        };
        if gate.is_some_and(|gate| !gate.refresh_bones) {
            continue;
        }
        let Some(pose) = player.get_default_output_pose() else {
            continue;
        };
        if follower.is_some_and(|follower| follower.skeleton != pose.skeleton) {
            continue;
        }
        let id = BoneId::from(*id);
        if lod.and_then(|lod| lod.omitted_bones(pose.skeleton.id()))
            .is_some_and(|omitted| omitted.contains(&id))
        {
            if let Some(reference) = skeletons.as_ref().and_then(|assets| assets.get(&pose.skeleton))
                .and_then(|skeleton| skeleton.default_transforms(id))
                && *transform != reference.local
            {
                *transform = reference.local;
            }
            if published.is_some() {
                commands.entity(entity).remove::<PublishedSourceBone>();
            }
            continue;
        }
        let mut rate = rates.get_mut(source).ok();
        if rate.as_ref().is_some_and(|rate| !rate.pose_to_publish()) {
            continue;
        }
        let started = rate.as_ref().and_then(|rate| rate.measure_cost.then(std::time::Instant::now));
        let Some(target) = pose.get_bone(id) else {
            continue;
        };
        let interpolated = rate.as_ref().and_then(|rate| rate.interpolate_bone(pose, id, target));
        let bone = interpolated.as_ref().unwrap_or(target);
        let precise = gate.filter(|_| follower.is_none() && interpolated.is_none()).and_then(|_| {
            let indexed = source_poses.entry(source).or_insert_with(|| {
                let AnimationSource::Graph(graph) = player.get_animation_source() else { return None; };
                let graph = graphs.as_ref()?.get(graph)?;
                let root = crate::animation_context_projection::root_pose_source_node(graph)?;
                let context = crate::extensions::animation_context::player_animation_context_state(player, root)?;
                let pose = context.source_pose.as_ref()?;
                let bones: bevy::platform::collections::HashMap<_, _> = pose.bones.iter().copied().collect();
                Some((bones, pose.length_scale))
            }).as_ref();
            let exact = indexed.and_then(|(bones, length_scale)| {
                let value = bones.get(&id)?;
                let mut projection = bevy_animation_graph::core::pose::BonePose::default();
                crate::extensions::source_pose::project_bone_transform(&mut projection, *value, *length_scale);
                Some((RigTransform {
                    translation: bevy::math::DVec3::new(value.translation.x, value.translation.z, value.translation.y)
                        * (*length_scale * 100.0),
                    rotation: bevy::math::DQuat::from_xyzw(-value.rotation.x, -value.rotation.z, -value.rotation.y, value.rotation.w),
                    scale: bevy::math::DVec3::new(value.scale.x, value.scale.z, value.scale.y),
                }, projection))
            });
            if let Some((value, projection)) = exact
                && (projection.translation, projection.rotation, projection.scale) == (bone.translation, bone.rotation, bone.scale) {
                return Some(RigTransform { rotation: rig_normalize(value.rotation), ..value });
            }
            let (cached, definition, state) = rig_outputs.entry(animated_by.0).or_insert_with(|| {
                let (context, node, state) = crate::extensions::root_graph_boundaries::current_control_rig(player, graphs.as_ref()?)?;
                let pose = context.node_caches.get_output_data_ref(node.id,
                    bevy_animation_graph::core::context::node_states::StateKey::Default, "Pose")?.as_pose().ok()?;
                let definition = node.try_inner_downcast_ref::<crate::extensions::root_graph_boundaries::ControlRigBoundaryNode>()?
                    .definition.as_ref()?;
                Some((pose, definition, state))
            }).as_ref()?;
            let rig_bone = cached.get_bone(id)?;
            if (rig_bone.translation, rig_bone.rotation, rig_bone.scale) != (bone.translation, bone.rotation, bone.scale) {
                return None;
            }
            let skeleton = skeletons.as_ref()?.get(&pose.skeleton)?;
            let path = skeleton.id_to_path(id)?;
            let name = path.parts.last()?;
            let index = definition.bones.iter().position(|bone| bone.name.eq_ignore_ascii_case(name.as_str()))?;
            let hierarchy = rig_hierarchies.entry(animated_by.0).or_insert_with(|| state.snapshot_hierarchy());
            let local = hierarchy.as_mut()?.transform(index,
                crate::extensions::control_rig_hierarchy::RigSpace::Local, false).ok()?;
            Some(RigTransform { translation: local.translation * (state.meters_per_unit() * 100.0),
                rotation: rig_normalize(local.rotation), ..local })
        });
        if let Some(value) = bone.translation {
            transform.translation = value;
        }
        if let Some(value) = bone.rotation {
            transform.rotation = if gate.is_some() {
                let value = value.as_dquat();
                #[cfg(target_arch = "aarch64")]
                let value = crate::extensions::control_rig_hierarchy::rig_normalize(value);
                #[cfg(not(target_arch = "aarch64"))]
                let value = {
                    let source = crate::extensions::control_rig_hierarchy::rig_normalize(
                        bevy::math::DQuat::from_xyzw(-value.x, -value.z, -value.y, value.w),
                    );
                    bevy::math::DQuat::from_xyzw(-source.x, -source.z, -source.y, source.w)
                };
                value.as_quat()
            } else {
                value
            };
        }
        if let Some(value) = bone.scale {
            transform.scale = value;
        }
        let precise = precise
            .zip(bone.rotation)
            .map(|(local_cm, graph_rotation)| {
                PublishedSourceBone::new(local_cm, graph_rotation, transform.rotation)
            });
        match (published, precise) {
            (Some(mut old), Some(value)) => *old = value,
            (None, Some(value)) => {
                commands.entity(entity).insert(value);
            }
            (Some(_), None) => {
                commands.entity(entity).remove::<PublishedSourceBone>();
            }
            (None, None) => {}
        }
        if let Some(weights) = &bone.weights
            && let Some(mut morphs) = morphs
        {
            for (target, source) in morphs.weights_mut().iter_mut().zip(weights) {
                *target = *source;
            }
        }
        if let Some((rate, started)) = rate.as_deref_mut().zip(started) {
            rate.record_writeback(started);
        }
    }
}

fn extract_late_root_motion(
    with_player: Query<(&AnimationGraphPlayer, &mut RootMotionOutput)>,
    with_scene: Query<
        (&AnimatedSceneInstance, &mut RootMotionOutput),
        Without<AnimationGraphPlayer>,
    >,
    players: Query<(&AnimationGraphPlayer, &Transform), Without<RootMotionOutput>>,
) {
    extract_root_motion(with_player, with_scene, players);
}

fn publish_sampled_motion<F: QueryFilter>(
    mut outputs: Query<(
        Entity,
        &mut RootMotionOutput,
        Option<&AnimatedSceneInstance>,
        Has<AnimationGraphPlayer>,
    )>,
    players: Query<
        (
            &AnimationGraphPlayer,
            Option<&Transform>,
            Has<RootMotionOutput>,
            Option<&AnimationPoseUpdateGate>,
        ),
        F,
    >,
) {
    for (entity, mut output, scene, direct) in &mut outputs {
        let (source, local, gate) = if direct {
            let Ok((player, _, _, gate)) = players.get(entity) else {
                continue;
            };
            (player, Transform::IDENTITY, gate)
        } else {
            let Some((player, Some(transform), false, gate)) =
                scene.and_then(|s| players.get(s.player_entity()).ok())
            else {
                continue;
            };
            (player, *transform, gate)
        };
        if gate.is_none_or(|gate| gate.update_graph) && let Some(delta) = source.get_root_motion() {
            output.translation_delta = local.rotation * (delta.translation * local.scale);
            output.rotation_delta = delta.rotation;
        } else {
            output.translation_delta = Vec3::ZERO;
            output.rotation_delta = Quat::IDENTITY;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_animation_batch_is_bounded_by_observed_work() {
        let step = Duration::from_millis(16);
        let previous = (8, Duration::from_millis(4));
        assert_eq!(batch_size_within_step(20, previous, step), 20);
        assert_eq!(batch_size_within_step(80, previous, step), 32);
        assert_eq!(batch_size_within_step(80, previous, Duration::ZERO), 1);
    }

    #[test]
    fn kinematic_ragdoll_targets_use_the_source_quaternion_difference() {
        use bevy::ecs::system::RunSystemOnce;
        use bevy_animation_graph::core::ragdoll::definition::BodyMode;

        let mut world = World::new();
        world.insert_resource(Time::<()>::default());
        let base_rotation = Quat::from_rotation_z(0.4);
        let base = world
            .spawn((Position(Vec3::ZERO), Rotation(base_rotation)))
            .id();
        let start = Quat::from_rotation_x(0.3);
        let angle = 0.6_f32;
        let end = -(Quat::from_rotation_y(angle) * start);
        let [source, other] = [true, false].map(|source| {
            let mut entity = world.spawn((
                Position(Vec3::ZERO),
                Rotation(base_rotation * start),
                RelativeKinematicBodyPositionBased {
                    relative_to: Some(base),
                    relative_target: Isometry3d::new(Vec3::X * 0.01, end),
                },
            ));
            if source {
                entity.insert(crate::extensions::RagdollActiveMode(BodyMode::Dynamic));
            }
            entity.id()
        });
        for (delta, physics_delta) in [
            (1.0 / 60.0, 1.0 / 60.0),
            (1.0 / 30.0, 1.0 / 30.0),
            (0.1, 1.0 / 30.0),
        ] {
            world
                .resource_mut::<Time>()
                .advance_by(std::time::Duration::from_secs_f32(delta));
            world
                .run_system_once(update_relative_kinematic_target_velocities)
                .unwrap();
            let source = world.get::<RelativeKinematicBody>(source).unwrap();
            let other = world.get::<RelativeKinematicBody>(other).unwrap();
            assert_eq!(source.relative_to, Some(base));
            assert!(
                source
                    .kinematic_linear_velocity
                    .abs_diff_eq(Vec3::X * (0.01 / physics_delta), 1e-6)
            );
            assert!(
                source
                    .kinematic_angular_velocity
                    .abs_diff_eq(Vec3::Y * (2.0 * (angle * 0.5).sin() / physics_delta), 1e-5)
            );
            assert!(
                other
                    .kinematic_linear_velocity
                    .abs_diff_eq(Vec3::X * (0.01 / delta), 1e-6)
            );
            assert!(
                other
                    .kinematic_angular_velocity
                    .abs_diff_eq(Vec3::Y * (angle / delta), 1e-5)
            );
        }
        assert_eq!(
            source_kinematic_angular_velocity(start, end, 1e-8),
            Vec3::ZERO
        );
    }

    #[test]
    fn root_and_linked_sequences_consume_the_player_clock() {
        use bevy::animation::animated_field;
        use bevy::ecs::system::RunSystemOnce;
        bevy::tasks::ComputeTaskPool::get_or_init(bevy::tasks::TaskPool::new);
        use crate::extensions::{
            EventClock, EventLifecycle, EventPhase, LayeredGraphNode, PlaybackDirection,
            SourceSequencePlayerNode, StateIntervalBoundary, TimedEvent, TimedEventKind,
            animation_context::node_context_state,
        };
        use bevy_animation_graph::core::{
            animation_clip::{EntityPath, GraphClip},
            animation_graph::AnimationGraph,
            animation_node::AnimationNode,
            context::node_states::StateKey,
            edge_data::{DataSpec, events::EventQueue},
            state_machine::high_level::StateMachine,
        };
        for linked in [false, true] {
            for configured in [false, true] {
                for (gated, birth) in [(false, false), (false, true), (true, false), (true, true)] {
                    let mut world = World::new();
                    world.init_resource::<Assets<AnimationGraph>>();
                    world.init_resource::<Assets<GraphClip>>();
                    world.init_resource::<Assets<Skeleton>>();
                    world.init_resource::<Assets<StateMachine>>();
                    let mut time = Time::<()>::default();
                    time.advance_by(std::time::Duration::from_millis(100));
                    world.insert_resource(time);
                    let path = EntityPath::from_slashed_string("root".into());
                    let bone = path.id();
                    let mut skeleton = Skeleton::default();
                    skeleton.set_root(bone);
                    skeleton.add_bone(path, Transform::IDENTITY, Transform::IDENTITY);
                    let skeleton = world.resource_mut::<Assets<Skeleton>>().add(skeleton);
                    let mut animation = bevy::animation::AnimationClip::default();
                    animation.add_curve_to_target(
                        bone.animation_target_id(),
                        AnimatableCurve::new(
                            animated_field!(Transform::translation),
                            UnevenSampleAutoCurve::new([(0.0, Vec3::ZERO), (2.0, Vec3::X * 4.0)])
                                .unwrap(),
                        ),
                    );
                    let clip =
                        world
                            .resource_mut::<Assets<GraphClip>>()
                            .add(GraphClip::from_bevy_clip(
                                animation,
                                skeleton.clone(),
                                default(),
                                None,
                            ));
                    let sequence = AnimationNode::new(
                        "unsynchronized sequence",
                        SourceSequencePlayerNode {
                            clip,
                            play_rate: 0.5,
                            expose_play_rate_pin: false,
                            ..default()
                        },
                    );
                    let mut graph = AnimationGraph::new();
                    graph.add_output_data("pose".into(), DataSpec::Pose);
                    graph.add_output_data_edge(sequence.id, "Pose", "pose");
                    graph.add_node(sequence.clone());
                    let mut graph = world.resource_mut::<Assets<AnimationGraph>>().add(graph);
                    if linked {
                        let layer = AnimationNode::new(
                            "linked layer",
                            LayeredGraphNode {
                                capture_context: true,
                                internalize_time: true,
                                ..LayeredGraphNode::new(graph)
                            },
                        );
                        let mut root = AnimationGraph::new();
                        root.add_output_data("pose".into(), DataSpec::Pose);
                        root.add_output_data_edge(layer.id, "pose", "pose");
                        root.add_node(layer);
                        graph = world.resource_mut::<Assets<AnimationGraph>>().add(root);
                    }
                    let mut player = AnimationGraphPlayer::new(skeleton).with_graph(graph);
                    if configured {
                        player.set_global_input_data("animation.delta_seconds", 0.04_f32);
                    }
                    let entity = world
                        .spawn((
                            player,
                            crate::extensions::SourceAnimationInputs::default(),
                            Name::new("root"),
                            Transform::IDENTITY,
                            GlobalTransform::IDENTITY,
                        ))
                        .id();
                    world.spawn(ChildOf(entity));
                    if gated {
                        world.entity_mut(entity).insert(AnimationPoseUpdateGate {
                            update_graph: false,
                            refresh_bones: true,
                            pending_update: true,
                        });
                        if !birth {
                            world.run_system_once(evaluate_players::<()>).unwrap();
                            let player = world.get::<AnimationGraphPlayer>(entity).unwrap();
                            assert_eq!(player.get_context_arena().unwrap().frame_index(), 1);
                            assert!(
                                player
                                    .get_default_output_pose()
                                    .unwrap()
                                    .get_bone(bone)
                                    .unwrap()
                                    .translation
                                    .unwrap()
                                    .abs_diff_eq(Vec3::ZERO, 1e-6)
                            );
                            assert!(
                                world
                                    .get::<AnimationPoseUpdateGate>(entity)
                                    .unwrap()
                                    .pending_update
                            );
                        }
                        world
                            .get_mut::<AnimationPoseUpdateGate>(entity)
                            .unwrap()
                            .update_graph = true;
                    }
                    if birth {
                        world
                            .entity_mut(entity)
                            .insert(InitialAnimation { ready: false });
                        world.run_system_once(evaluate_players::<()>).unwrap();
                        assert!(
                            world
                                .get::<AnimationGraphPlayer>(entity)
                                .unwrap()
                                .get_default_output_pose()
                                .is_none()
                        );
                        world.get_mut::<InitialAnimation>(entity).unwrap().ready = true;
                        world.run_system_once(evaluate_players::<()>).unwrap();
                        let player = world.get::<AnimationGraphPlayer>(entity).unwrap();
                        assert!(player.get_error().is_none(), "{:?}", player.get_error());
                        assert_eq!(player.get_context_arena().unwrap().frame_index(), 1);
                        assert!(
                            player
                                .get_default_output_pose()
                                .unwrap()
                                .get_bone(bone)
                                .unwrap()
                                .translation
                                .unwrap()
                                .abs_diff_eq(Vec3::ZERO, 1e-6)
                        );
                        world.entity_mut(entity).remove::<InitialAnimation>();
                    }
                    world.run_system_once(evaluate_players::<()>).unwrap();
                    let mut position = if configured { 0.02 } else { 0.05 };
                    let player = world.get::<AnimationGraphPlayer>(entity).unwrap();
                    assert!(player.get_error().is_none(), "{:?}", player.get_error());
                    let arena = player.get_context_arena().unwrap();
                    let context = arena
                        .iter_context_ids()
                        .find_map(|id| {
                            node_context_state(
                                &arena.get_context(id).unwrap().node_states,
                                StateKey::Default,
                                sequence.id,
                            )
                        })
                        .unwrap();
                    let state = context.relevant_asset.as_ref().unwrap();
                    assert!(
                        (state.position - position).abs() < 1e-6,
                        "First sequence update: linked={linked} configured={configured} gated={gated} birth={birth} position={} expected={position}",
                        state.position
                    );
                    assert!(
                        player
                            .get_default_output_pose()
                            .unwrap()
                            .get_bone(bone)
                            .unwrap()
                            .translation
                            .unwrap()
                            .abs_diff_eq(Vec3::X * (2.0 * position), 1e-6)
                    );
                    let event = TimedEvent {
                        id: "clock crossing".into(),
                        kind: TimedEventKind::Point,
                        start_time: 0.06,
                        end_time: 0.06,
                        weight_threshold: 0.0,
                        track: None,
                        state_boundary: StateIntervalBoundary::StartExclusiveEndInclusive,
                        trigger_on_follower: false,
                        trigger_on_dedicated_server: false,
                    };
                    let mut lifecycle = EventLifecycle::default();
                    for multiplier in [0.0_f32, 0.5, 1.0, 2.0, 0.0, 1.0] {
                        if configured {
                            world
                                .get_mut::<AnimationGraphPlayer>(entity)
                                .unwrap()
                                .set_global_input_data("animation.delta_seconds", 0.1 * multiplier);
                        }
                        let previous = position;
                        let delta = if configured { 0.1 * multiplier } else { 0.1 };
                        position += delta * 0.5;
                        world.run_system_once(evaluate_players::<()>).unwrap();
                        let player = world.get::<AnimationGraphPlayer>(entity).unwrap();
                        assert!(player.get_error().is_none(), "{:?}", player.get_error());
                        let arena = player.get_context_arena().unwrap();
                        let context = arena
                            .iter_context_ids()
                            .find_map(|id| {
                                node_context_state(
                                    &arena.get_context(id).unwrap().node_states,
                                    StateKey::Default,
                                    sequence.id,
                                )
                            })
                            .unwrap();
                        let state = context.relevant_asset.as_ref().unwrap();
                        assert!(
                            (state.position - position).abs() < 1e-6
                                && (state.delta_seconds - delta).abs() < 1e-6,
                            "Root sequence owner clock: linked={linked} configured={configured} gated={gated} multiplier={multiplier} position={} expected={position} delta={} expected_delta={delta}",
                            state.position,
                            state.delta_seconds
                        );
                        let pose = player.get_default_output_pose().unwrap();
                        assert!(
                            pose.get_bone(bone)
                                .unwrap()
                                .translation
                                .unwrap()
                                .abs_diff_eq(Vec3::X * (2.0 * position), 1e-6)
                        );
                        let clock = state;
                        let signals = lifecycle.update(
                            std::slice::from_ref(&event),
                            &EventQueue::default(),
                            Some(EventClock {
                                previous_time: clock.previous_position,
                                current_time: clock.position,
                                duration: clock.duration,
                                direction: PlaybackDirection::Forward,
                                wrapped: false,
                            }),
                            1.0,
                        );
                        assert_eq!(
                            signals
                                .iter()
                                .filter(|signal| signal.phase == EventPhase::Point)
                                .count(),
                            usize::from(previous < 0.06 && position >= 0.06)
                        );
                    }
                    world
                        .get_mut::<AnimationGraphPlayer>(entity)
                        .unwrap()
                        .pause();
                    world.run_system_once(evaluate_players::<()>).unwrap();
                    assert!(
                        (world
                            .get::<AnimationGraphPlayer>(entity)
                            .unwrap()
                            .get_default_output_pose()
                            .unwrap()
                            .timestamp
                            - position)
                            .abs()
                            < 1e-6
                    );
                }
            }
        }
    }

    #[test]
    fn loaded_ragdoll_flail_consumes_each_owner_clock_delta() {
        use crate::extensions::{SourceSequencePlayerNode, animation_context::node_context_state};
        use bevy_animation_graph::core::{
            animation_graph::AnimationGraph, context::node_states::StateKey,
        };
        let (mut app, actor, avatar) = crate::roll_drive::tests::loaded_roll_character();
        app.world_mut()
            .entity_mut(actor)
            .insert(crate::als::state::AlsNotifyLocomotionAction::Ragdolling);
        for _ in 0..12 {
            app.update();
        }
        let player_entity = app
            .world()
            .get::<AnimatedSceneInstance>(avatar)
            .unwrap()
            .player_entity();
        let bodies = app
            .world()
            .get::<AnimationGraphPlayer>(player_entity)
            .unwrap()
            .get_spawned_ragdoll()
            .unwrap()
            .bodies
            .values()
            .copied()
            .collect::<Vec<_>>();
        for multiplier in [0.0_f32, 0.5, 1.0, 2.0, 0.0, 1.0] {
            for body in &bodies {
                app.world_mut().entity_mut(*body).insert((
                    avian3d::prelude::LinearVelocity(Vec3::X * 5.0),
                    avian3d::prelude::GravityScale(0.0),
                ));
            }
            app.world_mut()
                .entity_mut(actor)
                .insert(crate::als_camera::AlsCameraCustomTimeDilation(multiplier));
            app.update();
            let player = app
                .world()
                .get::<AnimationGraphPlayer>(player_entity)
                .unwrap();
            assert!(player.get_error().is_none(), "{:?}", player.get_error());
            let delta = player
                .get_global_input_data("animation.delta_seconds")
                .unwrap()
                .as_f32()
                .unwrap();
            assert!(
                (delta - app.world().resource::<Time>().delta_secs() * multiplier).abs() < 1e-7
            );
            let arena = player.get_context_arena().unwrap();
            let graphs = app.world().resource::<Assets<AnimationGraph>>();
            let mut checked = 0;
            for id in arena.iter_context_ids() {
                let context = arena.get_context(id).unwrap();
                let graph = graphs.get(context.get_graph_id()).unwrap();
                for node in graph.nodes.values() {
                    let Some(sequence) = node.try_inner_downcast_ref::<SourceSequencePlayerNode>()
                    else {
                        continue;
                    };
                    if sequence.synchronization.is_some()
                        || !context.node_caches.is_updated(node.id, StateKey::Default)
                    {
                        continue;
                    }
                    let Some(state) =
                        node_context_state(&context.node_states, StateKey::Default, node.id)
                    else {
                        continue;
                    };
                    let clock = state.relevant_asset.as_ref().unwrap();
                    assert!(
                        (clock.delta_seconds - delta).abs() < 1e-7,
                        "Loaded Flail owner clock: multiplier={multiplier} actual={} expected={delta}",
                        clock.delta_seconds
                    );
                    checked += 1;
                }
            }
            assert!(
                checked > 0,
                "Ragdoll must evaluate its unsynchronized Flail sequence"
            );
            assert!(player.get_default_output_pose().unwrap().bones.len() > 0);
        }
    }

    #[test]
    fn weighted_ragdoll_readback_keeps_shortest_rotation_and_zero_weight_input() {
        use bevy::platform::collections::HashMap;
        use bevy_animation_graph::core::{
            animation_clip::{EntityPath, GraphClip},
            animation_graph::AnimationGraph,
            pose::BonePose,
            ragdoll::{
                bone_mapping::{BodyWeight, BoneMapping},
                definition::Body,
                read_pose_avian::read_pose,
            },
            state_machine::high_level::StateMachine,
        };
        fn read(
            In((input, map, spawned, id, enabled, entities)): In<(
                Pose,
                RagdollBoneMap,
                SpawnedRagdoll,
                BoneId,
                bool,
                HashMap<BoneId, Entity>,
            )>,
            resources: SystemResources,
            bodies: Query<(&Position, &Rotation)>,
        ) -> Transform {
            let fallback = PoseFallbackContext {
                entity_map: &entities,
                resources: &resources,
                fallback_to_identity: true,
            };
            let config = RagdollConfig {
                default_readback: Some(enabled),
                ..default()
            };
            read_pose(
                &spawned,
                &map,
                resources.skeleton_assets.get(&input.skeleton).unwrap(),
                &bodies,
                fallback,
                &input,
                &config,
            )
            .get_bone(id)
            .unwrap()
            .to_transform()
        }
        for parent in [Quat::IDENTITY, Quat::from_rotation_z(0.4)] {
            for offset in [Quat::IDENTITY, Quat::from_rotation_x(-0.3)] {
                for angle in [0.0_f32, 0.7, 2.9] {
                    for signs in [[1.0, 1.0], [1.0, -1.0], [-1.0, 1.0], [-1.0, -1.0]] {
                        for weights in [
                            [0.5_f32, 0.5],
                            [0.2, 0.8],
                            [1.0, 0.0],
                            [0.0, 1.0],
                            [0.0, 0.0],
                        ] {
                            for enabled in [false, true] {
                                for mapping in
                                    ["normal", "reversed", "duplicated", "missing", "unspawned"]
                                {
                                    let mut world = World::new();
                                    world.init_resource::<Assets<AnimationGraph>>();
                                    world.init_resource::<Assets<GraphClip>>();
                                    world.init_resource::<Assets<Skeleton>>();
                                    world.init_resource::<Assets<StateMachine>>();
                                    let root = EntityPath::from_slashed_string("root".into());
                                    let path = root.child("child");
                                    let id = path.id();
                                    let parent_transform =
                                        Transform::from_xyz(1.0, 2.0, 3.0).with_rotation(parent);
                                    let base = Transform::from_xyz(0.3, 0.2, 0.1)
                                        .with_rotation(Quat::from_rotation_y(-0.2));
                                    let mut skeleton = Skeleton::default();
                                    skeleton.add_bone(
                                        root.clone(),
                                        parent_transform,
                                        parent_transform,
                                    );
                                    skeleton.set_root(root.id());
                                    skeleton.add_bone(path.clone(), base, parent_transform * base);
                                    let skeleton =
                                        world.resource_mut::<Assets<Skeleton>>().add(skeleton);
                                    let mut input = Pose {
                                        skeleton: skeleton.clone(),
                                        ..default()
                                    };
                                    input.add_bone(
                                        BonePose::from_transform(parent_transform),
                                        root.id(),
                                    );
                                    input.add_bone(BonePose::from_transform(base), id);
                                    let rotations =
                                        [Quat::from_rotation_y(0.3), Quat::from_rotation_y(angle)];
                                    let mut spawned = SpawnedRagdoll::new(Entity::PLACEHOLDER);
                                    let bodies = [0, 1].map(|index| {
                                        let body = Body::new().id;
                                        spawned.bodies.insert(
                                            body,
                                            world
                                                .spawn((
                                                    Position(Vec3::new(2.0, 3.0, 4.0)),
                                                    Rotation(rotations[index] * signs[index]),
                                                ))
                                                .id(),
                                        );
                                        BodyWeight {
                                            body,
                                            weight: weights[index],
                                            offset: Isometry3d::new(Vec3::ZERO, offset),
                                            ..default()
                                        }
                                    });
                                    let mut mappings = bodies.to_vec();
                                    match mapping {
                                        "reversed" => mappings.reverse(),
                                        "duplicated" => {
                                            mappings[0].weight *= 0.5;
                                            mappings.push(mappings[0].clone());
                                        }
                                        "missing" => {
                                            world.despawn(spawned.bodies[&bodies[1].body]);
                                        }
                                        "unspawned" => {
                                            spawned.bodies.remove(&bodies[1].body);
                                        }
                                        _ => {}
                                    }
                                    let map = RagdollBoneMap {
                                        skeleton,
                                        ragdoll: default(),
                                        skeleton_symmetry: default(),
                                        bones_from_bodies: [(
                                            path.clone(),
                                            BoneMapping {
                                                bone_id: path,
                                                bodies: mappings,
                                                ..default()
                                            },
                                        )]
                                        .into(),
                                        bodies_from_bones: default(),
                                    };
                                    let root_entity = world
                                        .spawn((
                                            parent_transform,
                                            GlobalTransform::from(parent_transform),
                                        ))
                                        .id();
                                    let entities = [(root.id(), root_entity)].into();
                                    let actual = world
                                        .run_system_cached_with(
                                            read,
                                            (input, map, spawned, id, enabled, entities),
                                        )
                                        .unwrap();
                                    let effective_weights =
                                        if matches!(mapping, "missing" | "unspawned") {
                                            [weights[0], 0.0]
                                        } else {
                                            weights
                                        };
                                    let total_weight = effective_weights.iter().sum::<f32>();
                                    let expected = if enabled && total_weight > 0.0 {
                                        let first = parent.conjugate() * rotations[0] * offset;
                                        let second = parent.conjugate() * rotations[1] * offset;
                                        first.lerp(second, effective_weights[1] / total_weight)
                                    } else {
                                        base.rotation
                                    };
                                    assert!(
                                        actual.rotation.is_finite()
                                            && (actual.rotation.length_squared() - 1.0).abs()
                                                < 1e-5
                                            && actual.rotation.dot(expected).abs() > 1.0 - 1e-5,
                                        "Wrong weighted readback: angle={angle}, signs={signs:?}, weights={weights:?}, enabled={enabled}, mapping={mapping}, actual={actual:?}"
                                    );
                                    if !enabled || total_weight == 0.0 {
                                        assert!(
                                            actual.translation.abs_diff_eq(base.translation, 1e-6)
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn weighted_ragdoll_readback_preserves_antipodal_body_rotations() {
        use bevy::platform::collections::HashMap;
        use bevy_animation_graph::core::{
            animation_clip::{EntityPath, GraphClip},
            animation_graph::AnimationGraph,
            pose::BonePose,
            ragdoll::{
                bone_mapping::{BodyWeight, BoneMapping},
                definition::Body,
                read_pose_avian::read_pose,
            },
            state_machine::high_level::StateMachine,
        };
        fn read(
            In((input, map, spawned, id)): In<(Pose, RagdollBoneMap, SpawnedRagdoll, BoneId)>,
            resources: SystemResources,
            bodies: Query<(&Position, &Rotation)>,
        ) -> Quat {
            let entities = HashMap::new();
            let fallback = PoseFallbackContext {
                entity_map: &entities,
                resources: &resources,
                fallback_to_identity: true,
            };
            read_pose(
                &spawned,
                &map,
                resources.skeleton_assets.get(&input.skeleton).unwrap(),
                &bodies,
                fallback,
                &input,
                &RagdollConfig::default(),
            )
            .get_bone(id)
            .unwrap()
            .rotation
            .unwrap()
        }
        let mut world = World::new();
        world.init_resource::<Assets<AnimationGraph>>();
        world.init_resource::<Assets<GraphClip>>();
        world.init_resource::<Assets<Skeleton>>();
        world.init_resource::<Assets<StateMachine>>();
        let path = EntityPath::from_slashed_string("root".into());
        let id = path.id();
        let mut skeleton = Skeleton::default();
        skeleton.add_bone(path.clone(), Transform::IDENTITY, Transform::IDENTITY);
        skeleton.set_root(id);
        let skeleton = world.resource_mut::<Assets<Skeleton>>().add(skeleton);
        let mut input = Pose {
            skeleton: skeleton.clone(),
            ..default()
        };
        input.add_bone(BonePose::from_transform(Transform::IDENTITY), id);
        let rotation = Quat::from_rotation_y(0.7);
        let mut map = RagdollBoneMap {
            skeleton,
            ragdoll: default(),
            skeleton_symmetry: default(),
            bones_from_bodies: default(),
            bodies_from_bones: default(),
        };
        let mut spawned = SpawnedRagdoll::new(Entity::PLACEHOLDER);
        let bodies = [rotation, -rotation].map(|rotation| {
            let body = Body::new().id;
            spawned.bodies.insert(
                body,
                world.spawn((Position(Vec3::ZERO), Rotation(rotation))).id(),
            );
            BodyWeight {
                body,
                weight: 0.5,
                ..default()
            }
        });
        map.bones_from_bodies.insert(
            path.clone(),
            BoneMapping {
                bone_id: path,
                bodies: bodies.into(),
                ..default()
            },
        );
        let actual = world
            .run_system_cached_with(read, (input, map, spawned, id))
            .unwrap();
        assert!(
            actual.is_finite()
                && (actual.length_squared() - 1.0).abs() < 1e-6
                && actual.dot(rotation).abs() > 1.0 - 1e-6,
            "Antipodal body weights destroyed rotation: {actual:?}"
        );
    }
    use bevy_animation_graph::core::{
        animation_clip::{EntityPath, GraphClip},
        animation_graph::AnimationGraph,
        animation_node::AnimationNode,
        edge_data::{DataSpec, DataValue},
        pose::{BonePose, Pose, RootMotionDelta},
        ragdoll::bone_mapping::BoneMapping,
        skeleton::Skeleton,
        state_machine::high_level::StateMachine,
    };

    #[test]
    fn character_pose_publication_normalizes_only_refreshed_rotation_channels() {
        use bevy::ecs::system::RunSystemOnce;
        let mut world = World::new();
        let id = AnimationTargetId::from_name(&Name::new("foot"));
        let missing_id = AnimationTargetId::from_name(&Name::new("missing rotation"));
        let input = Quat::from_xyzw(
            0.02744010090827942,
            0.0263987984508276,
            0.7066669464111328,
            0.7065211534500122,
        );
        let expected = Quat::from_xyzw(
            0.02744009904563427,
            0.02639879658818245,
            0.706666886806488,
            0.7065210938453674,
        );
        let mut pose = Pose::default();
        pose.add_bone(
            BonePose::from_transform(
                Transform::from_xyz(1.0, 2.0, 3.0)
                    .with_rotation(input)
                    .with_scale(Vec3::new(1.0, 2.0, 3.0)),
            ),
            BoneId::from(id),
        );
        pose.add_bone(
            BonePose {
                translation: Some(Vec3::Y),
                ..default()
            },
            BoneId::from(missing_id),
        );
        let mut character = AnimationGraphPlayer::default();
        character.set_animation(
            bevy_animation_graph::core::animation_graph_player::AnimationSource::Pose(pose.clone()),
        );
        let character = world
            .spawn((
                character,
                AnimationPoseUpdateGate {
                    update_graph: true,
                    refresh_bones: false,
                    pending_update: false,
                },
            ))
            .id();
        let mut other = AnimationGraphPlayer::default();
        other.set_animation(
            bevy_animation_graph::core::animation_graph_player::AnimationSource::Pose(pose),
        );
        let other = world.spawn(other).id();
        let bone = world
            .spawn((Transform::IDENTITY, id, AnimatedBy(character)))
            .id();
        let other_bone = world
            .spawn((Transform::IDENTITY, id, AnimatedBy(other)))
            .id();
        let unchanged = Quat::from_rotation_x(0.3);
        let missing = world
            .spawn((
                Transform::from_rotation(unchanged),
                missing_id,
                AnimatedBy(character),
            ))
            .id();
        world
            .run_system_once(apply_animation_to_visible_targets)
            .unwrap();
        assert_eq!(*world.get::<Transform>(bone).unwrap(), Transform::IDENTITY);
        assert_eq!(world.get::<Transform>(other_bone).unwrap().rotation, input);
        world
            .get_mut::<AnimationPoseUpdateGate>(character)
            .unwrap()
            .refresh_bones = true;
        world
            .run_system_once(apply_animation_to_visible_targets)
            .unwrap();
        let published = world.get::<Transform>(bone).unwrap();
        assert_eq!(published.rotation, expected);
        assert_eq!(published.translation, Vec3::new(1.0, 2.0, 3.0));
        assert_eq!(published.scale, Vec3::new(1.0, 2.0, 3.0));
        assert_eq!(world.get::<Transform>(missing).unwrap().rotation, unchanged);
        assert_eq!(world.get::<Transform>(other_bone).unwrap().rotation, input);
        assert_eq!(
            world
                .get::<AnimationGraphPlayer>(character)
                .unwrap()
                .get_default_output_pose()
                .unwrap()
                .get_bone(BoneId::from(id))
                .unwrap()
                .rotation,
            Some(input)
        );
    }

    #[test]
    fn bone_visibility_gate_keeps_the_last_transforms_until_refresh_is_allowed() {
        use bevy::ecs::system::RunSystemOnce;
        let mut world = World::new();
        let id = AnimationTargetId::from_name(&Name::new("bone"));
        let expected = Transform::from_xyz(1.0, 2.0, 3.0)
            .with_rotation(Quat::from_rotation_y(0.4))
            .with_scale(Vec3::splat(2.0));
        let mut pose = Pose::default();
        pose.add_bone(BonePose::from_transform(expected), BoneId::from(id));
        let mut player = AnimationGraphPlayer::default();
        player.set_animation(
            bevy_animation_graph::core::animation_graph_player::AnimationSource::Pose(pose),
        );
        let player = world
            .spawn((
                player,
                AnimationPoseUpdateGate {
                    update_graph: true,
                    refresh_bones: false,
                    pending_update: false,
                },
            ))
            .id();
        let bone = world
            .spawn((Transform::IDENTITY, id, AnimatedBy(player)))
            .id();
        world
            .run_system_once(apply_animation_to_visible_targets)
            .unwrap();
        assert_eq!(*world.get::<Transform>(bone).unwrap(), Transform::IDENTITY);
        world
            .get_mut::<AnimationPoseUpdateGate>(player)
            .unwrap()
            .refresh_bones = true;
        world
            .run_system_once(apply_animation_to_visible_targets)
            .unwrap();
        let actual = world.get::<Transform>(bone).unwrap();
        assert_eq!(actual.translation, expected.translation);
        assert_eq!(actual.scale, expected.scale);
        assert!(actual.rotation.dot(expected.rotation).abs() > 0.999999);
    }

    #[test]
    fn scaled_readback_preserves_physics_world_transforms_across_animated_gaps() {
        use bevy::ecs::system::RunSystemOnce;
        use bevy::platform::collections::HashMap;
        use bevy_animation_graph::core::{
            context::pose_fallback::PoseFallbackContext,
            ragdoll::{bone_mapping::BodyWeight, definition::Body, read_pose_avian::read_pose},
            space_conversion::SpaceConversionContext,
        };

        fn readback(
            In((input, map, spawned, config, entities, ids, scale)): In<(
                Pose,
                RagdollBoneMap,
                SpawnedRagdoll,
                RagdollConfig,
                HashMap<BoneId, Entity>,
                [BoneId; 3],
                f32,
            )>,
            resources: SystemResources,
            bodies: Query<(&Position, &Rotation)>,
        ) -> (Pose, [Transform; 3], [Transform; 3]) {
            let skeleton = resources.skeleton_assets.get(&input.skeleton).unwrap();
            let fallback = PoseFallbackContext {
                entity_map: &entities,
                resources: &resources,
                fallback_to_identity: true,
            };
            let conversions = SpaceConversionContext {
                pose_fallback: fallback,
            };
            let mut pose = read_pose(&spawned, &map, skeleton, &bodies, fallback, &input, &config);
            let before = ids.map(|id| conversions.global_transform_of_bone(&pose, skeleton, id));
            let readback = successful_readback_bones(&map, &spawned, &config, &bodies);
            normalize_ragdoll_pose_scale(
                &mut pose,
                &map,
                skeleton,
                conversions,
                Some(scale),
                &readback,
            );
            let after = ids.map(|id| conversions.global_transform_of_bone(&pose, skeleton, id));
            (pose, before, after)
        }

        let scene = EntityPath::from_slashed_string("GltfNode0".into());
        let root = scene.child("root");
        let middle = root.child("middle");
        let child = middle.child("child");
        let paths = [root, middle, child];
        let ids = paths.each_ref().map(|path| path.id());
        for complex in [false, true] {
            for runtime_basis in [false, true] {
                for scale in [2.0, 0.5, 1.0] {
                    for root_readback in [true, false] {
                        for child_readback in [true, false] {
                            for gap in
                                ["unmapped", "masked", "missing", "unspawned", "zero", "read"]
                            {
                                let mut world = World::new();
                                world.init_resource::<Assets<AnimationGraph>>();
                                world.init_resource::<Assets<GraphClip>>();
                                world.init_resource::<Assets<StateMachine>>();
                                world.init_resource::<Assets<Skeleton>>();
                                let scene_transform = Transform {
                                    translation: if runtime_basis {
                                        Vec3::new(-15.0, -0.04, -15.0)
                                    } else if complex {
                                        Vec3::new(3.0, 2.0, -1.0)
                                    } else {
                                        Vec3::ZERO
                                    },
                                    rotation: if runtime_basis {
                                        Quat::from_rotation_y(-std::f32::consts::FRAC_PI_4)
                                    } else if complex {
                                        Quat::from_euler(EulerRot::YXZ, 0.4, -0.2, 0.1)
                                    } else {
                                        Quat::IDENTITY
                                    },
                                    scale: Vec3::splat(scale),
                                };
                                let locals = [
                                    if complex {
                                        Transform::from_xyz(0.0, 0.3, 0.0)
                                            .with_rotation(Quat::from_rotation_y(0.15))
                                            .with_scale(Vec3::splat(1.25))
                                    } else {
                                        Transform::IDENTITY
                                    },
                                    Transform::from_xyz(1.0, if complex { 0.2 } else { 0.0 }, 0.0)
                                        .with_rotation(if complex {
                                            Quat::from_rotation_z(0.25)
                                        } else {
                                            Quat::IDENTITY
                                        })
                                        .with_scale(Vec3::splat(if complex { 1.1 } else { 1.0 })),
                                    Transform::from_translation(Vec3::X),
                                ];
                                let scene_entity = world
                                    .spawn((
                                        scene_transform,
                                        GlobalTransform::from(scene_transform),
                                    ))
                                    .id();
                                let mut entities = HashMap::from([(scene.id(), scene_entity)]);
                                let mut skeleton = Skeleton::default();
                                skeleton.add_bone(
                                    scene.clone(),
                                    Transform::IDENTITY,
                                    Transform::IDENTITY,
                                );
                                skeleton.set_root(scene.id());
                                let mut character = Transform::IDENTITY;
                                for index in 0..3 {
                                    character = character * locals[index];
                                    skeleton.add_bone(
                                        paths[index].clone(),
                                        locals[index],
                                        character,
                                    );
                                    let entity = world
                                        .spawn((
                                            locals[index],
                                            GlobalTransform::from(scene_transform * character),
                                        ))
                                        .id();
                                    entities.insert(ids[index], entity);
                                }
                                let skeleton =
                                    world.resource_mut::<Assets<Skeleton>>().add(skeleton);
                                let mut input = Pose {
                                    skeleton: skeleton.clone(),
                                    ..default()
                                };
                                for index in 0..3 {
                                    input.add_bone(
                                        BonePose::from_transform(locals[index]),
                                        ids[index],
                                    );
                                }
                                let mut map = RagdollBoneMap {
                                    skeleton,
                                    ragdoll: default(),
                                    skeleton_symmetry: default(),
                                    bones_from_bodies: default(),
                                    bodies_from_bones: default(),
                                };
                                let mut spawned = SpawnedRagdoll::new(scene_entity);
                                let positions = [Vec3::ZERO, Vec3::X * 0.5, Vec3::X * 2.0]
                                    .map(|point| scene_transform.transform_point(point));
                                let rotations = [0.0, 0.1, -0.15].map(|angle| {
                                    scene_transform.rotation
                                        * if complex {
                                            Quat::from_rotation_x(angle)
                                        } else {
                                            Quat::IDENTITY
                                        }
                                });
                                for index in 0..3 {
                                    let body = Body::new().id;
                                    let entity = world
                                        .spawn((
                                            Position(positions[index]),
                                            Rotation(rotations[index]),
                                        ))
                                        .id();
                                    if index != 1 || gap != "unspawned" {
                                        spawned.bodies.insert(body, entity);
                                    }
                                    if index != 1 || gap != "unmapped" {
                                        map.bones_from_bodies.insert(
                                            paths[index].clone(),
                                            BoneMapping {
                                                bone_id: paths[index].clone(),
                                                bodies: vec![BodyWeight {
                                                    body,
                                                    weight: if index == 1 && gap == "zero" {
                                                        0.0
                                                    } else {
                                                        1.0
                                                    },
                                                    ..default()
                                                }],
                                                ..default()
                                            },
                                        );
                                    }
                                    if index == 1 && gap == "missing" {
                                        world.despawn(entity);
                                    }
                                }
                                let config = RagdollConfig {
                                    default_readback: Some(false),
                                    readback_overrides: [
                                        (ids[0], root_readback),
                                        (ids[1], gap != "masked"),
                                        (ids[2], child_readback),
                                    ]
                                    .into(),
                                    ..default()
                                };
                                let (pose, before, after) = world
                                    .run_system_once_with(
                                        readback,
                                        (input.clone(), map, spawned, config, entities, ids, scale),
                                    )
                                    .unwrap();
                                for (index, enabled) in
                                    [root_readback, gap == "read", child_readback]
                                        .into_iter()
                                        .enumerate()
                                {
                                    if enabled {
                                        assert!(
                                            before[index].translation.distance(positions[index])
                                                < 1e-4,
                                            "Raw physics readback must align"
                                        );
                                        assert!(
                                            after[index].translation.distance(positions[index])
                                                < 1e-4,
                                            "complex={complex} scale={scale} root={root_readback} child={child_readback} gap={gap} bone={index}: {} != {}",
                                            after[index].translation,
                                            positions[index]
                                        );
                                        assert!(
                                            after[index]
                                                .rotation
                                                .abs_diff_eq(rotations[index], 1e-5)
                                                || after[index]
                                                    .rotation
                                                    .abs_diff_eq(-rotations[index], 1e-5)
                                        );
                                        assert!(
                                            after[index].scale.distance(Vec3::splat(scale)) < 1e-4
                                        );
                                    } else {
                                        let actual = &pose.bones[pose.paths[&ids[index]]];
                                        let expected = &input.bones[input.paths[&ids[index]]];
                                        assert_eq!(actual.translation, expected.translation);
                                        assert_eq!(actual.rotation, expected.rotation);
                                        assert_eq!(actual.scale, expected.scale);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn nonuniform_readback_keeps_animated_scales_and_world_bone_positions() {
        use bevy::ecs::system::RunSystemOnce;
        use bevy::platform::collections::HashMap;
        use bevy_animation_graph::core::animation_clip::EntityPath;
        use bevy_animation_graph::core::ragdoll::bone_mapping::BoneMapping;

        let mut world = World::new();
        world.init_resource::<Assets<AnimationGraph>>();
        world.init_resource::<Assets<GraphClip>>();
        world.init_resource::<Assets<StateMachine>>();
        world.init_resource::<Assets<Skeleton>>();
        let scene_path = EntityPath::from_slashed_string("scene".into());
        let root = scene_path.child("root");
        let gap = root.child("gap");
        let child = gap.child("child");
        let paths = [root, gap, child];
        let ids = paths.each_ref().map(|path| path.id());
        let scene = Transform::from_xyz(2.0, 3.0, 4.0)
            .with_rotation(Quat::from_rotation_y(0.4))
            .with_scale(Vec3::new(1.4, 0.8, 0.6));
        let locals = [
            Transform::from_xyz(0.0, 0.3, 0.0)
                .with_rotation(Quat::from_rotation_x(0.4))
                .with_scale(Vec3::new(1.1, 0.9, 1.2)),
            Transform::from_xyz(0.7, 0.1, 0.2).with_rotation(Quat::from_rotation_z(0.25)),
            Transform::from_xyz(0.5, 0.0, -0.3).with_scale(Vec3::new(0.8, 1.3, 1.0)),
        ];
        let scene_entity = world.spawn((scene, GlobalTransform::from(scene))).id();
        let mut entities = HashMap::from([(scene_path.id(), scene_entity)]);
        let mut skeleton = Skeleton::default();
        skeleton.add_bone(scene_path.clone(), Transform::IDENTITY, Transform::IDENTITY);
        skeleton.set_root(scene_path.id());
        let mut character = Transform::IDENTITY;
        for (path, local) in paths.iter().zip(locals) {
            character = character * local;
            skeleton.add_bone(path.clone(), local, character);
            let entity = world
                .spawn((local, GlobalTransform::from(scene * character)))
                .id();
            entities.insert(path.id(), entity);
        }
        let skeleton = world.resource_mut::<Assets<Skeleton>>().add(skeleton);
        let mut raw_pose = Pose {
            skeleton: skeleton.clone(),
            ..default()
        };
        let raw_locals = [
            locals[0].with_scale(scene.scale.recip()),
            locals[1],
            locals[2].with_scale(Vec3::ONE),
        ];
        for (id, local) in ids.into_iter().zip(raw_locals) {
            raw_pose.add_bone(BonePose::from_transform(local), id);
        }
        let map = RagdollBoneMap {
            skeleton,
            ragdoll: default(),
            skeleton_symmetry: default(),
            bodies_from_bones: default(),
            bones_from_bodies: [0, 2]
                .map(|index| {
                    let path = paths[index].clone();
                    (
                        path.clone(),
                        BoneMapping {
                            bone_id: path,
                            ..default()
                        },
                    )
                })
                .into(),
        };
        let (pose, before, after) = world
            .run_system_once_with(
                move |In(mut pose): In<Pose>, resources: SystemResources| {
                    let skeleton = resources.skeleton_assets.get(&pose.skeleton).unwrap();
                    let conversions = SpaceConversionContext {
                        pose_fallback: PoseFallbackContext {
                            entity_map: &entities,
                            resources: &resources,
                            fallback_to_identity: true,
                        },
                    };
                    let before =
                        ids.map(|id| conversions.global_transform_of_bone(&pose, skeleton, id));
                    let readback = HashSet::from([ids[0], ids[2]]);
                    normalize_ragdoll_pose_scale(
                        &mut pose,
                        &map,
                        skeleton,
                        conversions,
                        None,
                        &readback,
                    );
                    let after =
                        ids.map(|id| conversions.global_transform_of_bone(&pose, skeleton, id));
                    (pose, before, after)
                },
                raw_pose,
            )
            .unwrap();

        let corrected = ids.map(|id| pose.get_bone(id).unwrap().to_transform());
        assert_eq!(
            corrected.map(|transform| transform.scale),
            locals.map(|transform| transform.scale)
        );
        assert_eq!(corrected[1], locals[1]);
        let root_world = scene.compute_affine() * corrected[0].compute_affine();
        let child_world =
            root_world * corrected[1].compute_affine() * corrected[2].compute_affine();
        assert!(
            root_world
                .transform_point3(Vec3::ZERO)
                .distance(before[0].translation)
                < 1e-5
        );
        assert!(
            child_world
                .transform_point3(Vec3::ZERO)
                .distance(before[2].translation)
                < 1e-5
        );
        for index in [0, 2] {
            assert!(before[index].rotation.dot(after[index].rotation).abs() > 1.0 - 1e-6);
        }
    }

    #[test]
    fn marked_players_evaluate_once_after_physics_while_samplers_stay_before_it() {
        use bevy::ecs::schedule::ScheduleLabel;
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<Assets<GraphClip>>().init_resource::<Assets<AnimationGraph>>()
            .init_resource::<Assets<Skeleton>>().init_resource::<Assets<StateMachine>>()
            .init_resource::<Assets<bevy_animation_graph::core::ragdoll::bone_mapping::RagdollBoneMap>>()
            .add_plugins(crate::extensions::SourceAnimationInputsPlugin)
            .configure_sets(PostUpdate, (
                AnimationPlaybackSystems::Sample.in_set(AnimationGraphSet::PrePhysics),
                AnimationPlaybackSystems::WritePose.in_set(AnimationGraphSet::Final),
                AnimationGraphSet::PrePhysics.before(PhysicsSystems::First),
                PhysicsSystems::Last.after(PhysicsSystems::First),
                AnimationGraphSet::PostPhysics.after(PhysicsSystems::Last),
            ));
        install_physics_playback(&mut app, PostUpdate.intern());
        install_pose_playback(&mut app, PostUpdate.intern());
        let mut graph = AnimationGraph::new();
        let node = AnimationNode::new(
            "input",
            crate::extensions::GlobalPoseInputNode {
                key: "input".into(),
                output: "Pose".into(),
            },
        );
        graph.add_node(node.clone());
        graph.add_output_data("pose".into(), DataSpec::Pose);
        graph.add_output_data_edge(node.id, "Pose", "pose");
        let handle = app
            .world_mut()
            .resource_mut::<Assets<AnimationGraph>>()
            .add(graph);
        let pose = |value| {
            let mut pose = Pose {
                timestamp: value,
                root_motion: Some(RootMotionDelta {
                    translation: Vec3::X * value,
                    rotation: Quat::IDENTITY,
                }),
                ..default()
            };
            pose.add_bone(
                BonePose::from_transform(Transform::from_xyz(value, 0.0, 0.0)),
                default(),
            );
            pose
        };
        let mut player = AnimationGraphPlayer::new(default()).with_graph(handle.clone());
        player.set_global_input_data("input", DataValue::Pose(pose(0.0)));
        let sampler = app
            .world_mut()
            .spawn((
                Name::new("sampler"),
                Transform::IDENTITY,
                GlobalTransform::IDENTITY,
                player,
                RootMotionOutput::default(),
            ))
            .id();
        let mut player = AnimationGraphPlayer::new(default()).with_graph(handle);
        player.set_global_input_data("input", DataValue::Pose(pose(0.0)));
        let character = app
            .world_mut()
            .spawn((
                Name::new("character"),
                Transform::IDENTITY,
                GlobalTransform::IDENTITY,
                player,
                RootMotionOutput::default(),
                PostPhysicsAnimation,
            ))
            .id();
        for parent in [sampler, character] {
            app.world_mut().spawn((
                Name::new("bone"),
                Transform::IDENTITY,
                GlobalTransform::IDENTITY,
                ChildOf(parent),
            ));
        }
        let mut frame = 0;
        app.add_systems(
            PostUpdate,
            (move |mut players: Query<&mut AnimationGraphPlayer>,
                   outputs: Query<&RootMotionOutput>| {
                assert_eq!(
                    outputs.get(sampler).unwrap().translation_delta.x,
                    frame as f32
                );
                frame += 1;
                for mut player in &mut players {
                    player.set_global_input_data("input", DataValue::Pose(pose(frame as f32)));
                }
            })
            .in_set(PhysicsSystems::First),
        );
        app.finish();
        app.cleanup();
        for frame in 1..=2 {
            for entity in [sampler, character] {
                app.world_mut()
                    .get_mut::<crate::extensions::SourceAnimationInputs>(entity)
                    .unwrap()
                    .set_rig_query_context(Some(crate::control_rig_query::RigQueryContext {
                        world: std::sync::Arc::new(
                            crate::control_rig_query::RigQueryWorld::default(),
                        ),
                        to_world: default(),
                        channel: "Visibility".into(),
                        filter: default(),
                    }));
            }
            app.world_mut().run_schedule(PostUpdate);
            for (entity, expected) in [(sampler, (frame - 1) as f32), (character, frame as f32)] {
                assert!(
                    app.world()
                        .get::<crate::extensions::SourceAnimationInputs>(entity)
                        .unwrap()
                        .rig_query_context()
                        .is_none()
                );
                let player = app.world().get::<AnimationGraphPlayer>(entity).unwrap();
                assert_eq!(
                    player.get_default_output_pose().unwrap().timestamp,
                    expected
                );
                assert_eq!(player.get_context_arena().unwrap().frame_index(), frame);
                assert_eq!(
                    app.world()
                        .get::<RootMotionOutput>(entity)
                        .unwrap()
                        .translation_delta
                        .x,
                    expected
                );
            }
        }
        app.world_mut()
            .entity_mut(character)
            .remove::<PostPhysicsAnimation>();
        app.world_mut().run_schedule(PostUpdate);
        let player = app.world().get::<AnimationGraphPlayer>(character).unwrap();
        assert_eq!(player.get_default_output_pose().unwrap().timestamp, 2.0);
        assert_ne!(player.get_default_output_pose().unwrap().timestamp, 3.0);
        assert_eq!(player.get_context_arena().unwrap().frame_index(), 3);
    }
}
    #[test]
    fn compatible_followers_publish_one_pose_with_independent_visual_roots() {
        use bevy::ecs::system::RunSystemOnce;
        use bevy_animation_graph::core::{animation_clip::EntityPath, pose::BonePose};

        let mut app = App::new();
        let world = app.world_mut();
        let mut skeletons = Assets::<Skeleton>::default();
        let skeleton = skeletons.add(Skeleton::default());
        let different = skeletons.add(Skeleton::default());
        let bone = EntityPath::default().child("bone").id();
        let pose_at = |translation| {
            let mut pose = Pose { skeleton: skeleton.clone(), ..default() };
            pose.add_bone(BonePose::from_transform(Transform::from_translation(translation)), bone);
            pose
        };
        let mut leader = AnimationGraphPlayer::default();
        leader.set_animation(AnimationSource::Pose(pose_at(Vec3::Y)));
        let leader = world.spawn(leader).id();
        let mut followers = Vec::new();
        for (skeleton, offset) in [(skeleton.clone(), 10.0), (skeleton.clone(), 20.0), (different, 30.0)] {
            let root = world.spawn((
                Transform::from_xyz(offset, 0.0, 0.0),
                AlsAnimationPoseFollower { leader, skeleton },
            )).id();
            let target = world.spawn((
                Transform::IDENTITY,
                ChildOf(root),
                AnimatedBy(root),
                bone.animation_target_id(),
            )).id();
            followers.push((root, target, offset));
        }
        world.run_system_once(apply_animation_to_visible_targets).unwrap();
        for (index, &(root, target, offset)) in followers.iter().enumerate() {
            assert_eq!(world.get::<Transform>(root).unwrap().translation.x, offset);
            assert_eq!(world.get::<Transform>(target).unwrap().translation,
                if index < 2 { Vec3::Y } else { Vec3::ZERO });
        }

        let mut independent = AnimationGraphPlayer::default();
        independent.set_animation(AnimationSource::Pose(pose_at(Vec3::Z)));
        world.entity_mut(followers[0].0).insert(independent);
        world.get_mut::<AnimationGraphPlayer>(leader).unwrap()
            .set_animation(AnimationSource::Pose(pose_at(Vec3::X)));
        world.run_system_once(apply_animation_to_visible_targets).unwrap();
        assert_eq!(world.get::<Transform>(followers[0].1).unwrap().translation, Vec3::Z);
        assert_eq!(world.get::<Transform>(followers[1].1).unwrap().translation, Vec3::X);
    }
