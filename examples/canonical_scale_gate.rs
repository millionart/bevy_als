use std::time::{Duration, Instant};

use als_core::control_scheme::AlsControlSchemeConfig;
use als_core::params::AlsMovementParams;
use als_ext::InterruptibleFsmState;
use avian3d::prelude::{Collider, RigidBody};
use bevy::animation::AnimationTargetId;
use bevy::ecs::message::MessageWriter;
use bevy::mesh::skinning::SkinnedMesh;
use bevy::prelude::*;
use bevy::window::ExitCondition;
use bevy_animation_graph::core::animated_scene::AnimatedSceneInstance;
use bevy_animation_graph::core::animation_graph::AnimationGraph;
use bevy_animation_graph::core::animation_graph_player::AnimationGraphPlayer;
use bevy_animation_graph::core::context::node_states::StateKey;
use bevy_animation_graph::core::edge_data::DataValue;
use bevy_animation_graph::core::plugin::AnimationGraphSet;
use bevy_animation_graph::core::pose::{BoneId, Pose};
use bevy_animation_graph::core::ragdoll::definition::BodyMode;

use bevy_als::animation_context_projection::AlsRootAnimationContext;
use bevy_als::control_rig_apply::AlsControlRigPropagationSet;
use bevy_als::head_layer::AlsHeadState;
use bevy_als::layering_layer::AlsLayeringState;
use bevy_als::playable_character::{
    AlsCharacterAvatar, AlsCharacterBlueprint, AlsPlayableCharacterPlugin, spawn_als_character,
};

const MIN_ALLOWED_SCALE: f32 = 0.95;
const MAX_ALLOWED_SCALE: f32 = 1.05;
const MIN_SKIN_HEAD_HEIGHT: f32 = 1.4;
const MAX_SKIN_HEAD_HEIGHT: f32 = 1.9;
const MIN_SKIN_FOOT_SEPARATION: f32 = 0.2;
const MAX_SKIN_FOOT_SEPARATION: f32 = 1.0;

#[derive(Resource)]
struct Deadline(Instant);

#[derive(Resource)]
struct RootGraph {
    root: Handle<AnimationGraph>,
    grounded: Handle<AnimationGraph>,
    standing: Handle<AnimationGraph>,
}

#[derive(Resource, Default)]
struct Probe {
    character: Option<Entity>,
    complete: bool,
    attempts: u32,
    pre_physics_pelvis: Option<Vec3>,
    physics_observed: bool,
    physics_changed_pose: bool,
    physics_config_valid: bool,
    physics_ticks: u32,
    bevy_pose_samples: u8,
}

fn main() {
    let mut app = App::new();
    app.add_plugins((
        DefaultPlugins
            .set(AssetPlugin {
                file_path: "assets".into(),
                ..default()
            })
            .set(WindowPlugin {
                primary_window: None,
                exit_condition: ExitCondition::DontExit,
                ..default()
            }),
        AlsPlayableCharacterPlugin,
    ))
    .init_resource::<Probe>()
    .insert_resource(Deadline(Instant::now() + Duration::from_secs(12)))
    .add_systems(Startup, setup)
    .add_systems(Update, timeout)
    .add_systems(
        FixedPostUpdate,
        (
            capture_pre_physics.after(AnimationGraphSet::PrePhysics),
            capture_post_physics.after(AnimationGraphSet::PostPhysics),
        ),
    )
    .add_systems(PostUpdate, observe.after(AlsControlRigPropagationSet));
    app.run();
}

fn setup(
    mut commands: Commands,
    assets: Res<AssetServer>,
    mut params_assets: ResMut<Assets<AlsMovementParams>>,
    mut config_assets: ResMut<Assets<AlsControlSchemeConfig>>,
    mut probe: ResMut<Probe>,
) {
    commands.spawn((
        RigidBody::Static,
        Collider::cuboid(20.0, 0.1, 20.0),
        Transform::from_xyz(0.0, -0.1, 0.0),
    ));
    let character = spawn_als_character(
        &mut commands,
        &assets,
        &mut params_assets,
        &mut config_assets,
        AlsCharacterBlueprint::new(Transform::from_xyz(0.0, 0.95, 0.0)),
    );
    probe.character = Some(character);
    commands.insert_resource(RootGraph {
        root: assets.load("graphs/ab_als/ab_als.animgraph.ron"),
        grounded: assets.load("graphs/ab_als/grounded.animgraph.ron"),
        standing: assets.load("graphs/ab_als/stances/standing.animgraph.ron"),
    });
    info!("[CANONICAL-SCALE] canonical AB_Als character spawned");
}

fn skin_joint_target(
    name: &str,
    skins: &Query<&SkinnedMesh>,
    joint_targets: &Query<(&Name, &AnimationTargetId)>,
) -> Option<AnimationTargetId> {
    let skin = skins.iter().max_by_key(|skin| skin.joints.len())?;
    skin.joints.iter().find_map(|entity| {
        joint_targets
            .get(*entity)
            .ok()
            .filter(|(joint_name, _)| joint_name.as_str() == name)
            .map(|(_, target)| *target)
    })
}

fn player_pelvis_state(
    avatars: &Query<(&ChildOf, &AnimatedSceneInstance), With<AlsCharacterAvatar>>,
    players: &Query<&AnimationGraphPlayer>,
    skins: &Query<&SkinnedMesh>,
    joint_targets: &Query<(&Name, &AnimationTargetId)>,
) -> Option<(Vec3, bool)> {
    let (_, instance) = avatars.iter().next()?;
    let player = players.get(instance.player_entity()).ok()?;
    let target = skin_joint_target("pelvis", skins, joint_targets)?;
    let pelvis = player
        .get_default_output_pose()?
        .get_bone(BoneId::from(target))?
        .translation?;
    let config_valid = matches!(
        player.get_outputs().get("ragdoll_config"),
        Some(DataValue::RagdollConfig(config))
            if config.default_mode == Some(BodyMode::Kinematic)
                && config.default_readback == Some(false)
    );
    Some((pelvis, config_valid))
}

fn global_f32(player: &AnimationGraphPlayer, key: &str) -> Option<f32> {
    match player.get_global_input_data(key) {
        Some(DataValue::F32(value)) => Some(*value),
        _ => None,
    }
}

fn cached_pose(
    player: &AnimationGraphPlayer,
    graph: &AnimationGraph,
    node_name: &str,
) -> Option<Pose> {
    let node = graph.nodes.values().find(|node| node.name == node_name)?;
    let arena = player.get_context_arena()?;
    match arena.get_toplevel().node_caches.get_output_data(
        node.id,
        StateKey::Default,
        "pose".to_owned(),
    ) {
        Some(DataValue::Pose(pose)) => Some(pose),
        _ => None,
    }
}

fn pose_joint_snapshot(
    pose: &Pose,
    skins: &Query<&SkinnedMesh>,
    joint_targets: &Query<(&Name, &AnimationTargetId)>,
) -> Option<(usize, Vec3, Vec3, Vec3, Vec3, Vec3)> {
    let translation = |name| {
        let target = skin_joint_target(name, skins, joint_targets)?;
        pose.get_bone(BoneId::from(target))?.translation
    };
    Some((
        pose.bones.len(),
        translation("root")?,
        translation("pelvis")?,
        translation("head")?,
        translation("foot_l")?,
        translation("foot_r")?,
    ))
}

fn log_bevy_pose(
    fixed_frame: u32,
    player: &AnimationGraphPlayer,
    pose: &Pose,
    skins: &Query<&SkinnedMesh>,
    joint_transforms: &Query<(
        Entity,
        &Name,
        &Transform,
        &GlobalTransform,
        Option<&AnimationTargetId>,
    )>,
) {
    let Some(skin) = skins.iter().max_by_key(|skin| skin.joints.len()) else {
        return;
    };
    let find = |name: &str| {
        skin.joints.iter().find_map(|entity| {
            joint_transforms
                .get(*entity)
                .ok()
                .filter(|(_, joint_name, _, _, _)| joint_name.as_str() == name)
        })
    };
    let Some((_, _, _, root_global, _)) = find("root") else {
        return;
    };
    for name in ["root", "pelvis", "head", "foot_l", "foot_r"] {
        let Some((entity, _, local, global, target)) = find(name) else {
            continue;
        };
        let component = global.reparented_to(root_global);
        let output = target
            .and_then(|target| pose.get_bone(BoneId::from(*target)))
            .cloned();
        let output_translation = output
            .as_ref()
            .and_then(|bone| bone.translation)
            .unwrap_or(Vec3::ZERO);
        let output_rotation = output
            .as_ref()
            .and_then(|bone| bone.rotation)
            .unwrap_or(Quat::IDENTITY);
        let mapping_matches =
            target.is_some_and(|target| player.bone_entity(BoneId::from(*target)) == Some(entity));
        info!(
            "[BEVY-POSE] fixed_frame={fixed_frame} bone={name} mapping_matches={mapping_matches} local_t={:?} local_q={:?} component_t={:?} component_q={:?} output_t={output_translation:?} output_q={output_rotation:?}",
            local.translation, local.rotation, component.translation, component.rotation,
        );
    }
}

fn log_fsm_pose(
    fixed_frame: u32,
    label: &str,
    player: &AnimationGraphPlayer,
    graph: &AnimationGraph,
    node_name: &str,
    skins: &Query<&SkinnedMesh>,
    joint_targets: &Query<(&Name, &AnimationTargetId)>,
) {
    let Some(node) = graph.nodes.values().find(|node| node.name == node_name) else {
        return;
    };
    let Some(arena) = player.get_context_arena() else {
        return;
    };
    for context_id in arena.iter_context_ids() {
        let Some(context) = arena.get_context(context_id) else {
            continue;
        };
        let Ok(state) = context
            .node_states
            .get::<InterruptibleFsmState>(node.id, StateKey::Default)
        else {
            continue;
        };
        let snapshot = state
            .last_output_pose()
            .and_then(|pose| pose_joint_snapshot(pose, skins, joint_targets));
        info!(
            "[BEVY-FSM] fixed_frame={fixed_frame} label={label} context={context_id:?} state={:?} entered={:.6} last_pose={snapshot:?}",
            state.state, state.state_entered_time,
        );
        let upstream_names: &[&str] = match label {
            "grounded" => &[
                "Standing Input linked graph",
                "Crouching Input linked graph",
                "Grounded States",
            ],
            "standing" => &[
                "standing movement details cache",
                "standing states",
                "standing turn blend",
                "standing quick stop blend",
                "standing lean additive",
            ],
            _ => &[],
        };
        for upstream_name in upstream_names {
            let Some(upstream) = graph
                .nodes
                .values()
                .find(|node| node.name == *upstream_name)
            else {
                continue;
            };
            let snapshot = match context.node_caches.get_output_data(
                upstream.id,
                StateKey::Default,
                "pose".to_owned(),
            ) {
                Some(DataValue::Pose(pose)) => pose_joint_snapshot(&pose, skins, joint_targets),
                _ => None,
            };
            info!(
                "[BEVY-CACHE] fixed_frame={fixed_frame} label={label} context={context_id:?} node={upstream_name} pose={snapshot:?}"
            );
        }
    }
}

fn capture_pre_physics(
    mut probe: ResMut<Probe>,
    avatars: Query<(&ChildOf, &AnimatedSceneInstance), With<AlsCharacterAvatar>>,
    players: Query<&AnimationGraphPlayer>,
    skins: Query<&SkinnedMesh>,
    joint_targets: Query<(&Name, &AnimationTargetId)>,
) {
    if let Some((pelvis, _)) = player_pelvis_state(&avatars, &players, &skins, &joint_targets) {
        probe.pre_physics_pelvis = Some(pelvis);
    }
}

fn capture_post_physics(
    mut probe: ResMut<Probe>,
    avatars: Query<(&ChildOf, &AnimatedSceneInstance), With<AlsCharacterAvatar>>,
    players: Query<&AnimationGraphPlayer>,
    skins: Query<&SkinnedMesh>,
    joint_targets: Query<(&Name, &AnimationTargetId)>,
) {
    let Some(pre) = probe.pre_physics_pelvis else {
        return;
    };
    let Some((post, config_valid)) =
        player_pelvis_state(&avatars, &players, &skins, &joint_targets)
    else {
        return;
    };
    probe.physics_ticks += 1;
    if !probe.physics_observed {
        probe.physics_changed_pose = (post - pre).length() > 1e-4;
        probe.physics_config_valid = config_valid;
        probe.physics_observed = true;
        info!(
            "[CANONICAL-SCALE][PHYSICS] pre_pelvis={pre:?} post_pelvis={post:?} changed={} config_kinematic_no_readback={config_valid}",
            probe.physics_changed_pose,
        );
    }
}

fn observe(
    mut probe: ResMut<Probe>,
    avatars: Query<(&ChildOf, &AnimatedSceneInstance), With<AlsCharacterAvatar>>,
    players: Query<&AnimationGraphPlayer>,
    contexts: Query<&AlsRootAnimationContext>,
    layering: Query<&AlsLayeringState>,
    heads: Query<&AlsHeadState>,
    targets: Query<&GlobalTransform, With<AnimationTargetId>>,
    skins: Query<&SkinnedMesh>,
    joint_targets: Query<(&Name, &AnimationTargetId)>,
    joint_transforms: Query<(
        Entity,
        &Name,
        &Transform,
        &GlobalTransform,
        Option<&AnimationTargetId>,
    )>,
    body_modes: Query<&RigidBody>,
    root_graph: Res<RootGraph>,
    graphs: Res<Assets<AnimationGraph>>,
    mut exit: MessageWriter<AppExit>,
) {
    if probe.complete {
        return;
    }
    let Some(character) = probe.character else {
        return;
    };
    let Some((_, instance)) = avatars
        .iter()
        .find(|(parent, _)| parent.parent() == character)
    else {
        return;
    };
    let Ok(player) = players.get(instance.player_entity()) else {
        return;
    };
    let context = contexts.get(instance.player_entity()).ok();
    probe.attempts += 1;
    let target_count = targets.iter().count();
    let Some(DataValue::Pose(pose)) = player.get_outputs().get("pose") else {
        if probe.attempts % 120 == 0 {
            info!(
                "[CANONICAL-SCALE] waiting pose targets={} error={:?}",
                target_count,
                player.get_error(),
            );
        }
        return;
    };
    if target_count < 50 {
        if probe.attempts % 120 == 0 {
            info!("[CANONICAL-SCALE] waiting targets={target_count}");
        }
        return;
    }
    let Ok(layering) = layering.get(character) else {
        return;
    };
    let trace_frame = match probe.bevy_pose_samples {
        0 if probe.physics_ticks >= 60 => Some(60),
        1 if probe.physics_ticks >= 180 => Some(180),
        _ => None,
    };
    if let Some(fixed_frame) = trace_frame {
        log_bevy_pose(fixed_frame, player, pose, &skins, &joint_transforms);
        let state_values = [
            ("grounded", global_f32(player, "als_grounded_state_index")),
            (
                "locomotion",
                global_f32(player, "als_locomotion_state_index"),
            ),
            ("standing", global_f32(player, "als_standing_state_index")),
            (
                "standing_clip",
                global_f32(player, "als_standing_clip_index"),
            ),
            (
                "standing_turn_weight",
                global_f32(player, "als_standing_turn_weight"),
            ),
            (
                "standing_quick_stop_weight",
                global_f32(player, "als_standing_quick_stop_weight"),
            ),
            (
                "standing_lean_weight",
                global_f32(player, "als_standing_lean_weight"),
            ),
            ("crouching", global_f32(player, "als_crouching_state_index")),
        ];
        info!("[BEVY-STATE] fixed_frame={fixed_frame} values={state_values:?}");
        if let Some(graph) = graphs.get(&root_graph.root) {
            for name in [
                "AB_Als_Grounded Slot Transition",
                "AB_Als_Locomotion Slot PostLocomotion",
                "ALI_Overlay Overlay Inertialization",
                "AB_Als_Layering",
                "AB_Als_Head",
                "ControlRig CR_Als",
                "AB_Als_Ragdolling",
            ] {
                let snapshot = cached_pose(player, graph, name)
                    .and_then(|pose| pose_joint_snapshot(&pose, &skins, &joint_targets));
                info!("[BEVY-STAGE] fixed_frame={fixed_frame} name={name} joints={snapshot:?}");
            }
        }
        if let Some(graph) = graphs.get(&root_graph.grounded) {
            log_fsm_pose(
                fixed_frame,
                "grounded",
                player,
                graph,
                "Grounded States",
                &skins,
                &joint_targets,
            );
        }
        if let Some(graph) = graphs.get(&root_graph.standing) {
            log_fsm_pose(
                fixed_frame,
                "standing",
                player,
                graph,
                "standing states",
                &skins,
                &joint_targets,
            );
        }
        probe.bevy_pose_samples += 1;
    }
    if probe.physics_ticks < 180 {
        return;
    }
    let target_scales = targets
        .iter()
        .map(|transform| transform.to_scale_rotation_translation().0.abs())
        .collect::<Vec<_>>();
    let min_global_scale = target_scales
        .iter()
        .map(|scale| scale.min_element())
        .fold(f32::INFINITY, f32::min);
    let max_global_scale = target_scales
        .iter()
        .map(|scale| scale.max_element())
        .fold(0.0_f32, f32::max);
    let pose_scale_entries = pose
        .bones
        .iter()
        .filter(|bone| bone.scale.is_some())
        .count();
    let min_pose_scale = pose
        .bones
        .iter()
        .filter_map(|bone| bone.scale)
        .map(|scale| scale.abs().min_element())
        .fold(f32::INFINITY, f32::min);
    let max_pose_scale = pose
        .bones
        .iter()
        .filter_map(|bone| bone.scale)
        .map(|scale| scale.abs().max_element())
        .fold(0.0_f32, f32::max);
    let min_pose_rotation_length = pose
        .bones
        .iter()
        .filter_map(|bone| bone.rotation)
        .map(Quat::length)
        .fold(f32::INFINITY, f32::min);
    let max_pose_rotation_length = pose
        .bones
        .iter()
        .filter_map(|bone| bone.rotation)
        .map(Quat::length)
        .fold(0.0_f32, f32::max);
    let curve = |name| {
        context
            .and_then(|context| context.f32(name))
            .unwrap_or_default()
    };
    let layering_feedback = [
        (layering.head_blend_amount, curve("LayerHead")),
        (
            layering.head_additive_blend_amount,
            curve("LayerHeadAdditive"),
        ),
        (layering.head_slot_blend_amount, curve("LayerHeadSlot")),
        (layering.arm_left_blend_amount, curve("LayerArmLeft")),
        (layering.arm_right_blend_amount, curve("LayerArmRight")),
        (layering.spine_blend_amount, curve("LayerSpine")),
        (layering.pelvis_blend_amount, curve("LayerPelvis")),
        (layering.legs_blend_amount, curve("LayerLegs")),
    ]
    .into_iter()
    .all(|(state, output)| (state - output).abs() <= 1e-4);
    let layering_active = layering.legs_blend_amount > 0.99
        && layering.pelvis_blend_amount > 0.99
        && layering.spine_blend_amount > 0.99
        && layering.head_blend_amount > 0.99;
    let skin_joint_stats = skins
        .iter()
        .max_by_key(|skin| skin.joints.len())
        .and_then(|skin| {
            let lookup = |name: &str| {
                skin.joints.iter().find_map(|entity| {
                    joint_transforms
                        .get(*entity)
                        .ok()
                        .filter(|(_, joint_name, _, _, _)| joint_name.as_str() == name)
                        .map(|(_, _, _, transform, _)| transform.translation())
                })
            };
            let root = lookup("root")?;
            let head = lookup("head")?;
            let left_foot = lookup("foot_l")?;
            let right_foot = lookup("foot_r")?;
            Some((
                skin.joints.len(),
                head.y - root.y,
                (left_foot - right_foot).length(),
                (left_foot.y - root.y)
                    .abs()
                    .max((right_foot.y - root.y).abs()),
            ))
        });
    let skin_geometry_valid =
        skin_joint_stats.is_some_and(|(_, head_height, foot_separation, _)| {
            (MIN_SKIN_HEAD_HEIGHT..=MAX_SKIN_HEAD_HEIGHT).contains(&head_height)
                && (MIN_SKIN_FOOT_SEPARATION..=MAX_SKIN_FOOT_SEPARATION).contains(&foot_separation)
        });
    let head_state = heads.get(character).ok().copied();
    let head_pitch = global_f32(player, "als_head_pitch_angle");
    let head_yaw_amount = global_f32(player, "als_head_yaw_amount");
    let head_blend_amount = global_f32(player, "als_head_blend_amount");
    let ragdoll_modes = player.get_spawned_ragdoll().map(|spawned| {
        spawned
            .bodies
            .values()
            .filter_map(|entity| body_modes.get(*entity).ok())
            .fold(
                (0_usize, 0_usize),
                |(kinematic, dynamic), mode| match mode {
                    RigidBody::Kinematic => (kinematic + 1, dynamic),
                    RigidBody::Dynamic => (kinematic, dynamic + 1),
                    _ => (kinematic, dynamic),
                },
            )
    });
    if let Some(graph) = graphs.get(&root_graph.root) {
        for name in [
            "AB_Als_Grounded Slot Transition",
            "AB_Als_Locomotion Slot PostLocomotion",
            "ALI_Overlay Overlay Inertialization",
            "AB_Als_Layering",
            "AB_Als_Head",
            "ControlRig CR_Als",
            "AB_Als_Ragdolling",
        ] {
            let snapshot = cached_pose(player, graph, name)
                .and_then(|pose| pose_joint_snapshot(&pose, &skins, &joint_targets));
            info!("[CANONICAL-SCALE][STAGE] name={name} joints={snapshot:?}");
        }
    }
    let pass = min_global_scale >= MIN_ALLOWED_SCALE
        && max_global_scale <= MAX_ALLOWED_SCALE
        && (pose_scale_entries == 0
            || (min_pose_scale >= MIN_ALLOWED_SCALE && max_pose_scale <= MAX_ALLOWED_SCALE))
        && layering_feedback
        && layering_active
        && probe.physics_observed
        && !probe.physics_changed_pose
        && probe.physics_config_valid
        && skin_geometry_valid;
    info!(
        "[CANONICAL-SCALE] targets={} pose_scale_entries={} pose_scale=[{min_pose_scale:.6},{max_pose_scale:.6}] pose_rotation_length=[{min_pose_rotation_length:.6},{max_pose_rotation_length:.6}] global_scale=[{min_global_scale:.6},{max_global_scale:.6}] layering_feedback={layering_feedback} layering_active={layering_active} physics_observed={} physics_changed={} physics_config_valid={} skin_geometry_valid={skin_geometry_valid} legs={:.6} pelvis={:.6} spine={:.6} head={:.6} allowed=[{MIN_ALLOWED_SCALE:.6},{MAX_ALLOWED_SCALE:.6}]",
        target_count,
        pose_scale_entries,
        probe.physics_observed,
        probe.physics_changed_pose,
        probe.physics_config_valid,
        layering.legs_blend_amount,
        layering.pelvis_blend_amount,
        layering.spine_blend_amount,
        layering.head_blend_amount,
    );
    info!(
        "[CANONICAL-SCALE][VERDICT] canonical={} -> {}",
        pass,
        if pass { "PASS" } else { "FAIL" }
    );
    info!("[CANONICAL-SCALE][SKIN] joints={skin_joint_stats:?}");
    info!(
        "[CANONICAL-SCALE][HEAD] state={head_state:?} pitch={head_pitch:?} yaw_amount={head_yaw_amount:?} blend_amount={head_blend_amount:?}"
    );
    info!("[CANONICAL-SCALE][RAGDOLL] modes={ragdoll_modes:?}");
    if let Some(skin) = skins.iter().max_by_key(|skin| skin.joints.len()) {
        for name in ["root", "pelvis", "head", "foot_l", "foot_r"] {
            if let Some((entity, _, local, transform, target)) =
                skin.joints.iter().find_map(|entity| {
                    joint_transforms
                        .get(*entity)
                        .ok()
                        .filter(|(_, joint_name, _, _, _)| joint_name.as_str() == name)
                })
            {
                let pose_translation = target
                    .and_then(|target| pose.get_bone(BoneId::from(*target)))
                    .and_then(|bone| bone.translation);
                let player_mapping_matches = target.is_some_and(|target| {
                    player.bone_entity(BoneId::from(*target)) == Some(entity)
                });
                info!(
                    "[CANONICAL-SCALE][SKIN-JOINT] name={name} entity={entity:?} target={} player_mapping_matches={player_mapping_matches} local={:?} pose_translation={pose_translation:?} global={:?}",
                    target.is_some(),
                    local.translation,
                    transform.translation(),
                );
            }
        }
    }
    probe.complete = true;
    if pass {
        exit.write(AppExit::Success);
    } else {
        exit.write(AppExit::Error(std::num::NonZeroU8::new(10).unwrap()));
    }
}

fn timeout(probe: Res<Probe>, deadline: Res<Deadline>, mut exit: MessageWriter<AppExit>) {
    if !probe.complete && Instant::now() >= deadline.0 {
        warn!("[CANONICAL-SCALE][VERDICT] timeout -> FAIL");
        exit.write(AppExit::Error(std::num::NonZeroU8::new(11).unwrap()));
    }
}
