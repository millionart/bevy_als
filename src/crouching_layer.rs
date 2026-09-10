use als_core::{
    character::AlsCharacter,
    foot_ik::AlsFootLockInput,
    input::AlsInputCommand,
    motion::{AlsInputOverride, AlsRotationAnimationState, AlsRotationResponseSet},
    state::Stance,
};
use als_ext::{
    InterruptibleFsmNode, InterruptibleFsmState, source_instance::source_player_property,
};
use bevy::prelude::*;
use bevy_animation_graph::core::{
    animated_scene::AnimatedSceneInstance,
    animation_graph::AnimationGraph,
    animation_graph_player::AnimationGraphPlayer,
    context::node_states::StateKey,
    plugin::AnimationGraphSet,
    state_machine::{high_level::StateMachine, low_level::LowLevelStateId},
};

use crate::{
    actual_motion::AlsActualMotion,
    animation_context_projection::{AlsRootAnimationContext, AlsRootAnimationContextSet},
    character_assembly::AlsCharacterAvatar,
    standing_layer::source_moving_smooth,
};

const OUTER_RUNTIME_GRAPH: &str =
    "generated/ALS/Character/AnimationInstances/Stances/AB_Als_Crouching/AnimGraph.animgraph.ron";
const OUTER_FSM_TITLE: &str = "Crouching States\nState Machine";
const MOVEMENT_FSM_TITLE: &str = "Movement States\nState Machine";
const MOVEMENT_SYNC_GROUP: &str = "sync_groups.ab_als_crouching.movement.phase";

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AlsCrouchingLayerSet;

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AlsCrouchingTelemetrySet;

#[derive(Resource)]
struct CrouchingGraphHandle(Handle<AnimationGraph>);

#[derive(Component, Debug, Default, Clone, Copy, PartialEq)]
pub struct AlsCrouchingGraphTelemetry {
    pub actual_outer_state: Option<u8>,
    pub actual_outer_weights: [f32; 6],
    pub actual_outer_transition_active: Option<bool>,
    pub actual_outer_pose_ready: Option<bool>,
    pub actual_movement_state: Option<u8>,
    pub actual_movement_states: u8,
    pub actual_movement_transition_active: Option<bool>,
    pub actual_movement_pose_ready: Option<bool>,
    pub graph_stride_amount: Option<f32>,
    pub graph_play_rate: Option<f32>,
    pub graph_marker_phase: Option<f32>,
    pub graph_pose_moving: Option<f32>,
    pub graph_turn_curve_direct_yaw_speed_target: Option<f32>,
    pub dominant_velocity_index: Option<u8>,
    pub dominant_velocity_weight: Option<f32>,
    pub lean_right_amount: Option<f32>,
    pub lean_forward_amount: Option<f32>,
}

pub struct CrouchingLayerPlugin;

impl Plugin for CrouchingLayerPlugin {
    fn build(&self, app: &mut App) {
        app.configure_sets(
            PostUpdate,
            AlsCrouchingLayerSet
                .after(crate::standing_layer::AlsStandingLayerSet)
                .before(AlsRotationResponseSet)
                .before(als_ext::post_physics_animation::PostPhysicsAnimationSet::Evaluate),
        )
        .configure_sets(
            PostUpdate,
            AlsCrouchingTelemetrySet
                .after(AnimationGraphSet::Final)
                .after(AlsRootAnimationContextSet),
        )
        .add_systems(
            Startup,
            |mut commands: Commands, assets: Res<AssetServer>| {
                commands.insert_resource(CrouchingGraphHandle(assets.load(OUTER_RUNTIME_GRAPH)));
            },
        )
        .add_systems(
            PostUpdate,
            crouching_layer_writer.in_set(AlsCrouchingLayerSet),
        )
        .add_systems(
            PostUpdate,
            capture_crouching_graph_telemetry.in_set(AlsCrouchingTelemetrySet),
        );
    }
}

fn crouching_outer_state_index(label: &str) -> Option<u8> {
    match label {
        "Idle" => Some(0),
        "Move" => Some(1),
        "Rotate Left" => Some(2),
        "Rotate Right" => Some(3),
        "Stop" => Some(4),
        _ => None,
    }
}

fn crouching_movement_state_index(label: &str) -> Option<u8> {
    match label {
        "Move Forward" => Some(0),
        "Move Backward" => Some(1),
        "Move Right Forward" => Some(2),
        "Move Right Backward" => Some(3),
        "Move Left Forward" => Some(4),
        "Move Left Backward" => Some(5),
        _ => None,
    }
}

fn actual_crouching_fsm(
    player: &AnimationGraphPlayer,
    graph: &AnimationGraph,
    graph_handle: &Handle<AnimationGraph>,
    state_machines: &Assets<StateMachine>,
    title: &str,
    state_index: fn(&str) -> Option<u8>,
) -> Option<(Option<u8>, bool, bool, [f32; 6])> {
    let node = graph.nodes.values().find(|node| node.name == title)?;
    let fsm = node.try_inner_downcast_ref::<InterruptibleFsmNode>()?;
    let state_machine = state_machines.get(&fsm.fsm)?;
    let arena = player.get_context_arena()?;
    arena.iter_context_ids().find_map(|context_id| {
        let context = arena.get_context(context_id)?;
        if context.get_graph_id() != graph_handle.id()
            || !context.node_caches.is_updated(node.id, StateKey::Default)
        {
            return None;
        }
        let state = context
            .node_states
            .get_all_upcoming_states::<InterruptibleFsmState>(node.id)
            .ok()?
            .find(|state| state.last_output_pose().is_some())?;
        let actual_state = match &state.state {
            LowLevelStateId::HlState(id) => state_machine
                .states
                .get(id)
                .and_then(|state| state_index(&state.label)),
            LowLevelStateId::HlTransition(_) => None,
        };
        let mut weights = [0.0; 6];
        for (id, entry) in &state_machine.states {
            if let Some(index) = state_index(&entry.label) {
                weights[index as usize] = state.recorded_state_weight(&id.uuid().to_string());
            }
        }
        Some((
            actual_state,
            matches!(state.state, LowLevelStateId::HlTransition(_)),
            true,
            weights,
        ))
    })
}

fn capture_crouching_graph_telemetry(
    mut commands: Commands,
    avatars: Query<(&AlsCharacterAvatar, &AnimatedSceneInstance)>,
    players: Query<(&AnimationGraphPlayer, Option<&AlsRootAnimationContext>)>,
    graphs: Res<Assets<AnimationGraph>>,
    state_machines: Res<Assets<StateMachine>>,
    handle: Res<CrouchingGraphHandle>,
    mut characters: Query<Option<&mut AlsCrouchingGraphTelemetry>, With<AlsCharacter>>,
) {
    for (avatar, instance) in &avatars {
        let Ok((player, context)) = players.get(instance.player_entity()) else {
            continue;
        };
        let read_fsm = |title, index| {
            graphs.get(&handle.0).and_then(|graph| {
                actual_crouching_fsm(player, graph, &handle.0, &state_machines, title, index)
            })
        };
        let outer = read_fsm(
            OUTER_FSM_TITLE,
            crouching_outer_state_index as fn(&str) -> Option<u8>,
        );
        let movement = read_fsm(
            MOVEMENT_FSM_TITLE,
            crouching_movement_state_index as fn(&str) -> Option<u8>,
        );
        let scalar =
            |key: &str| source_player_property(player, key).and_then(|value| value.as_f32().ok());
        let weights = ["forward", "backward", "left", "right"].map(|side| {
            scalar(&format!("grounded_state.velocity_blend.{side}_amount")).unwrap_or(0.0)
        });
        let dominant = weights
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.total_cmp(b));
        let total = weights.iter().sum::<f32>();
        let telemetry = AlsCrouchingGraphTelemetry {
            actual_outer_state: outer.and_then(|value| value.0),
            actual_outer_weights: outer.map_or([0.0; 6], |value| value.3),
            actual_outer_transition_active: outer.map(|value| value.1),
            actual_outer_pose_ready: outer.map(|value| value.2),
            actual_movement_state: movement.and_then(|value| value.0),
            actual_movement_states: movement.map_or(0, |value| {
                value
                    .3
                    .iter()
                    .enumerate()
                    .filter(|(_, weight)| **weight > 1e-5)
                    .fold(0, |mask, (index, _)| mask | (1 << index))
            }),
            actual_movement_transition_active: movement.map(|value| value.1),
            actual_movement_pose_ready: movement.map(|value| value.2),
            graph_stride_amount: player
                .get_global_input_data("crouching_state.stride_blend_amount")
                .and_then(|value| value.as_f32().ok()),
            graph_play_rate: player
                .get_global_input_data("crouching_state.play_rate")
                .and_then(|value| value.as_f32().ok()),
            graph_marker_phase: player.get_context_arena().and_then(|arena| {
                als_ext::sync_group_phase::source_sync_phase(arena, &graphs, MOVEMENT_SYNC_GROUP)
            }),
            graph_pose_moving: context.and_then(|state| state.f32("PoseMoving")),
            graph_turn_curve_direct_yaw_speed_target: context
                .and_then(|state| state.f32("RotationYawSpeed")),
            dominant_velocity_index: dominant
                .filter(|_| total > f32::EPSILON)
                .map(|(index, _)| index as u8),
            dominant_velocity_weight: dominant
                .filter(|_| total > f32::EPSILON)
                .map(|(_, weight)| weight / total),
            lean_right_amount: scalar("lean_state.right_amount"),
            lean_forward_amount: scalar("lean_state.forward_amount"),
        };
        let Ok(current) = characters.get_mut(avatar.character) else {
            continue;
        };
        if let Some(mut current) = current {
            *current = telemetry;
        } else {
            commands.entity(avatar.character).insert(telemetry);
        }
    }
}

fn crouching_curve_outputs(
    context: Option<&AlsRootAnimationContext>,
    moving_smooth: bool,
) -> (AlsFootLockInput, AlsRotationAnimationState) {
    let curve = |name| context.and_then(|state| state.f32(name)).unwrap_or(0.0);
    (
        AlsFootLockInput {
            left_ik_amount: curve("FootLeftIk").clamp(0.0, 1.0),
            right_ik_amount: curve("FootRightIk").clamp(0.0, 1.0),
            left_lock_amount: curve("FootLeftLock").clamp(0.0, 1.0),
            right_lock_amount: curve("FootRightLock").clamp(0.0, 1.0),
            allow_transitions: curve("AllowTransitions") >= 1.0 - 1e-4,
            moving_smooth,
            foot_planted_amount: curve("FootPlanted").clamp(-1.0, 1.0),
            feet_crossing_amount: curve("FeetCrossing").clamp(0.0, 1.0),
            ..default()
        },
        AlsRotationAnimationState {
            yaw_offset_degrees: curve("RotationYawOffset"),
            yaw_speed_degrees: curve("RotationYawSpeed"),
        },
    )
}

fn crouching_layer_writer(
    mut commands: Commands,
    characters: Query<
        (
            &Stance,
            &AlsActualMotion,
            &AlsInputCommand,
            Option<&AlsInputOverride>,
        ),
        With<AlsCharacter>,
    >,
    avatars: Query<(&AlsCharacterAvatar, &AnimatedSceneInstance)>,
    players: Query<Option<&AlsRootAnimationContext>, With<AnimationGraphPlayer>>,
) {
    for (avatar, instance) in &avatars {
        let Ok((stance, motion, input, input_override)) = characters.get(avatar.character) else {
            continue;
        };
        if *stance != Stance::Crouching {
            continue;
        }
        let Ok(context) = players.get(instance.player_entity()) else {
            continue;
        };
        let has_input = input_override
            .map_or(input, |value| &value.0)
            .move_dir
            .length_squared()
            > f32::EPSILON;
        commands
            .entity(avatar.character)
            .insert(crouching_curve_outputs(
                context,
                source_moving_smooth(has_input, motion.horizontal_speed),
            ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crouching_telemetry_reads_the_generated_root_used_by_the_character() {
        let root = include_str!("../assets/generated/ALS/Character/AB_Als/AnimGraph.animgraph.ron");
        assert!(root.contains(OUTER_RUNTIME_GRAPH));
        let text = include_str!(
            "../assets/generated/ALS/Character/AnimationInstances/Stances/AB_Als_Crouching/AnimGraph.animgraph.ron"
        );
        for title in [OUTER_FSM_TITLE, MOVEMENT_FSM_TITLE] {
            assert!(text.contains(&format!("name: {:?}", title)), "{title}");
        }
        assert_eq!(crouching_outer_state_index("Stop"), Some(4));
        assert_eq!(
            crouching_movement_state_index("Move Left Backward"),
            Some(5)
        );
        assert_eq!(crouching_movement_state_index("Idle"), None);
    }

    #[test]
    fn crouching_curve_consumers_use_the_evaluated_root_curves() {
        let context = AlsRootAnimationContext {
            curves: [
                ("FootLeftIk".into(), 0.6),
                ("FootRightIk".into(), 1.4),
                ("FootLeftLock".into(), 0.4),
                ("FootRightLock".into(), 0.7),
                ("FootPlanted".into(), -0.5),
                ("FeetCrossing".into(), 0.3),
                ("AllowTransitions".into(), 1.0),
                ("RotationYawOffset".into(), -25.0),
                ("RotationYawSpeed".into(), 42.0),
            ]
            .into_iter()
            .collect(),
        };
        let (feet, rotation) = crouching_curve_outputs(Some(&context), true);
        assert_eq!((feet.left_ik_amount, feet.right_ik_amount), (0.6, 1.0));
        assert_eq!((feet.left_lock_amount, feet.right_lock_amount), (0.4, 0.7));
        assert_eq!(
            (feet.foot_planted_amount, feet.feet_crossing_amount),
            (-0.5, 0.3)
        );
        assert!(feet.allow_transitions && feet.moving_smooth);
        assert_eq!(
            (rotation.yaw_offset_degrees, rotation.yaw_speed_degrees),
            (-25.0, 42.0)
        );
        let (feet, rotation) = crouching_curve_outputs(None, false);
        assert_eq!(
            (feet.left_lock_amount, rotation.yaw_speed_degrees),
            (0.0, 0.0)
        );
    }
}
