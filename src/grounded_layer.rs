use als_core::character::AlsCharacter;
use als_core::roll::{AlsRollState, ROLL_PLAY_RATE};
use als_core::state::Stance;
use als_ext::{
    AlsAnimCurves, CurveAsset, InterruptibleFsmNode, InterruptibleFsmState, LayeredGraphNode,
};
use avian3d::prelude::LinearVelocity;
use bevy::ecs::hierarchy::ChildOf;
use bevy::prelude::*;
use bevy_animation_graph::core::animated_scene::AnimatedSceneInstance;
use bevy_animation_graph::core::animation_graph::AnimationGraph;
use bevy_animation_graph::core::animation_graph_player::AnimationGraphPlayer;
use bevy_animation_graph::core::context::node_states::StateKey;
use bevy_animation_graph::core::plugin::AnimationGraphSet;
use bevy_animation_graph::core::state_machine::{
    high_level::StateMachine, low_level::LowLevelStateId,
};

use crate::character_assembly::AlsCharacterAvatar;
use crate::crouching_layer::AlsCrouchingLayerSet;
use crate::turn_layer::AlsTurnDrive;

const MOVING_SMOOTH_SPEED_THRESHOLD: f32 = 1.5;
const STANCE_CLIP_DURATION: f32 = 1.0;
const STANCE_CLIP_PLAY_RATE: f32 = 1.2;
const STANCE_CLIP_PLAY_DURATION: f32 = STANCE_CLIP_DURATION / STANCE_CLIP_PLAY_RATE;
const STAND_TO_CROUCH_CURVES: &str = "animations/Transitions/A_Als_StandToCrouch.gltf#AlsCurves";
const CROUCH_TO_STAND_CURVES: &str = "animations/Transitions/A_Als_CrouchToStand.gltf#AlsCurves";
const ROLL_CURVES: &str = "animations/Actions/Roll/A_Als_Roll.gltf#AlsCurves";
const STANCE_CHANGE_CURVE: &str =
    "curves/ALS/Data/AnimationInstance/Blending/CF_Als_StanceChange.curve.ron";
const GROUNDED_GRAPH: &str = "graphs/ab_als/grounded.animgraph.ron";
const GROUNDED_INTERNAL_TIME_KEY: &str = "als_grounded_delta_seconds";

#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AlsGroundedEntryMode {
    #[default]
    None,
    FromRoll,
}

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AlsGroundedLayerSet;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum GroundedState {
    #[default]
    EntryConduit,
    Standing,
    Crouching,
    StandingToCrouching,
    CrouchingToStanding,
    Roll,
}

impl GroundedState {
    fn index(self) -> f32 {
        match self {
            Self::EntryConduit => 0.0,
            Self::Standing => 1.0,
            Self::Crouching => 2.0,
            Self::StandingToCrouching => 3.0,
            Self::CrouchingToStanding => 4.0,
            Self::Roll => 5.0,
        }
    }

    fn clip_index(self) -> f32 {
        match self {
            Self::EntryConduit | Self::Standing => 0.0,
            Self::Crouching => 1.0,
            Self::StandingToCrouching => 2.0,
            Self::CrouchingToStanding => 3.0,
            Self::Roll => 4.0,
        }
    }

    fn looped(self) -> f32 {
        match self {
            Self::StandingToCrouching | Self::CrouchingToStanding => 0.0,
            Self::EntryConduit | Self::Standing | Self::Crouching | Self::Roll => 1.0,
        }
    }

    fn play_rate(self) -> f32 {
        match self {
            Self::StandingToCrouching | Self::CrouchingToStanding => STANCE_CLIP_PLAY_RATE,
            Self::EntryConduit | Self::Standing | Self::Crouching | Self::Roll => 1.0,
        }
    }
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GroundedTransition {
    EntryToRoll,
    EntryToCrouching,
    EntryToStanding,
    EntryFallbackToStanding,
    StandingToCrouching,
    StandingToStandingToCrouching,
    CrouchingToStanding,
    CrouchingToCrouchingToStanding,
    StandingToCrouchingAutomatic,
    StandingToCrouchingToStanding,
    StandingToCrouchingToCrouching,
    StandingToCrouchingToCrouchingToStanding,
    StandingToCrouchingSelf,
    CrouchingToStandingAutomatic,
    CrouchingToStandingToStanding,
    CrouchingToStandingToCrouching,
    CrouchingToStandingSelf,
    CrouchingToStandingToStandingToCrouching,
    RollToCrouching,
    RollToStanding,
}

#[derive(Clone, Copy)]
struct GroundedTransitionProfile {
    source_static_contract: &'static str,
    duration: f32,
}

const GROUNDED_TRANSITION_PROFILES: [GroundedTransitionProfile; 12] = [
    GroundedTransitionProfile {
        source_static_contract: "AnimStateTransitionNode_20|PriorityOrder=1|CrossfadeDuration=0.000000|BlendMode=HermiteCubic|bAutomaticRuleBasedOnSequencePlayerInState=True|AutomaticRuleTriggerTime=-1.000000|SyncGroupNameToRequireValidMarkersRule=None|bAllowInertializationForSelfTransitions=False|bSharedRules=False|bSharedCrossfade=False",
        duration: 0.0,
    },
    GroundedTransitionProfile {
        source_static_contract: "AnimStateTransitionNode_24|PriorityOrder=1|CrossfadeDuration=0.000000|BlendMode=HermiteCubic|bAutomaticRuleBasedOnSequencePlayerInState=True|AutomaticRuleTriggerTime=-1.000000|SyncGroupNameToRequireValidMarkersRule=None|bAllowInertializationForSelfTransitions=False|bSharedRules=False|bSharedCrossfade=False",
        duration: 0.0,
    },
    GroundedTransitionProfile {
        source_static_contract: "AnimStateTransitionNode_11|PriorityOrder=1|CrossfadeDuration=0.600000|BlendMode=Cubic|bAutomaticRuleBasedOnSequencePlayerInState=False|AutomaticRuleTriggerTime=-1.000000|SyncGroupNameToRequireValidMarkersRule=None|bAllowInertializationForSelfTransitions=False|bSharedRules=False|bSharedCrossfade=False",
        duration: 0.6,
    },
    GroundedTransitionProfile {
        source_static_contract: "AnimStateTransitionNode_15|PriorityOrder=1|CrossfadeDuration=0.600000|BlendMode=Cubic|bAutomaticRuleBasedOnSequencePlayerInState=False|AutomaticRuleTriggerTime=-1.000000|SyncGroupNameToRequireValidMarkersRule=None|bAllowInertializationForSelfTransitions=False|bSharedRules=False|bSharedCrossfade=False",
        duration: 0.6,
    },
    GroundedTransitionProfile {
        source_static_contract: "AnimStateTransitionNode_9|PriorityOrder=1|CrossfadeDuration=0.500000|BlendMode=Custom|bAutomaticRuleBasedOnSequencePlayerInState=False|AutomaticRuleTriggerTime=-1.000000|SyncGroupNameToRequireValidMarkersRule=None|bAllowInertializationForSelfTransitions=False|bSharedRules=False|bSharedCrossfade=False",
        duration: 0.5,
    },
    GroundedTransitionProfile {
        source_static_contract: "AnimStateTransitionNode_18|PriorityOrder=2|CrossfadeDuration=0.300000|BlendMode=HermiteCubic|bAutomaticRuleBasedOnSequencePlayerInState=False|AutomaticRuleTriggerTime=-1.000000|SyncGroupNameToRequireValidMarkersRule=None|bAllowInertializationForSelfTransitions=False|bSharedRules=False|bSharedCrossfade=False",
        duration: 0.3,
    },
    GroundedTransitionProfile {
        source_static_contract: "AnimStateTransitionNode_19|PriorityOrder=1|CrossfadeDuration=0.500000|BlendMode=Custom|bAutomaticRuleBasedOnSequencePlayerInState=False|AutomaticRuleTriggerTime=-1.000000|SyncGroupNameToRequireValidMarkersRule=None|bAllowInertializationForSelfTransitions=False|bSharedRules=False|bSharedCrossfade=False",
        duration: 0.5,
    },
    GroundedTransitionProfile {
        source_static_contract: "AnimStateTransitionNode_21|PriorityOrder=2|CrossfadeDuration=0.300000|BlendMode=HermiteCubic|bAutomaticRuleBasedOnSequencePlayerInState=False|AutomaticRuleTriggerTime=-1.000000|SyncGroupNameToRequireValidMarkersRule=None|bAllowInertializationForSelfTransitions=False|bSharedRules=False|bSharedCrossfade=False",
        duration: 0.3,
    },
    GroundedTransitionProfile {
        source_static_contract: "AnimStateTransitionNode_1|PriorityOrder=2|CrossfadeDuration=0.200000|BlendMode=HermiteCubic|bAutomaticRuleBasedOnSequencePlayerInState=False|AutomaticRuleTriggerTime=-1.000000|SyncGroupNameToRequireValidMarkersRule=None|bAllowInertializationForSelfTransitions=False|bSharedRules=False|bSharedCrossfade=False",
        duration: 0.2,
    },
    GroundedTransitionProfile {
        source_static_contract: "AnimStateTransitionNode_2|PriorityOrder=1|CrossfadeDuration=0.200000|BlendMode=HermiteCubic|bAutomaticRuleBasedOnSequencePlayerInState=False|AutomaticRuleTriggerTime=-1.000000|SyncGroupNameToRequireValidMarkersRule=None|bAllowInertializationForSelfTransitions=False|bSharedRules=False|bSharedCrossfade=False",
        duration: 0.2,
    },
    GroundedTransitionProfile {
        source_static_contract: "AnimStateTransitionNode_3|PriorityOrder=2|CrossfadeDuration=0.200000|BlendMode=HermiteCubic|bAutomaticRuleBasedOnSequencePlayerInState=False|AutomaticRuleTriggerTime=-1.000000|SyncGroupNameToRequireValidMarkersRule=None|bAllowInertializationForSelfTransitions=False|bSharedRules=False|bSharedCrossfade=False",
        duration: 0.2,
    },
    GroundedTransitionProfile {
        source_static_contract: "AnimStateTransitionNode_0|PriorityOrder=3|CrossfadeDuration=0.200000|BlendMode=HermiteCubic|bAutomaticRuleBasedOnSequencePlayerInState=False|AutomaticRuleTriggerTime=-1.000000|SyncGroupNameToRequireValidMarkersRule=None|bAllowInertializationForSelfTransitions=False|bSharedRules=False|bSharedCrossfade=False",
        duration: 0.2,
    },
];

impl GroundedTransition {
    fn profile(self) -> &'static GroundedTransitionProfile {
        &GROUNDED_TRANSITION_PROFILES[self.index() as usize]
    }

    fn index(self) -> f32 {
        match self {
            Self::EntryToRoll => 9.0,
            Self::EntryToCrouching => 10.0,
            Self::EntryToStanding => 8.0,
            Self::EntryFallbackToStanding => 11.0,
            Self::StandingToCrouching => 6.0,
            Self::StandingToStandingToCrouching => 7.0,
            Self::CrouchingToStanding => 4.0,
            Self::CrouchingToCrouchingToStanding => 5.0,
            Self::StandingToCrouchingAutomatic => 0.0,
            Self::StandingToCrouchingToStanding => 4.0,
            Self::StandingToCrouchingToCrouching => 6.0,
            Self::StandingToCrouchingToCrouchingToStanding => 5.0,
            Self::StandingToCrouchingSelf => 7.0,
            Self::CrouchingToStandingAutomatic => 1.0,
            Self::CrouchingToStandingToStanding => 4.0,
            Self::CrouchingToStandingToCrouching => 6.0,
            Self::CrouchingToStandingSelf => 5.0,
            Self::CrouchingToStandingToStandingToCrouching => 7.0,
            Self::RollToCrouching => 3.0,
            Self::RollToStanding => 2.0,
        }
    }

    fn target(self) -> GroundedState {
        match self {
            Self::EntryToRoll => GroundedState::Roll,
            Self::EntryToCrouching
            | Self::StandingToCrouching
            | Self::StandingToCrouchingAutomatic
            | Self::StandingToCrouchingToCrouching
            | Self::CrouchingToStandingToCrouching
            | Self::RollToCrouching => GroundedState::Crouching,
            Self::EntryToStanding
            | Self::EntryFallbackToStanding
            | Self::CrouchingToStanding
            | Self::CrouchingToStandingAutomatic
            | Self::StandingToCrouchingToStanding
            | Self::CrouchingToStandingToStanding
            | Self::RollToStanding => GroundedState::Standing,
            Self::StandingToStandingToCrouching
            | Self::CrouchingToStandingToStandingToCrouching => GroundedState::StandingToCrouching,
            Self::CrouchingToCrouchingToStanding
            | Self::StandingToCrouchingToCrouchingToStanding => GroundedState::CrouchingToStanding,
            Self::StandingToCrouchingSelf => GroundedState::StandingToCrouching,
            Self::CrouchingToStandingSelf => GroundedState::CrouchingToStanding,
        }
    }

    fn duration(self) -> f32 {
        let profile = self.profile();
        debug_assert!(!profile.source_static_contract.is_empty());
        profile.duration
    }

    fn uses_stance_change_curve(self) -> bool {
        matches!(
            self,
            Self::StandingToCrouching
                | Self::CrouchingToStanding
                | Self::StandingToCrouchingToStanding
                | Self::StandingToCrouchingToCrouching
                | Self::CrouchingToStandingToStanding
                | Self::CrouchingToStandingToCrouching
        )
    }
}

#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
struct GroundedLayerRuntime {
    initialized: bool,
    state: GroundedState,
    state_elapsed: f32,
    transition_elapsed: f32,
    transition_duration: f32,
    last_transition: Option<GroundedTransition>,
    entry_index: f32,
}

#[derive(Resource)]
struct GroundedCurveHandles {
    stand_to_crouch: Handle<AlsAnimCurves>,
    crouch_to_stand: Handle<AlsAnimCurves>,
    roll: Handle<AlsAnimCurves>,
    stance_change: Handle<CurveAsset>,
    graph: Handle<AnimationGraph>,
}

#[derive(Component, Debug, Default, Clone, Copy, PartialEq)]
pub struct AlsGroundedGraphTelemetry {
    pub actual_state: Option<u8>,
    pub transition_active: Option<bool>,
    pub pose_ready: Option<bool>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct GroundedCurves {
    pose_standing: f32,
    pose_crouching: f32,
    pose_moving: f32,
    rotation_yaw_offset: f32,
    foot_left_lock: f32,
    foot_right_lock: f32,
    allow_transitions: f32,
    rotation_yaw_speed: f32,
}

impl GroundedCurves {
    fn sample(assets: &Assets<AlsAnimCurves>, handle: &Handle<AlsAnimCurves>, time: f32) -> Self {
        let sample = |name| {
            assets
                .get(handle)
                .and_then(AlsAnimCurves::first)
                .and_then(|curves| curves.sample(name, time))
                .unwrap_or(0.0)
        };
        Self {
            pose_standing: sample("PoseStanding").clamp(0.0, 1.0),
            pose_crouching: sample("PoseCrouching").clamp(0.0, 1.0),
            pose_moving: sample("PoseMoving").clamp(0.0, 1.0),
            rotation_yaw_offset: sample("RotationYawOffset"),
            foot_left_lock: sample("FootLeftLock").clamp(0.0, 1.0),
            foot_right_lock: sample("FootRightLock").clamp(0.0, 1.0),
            allow_transitions: sample("AllowTransitions").clamp(0.0, 1.0),
            rotation_yaw_speed: sample("RotationYawSpeed"),
        }
    }
}

pub struct GroundedLayerPlugin;

impl Plugin for GroundedLayerPlugin {
    fn build(&self, app: &mut App) {
        app.configure_sets(
            PostUpdate,
            AlsGroundedLayerSet
                .after(AlsCrouchingLayerSet)
                .before(AnimationGraphSet::PrePhysics),
        )
        .add_systems(Startup, load_grounded_curve_assets)
        .add_systems(
            PostUpdate,
            grounded_layer_writer.in_set(AlsGroundedLayerSet),
        )
        .add_systems(
            PostUpdate,
            capture_grounded_graph_telemetry.after(AnimationGraphSet::Final),
        );
    }
}

fn load_grounded_curve_assets(mut commands: Commands, assets: Res<AssetServer>) {
    commands.insert_resource(GroundedCurveHandles {
        stand_to_crouch: assets.load(STAND_TO_CROUCH_CURVES),
        crouch_to_stand: assets.load(CROUCH_TO_STAND_CURVES),
        roll: assets.load(ROLL_CURVES),
        stance_change: assets.load(STANCE_CHANGE_CURVE),
        graph: assets.load(GROUNDED_GRAPH),
    });
}

fn horizontal_speed(velocity: Vec3) -> f32 {
    Vec2::new(velocity.x, velocity.z).length()
}

fn entry_transition(
    entry_mode: AlsGroundedEntryMode,
    crouching_amount: f32,
    standing_amount: f32,
    moving_or_rotating: bool,
) -> GroundedTransition {
    if entry_mode == AlsGroundedEntryMode::FromRoll && !moving_or_rotating {
        GroundedTransition::EntryToRoll
    } else if crouching_amount >= 0.5 {
        GroundedTransition::EntryToCrouching
    } else if standing_amount >= 0.5 {
        GroundedTransition::EntryToStanding
    } else {
        GroundedTransition::EntryFallbackToStanding
    }
}

fn next_transition(
    state: GroundedState,
    state_elapsed: f32,
    crouching: bool,
    moving_or_rotating: bool,
) -> Option<GroundedTransition> {
    match state {
        GroundedState::EntryConduit => None,
        GroundedState::Standing if crouching && moving_or_rotating => {
            Some(GroundedTransition::StandingToCrouching)
        }
        GroundedState::Standing if crouching => {
            Some(GroundedTransition::StandingToStandingToCrouching)
        }
        GroundedState::Crouching if !crouching && moving_or_rotating => {
            Some(GroundedTransition::CrouchingToStanding)
        }
        GroundedState::Crouching if !crouching => {
            Some(GroundedTransition::CrouchingToCrouchingToStanding)
        }
        GroundedState::StandingToCrouching if !crouching && moving_or_rotating => {
            Some(GroundedTransition::StandingToCrouchingToStanding)
        }
        GroundedState::StandingToCrouching if !crouching => {
            Some(GroundedTransition::StandingToCrouchingToCrouchingToStanding)
        }
        GroundedState::StandingToCrouching if moving_or_rotating => {
            Some(GroundedTransition::StandingToCrouchingToCrouching)
        }
        GroundedState::StandingToCrouching if state_elapsed >= STANCE_CLIP_PLAY_DURATION => {
            Some(GroundedTransition::StandingToCrouchingAutomatic)
        }
        GroundedState::CrouchingToStanding if crouching && moving_or_rotating => {
            Some(GroundedTransition::CrouchingToStandingToCrouching)
        }
        GroundedState::CrouchingToStanding if crouching => {
            Some(GroundedTransition::CrouchingToStandingToStandingToCrouching)
        }
        GroundedState::CrouchingToStanding if moving_or_rotating => {
            Some(GroundedTransition::CrouchingToStandingToStanding)
        }
        GroundedState::CrouchingToStanding if state_elapsed >= STANCE_CLIP_PLAY_DURATION => {
            Some(GroundedTransition::CrouchingToStandingAutomatic)
        }
        GroundedState::Roll if crouching => Some(GroundedTransition::RollToCrouching),
        GroundedState::Roll => Some(GroundedTransition::RollToStanding),
        _ => None,
    }
}

fn advance_runtime(
    runtime: &mut GroundedLayerRuntime,
    delta_seconds: f32,
    entry_mode: AlsGroundedEntryMode,
    crouching_amount: f32,
    standing_amount: f32,
    crouching: bool,
    moving_or_rotating: bool,
) -> Option<GroundedTransition> {
    let delta_seconds = delta_seconds.max(0.0);
    runtime.state_elapsed += delta_seconds;
    runtime.transition_elapsed =
        (runtime.transition_elapsed + delta_seconds).min(runtime.transition_duration.max(0.0));
    let transition = if !runtime.initialized {
        runtime.initialized = true;
        let transition = entry_transition(
            entry_mode,
            crouching_amount,
            standing_amount,
            moving_or_rotating,
        );
        runtime.entry_index = transition.index();
        Some(transition)
    } else {
        next_transition(
            runtime.state,
            runtime.state_elapsed,
            crouching,
            moving_or_rotating,
        )
    };
    if let Some(transition) = transition {
        runtime.state = transition.target();
        runtime.state_elapsed = 0.0;
        runtime.transition_elapsed = 0.0;
        runtime.transition_duration = transition.duration();
        runtime.last_transition = Some(transition);
    }
    transition
}

fn current_transition_alpha(
    runtime: GroundedLayerRuntime,
    curves: &Assets<CurveAsset>,
    handles: &GroundedCurveHandles,
) -> f32 {
    let Some(transition) = runtime.last_transition else {
        return 0.0;
    };
    if !transition.uses_stance_change_curve() || runtime.transition_duration <= f32::EPSILON {
        return (runtime.transition_elapsed / runtime.transition_duration.max(f32::EPSILON))
            .clamp(0.0, 1.0);
    }
    let progress = (runtime.transition_elapsed / runtime.transition_duration).clamp(0.0, 1.0);
    curves
        .get(&handles.stance_change)
        .map(|curve| curve.sample(progress))
        .unwrap_or(progress)
        .clamp(0.0, 1.0)
}

fn resets_entry_mode(request: Option<GroundedTransition>, entry_mode_missing: bool) -> bool {
    entry_mode_missing
        || request.is_some_and(|transition| {
            matches!(
                transition.target(),
                GroundedState::Standing | GroundedState::Crouching | GroundedState::Roll
            )
        })
}

fn transition_curves(
    runtime: GroundedLayerRuntime,
    curves: &Assets<AlsAnimCurves>,
    handles: &GroundedCurveHandles,
    roll_clip_time: Option<f32>,
) -> GroundedCurves {
    match runtime.state {
        GroundedState::StandingToCrouching => GroundedCurves::sample(
            curves,
            &handles.stand_to_crouch,
            runtime.state_elapsed * STANCE_CLIP_PLAY_RATE,
        ),
        GroundedState::CrouchingToStanding => GroundedCurves::sample(
            curves,
            &handles.crouch_to_stand,
            runtime.state_elapsed * STANCE_CLIP_PLAY_RATE,
        ),
        GroundedState::Roll => {
            let mut values = GroundedCurves::sample(
                curves,
                &handles.roll,
                roll_clip_time.unwrap_or(runtime.state_elapsed),
            );
            values.foot_left_lock = 1.0;
            values.foot_right_lock = 1.0;
            values
        }
        GroundedState::EntryConduit | GroundedState::Standing | GroundedState::Crouching => {
            GroundedCurves::default()
        }
    }
}

fn global_f32(player: &AnimationGraphPlayer, key: &str) -> f32 {
    player
        .get_global_input_data(key)
        .and_then(|value| value.as_f32().ok())
        .unwrap_or(0.0)
}

fn write_globals(
    player: &mut AnimationGraphPlayer,
    runtime: GroundedLayerRuntime,
    request: Option<GroundedTransition>,
    values: GroundedCurves,
    stance_alpha: f32,
    delta_seconds: f32,
) {
    for (key, value) in [
        (
            "als_grounded_transition_index",
            request.map_or(-1.0, GroundedTransition::index),
        ),
        ("als_grounded_state_index", runtime.state.index()),
        ("als_grounded_entry_index", runtime.entry_index),
        ("als_grounded_clip_index", runtime.state.clip_index()),
        ("als_grounded_loop", runtime.state.looped()),
        ("als_grounded_play_rate", runtime.state.play_rate()),
        (
            "als_grounded_transition_duration",
            runtime.transition_duration,
        ),
        ("als_grounded_stance_change_alpha", stance_alpha),
        ("als_grounded_curve_PoseStanding", values.pose_standing),
        ("als_grounded_curve_PoseCrouching", values.pose_crouching),
        ("als_grounded_curve_PoseMoving", values.pose_moving),
        (
            "als_grounded_curve_RotationYawOffset",
            values.rotation_yaw_offset,
        ),
        ("als_grounded_curve_FootLeftLock", values.foot_left_lock),
        ("als_grounded_curve_FootRightLock", values.foot_right_lock),
        (
            "als_grounded_curve_AllowTransitions",
            values.allow_transitions,
        ),
        (
            "als_grounded_curve_RotationYawSpeed",
            values.rotation_yaw_speed,
        ),
        ("als_grounded_roll_explicit_frame", 45.0),
        (GROUNDED_INTERNAL_TIME_KEY, delta_seconds.max(0.0)),
        ("als_grounded_curve_PoseGroundedInput", 0.0),
        ("als_grounded_curve_FootLeftIkInput", 0.0),
        ("als_grounded_curve_FootRightIkInput", 0.0),
    ] {
        player.set_global_input_data(key, value);
    }
}

#[allow(clippy::type_complexity)]
fn grounded_layer_writer(
    time: Res<Time>,
    mut commands: Commands,
    characters: Query<
        (
            Option<&Stance>,
            Option<&AlsGroundedEntryMode>,
            Option<&AlsRollState>,
            &LinearVelocity,
        ),
        With<AlsCharacter>,
    >,
    mut avatars: Query<
        (
            Entity,
            &ChildOf,
            &AnimatedSceneInstance,
            Option<&mut GroundedLayerRuntime>,
            Option<&AlsTurnDrive>,
        ),
        With<AlsCharacterAvatar>,
    >,
    curves: Res<Assets<CurveAsset>>,
    animation_curves: Res<Assets<AlsAnimCurves>>,
    handles: Res<GroundedCurveHandles>,
    mut players: Query<&mut AnimationGraphPlayer>,
) {
    for (avatar, parent, instance, current_runtime, turn) in &mut avatars {
        let Ok((stance, entry_mode, roll, velocity)) = characters.get(parent.parent()) else {
            continue;
        };
        let Ok(mut player) = players.get_mut(instance.player_entity()) else {
            continue;
        };
        let standing_amount = global_f32(&player, "als_standing_curve_pose_standing");
        let crouching_amount = global_f32(&player, "als_crouching_curve_pose_crouching");
        let moving_or_rotating = horizontal_speed(velocity.0) >= MOVING_SMOOTH_SPEED_THRESHOLD
            || turn.is_some_and(|drive| drive.rotate_active);
        let mut runtime = current_runtime.as_deref().copied().unwrap_or_default();
        let roll_clip_time = roll
            .filter(|roll| roll.active)
            .map(|roll| roll.time * ROLL_PLAY_RATE);
        let request = if let Some(roll_clip_time) = roll_clip_time {
            let entered = runtime.state != GroundedState::Roll;
            runtime.initialized = true;
            runtime.state = GroundedState::Roll;
            runtime.state_elapsed = roll_clip_time;
            runtime.transition_elapsed = 0.0;
            runtime.transition_duration = GroundedTransition::EntryToRoll.duration();
            if entered {
                runtime.entry_index = GroundedTransition::EntryToRoll.index();
                runtime.last_transition = Some(GroundedTransition::EntryToRoll);
                Some(GroundedTransition::EntryToRoll)
            } else {
                None
            }
        } else {
            advance_runtime(
                &mut runtime,
                time.delta_secs(),
                entry_mode.copied().unwrap_or_default(),
                crouching_amount,
                standing_amount,
                stance.is_some_and(|stance| *stance == Stance::Crouching),
                moving_or_rotating,
            )
        };
        if resets_entry_mode(request, entry_mode.is_none()) {
            commands
                .entity(parent.parent())
                .insert(AlsGroundedEntryMode::None);
        }
        let values = transition_curves(runtime, &animation_curves, &handles, roll_clip_time);
        let stance_alpha = current_transition_alpha(runtime, &curves, &handles);
        write_globals(
            &mut player,
            runtime,
            request,
            values,
            stance_alpha,
            time.delta_secs(),
        );
        if let Some(mut current_runtime) = current_runtime {
            *current_runtime = runtime;
        } else {
            commands.entity(avatar).insert(runtime);
        }
    }
}

fn grounded_state_index(label: &str) -> Option<u8> {
    match label {
        "Entry Conduit" => Some(0),
        "Standing" => Some(1),
        "Crouching" => Some(2),
        "Standing To Crouching" => Some(3),
        "Crouching To Standing" => Some(4),
        "Roll" => Some(5),
        _ => None,
    }
}

fn actual_grounded_fsm_telemetry(
    player: &AnimationGraphPlayer,
    graph: &AnimationGraph,
    graph_handle: &Handle<AnimationGraph>,
    graph_assets: &Assets<AnimationGraph>,
    state_machines: &Assets<StateMachine>,
) -> Option<(Option<u8>, bool, bool)> {
    let node = graph
        .nodes
        .values()
        .find(|node| node.name == "Grounded States")?;
    let (fsm_handle, fsm_node, fsm_graph) =
        if let Some(fsm) = node.try_inner_downcast_ref::<InterruptibleFsmNode>() {
            (fsm.fsm.clone(), node.id, graph_handle.id())
        } else {
            let layered = node.try_inner_downcast_ref::<LayeredGraphNode>()?;
            let runtime_graph = graph_assets.get(&layered.graph)?;
            let runtime_node = runtime_graph.nodes.values().find(|node| {
                node.try_inner_downcast_ref::<InterruptibleFsmNode>()
                    .is_some()
            })?;
            let fsm = runtime_node.try_inner_downcast_ref::<InterruptibleFsmNode>()?;
            (fsm.fsm.clone(), runtime_node.id, layered.graph.id())
        };
    let machine = state_machines.get(&fsm_handle)?;
    let arena = player.get_context_arena()?;
    arena.iter_context_ids().find_map(|context_id| {
        let context = arena.get_context(context_id)?;
        if context.get_graph_id() != fsm_graph {
            return None;
        }
        let state = context
            .node_states
            .get::<InterruptibleFsmState>(fsm_node, StateKey::Default)
            .ok()?;
        let actual_state = match &state.state {
            LowLevelStateId::HlState(state) => machine
                .states
                .get(state)
                .and_then(|state| grounded_state_index(&state.label)),
            LowLevelStateId::HlTransition(_) => None,
        };
        Some((
            actual_state,
            matches!(state.state, LowLevelStateId::HlTransition(_)),
            state.last_output_pose().is_some(),
        ))
    })
}

fn capture_grounded_graph_telemetry(
    mut commands: Commands,
    avatars: Query<(&ChildOf, &AnimatedSceneInstance), With<AlsCharacterAvatar>>,
    players: Query<&AnimationGraphPlayer>,
    graph_assets: Res<Assets<AnimationGraph>>,
    state_machines: Res<Assets<StateMachine>>,
    handles: Res<GroundedCurveHandles>,
    mut characters: Query<Option<&mut AlsGroundedGraphTelemetry>, With<AlsCharacter>>,
) {
    for (parent, instance) in &avatars {
        let Ok(player) = players.get(instance.player_entity()) else {
            continue;
        };
        let actual = graph_assets.get(&handles.graph).and_then(|graph| {
            actual_grounded_fsm_telemetry(
                player,
                graph,
                &handles.graph,
                &graph_assets,
                &state_machines,
            )
        });
        let telemetry = AlsGroundedGraphTelemetry {
            actual_state: actual.and_then(|value| value.0),
            transition_active: actual.map(|value| value.1),
            pose_ready: actual.map(|value| value.2),
        };
        let Ok(current) = characters.get_mut(parent.parent()) else {
            continue;
        };
        if let Some(mut current) = current {
            *current = telemetry;
        } else {
            commands.entity(parent.parent()).insert(telemetry);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entry_priority_matches_the_source_conduit() {
        assert_eq!(
            entry_transition(AlsGroundedEntryMode::FromRoll, 1.0, 1.0, false),
            GroundedTransition::EntryToRoll
        );
        assert_eq!(
            entry_transition(AlsGroundedEntryMode::None, 0.5, 1.0, false),
            GroundedTransition::EntryToCrouching
        );
        assert_eq!(
            entry_transition(AlsGroundedEntryMode::None, 0.0, 0.5, false),
            GroundedTransition::EntryToStanding
        );
        assert_eq!(
            entry_transition(AlsGroundedEntryMode::None, 0.0, 0.0, false),
            GroundedTransition::EntryFallbackToStanding
        );
    }

    #[test]
    fn aliases_use_direct_when_moving_and_inertial_when_stationary() {
        assert_eq!(
            next_transition(GroundedState::Standing, 0.0, true, true),
            Some(GroundedTransition::StandingToCrouching)
        );
        assert_eq!(
            next_transition(GroundedState::Standing, 0.0, true, false),
            Some(GroundedTransition::StandingToStandingToCrouching)
        );
        assert_eq!(
            next_transition(GroundedState::Crouching, 0.0, false, true),
            Some(GroundedTransition::CrouchingToStanding)
        );
        assert_eq!(
            next_transition(GroundedState::Crouching, 0.0, false, false),
            Some(GroundedTransition::CrouchingToCrouchingToStanding)
        );
    }

    #[test]
    fn stance_clip_automatic_edges_wait_for_source_play_duration() {
        assert_eq!(
            next_transition(
                GroundedState::StandingToCrouching,
                STANCE_CLIP_PLAY_DURATION - 0.001,
                true,
                false,
            ),
            None
        );
        assert_eq!(
            next_transition(
                GroundedState::StandingToCrouching,
                STANCE_CLIP_PLAY_DURATION,
                true,
                false,
            ),
            Some(GroundedTransition::StandingToCrouchingAutomatic)
        );
    }

    #[test]
    fn roll_is_an_entry_bridge_with_source_exit_edges() {
        assert_eq!(
            next_transition(GroundedState::Roll, 0.0, true, false),
            Some(GroundedTransition::RollToCrouching)
        );
        assert_eq!(
            next_transition(GroundedState::Roll, 0.0, false, false),
            Some(GroundedTransition::RollToStanding)
        );
        assert_eq!(GroundedTransition::RollToStanding.duration(), 0.6);
    }

    #[test]
    fn stable_targets_clear_the_source_entry_mode() {
        assert!(resets_entry_mode(
            Some(GroundedTransition::EntryToRoll),
            false
        ));
        assert!(resets_entry_mode(
            Some(GroundedTransition::StandingToCrouching),
            false
        ));
        assert!(!resets_entry_mode(
            Some(GroundedTransition::StandingToStandingToCrouching),
            false
        ));
        assert!(resets_entry_mode(None, true));
    }

    #[test]
    fn grounded_graph_telemetry_labels_match_the_loaded_fsm() {
        assert_eq!(grounded_state_index("Entry Conduit"), Some(0));
        assert_eq!(grounded_state_index("Standing"), Some(1));
        assert_eq!(grounded_state_index("Crouching"), Some(2));
        assert_eq!(grounded_state_index("Standing To Crouching"), Some(3));
        assert_eq!(grounded_state_index("Crouching To Standing"), Some(4));
        assert_eq!(grounded_state_index("Roll"), Some(5));
        assert_eq!(grounded_state_index("Unknown"), None);
    }

    #[test]
    fn grounded_transition_profiles_cover_the_source_fsm_order() {
        assert_eq!(GROUNDED_TRANSITION_PROFILES.len(), 12);
        assert!(
            GROUNDED_TRANSITION_PROFILES[0]
                .source_static_contract
                .starts_with("AnimStateTransitionNode_20|")
        );
        assert!(
            GROUNDED_TRANSITION_PROFILES[11]
                .source_static_contract
                .starts_with("AnimStateTransitionNode_0|")
        );
        assert_eq!(GroundedTransition::RollToStanding.duration(), 0.6);
        assert_eq!(GroundedTransition::EntryToStanding.duration(), 0.2);
    }

    #[test]
    fn grounded_runtime_fsm_has_a_writer_backed_internal_time_contract() {
        let runtime =
            include_str!("../assets/graphs/ab_als/grounded/grounded_runtime.animgraph.ron");
        assert!(runtime.contains(&format!(
            "internal_time_key: \"{GROUNDED_INTERNAL_TIME_KEY}\""
        )));
        let mut player = AnimationGraphPlayer::default();
        write_globals(
            &mut player,
            GroundedLayerRuntime::default(),
            None,
            GroundedCurves::default(),
            0.0,
            1.0 / 60.0,
        );
        assert_eq!(
            player
                .get_global_input_data(GROUNDED_INTERNAL_TIME_KEY)
                .and_then(|value| value.as_f32().ok()),
            Some(1.0 / 60.0)
        );
    }

    #[test]
    fn grounded_cached_state_proxies_supply_the_opposite_stance_curve_default() {
        for (graph, curve) in [
            (
                include_str!(
                    "../assets/graphs/ab_als/grounded/state_entry_conduit_runtime.animgraph.ron"
                ),
                "PoseCrouching",
            ),
            (
                include_str!(
                    "../assets/graphs/ab_als/grounded/state_alias_standing_runtime.animgraph.ron"
                ),
                "PoseCrouching",
            ),
            (
                include_str!("../assets/graphs/ab_als/grounded/state_standing.animgraph.ron"),
                "PoseCrouching",
            ),
            (
                include_str!(
                    "../assets/graphs/ab_als/grounded/state_alias_crouching_runtime.animgraph.ron"
                ),
                "PoseStanding",
            ),
            (
                include_str!("../assets/graphs/ab_als/grounded/state_crouching.animgraph.ron"),
                "PoseStanding",
            ),
        ] {
            assert!(graph.contains(&format!(
                "context_defaults: [(curve: \"{curve}\", value: 0.0)]"
            )));
        }
    }

    #[test]
    fn grounded_inertialization_defaults_curves_before_the_visible_modifier() {
        let graph = include_str!("../assets/graphs/ab_als/grounded.animgraph.ron");
        assert!(graph.contains("missing_curve_value: Some(0.0)"));
        assert!(graph.contains("pin_defaults: [1.0, 1.0, 1.0]"));
    }
}
