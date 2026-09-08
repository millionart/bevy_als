use als_core::character::{AlsCharacter, AlsMovementParamsHandle};
use als_core::dynamics::AlsGroundedDynamics;
use als_core::foot_ik::AlsFootLockInput;
use als_core::input::AlsInputCommand;
use als_core::motion::{AlsInputOverride, AlsRotationAnimationState, AlsViewYaw};
use als_core::movement_direction::{
    AlsMovementDirection, AlsMovementDirectionState, advance_standing_movement_state,
    classify_grounded_movement_intent,
};
use als_core::params::AlsMovementParams;
use als_core::state::{Gait, LocomotionState, RotationMode, Stance};
use std::{path::Path, sync::LazyLock};

use als_ext::{
    AlphaBlendMode, AlsAnimCurves, InterruptibleFsmNode, InterruptibleFsmState,
    SyncMarker, SyncMarkerPhaseInput, SyncPhaseState, advance_marker_sync_phase,
    build_branch_bone_mask, marker_phase_sample_time, transition_alpha,
};
use bevy::ecs::hierarchy::ChildOf;
use bevy::ecs::message::{Message, MessageReader};
use bevy::prelude::*;
use bevy_animation_graph::core::animated_scene::AnimatedSceneInstance;
use bevy_animation_graph::core::animation_graph::AnimationGraph;
use bevy_animation_graph::core::animation_graph_player::AnimationGraphPlayer;
use bevy_animation_graph::core::edge_data::{DataValue, bone_mask::BoneMask};
use bevy_animation_graph::core::plugin::AnimationGraphSet;
use bevy_animation_graph::core::skeleton::Skeleton;
use bevy_animation_graph::core::state_machine::{
    high_level::StateMachine, low_level::LowLevelStateId,
};

use crate::actual_motion::AlsActualMotion;
use crate::animation_context_projection::AlsRootAnimationContext;
use crate::animation_events::AlsStandingEventClock;
use crate::character_assembly::AlsCharacterAvatar;
use crate::direction_layer::view_space_direction_angle_from_radians;
use crate::standing_source::{standing_rotation_yaw_offsets, standing_stride_amount};
use crate::stop_link::{AlsQuickStopNotify, AlsTransitionKind, AlsTransitionPlayback};
use crate::turn_layer::{AlsTurnDrive, AlsTurnMontageStack};

const VELOCITY_EPSILON: f32 = 0.01;
const MOVING_SMOOTH_SPEED_THRESHOLD: f32 = 1.5;
const VELOCITY_BLEND_HALF_LIFE: f32 = 0.1;
const LEAN_HALF_LIFE: f32 = 0.2;
const MIN_PLAY_RATE: f32 = 1e-4;
const RUN_START_DURATION: f32 = 0.72;
const RUN_START_FROM_WALK_DURATION: f32 = 0.68;
const PIVOT_DURATION: f32 = 0.6;
const ROTATE_COMPLETION_EPSILON: f32 = 1e-4;
const MOVEMENT_BLENDSPACE_START_PHASE: f32 = 0.2;
const SPRINT_START_TIME: f32 = 0.166667;
const SPRINT_ACCELERATION_RATE_SCALE: f32 = 0.833333;
const SPRINT_DURATION_SECS: f32 = 0.6;
const GENERATED_STANDING_GRAPH: &str =
    "generated/ALS/Character/AnimationInstances/Stances/AB_Als_Standing/AnimGraph.animgraph.ron";

fn is_generated_standing_graph(path: &Path) -> bool {
    path == Path::new(GENERATED_STANDING_GRAPH)
}

pub(crate) fn source_moving_smooth(has_input: bool, horizontal_speed: f32) -> bool {
    (has_input && horizontal_speed >= VELOCITY_EPSILON)
        || horizontal_speed > MOVING_SMOOTH_SPEED_THRESHOLD
}

fn marker_set(entries: &[(&str, f32)]) -> Vec<SyncMarker> {
    entries
        .iter()
        .map(|(name, time)| SyncMarker {
            name: (*name).to_owned(),
            time: *time,
        })
        .collect()
}

static MOVEMENT_PHASE_MARKERS: LazyLock<Vec<SyncMarker>> =
    LazyLock::new(|| marker_set(&[("Left", 0.0), ("Right", 0.5)]));
static WALK_FORWARD_MARKERS: LazyLock<Vec<SyncMarker>> =
    LazyLock::new(|| marker_set(&[("Left", 0.083333), ("Right", 0.65)]));
static RUN_FORWARD_MARKERS: LazyLock<Vec<SyncMarker>> =
    LazyLock::new(|| marker_set(&[("Left", 0.033333), ("Right", 0.433333)]));
static WALK_RIGHT_FORWARD_MARKERS: LazyLock<Vec<SyncMarker>> =
    LazyLock::new(|| marker_set(&[("Left", 0.091667), ("Right", 0.65)]));
static RUN_RIGHT_FORWARD_MARKERS: LazyLock<Vec<SyncMarker>> =
    LazyLock::new(|| marker_set(&[("Left", 0.05), ("Right", 0.433333)]));
static WALK_RIGHT_BACKWARD_MARKERS: LazyLock<Vec<SyncMarker>> =
    LazyLock::new(|| marker_set(&[("Right", 0.491667), ("Left", 1.075)]));
static RUN_RIGHT_BACKWARD_MARKERS: LazyLock<Vec<SyncMarker>> =
    LazyLock::new(|| marker_set(&[("Right", 0.316667), ("Left", 0.65)]));
static WALK_BACKWARD_MARKERS: LazyLock<Vec<SyncMarker>> =
    LazyLock::new(|| marker_set(&[("Right", 0.516667), ("Left", 1.083333)]));
static RUN_BACKWARD_MARKERS: LazyLock<Vec<SyncMarker>> =
    LazyLock::new(|| marker_set(&[("Left", 0.05), ("Right", 0.383333)]));
static WALK_LEFT_BACKWARD_MARKERS: LazyLock<Vec<SyncMarker>> =
    LazyLock::new(|| marker_set(&[("Right", 0.508333), ("Left", 1.058333)]));
static RUN_LEFT_BACKWARD_MARKERS: LazyLock<Vec<SyncMarker>> =
    LazyLock::new(|| marker_set(&[("Right", 0.316667), ("Left", 0.65)]));
static WALK_LEFT_FORWARD_MARKERS: LazyLock<Vec<SyncMarker>> =
    LazyLock::new(|| marker_set(&[("Left", 0.083333), ("Right", 0.658333)]));
static RUN_LEFT_FORWARD_MARKERS: LazyLock<Vec<SyncMarker>> =
    LazyLock::new(|| marker_set(&[("Left", 0.033333), ("Right", 0.45)]));
static SPRINT_MARKERS: LazyLock<Vec<SyncMarker>> =
    LazyLock::new(|| marker_set(&[("Left", 0.016667), ("Right", 0.316667)]));

const STANDING_SKELETON: &str = "skeletons/SKM_Als.skn.ron";
const STOP_LEFT_LEG_BRANCHES: &[&str] = &["thigh_l", "ik_foot_l", "VB foot_l"];
const STOP_RIGHT_LEG_BRANCHES: &[&str] = &["thigh_r", "ik_foot_r", "VB foot_r"];
const ROTATE_LEFT_CURVE_ASSET: &str =
    "animations/RotateInPlace/A_Als_Rotate_90_Left.gltf#AlsCurves";
const ROTATE_RIGHT_CURVE_ASSET: &str =
    "animations/RotateInPlace/A_Als_Rotate_90_Right.gltf#AlsCurves";
const TURN_LEFT_90_CURVE_ASSET: &str = "animations/TurnInPlace/A_Als_Turn_90_Left.gltf#AlsCurves";
const TURN_RIGHT_90_CURVE_ASSET: &str = "animations/TurnInPlace/A_Als_Turn_90_Right.gltf#AlsCurves";
const TURN_LEFT_180_CURVE_ASSET: &str = "animations/TurnInPlace/A_Als_Turn_180_Left.gltf#AlsCurves";
const TURN_RIGHT_180_CURVE_ASSET: &str =
    "animations/TurnInPlace/A_Als_Turn_180_Right.gltf#AlsCurves";
const QUICK_STOP_LEFT_CURVE_ASSET: &str = "animations/Transitions/A_Als_Stop_Left.gltf#AlsCurves";
const QUICK_STOP_RIGHT_CURVE_ASSET: &str = "animations/Transitions/A_Als_Stop_Right.gltf#AlsCurves";
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AlsStandingLayerSet;

#[derive(Message, Debug, Clone, Copy, PartialEq)]
pub struct AlsPivotNotify {
    pub character: Entity,
}

#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq)]
struct AlsPivotRequest;

#[derive(Resource)]
pub(crate) struct StandingCurveHandles {
    rotate_left: Handle<AlsAnimCurves>,
    rotate_right: Handle<AlsAnimCurves>,
    turn_left_90: Handle<AlsAnimCurves>,
    turn_right_90: Handle<AlsAnimCurves>,
    turn_left_180: Handle<AlsAnimCurves>,
    turn_right_180: Handle<AlsAnimCurves>,
    quick_stop_left: Handle<AlsAnimCurves>,
    quick_stop_right: Handle<AlsAnimCurves>,
    skeleton: Handle<Skeleton>,
    stop_left_leg_mask: Option<BoneMask>,
    stop_right_leg_mask: Option<BoneMask>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct VelocityBlend {
    forward: f32,
    backward: f32,
    right: f32,
    left: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum StandingDetail {
    #[default]
    Walk,
    Run,
    SecondPivot,
    RunStartFromWalk,
    RunStart,
    FirstPivot,
}

#[derive(Clone, Copy, Debug)]
struct StandingTransitionStaticProfile {
    source_static_contract: &'static str,
    duration: f32,
}

const STANDING_DETAIL_TRANSITION_PROFILES: [StandingTransitionStaticProfile; 11] = [
    StandingTransitionStaticProfile {
        source_static_contract: "AnimStateTransitionNode_0|PriorityOrder=1|CrossfadeDuration=0.200000|BlendMode=HermiteCubic|bAutomaticRuleBasedOnSequencePlayerInState=False|AutomaticRuleTriggerTime=-1.000000|SyncGroupNameToRequireValidMarkersRule=None|bAllowInertializationForSelfTransitions=False|bSharedRules=False|bSharedCrossfade=False",
        duration: 0.2,
    },
    StandingTransitionStaticProfile {
        source_static_contract: "AnimStateTransitionNode_1|PriorityOrder=1|CrossfadeDuration=0.100000|BlendMode=HermiteCubic|bAutomaticRuleBasedOnSequencePlayerInState=False|AutomaticRuleTriggerTime=-1.000000|SyncGroupNameToRequireValidMarkersRule=None|bAllowInertializationForSelfTransitions=False|bSharedRules=True|bSharedCrossfade=False",
        duration: 0.1,
    },
    StandingTransitionStaticProfile {
        source_static_contract: "AnimStateTransitionNode_2|PriorityOrder=1|CrossfadeDuration=0.100000|BlendMode=HermiteCubic|bAutomaticRuleBasedOnSequencePlayerInState=False|AutomaticRuleTriggerTime=-1.000000|SyncGroupNameToRequireValidMarkersRule=None|bAllowInertializationForSelfTransitions=False|bSharedRules=True|bSharedCrossfade=False",
        duration: 0.1,
    },
    StandingTransitionStaticProfile {
        source_static_contract: "AnimStateTransitionNode_3|PriorityOrder=1|CrossfadeDuration=0.200000|BlendMode=HermiteCubic|bAutomaticRuleBasedOnSequencePlayerInState=True|AutomaticRuleTriggerTime=0.000000|SyncGroupNameToRequireValidMarkersRule=None|bAllowInertializationForSelfTransitions=False|bSharedRules=False|bSharedCrossfade=False",
        duration: 0.2,
    },
    StandingTransitionStaticProfile {
        source_static_contract: "AnimStateTransitionNode_7|PriorityOrder=1|CrossfadeDuration=0.200000|BlendMode=HermiteCubic|bAutomaticRuleBasedOnSequencePlayerInState=True|AutomaticRuleTriggerTime=0.000000|SyncGroupNameToRequireValidMarkersRule=None|bAllowInertializationForSelfTransitions=False|bSharedRules=False|bSharedCrossfade=False",
        duration: 0.2,
    },
    StandingTransitionStaticProfile {
        source_static_contract: "AnimStateTransitionNode_8|PriorityOrder=1|CrossfadeDuration=0.100000|BlendMode=HermiteCubic|bAutomaticRuleBasedOnSequencePlayerInState=False|AutomaticRuleTriggerTime=-1.000000|SyncGroupNameToRequireValidMarkersRule=None|bAllowInertializationForSelfTransitions=False|bSharedRules=False|bSharedCrossfade=False",
        duration: 0.1,
    },
    StandingTransitionStaticProfile {
        source_static_contract: "AnimStateTransitionNode_9|PriorityOrder=1|CrossfadeDuration=0.100000|BlendMode=HermiteCubic|bAutomaticRuleBasedOnSequencePlayerInState=False|AutomaticRuleTriggerTime=-1.000000|SyncGroupNameToRequireValidMarkersRule=None|bAllowInertializationForSelfTransitions=False|bSharedRules=False|bSharedCrossfade=False",
        duration: 0.1,
    },
    StandingTransitionStaticProfile {
        source_static_contract: "AnimStateTransitionNode_15|PriorityOrder=1|CrossfadeDuration=0.100000|BlendMode=HermiteCubic|bAutomaticRuleBasedOnSequencePlayerInState=False|AutomaticRuleTriggerTime=-1.000000|SyncGroupNameToRequireValidMarkersRule=None|bAllowInertializationForSelfTransitions=False|bSharedRules=False|bSharedCrossfade=False",
        duration: 0.1,
    },
    StandingTransitionStaticProfile {
        source_static_contract: "AnimStateTransitionNode_16|PriorityOrder=1|CrossfadeDuration=0.100000|BlendMode=HermiteCubic|bAutomaticRuleBasedOnSequencePlayerInState=True|AutomaticRuleTriggerTime=0.000000|SyncGroupNameToRequireValidMarkersRule=None|bAllowInertializationForSelfTransitions=False|bSharedRules=False|bSharedCrossfade=False",
        duration: 0.1,
    },
    StandingTransitionStaticProfile {
        source_static_contract: "AnimStateTransitionNode_32|PriorityOrder=1|CrossfadeDuration=0.100000|BlendMode=HermiteCubic|bAutomaticRuleBasedOnSequencePlayerInState=False|AutomaticRuleTriggerTime=-1.000000|SyncGroupNameToRequireValidMarkersRule=None|bAllowInertializationForSelfTransitions=False|bSharedRules=False|bSharedCrossfade=False",
        duration: 0.1,
    },
    StandingTransitionStaticProfile {
        source_static_contract: "AnimStateTransitionNode_33|PriorityOrder=1|CrossfadeDuration=0.200000|BlendMode=HermiteCubic|bAutomaticRuleBasedOnSequencePlayerInState=True|AutomaticRuleTriggerTime=0.000000|SyncGroupNameToRequireValidMarkersRule=None|bAllowInertializationForSelfTransitions=False|bSharedRules=False|bSharedCrossfade=False",
        duration: 0.2,
    },
];

impl StandingDetail {
    fn index(self) -> f32 {
        match self {
            Self::Walk => 0.0,
            Self::Run => 1.0,
            Self::SecondPivot => 2.0,
            Self::RunStartFromWalk => 3.0,
            Self::RunStart => 4.0,
            Self::FirstPivot => 5.0,
        }
    }

    fn transition_index(self, target: Self) -> Option<f32> {
        match (self, target) {
            (Self::Walk, Self::Run) => Some(0.0),
            (Self::FirstPivot, Self::SecondPivot) => Some(1.0),
            (Self::SecondPivot, Self::FirstPivot) => Some(2.0),
            (Self::SecondPivot, Self::Run) => Some(3.0),
            (Self::RunStart, Self::Run) => Some(4.0),
            (Self::Walk, Self::RunStart) => Some(5.0),
            (Self::Walk, Self::RunStartFromWalk) => Some(6.0),
            (Self::Run, Self::Walk) => Some(7.0),
            (Self::RunStartFromWalk, Self::Run) => Some(8.0),
            (Self::Run, Self::FirstPivot) => Some(9.0),
            (Self::FirstPivot, Self::Run) => Some(10.0),
            _ => None,
        }
    }

    fn transition_duration(index: f32) -> f32 {
        let profile = STANDING_DETAIL_TRANSITION_PROFILES
            .get(index as usize)
            .copied()
            .unwrap_or(StandingTransitionStaticProfile {
                source_static_contract: "",
                duration: 0.2,
            });
        debug_assert!(!profile.source_static_contract.is_empty() || index < 0.0);
        profile.duration
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum StandingMovementState {
    #[default]
    Forward,
    Backward,
    RightForward,
    RightBackward,
    LeftForward,
    LeftBackward,
}

#[derive(Clone, Copy, Debug)]
struct StandingMovementTransitionProfile {
    source_static_contract: &'static str,
    duration: f32,
    blend_mode: AlphaBlendMode,
}

const STANDING_MOVEMENT_TRANSITION_PROFILES: [StandingMovementTransitionProfile; 24] = [
    StandingMovementTransitionProfile {
        source_static_contract: "AnimStateTransitionNode_0|PriorityOrder=1|bSharedRules=True",
        duration: 0.5,
        blend_mode: AlphaBlendMode::Cubic,
    },
    StandingMovementTransitionProfile {
        source_static_contract: "AnimStateTransitionNode_1|PriorityOrder=1|bSharedRules=True",
        duration: 0.5,
        blend_mode: AlphaBlendMode::Cubic,
    },
    StandingMovementTransitionProfile {
        source_static_contract: "AnimStateTransitionNode_2|PriorityOrder=1|bSharedRules=True",
        duration: 0.7,
        blend_mode: AlphaBlendMode::Cubic,
    },
    StandingMovementTransitionProfile {
        source_static_contract: "AnimStateTransitionNode_3|PriorityOrder=1|bSharedRules=True",
        duration: 0.7,
        blend_mode: AlphaBlendMode::Cubic,
    },
    StandingMovementTransitionProfile {
        source_static_contract: "AnimStateTransitionNode_4|PriorityOrder=1|bSharedRules=True",
        duration: 0.7,
        blend_mode: AlphaBlendMode::Cubic,
    },
    StandingMovementTransitionProfile {
        source_static_contract: "AnimStateTransitionNode_5|PriorityOrder=1|bSharedRules=True",
        duration: 0.7,
        blend_mode: AlphaBlendMode::Cubic,
    },
    StandingMovementTransitionProfile {
        source_static_contract: "AnimStateTransitionNode_6|PriorityOrder=1|bSharedRules=True",
        duration: 0.7,
        blend_mode: AlphaBlendMode::Cubic,
    },
    StandingMovementTransitionProfile {
        source_static_contract: "AnimStateTransitionNode_7|PriorityOrder=1|bSharedRules=True",
        duration: 0.7,
        blend_mode: AlphaBlendMode::Cubic,
    },
    StandingMovementTransitionProfile {
        source_static_contract: "AnimStateTransitionNode_8|PriorityOrder=1|bSharedRules=True",
        duration: 0.7,
        blend_mode: AlphaBlendMode::Cubic,
    },
    StandingMovementTransitionProfile {
        source_static_contract: "AnimStateTransitionNode_9|PriorityOrder=1|bSharedRules=True",
        duration: 0.7,
        blend_mode: AlphaBlendMode::Cubic,
    },
    StandingMovementTransitionProfile {
        source_static_contract: "AnimStateTransitionNode_10|PriorityOrder=1|bSharedRules=True",
        duration: 0.5,
        blend_mode: AlphaBlendMode::Cubic,
    },
    StandingMovementTransitionProfile {
        source_static_contract: "AnimStateTransitionNode_11|PriorityOrder=1|bSharedRules=True",
        duration: 0.5,
        blend_mode: AlphaBlendMode::Cubic,
    },
    StandingMovementTransitionProfile {
        source_static_contract: "AnimStateTransitionNode_12|PriorityOrder=1|bSharedRules=True",
        duration: 0.5,
        blend_mode: AlphaBlendMode::Cubic,
    },
    StandingMovementTransitionProfile {
        source_static_contract: "AnimStateTransitionNode_13|PriorityOrder=1|bSharedRules=True",
        duration: 0.5,
        blend_mode: AlphaBlendMode::Cubic,
    },
    StandingMovementTransitionProfile {
        source_static_contract: "AnimStateTransitionNode_14|PriorityOrder=1|bSharedRules=False",
        duration: 0.75,
        blend_mode: AlphaBlendMode::QuadraticInOut,
    },
    StandingMovementTransitionProfile {
        source_static_contract: "AnimStateTransitionNode_15|PriorityOrder=1|bSharedRules=False",
        duration: 0.75,
        blend_mode: AlphaBlendMode::QuadraticInOut,
    },
    StandingMovementTransitionProfile {
        source_static_contract: "AnimStateTransitionNode_16|PriorityOrder=2|bSharedRules=True",
        duration: 0.75,
        blend_mode: AlphaBlendMode::QuadraticInOut,
    },
    StandingMovementTransitionProfile {
        source_static_contract: "AnimStateTransitionNode_17|PriorityOrder=2|bSharedRules=True",
        duration: 0.75,
        blend_mode: AlphaBlendMode::QuadraticInOut,
    },
    StandingMovementTransitionProfile {
        source_static_contract: "AnimStateTransitionNode_18|PriorityOrder=1|bSharedRules=True",
        duration: 0.7,
        blend_mode: AlphaBlendMode::Cubic,
    },
    StandingMovementTransitionProfile {
        source_static_contract: "AnimStateTransitionNode_19|PriorityOrder=1|bSharedRules=True",
        duration: 0.7,
        blend_mode: AlphaBlendMode::Cubic,
    },
    StandingMovementTransitionProfile {
        source_static_contract: "AnimStateTransitionNode_20|PriorityOrder=1|bSharedRules=True",
        duration: 0.75,
        blend_mode: AlphaBlendMode::QuadraticInOut,
    },
    StandingMovementTransitionProfile {
        source_static_contract: "AnimStateTransitionNode_21|PriorityOrder=1|bSharedRules=True",
        duration: 0.7,
        blend_mode: AlphaBlendMode::Cubic,
    },
    StandingMovementTransitionProfile {
        source_static_contract: "AnimStateTransitionNode_23|PriorityOrder=2|bSharedRules=True",
        duration: 0.75,
        blend_mode: AlphaBlendMode::QuadraticInOut,
    },
    StandingMovementTransitionProfile {
        source_static_contract: "AnimStateTransitionNode_28|PriorityOrder=2|bSharedRules=True",
        duration: 0.75,
        blend_mode: AlphaBlendMode::QuadraticInOut,
    },
];

impl StandingMovementState {
    const ALL: [Self; 6] = [
        Self::Forward,
        Self::Backward,
        Self::RightForward,
        Self::RightBackward,
        Self::LeftForward,
        Self::LeftBackward,
    ];

    fn from_direction(direction: AlsMovementDirection) -> Self {
        match direction {
            AlsMovementDirection::Forward => Self::Forward,
            AlsMovementDirection::Backward => Self::Backward,
            AlsMovementDirection::RightForward => Self::RightForward,
            AlsMovementDirection::RightBackward => Self::RightBackward,
            AlsMovementDirection::LeftForward => Self::LeftForward,
            AlsMovementDirection::LeftBackward => Self::LeftBackward,
        }
    }

    fn index(self) -> f32 {
        match self {
            Self::Forward => 0.0,
            Self::Backward => 1.0,
            Self::RightForward => 2.0,
            Self::RightBackward => 3.0,
            Self::LeftForward => 4.0,
            Self::LeftBackward => 5.0,
        }
    }

    fn direction(self) -> AlsMovementDirection {
        match self {
            Self::Forward => AlsMovementDirection::Forward,
            Self::Backward => AlsMovementDirection::Backward,
            Self::RightForward => AlsMovementDirection::RightForward,
            Self::RightBackward => AlsMovementDirection::RightBackward,
            Self::LeftForward => AlsMovementDirection::LeftForward,
            Self::LeftBackward => AlsMovementDirection::LeftBackward,
        }
    }

    fn transition_index(
        self,
        target: Self,
        hips_lock: f32,
        feet_crossing: f32,
        source_full_weight: bool,
    ) -> Option<f32> {
        match (self, target) {
            (Self::Forward, Self::Backward) => Some(0.0),
            (Self::Backward, Self::Forward) => Some(1.0),
            (Self::Forward, Self::RightForward) => Some(2.0),
            (Self::Forward, Self::LeftForward) => Some(3.0),
            (Self::Backward, Self::RightBackward) => Some(4.0),
            (Self::Backward, Self::LeftBackward) => Some(5.0),
            (Self::RightForward, Self::Forward) => Some(6.0),
            (Self::LeftForward, Self::Forward) => Some(7.0),
            (Self::LeftBackward, Self::Backward) => Some(8.0),
            (Self::RightBackward, Self::Backward) => Some(9.0),
            (Self::LeftForward, Self::RightBackward) => Some(10.0),
            (Self::RightForward, Self::LeftBackward) => Some(11.0),
            (Self::RightBackward, Self::LeftForward) => Some(12.0),
            (Self::LeftBackward, Self::RightForward) => Some(13.0),
            (Self::LeftBackward, Self::LeftForward) if feet_crossing <= 0.0 => {
                if source_full_weight && hips_lock.abs() < 0.5 {
                    Some(14.0)
                } else if hips_lock <= -0.5 {
                    Some(17.0)
                } else {
                    Some(18.0)
                }
            }
            (Self::LeftBackward, Self::LeftForward) => Some(18.0),
            (Self::RightBackward, Self::RightForward) if feet_crossing <= 0.0 => {
                if source_full_weight && hips_lock.abs() < 0.5 {
                    Some(15.0)
                } else if hips_lock >= 0.5 {
                    Some(23.0)
                } else {
                    Some(20.0)
                }
            }
            (Self::RightBackward, Self::RightForward) => Some(20.0),
            (Self::RightForward, Self::RightBackward)
                if feet_crossing <= 0.0 && hips_lock <= -0.5 =>
            {
                Some(16.0)
            }
            (Self::RightForward, Self::RightBackward) => Some(21.0),
            (Self::LeftForward, Self::LeftBackward) if feet_crossing <= 0.0 && hips_lock >= 0.5 => {
                Some(22.0)
            }
            (Self::LeftForward, Self::LeftBackward) => Some(19.0),
            _ => None,
        }
    }

    fn next_transition_toward(
        self,
        target: Self,
        hips_lock: f32,
        feet_crossing: f32,
        source_full_weight: bool,
    ) -> Option<(Self, f32)> {
        if let Some(index) =
            self.transition_index(target, hips_lock, feet_crossing, source_full_weight)
        {
            return Some((target, index));
        }
        let mut visited = [false; 6];
        let mut queue = [(self, self); 6];
        let mut queue_head = 0;
        let mut queue_tail = 1;
        visited[self.index() as usize] = true;
        while queue_head < queue_tail {
            let (current, first_hop) = queue[queue_head];
            queue_head += 1;
            for next in Self::ALL {
                if current
                    .transition_index(next, hips_lock, feet_crossing, source_full_weight)
                    .is_none()
                {
                    continue;
                }
                let first_hop = if current == self { next } else { first_hop };
                if next == target {
                    return self
                        .transition_index(first_hop, hips_lock, feet_crossing, source_full_weight)
                        .map(|index| (first_hop, index));
                }
                let next_index = next.index() as usize;
                if !visited[next_index] {
                    visited[next_index] = true;
                    queue[queue_tail] = (next, first_hop);
                    queue_tail += 1;
                }
            }
        }
        None
    }

    fn transition_duration(index: f32) -> f32 {
        let profile = STANDING_MOVEMENT_TRANSITION_PROFILES
            .get(index as usize)
            .copied()
            .unwrap_or(StandingMovementTransitionProfile {
                source_static_contract: "",
                duration: 0.7,
                blend_mode: AlphaBlendMode::Cubic,
            });
        debug_assert!(!profile.source_static_contract.is_empty() || index < 0.0);
        profile.duration
    }

    fn transition_alpha(index: f32, progress: f32) -> f32 {
        let mode = STANDING_MOVEMENT_TRANSITION_PROFILES
            .get(index as usize)
            .map(|profile| profile.blend_mode)
            .unwrap_or(AlphaBlendMode::Cubic);
        transition_alpha(progress, mode, &[])
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum StandingOuterState {
    #[default]
    Idle,
    Move,
    Stop,
    RotateLeft,
    RotateRight,
}

impl StandingOuterState {
    fn index(self) -> f32 {
        match self {
            Self::Idle => 0.0,
            Self::Move => 1.0,
            Self::Stop => 2.0,
            Self::RotateLeft => 3.0,
            Self::RotateRight => 4.0,
        }
    }

    fn is_rotate(self) -> bool {
        matches!(self, Self::RotateLeft | Self::RotateRight)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum StandingOuterTransition {
    IdleToMove,
    MoveToStop,
    StopToIdle,
    MoveToIdleQuickStop,
    IdleToRotateLeft,
    IdleToRotateRight,
    RotateLeftToIdleFinished,
    RotateRightToIdleFinished,
    RotateLeftToRotateRight,
    RotateRightToRotateLeft,
    RotateLeftToIdleMoving,
    RotateRightToIdleMoving,
}

const STANDING_OUTER_TRANSITION_PROFILES: [StandingTransitionStaticProfile; 12] = [
    StandingTransitionStaticProfile {
        source_static_contract: "AnimStateTransitionNode_0|PriorityOrder=1|CrossfadeDuration=0.200000|BlendMode=HermiteCubic|bAutomaticRuleBasedOnSequencePlayerInState=False|AutomaticRuleTriggerTime=-1.000000|SyncGroupNameToRequireValidMarkersRule=None|bAllowInertializationForSelfTransitions=False|bSharedRules=True|bSharedCrossfade=False",
        duration: 0.2,
    },
    StandingTransitionStaticProfile {
        source_static_contract: "AnimStateTransitionNode_8|PriorityOrder=1|CrossfadeDuration=0.000000|BlendMode=HermiteCubic|bAutomaticRuleBasedOnSequencePlayerInState=False|AutomaticRuleTriggerTime=-1.000000|SyncGroupNameToRequireValidMarkersRule=None|bAllowInertializationForSelfTransitions=False|bSharedRules=False|bSharedCrossfade=False",
        duration: 0.0,
    },
    StandingTransitionStaticProfile {
        source_static_contract: "AnimStateTransitionNode_10|PriorityOrder=1|CrossfadeDuration=0.300000|BlendMode=HermiteCubic|bAutomaticRuleBasedOnSequencePlayerInState=False|AutomaticRuleTriggerTime=-1.000000|SyncGroupNameToRequireValidMarkersRule=None|bAllowInertializationForSelfTransitions=False|bSharedRules=False|bSharedCrossfade=False",
        duration: 0.3,
    },
    StandingTransitionStaticProfile {
        source_static_contract: "AnimStateTransitionNode_18|PriorityOrder=2|CrossfadeDuration=0.200000|BlendMode=HermiteCubic|bAutomaticRuleBasedOnSequencePlayerInState=False|AutomaticRuleTriggerTime=-1.000000|SyncGroupNameToRequireValidMarkersRule=None|bAllowInertializationForSelfTransitions=False|bSharedRules=False|bSharedCrossfade=False",
        duration: 0.2,
    },
    StandingTransitionStaticProfile {
        source_static_contract: "AnimStateTransitionNode_19|PriorityOrder=1|CrossfadeDuration=0.200000|BlendMode=HermiteCubic|bAutomaticRuleBasedOnSequencePlayerInState=False|AutomaticRuleTriggerTime=-1.000000|SyncGroupNameToRequireValidMarkersRule=None|bAllowInertializationForSelfTransitions=False|bSharedRules=True|bSharedCrossfade=False",
        duration: 0.2,
    },
    StandingTransitionStaticProfile {
        source_static_contract: "AnimStateTransitionNode_20|PriorityOrder=1|CrossfadeDuration=0.200000|BlendMode=HermiteCubic|bAutomaticRuleBasedOnSequencePlayerInState=False|AutomaticRuleTriggerTime=-1.000000|SyncGroupNameToRequireValidMarkersRule=None|bAllowInertializationForSelfTransitions=False|bSharedRules=True|bSharedCrossfade=False",
        duration: 0.2,
    },
    StandingTransitionStaticProfile {
        source_static_contract: "AnimStateTransitionNode_21|PriorityOrder=1|CrossfadeDuration=0.000000|BlendMode=HermiteCubic|bAutomaticRuleBasedOnSequencePlayerInState=True|AutomaticRuleTriggerTime=-1.000000|SyncGroupNameToRequireValidMarkersRule=None|bAllowInertializationForSelfTransitions=False|bSharedRules=False|bSharedCrossfade=False",
        duration: 0.0,
    },
    StandingTransitionStaticProfile {
        source_static_contract: "AnimStateTransitionNode_22|PriorityOrder=1|CrossfadeDuration=0.000000|BlendMode=HermiteCubic|bAutomaticRuleBasedOnSequencePlayerInState=True|AutomaticRuleTriggerTime=-1.000000|SyncGroupNameToRequireValidMarkersRule=None|bAllowInertializationForSelfTransitions=False|bSharedRules=False|bSharedCrossfade=False",
        duration: 0.0,
    },
    StandingTransitionStaticProfile {
        source_static_contract: "AnimStateTransitionNode_23|PriorityOrder=1|CrossfadeDuration=0.200000|BlendMode=HermiteCubic|bAutomaticRuleBasedOnSequencePlayerInState=False|AutomaticRuleTriggerTime=-1.000000|SyncGroupNameToRequireValidMarkersRule=None|bAllowInertializationForSelfTransitions=False|bSharedRules=True|bSharedCrossfade=False",
        duration: 0.2,
    },
    StandingTransitionStaticProfile {
        source_static_contract: "AnimStateTransitionNode_24|PriorityOrder=1|CrossfadeDuration=0.200000|BlendMode=HermiteCubic|bAutomaticRuleBasedOnSequencePlayerInState=False|AutomaticRuleTriggerTime=-1.000000|SyncGroupNameToRequireValidMarkersRule=None|bAllowInertializationForSelfTransitions=False|bSharedRules=True|bSharedCrossfade=False",
        duration: 0.2,
    },
    StandingTransitionStaticProfile {
        source_static_contract: "AnimStateTransitionNode_25|PriorityOrder=1|CrossfadeDuration=0.200000|BlendMode=HermiteCubic|bAutomaticRuleBasedOnSequencePlayerInState=False|AutomaticRuleTriggerTime=-1.000000|SyncGroupNameToRequireValidMarkersRule=None|bAllowInertializationForSelfTransitions=False|bSharedRules=True|bSharedCrossfade=False",
        duration: 0.2,
    },
    StandingTransitionStaticProfile {
        source_static_contract: "AnimStateTransitionNode_26|PriorityOrder=1|CrossfadeDuration=0.200000|BlendMode=HermiteCubic|bAutomaticRuleBasedOnSequencePlayerInState=False|AutomaticRuleTriggerTime=-1.000000|SyncGroupNameToRequireValidMarkersRule=None|bAllowInertializationForSelfTransitions=False|bSharedRules=True|bSharedCrossfade=False",
        duration: 0.2,
    },
];

impl StandingOuterTransition {
    fn index(self) -> f32 {
        match self {
            Self::IdleToMove => 0.0,
            Self::MoveToStop => 1.0,
            Self::StopToIdle => 2.0,
            Self::MoveToIdleQuickStop => 3.0,
            Self::IdleToRotateLeft => 4.0,
            Self::IdleToRotateRight => 5.0,
            Self::RotateLeftToIdleFinished => 6.0,
            Self::RotateRightToIdleFinished => 7.0,
            Self::RotateLeftToRotateRight => 8.0,
            Self::RotateRightToRotateLeft => 9.0,
            Self::RotateLeftToIdleMoving => 10.0,
            Self::RotateRightToIdleMoving => 11.0,
        }
    }

    fn target(self) -> StandingOuterState {
        match self {
            Self::IdleToMove => StandingOuterState::Move,
            Self::MoveToStop => StandingOuterState::Stop,
            Self::StopToIdle
            | Self::MoveToIdleQuickStop
            | Self::RotateLeftToIdleFinished
            | Self::RotateRightToIdleFinished
            | Self::RotateLeftToIdleMoving
            | Self::RotateRightToIdleMoving => StandingOuterState::Idle,
            Self::IdleToRotateLeft | Self::RotateRightToRotateLeft => {
                StandingOuterState::RotateLeft
            }
            Self::IdleToRotateRight | Self::RotateLeftToRotateRight => {
                StandingOuterState::RotateRight
            }
        }
    }

    fn duration(self) -> f32 {
        let profile = STANDING_OUTER_TRANSITION_PROFILES[self.index() as usize];
        debug_assert!(!profile.source_static_contract.is_empty());
        profile.duration
    }

    fn starts_quick_stop(self) -> bool {
        self == Self::MoveToIdleQuickStop
    }
}

#[derive(Component, Clone, Debug, Default, PartialEq)]
struct StandingLayerRuntime {
    initialized: bool,
    previous_velocity: Vec3,
    previous_gait: Gait,
    previous_moving_smooth: bool,
    detail: StandingDetail,
    detail_remaining: f32,
    detail_transition_request: Option<f32>,
    detail_transition_request_remaining: f32,
    pivot_pending: bool,
    velocity_blend: VelocityBlend,
    lean: Vec2,
    sprint_time: f32,
    movement_marker_phase: SyncPhaseState,
    hips_direction: AlsMovementDirection,
    movement_state: StandingMovementState,
    movement_transition_source: Option<StandingMovementState>,
    movement_transition_request: Option<f32>,
    movement_transition_remaining: f32,
    previous_movement_state: Option<u8>,
    previous_stop_state: Option<u8>,
    outer_state: StandingOuterState,
    outer_transition_remaining: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct StandingGraphInputs {
    state_index: f32,
    transition_index: f32,
    movement_state_index: f32,
    movement_transition_index: f32,
    movement_detail_state_index: f32,
    movement_detail_transition_index: f32,
    stop_state_index: f32,
    direction_index: f32,
    hips_direction: f32,
    clip_index: f32,
    looped: f32,
    output_play_rate: f32,
    transition_duration: f32,
    pose_standing: f32,
    pose_moving: f32,
    rotation_yaw_offset: f32,
    velocity_blend: VelocityBlend,
    stride_amount: f32,
    walk_run_amount: f32,
    play_rate: f32,
    sprint_amount: f32,
    gait_is_sprinting: f32,
    sprint_block_amount: f32,
    sprint_acceleration_amount: f32,
    acceleration_forward_amount: f32,
    sprint_play_rate: f32,
    movement_marker_phase: f32,
    pivot_active: bool,
    rotation_play_rate: f32,
    rotate_left_loop: bool,
    rotate_right_loop: bool,
    rotate_clip_time: f32,
    turn_index: f32,
    turn_weight: f32,
    turn_play_rate: f32,
    quick_stop_index: f32,
    quick_stop_weight: f32,
    quick_stop_play_rate: f32,
    lean_right_amount: f32,
    lean_forward_amount: f32,
    lean_weight: f32,
    unweighted_gait_running_amount: f32,
    unweighted_gait_sprinting_amount: f32,
    foot_left_lock: f32,
    foot_right_lock: f32,
    allow_transitions: f32,
    rotation_yaw_speed: f32,
    foot_planted_amount: f32,
    feet_crossing_amount: f32,
    moving_smooth: bool,
    stop_left_backward_weight: f32,
    stop_right_backward_weight: f32,
    stop_transition_side: Option<f32>,
    quick_stop_notify: bool,
}

#[derive(Component, Debug, Default, Clone, Copy, PartialEq)]
pub struct AlsStandingGraphTelemetry {
    pub outer_state: Option<u8>,
    pub outer_transition_active: Option<bool>,
    pub detail_state: Option<u8>,
    pub detail_transition_active: Option<bool>,
    pub actual_movement_state: Option<u8>,
    pub actual_movement_transition_active: Option<bool>,
    pub actual_movement_pose_ready: Option<bool>,
    pub actual_detail_state: Option<u8>,
    pub actual_detail_transition_active: Option<bool>,
    pub actual_detail_pose_ready: Option<bool>,
    pub actual_standing_state: Option<u8>,
    pub actual_standing_transition_active: Option<bool>,
    pub actual_standing_pose_ready: Option<bool>,
    pub dominant_detail_index: Option<u8>,
    pub dominant_detail_weight: Option<f32>,
}

pub struct StandingLayerPlugin;

impl Plugin for StandingLayerPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<crate::standing_source::StandingSourceRefreshNode>();
        app.configure_sets(
            PostUpdate,
            AlsStandingLayerSet
                .after(crate::gait_layer::AlsGaitLayerSet)
                .before(AnimationGraphSet::PrePhysics),
        )
        .add_message::<AlsPivotNotify>()
        .add_systems(Startup, load_standing_curve_assets)
        .add_systems(
            PostUpdate,
            (pivot_notify_system, standing_layer_writer)
                .chain()
                .in_set(AlsStandingLayerSet),
        )
        .add_systems(
            PostUpdate,
            capture_standing_graph_telemetry.after(AnimationGraphSet::Final),
        );
    }
}

fn pivot_notify_system(mut commands: Commands, mut notices: MessageReader<AlsPivotNotify>) {
    for notice in notices.read() {
        commands.entity(notice.character).insert(AlsPivotRequest);
    }
}

fn load_standing_curve_assets(mut commands: Commands, assets: Res<AssetServer>) {
    commands.insert_resource(StandingCurveHandles {
        rotate_left: assets.load(ROTATE_LEFT_CURVE_ASSET),
        rotate_right: assets.load(ROTATE_RIGHT_CURVE_ASSET),
        turn_left_90: assets.load(TURN_LEFT_90_CURVE_ASSET),
        turn_right_90: assets.load(TURN_RIGHT_90_CURVE_ASSET),
        turn_left_180: assets.load(TURN_LEFT_180_CURVE_ASSET),
        turn_right_180: assets.load(TURN_RIGHT_180_CURVE_ASSET),
        quick_stop_left: assets.load(QUICK_STOP_LEFT_CURVE_ASSET),
        quick_stop_right: assets.load(QUICK_STOP_RIGHT_CURVE_ASSET),
        skeleton: assets.load(STANDING_SKELETON),
        stop_left_leg_mask: None,
        stop_right_leg_mask: None,
    });
}

fn update_stop_leg_masks(handles: &mut StandingCurveHandles, skeletons: &Assets<Skeleton>) {
    let Some(skeleton) = skeletons.get(&handles.skeleton) else {
        return;
    };
    if handles.stop_left_leg_mask.is_none() {
        handles.stop_left_leg_mask = Some(build_branch_bone_mask(
            skeleton,
            STOP_LEFT_LEG_BRANCHES,
            1.0,
        ));
    }
    if handles.stop_right_leg_mask.is_none() {
        handles.stop_right_leg_mask = Some(build_branch_bone_mask(
            skeleton,
            STOP_RIGHT_LEG_BRANCHES,
            1.0,
        ));
    }
}

pub fn damper_exact_alpha(delta_seconds: f32, half_life: f32) -> f32 {
    if half_life <= 0.0 {
        1.0
    } else {
        1.0 - 2.0_f32.powf(-delta_seconds.max(0.0) / half_life)
    }
}

fn local_horizontal(vector: Vec3, transform: &Transform) -> Vec2 {
    let forward = transform.rotation * Vec3::NEG_Z;
    let right = transform.rotation * Vec3::X;
    Vec2::new(vector.dot(forward), vector.dot(right))
}

fn animation_world_scale(transform: &GlobalTransform) -> f32 {
    transform
        .to_scale_rotation_translation()
        .0
        .y
        .abs()
        .max(f32::EPSILON)
}

fn unweighted_gait_amounts(gait_amount: f32, pose_grounded_amount: f32) -> (f32, f32) {
    let unweighted = if pose_grounded_amount > 1e-8 {
        gait_amount / pose_grounded_amount
    } else {
        gait_amount
    };
    (
        (unweighted - 1.0).clamp(0.0, 1.0),
        (unweighted - 2.0).clamp(0.0, 1.0),
    )
}

fn damp_velocity_blend(current: VelocityBlend, target: VelocityBlend, alpha: f32) -> VelocityBlend {
    VelocityBlend {
        forward: current.forward.lerp(target.forward, alpha),
        backward: current.backward.lerp(target.backward, alpha),
        right: current.right.lerp(target.right, alpha),
        left: current.left.lerp(target.left, alpha),
    }
}

fn acceleration_amount(
    acceleration: Vec3,
    velocity: Vec3,
    transform: &Transform,
    maximum_acceleration: f32,
    maximum_braking_deceleration: f32,
) -> Vec2 {
    let maximum = if acceleration.dot(velocity) >= 0.0 {
        maximum_acceleration
    } else {
        maximum_braking_deceleration
    };
    if maximum <= 1e-6 {
        return Vec2::ZERO;
    }
    local_horizontal((acceleration / maximum).clamp_length_max(1.0), transform)
}

pub fn standing_play_rate(
    params: &AlsMovementParams,
    speed: f32,
    stride_amount: f32,
    running: f32,
    sprinting: f32,
) -> f32 {
    let speeds = &params.speeds.standing;
    let divide = |value: f32, denominator: f32| {
        if denominator <= f32::EPSILON {
            0.0
        } else {
            value / denominator
        }
    };
    let walk_run = divide(speed, speeds.animated_walk_speed).lerp(
        divide(speed, speeds.animated_run_speed),
        running.clamp(0.0, 1.0),
    );
    let sprint = walk_run.lerp(
        divide(speed, speeds.animated_sprint_speed),
        sprinting.clamp(0.0, 1.0),
    );
    (sprint / stride_amount.max(f32::EPSILON)).clamp(MIN_PLAY_RATE, 3.0)
}

#[derive(Clone, Copy)]
struct StandingMovementSyncLeader {
    duration: f32,
    markers: &'static [SyncMarker],
    sprint: bool,
}

fn standing_movement_sync_leader(
    gait: Gait,
    direction: StandingMovementState,
    stride_amount: f32,
    sprint_amount: f32,
) -> StandingMovementSyncLeader {
    if gait == Gait::Sprinting && sprint_amount > 0.5 {
        return StandingMovementSyncLeader {
            duration: SPRINT_DURATION_SECS,
            markers: &SPRINT_MARKERS,
            sprint: true,
        };
    }
    let pose = stride_amount <= 0.5;
    let (duration, markers) = if gait == Gait::Walking {
        match direction {
            StandingMovementState::Forward => (1.133333, &*WALK_FORWARD_MARKERS),
            StandingMovementState::RightForward => (1.133333, &*WALK_RIGHT_FORWARD_MARKERS),
            StandingMovementState::RightBackward if pose => (1.133333, &*WALK_LEFT_FORWARD_MARKERS),
            StandingMovementState::RightBackward => (1.133333, &*WALK_RIGHT_BACKWARD_MARKERS),
            StandingMovementState::Backward if pose => (1.133333, &*WALK_FORWARD_MARKERS),
            StandingMovementState::Backward => (1.133333, &*WALK_BACKWARD_MARKERS),
            StandingMovementState::LeftBackward if pose => (1.133333, &*WALK_RIGHT_FORWARD_MARKERS),
            StandingMovementState::LeftBackward => (1.133333, &*WALK_LEFT_BACKWARD_MARKERS),
            StandingMovementState::LeftForward => (1.133333, &*WALK_LEFT_FORWARD_MARKERS),
        }
    } else {
        match direction {
            StandingMovementState::Forward => (0.8, &*RUN_FORWARD_MARKERS),
            StandingMovementState::RightForward => (0.8, &*RUN_RIGHT_FORWARD_MARKERS),
            StandingMovementState::RightBackward if pose => (0.8, &*RUN_LEFT_FORWARD_MARKERS),
            StandingMovementState::RightBackward => (0.666667, &*RUN_RIGHT_BACKWARD_MARKERS),
            StandingMovementState::Backward if pose => (0.8, &*RUN_FORWARD_MARKERS),
            StandingMovementState::Backward => (0.666667, &*RUN_BACKWARD_MARKERS),
            StandingMovementState::LeftBackward if pose => (0.8, &*RUN_RIGHT_FORWARD_MARKERS),
            StandingMovementState::LeftBackward => (0.666667, &*RUN_LEFT_BACKWARD_MARKERS),
            StandingMovementState::LeftForward => (0.8, &*RUN_LEFT_FORWARD_MARKERS),
        }
    };
    StandingMovementSyncLeader {
        duration,
        markers,
        sprint: false,
    }
}

fn advance_standing_movement_marker_phase(
    runtime: &mut StandingLayerRuntime,
    active: bool,
    gait: Gait,
    direction: StandingMovementState,
    stride_amount: f32,
    sprint_amount: f32,
    sprint_acceleration_amount: f32,
    play_rate: f32,
    delta_seconds: f32,
) -> f32 {
    let leader = standing_movement_sync_leader(gait, direction, stride_amount, sprint_amount);
    let start_time = if leader.sprint {
        SPRINT_START_TIME
    } else {
        MOVEMENT_BLENDSPACE_START_PHASE * leader.duration
    };
    let rate_scale = if leader.sprint && sprint_acceleration_amount > 0.5 {
        SPRINT_ACCELERATION_RATE_SCALE
    } else {
        1.0
    };
    advance_marker_sync_phase(
        &mut runtime.movement_marker_phase,
        SyncMarkerPhaseInput {
            active,
            reset: false,
            rate: play_rate * rate_scale,
            leader_duration: leader.duration,
            leader_markers: leader.markers,
            phase_duration: 1.0,
            phase_markers: &MOVEMENT_PHASE_MARKERS,
            start_time,
            looped: true,
            delta_seconds,
        },
    )
}

fn sprint_camera_animation_time(movement_marker_phase: f32) -> f32 {
    marker_phase_sample_time(
        movement_marker_phase,
        1.0,
        &MOVEMENT_PHASE_MARKERS,
        SPRINT_DURATION_SECS,
        &SPRINT_MARKERS,
        true,
    )
}

fn direction_index(direction: AlsMovementDirection) -> f32 {
    match direction {
        AlsMovementDirection::Forward => 0.0,
        AlsMovementDirection::RightForward => 1.0,
        AlsMovementDirection::RightBackward => 2.0,
        AlsMovementDirection::Backward => 3.0,
        AlsMovementDirection::LeftBackward => 4.0,
        AlsMovementDirection::LeftForward => 5.0,
    }
}

fn movement_state_index(direction: AlsMovementDirection) -> f32 {
    StandingMovementState::from_direction(direction).index()
}

fn update_hips_direction(
    runtime: &mut StandingLayerRuntime,
    movement_active: bool,
    direction: AlsMovementDirection,
    state_index: u8,
) {
    if movement_active {
        if runtime.previous_movement_state != Some(state_index) {
            runtime.hips_direction = direction;
        }
        runtime.previous_movement_state = Some(state_index);
    } else {
        runtime.previous_movement_state = None;
    }
}

fn stop_backward_weights(hips_direction: AlsMovementDirection) -> (f32, f32) {
    (
        (hips_direction == AlsMovementDirection::LeftBackward) as u8 as f32,
        (hips_direction == AlsMovementDirection::RightBackward) as u8 as f32,
    )
}

fn source_hips_direction_index(direction: AlsMovementDirection) -> f32 {
    match direction {
        AlsMovementDirection::Forward => 0.0,
        AlsMovementDirection::Backward => 1.0,
        AlsMovementDirection::LeftForward => 2.0,
        AlsMovementDirection::LeftBackward => 3.0,
        AlsMovementDirection::RightForward => 4.0,
        AlsMovementDirection::RightBackward => 5.0,
    }
}

fn stop_state_index_for_foot_planted_amount(foot_planted_amount: f32) -> f32 {
    if foot_planted_amount <= -0.5 {
        1.0
    } else if foot_planted_amount > 0.5 {
        2.0
    } else if foot_planted_amount <= 0.0 {
        3.0
    } else if foot_planted_amount > 0.0 {
        4.0
    } else {
        3.0
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct StandingSequenceCurves {
    foot_left_lock: f32,
    foot_right_lock: f32,
    allow_transitions: f32,
    rotation_yaw_speed: f32,
}

impl StandingSequenceCurves {
    fn blend(self, other: Self, alpha: f32) -> Self {
        let alpha = alpha.clamp(0.0, 1.0);
        Self {
            foot_left_lock: self.foot_left_lock.lerp(other.foot_left_lock, alpha),
            foot_right_lock: self.foot_right_lock.lerp(other.foot_right_lock, alpha),
            allow_transitions: self.allow_transitions.lerp(other.allow_transitions, alpha),
            rotation_yaw_speed: self
                .rotation_yaw_speed
                .lerp(other.rotation_yaw_speed, alpha),
        }
    }

    fn with_rotation_yaw_speed_scale(mut self, scale: f32) -> Self {
        self.rotation_yaw_speed *= scale.max(0.0);
        self
    }
}

fn standing_state_curve_overrides(state_index: u8, stop_state_index: u8) -> StandingSequenceCurves {
    match state_index {
        0 => StandingSequenceCurves {
            foot_left_lock: 1.0,
            foot_right_lock: 1.0,
            allow_transitions: 1.0,
            rotation_yaw_speed: 0.0,
        },
        2 => match stop_state_index {
            2 | 4 => StandingSequenceCurves {
                foot_right_lock: 1.0,
                ..Default::default()
            },
            1 | 3 => StandingSequenceCurves {
                foot_left_lock: 1.0,
                ..Default::default()
            },
            _ => StandingSequenceCurves::default(),
        },
        _ => StandingSequenceCurves::default(),
    }
}

fn sample_sequence_curve(
    assets: &Assets<AlsAnimCurves>,
    handle: &Handle<AlsAnimCurves>,
    name: &str,
    time: f32,
) -> f32 {
    assets
        .get(handle)
        .and_then(AlsAnimCurves::first)
        .and_then(|curves| curves.sample(name, time))
        .unwrap_or(0.0)
}

fn sample_sequence_curves(
    assets: &Assets<AlsAnimCurves>,
    handle: &Handle<AlsAnimCurves>,
    time: f32,
) -> StandingSequenceCurves {
    StandingSequenceCurves {
        foot_left_lock: sample_sequence_curve(assets, handle, "FootLeftLock", time).clamp(0.0, 1.0),
        foot_right_lock: sample_sequence_curve(assets, handle, "FootRightLock", time)
            .clamp(0.0, 1.0),
        allow_transitions: sample_sequence_curve(assets, handle, "AllowTransitions", time),
        rotation_yaw_speed: sample_sequence_curve(assets, handle, "RotationYawSpeed", time),
    }
}

fn rotate_curve_handle<'a>(
    handles: &'a StandingCurveHandles,
    drive: &AlsTurnDrive,
) -> &'a Handle<AlsAnimCurves> {
    if drive.rotate_lr <= 0.5 {
        &handles.rotate_left
    } else {
        &handles.rotate_right
    }
}

fn turn_curve_handle<'a>(
    handles: &'a StandingCurveHandles,
    drive: &AlsTurnDrive,
) -> &'a Handle<AlsAnimCurves> {
    turn_curve_handle_for(handles, drive.is_180, drive.lr)
}

fn turn_curve_handle_for<'a>(
    handles: &'a StandingCurveHandles,
    is_180: bool,
    lr: f32,
) -> &'a Handle<AlsAnimCurves> {
    match (is_180, lr <= 0.5) {
        (false, true) => &handles.turn_left_90,
        (false, false) => &handles.turn_right_90,
        (true, true) => &handles.turn_left_180,
        (true, false) => &handles.turn_right_180,
    }
}

fn blend_turn_sequence_curves(
    base: StandingSequenceCurves,
    samples: Vec<(StandingSequenceCurves, f32)>,
) -> StandingSequenceCurves {
    let total_weight = samples.iter().map(|(_, weight)| *weight).sum::<f32>();
    if total_weight <= f32::EPSILON {
        return base;
    }
    let mut samples = samples.into_iter();
    let base_weight = (1.0 - total_weight).max(0.0);
    let (mut output, mut accumulated) = if base_weight > f32::EPSILON {
        (base, base_weight)
    } else {
        let Some((first, weight)) = samples.next() else {
            return base;
        };
        (first, weight)
    };
    for (curve, weight) in samples {
        let next = accumulated + weight;
        output = output.blend(curve, weight / next.max(f32::EPSILON));
        accumulated = next;
    }
    output
}

fn turn_montage_sequence_curves(
    base: StandingSequenceCurves,
    stack: Option<&AlsTurnMontageStack>,
    fallback: Option<&AlsTurnDrive>,
    animation_curves: &Assets<AlsAnimCurves>,
    handles: &StandingCurveHandles,
) -> StandingSequenceCurves {
    let samples = stack
        .map(|stack| {
            stack
                .entries
                .iter()
                .filter_map(|entry| {
                    let weight = entry.weight();
                    (weight > f32::EPSILON).then(|| {
                        (
                            sample_sequence_curves(
                                animation_curves,
                                turn_curve_handle_for(handles, entry.is_180, entry.lr),
                                entry.elapsed.min(entry.duration),
                            )
                            .with_rotation_yaw_speed_scale(entry.play_rate),
                            weight,
                        )
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_else(|| {
            fallback
                .filter(|drive| !drive.rotate_active && drive.weight > f32::EPSILON)
                .map(|drive| {
                    vec![(
                        sample_sequence_curves(
                            animation_curves,
                            turn_curve_handle(handles, drive),
                            drive.clip_time,
                        )
                        .with_rotation_yaw_speed_scale(drive.play_rate),
                        drive.weight,
                    )]
                })
                .unwrap_or_default()
        });
    blend_turn_sequence_curves(base, samples)
}

fn direction_for(
    velocity_yaw: f32,
    view_yaw: f32,
    rotation_mode: RotationMode,
    gait: Gait,
    current_state: AlsMovementDirection,
) -> AlsMovementDirection {
    if rotation_mode == RotationMode::VelocityDirection || gait == Gait::Sprinting {
        AlsMovementDirection::Forward
    } else {
        let angle = view_space_direction_angle_from_radians(velocity_yaw, view_yaw);
        advance_standing_movement_state(current_state, classify_grounded_movement_intent(angle))
    }
}

fn rotation_yaw_offset(direction: AlsMovementDirection, runtime_angle: f32) -> f32 {
    let (forward, _, left, right) = standing_rotation_yaw_offsets(runtime_angle);
    match direction {
        AlsMovementDirection::Forward | AlsMovementDirection::Backward => forward,
        AlsMovementDirection::RightForward | AlsMovementDirection::RightBackward => right,
        AlsMovementDirection::LeftForward | AlsMovementDirection::LeftBackward => left,
    }
}

fn detail_for(
    runtime: &mut StandingLayerRuntime,
    gait: Gait,
    running_amount: f32,
    movement_fully_blended: bool,
    moving_smooth: bool,
    pivot: bool,
    delta_seconds: f32,
) -> StandingDetail {
    if !moving_smooth {
        runtime.detail_remaining = 0.0;
        runtime.detail = if gait == Gait::Walking {
            StandingDetail::Walk
        } else {
            StandingDetail::Run
        };
        return runtime.detail;
    }
    runtime.detail_remaining = (runtime.detail_remaining - delta_seconds.max(0.0)).max(0.0);
    if runtime.detail_remaining > 0.0 {
        return runtime.detail;
    }
    match runtime.detail {
        StandingDetail::RunStart | StandingDetail::RunStartFromWalk => {
            runtime.detail = StandingDetail::Run;
            return runtime.detail;
        }
        StandingDetail::FirstPivot => {
            runtime.detail = if pivot {
                StandingDetail::SecondPivot
            } else {
                StandingDetail::Run
            };
            runtime.detail_remaining = (runtime.detail == StandingDetail::SecondPivot)
                .then_some(PIVOT_DURATION)
                .unwrap_or(0.0);
            return runtime.detail;
        }
        StandingDetail::SecondPivot => {
            runtime.detail = if pivot {
                StandingDetail::FirstPivot
            } else {
                StandingDetail::Run
            };
            runtime.detail_remaining = (runtime.detail == StandingDetail::FirstPivot)
                .then_some(PIVOT_DURATION)
                .unwrap_or(0.0);
            return runtime.detail;
        }
        _ => {}
    }
    if pivot && runtime.detail == StandingDetail::Run {
        runtime.detail = StandingDetail::FirstPivot;
        runtime.detail_remaining = PIVOT_DURATION;
    } else if !runtime.previous_moving_smooth && gait != Gait::Walking {
        runtime.detail = StandingDetail::RunStart;
        runtime.detail_remaining = RUN_START_DURATION;
    } else if runtime.previous_gait == Gait::Walking
        && gait != Gait::Walking
        && movement_fully_blended
    {
        runtime.detail = StandingDetail::RunStartFromWalk;
        runtime.detail_remaining = RUN_START_FROM_WALK_DURATION;
    } else if gait == Gait::Walking
        && runtime.detail == StandingDetail::Run
        && running_amount >= 0.2
    {
    } else {
        runtime.detail = if gait == Gait::Walking {
            StandingDetail::Walk
        } else {
            StandingDetail::Run
        };
    }
    runtime.detail
}

fn source_pivot_activates(requested: bool, horizontal_speed: f32) -> bool {
    requested && horizontal_speed.is_finite() && horizontal_speed < 2.0
}

fn quick_stop_values(transition: Option<&AlsTransitionPlayback>) -> (f32, f32, f32) {
    transition
        .filter(|state| state.active && state.kind == AlsTransitionKind::QuickStop)
        .map_or((0.0, 0.0, 1.75), |state| {
            (state.index, state.weight, state.play_rate)
        })
}

fn transition_curve_handle<'a>(
    handles: &'a StandingCurveHandles,
    state: &AlsTransitionPlayback,
) -> &'a Handle<AlsAnimCurves> {
    if state.index <= 0.5 {
        &handles.quick_stop_left
    } else {
        &handles.quick_stop_right
    }
}

fn stop_transition_side(previous: Option<u8>, current: Option<u8>) -> Option<f32> {
    match current {
        Some(4) if previous != current => Some(1.0),
        Some(3) if previous != current => Some(0.0),
        _ => None,
    }
}

fn rotate_finished(drive: Option<&AlsTurnDrive>) -> bool {
    drive.is_some_and(|drive| {
        !drive.rotate_active
            && drive.rotate_clip_time >= crate::turn_layer::ROTATE_DUR - ROTATE_COMPLETION_EPSILON
    })
}

fn outer_transition_for(
    runtime: &StandingLayerRuntime,
    moving_smooth: bool,
    turn: Option<&AlsTurnDrive>,
) -> Option<StandingOuterTransition> {
    let rotate_side = turn
        .filter(|drive| drive.rotate_active)
        .map(|drive| drive.rotate_lr <= 0.5);
    match runtime.outer_state {
        StandingOuterState::Idle => {
            if moving_smooth {
                Some(StandingOuterTransition::IdleToMove)
            } else if rotate_side == Some(true) {
                Some(StandingOuterTransition::IdleToRotateLeft)
            } else if rotate_side == Some(false) {
                Some(StandingOuterTransition::IdleToRotateRight)
            } else {
                None
            }
        }
        StandingOuterState::Move => {
            if moving_smooth {
                None
            } else if runtime.outer_transition_remaining > f32::EPSILON {
                Some(StandingOuterTransition::MoveToIdleQuickStop)
            } else {
                Some(StandingOuterTransition::MoveToStop)
            }
        }
        StandingOuterState::Stop => (runtime.outer_transition_remaining <= f32::EPSILON)
            .then_some(StandingOuterTransition::StopToIdle),
        StandingOuterState::RotateLeft => {
            if moving_smooth {
                Some(StandingOuterTransition::RotateLeftToIdleMoving)
            } else if rotate_side == Some(false) {
                Some(StandingOuterTransition::RotateLeftToRotateRight)
            } else if rotate_finished(turn) {
                Some(StandingOuterTransition::RotateLeftToIdleFinished)
            } else {
                None
            }
        }
        StandingOuterState::RotateRight => {
            if moving_smooth {
                Some(StandingOuterTransition::RotateRightToIdleMoving)
            } else if rotate_side == Some(true) {
                Some(StandingOuterTransition::RotateRightToRotateLeft)
            } else if rotate_finished(turn) {
                Some(StandingOuterTransition::RotateRightToIdleFinished)
            } else {
                None
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn update_standing_inputs(
    runtime: &mut StandingLayerRuntime,
    delta_seconds: f32,
    transform: &Transform,
    animation_world_scale: f32,
    actual_motion: &AlsActualMotion,
    params: &AlsMovementParams,
    dynamics: Option<&AlsGroundedDynamics>,
    locomotion: LocomotionState,
    stance: Stance,
    gait: Gait,
    gait_running_amount: f32,
    gait_sprinting_amount: f32,
    unweighted_gait_running_amount: f32,
    unweighted_gait_sprinting_amount: f32,
    gait_is_sprinting: f32,
    sprint_block_amount: f32,
    pivot_requested: bool,
    rotation_mode: RotationMode,
    view_yaw: f32,
    input: &AlsInputCommand,
    _direction_state: Option<&AlsMovementDirectionState>,
    transition: Option<&AlsTransitionPlayback>,
    turn: Option<&AlsTurnDrive>,
    turn_stack: Option<&AlsTurnMontageStack>,
    hips_direction_lock_amount: f32,
    animation_curves: &Assets<AlsAnimCurves>,
    curve_handles: &StandingCurveHandles,
    source_context: Option<&AlsRootAnimationContext>,
) -> StandingGraphInputs {
    let velocity = actual_motion.world_velocity;
    let horizontal_speed = actual_motion.horizontal_speed;
    let has_input = input.move_dir.length_squared() > f32::EPSILON;
    let moving_smooth = source_moving_smooth(has_input, horizontal_speed);
    let acceleration = if runtime.initialized && delta_seconds > 1e-8 {
        (velocity - runtime.previous_velocity) / delta_seconds
    } else {
        Vec3::ZERO
    };
    let target_velocity_blend = VelocityBlend {
        forward: actual_motion.velocity_blend.forward,
        backward: actual_motion.velocity_blend.backward,
        right: actual_motion.velocity_blend.right,
        left: actual_motion.velocity_blend.left,
    };
    let (maximum_acceleration, maximum_braking_deceleration) = dynamics
        .map(|dynamics| (dynamics.0.acceleration, dynamics.0.braking_deceleration))
        .unwrap_or((
            params.acceleration.max_acceleration,
            params.acceleration.braking_deceleration,
        ));
    let target_lean = acceleration_amount(
        acceleration,
        velocity,
        transform,
        maximum_acceleration,
        maximum_braking_deceleration,
    );
    if runtime.initialized {
        runtime.velocity_blend = damp_velocity_blend(
            runtime.velocity_blend,
            target_velocity_blend,
            damper_exact_alpha(delta_seconds, VELOCITY_BLEND_HALF_LIFE),
        );
        runtime.lean = runtime.lean.lerp(
            target_lean,
            damper_exact_alpha(delta_seconds, LEAN_HALF_LIFE),
        );
    } else {
        runtime.velocity_blend = target_velocity_blend;
        runtime.lean = target_lean;
    }

    let walk_run_amount = if gait == Gait::Walking { 0.0 } else { 1.0 };
    let sprinting = gait_is_sprinting;
    let animation_speed = horizontal_speed / animation_world_scale;
    let stride_amount =
        standing_stride_amount(unweighted_gait_running_amount, animation_speed);
    let play_rate = standing_play_rate(
        params,
        animation_speed,
        stride_amount,
        unweighted_gait_running_amount,
        unweighted_gait_sprinting_amount,
    );
    if sprinting > 0.0 {
        runtime.sprint_time = if runtime.initialized {
            runtime.sprint_time + delta_seconds.max(0.0)
        } else {
            0.5
        };
    } else {
        runtime.sprint_time = 0.0;
    }
    let sprint_acceleration_amount = if runtime.sprint_time < 0.5 {
        target_lean.x
    } else {
        0.0
    };
    let direction = direction_for(
        actual_motion.velocity_yaw_degrees,
        view_yaw,
        rotation_mode,
        gait,
        runtime.movement_state.direction(),
    );
    let direction_index = direction_index(direction);
    let movement_state_index = movement_state_index(direction);
    let velocity_yaw = actual_motion.velocity_yaw_degrees;
    if source_pivot_activates(pivot_requested, horizontal_speed) {
        runtime.pivot_pending = true;
    }
    let pivot = runtime.pivot_pending;
    let source_curve = |name| {
        source_context
            .and_then(|context| context.f32(name))
            .unwrap_or(0.0)
    };
    let foot_planted_amount = source_curve("FootPlanted").clamp(-1.0, 1.0);
    let feet_crossing_amount = source_curve("FeetCrossing").clamp(0.0, 1.0);
    runtime.movement_transition_remaining =
        (runtime.movement_transition_remaining - delta_seconds.max(0.0)).max(0.0);
    if runtime.movement_transition_remaining <= f32::EPSILON {
        runtime.movement_transition_source = None;
    }
    runtime.detail_transition_request_remaining =
        (runtime.detail_transition_request_remaining - delta_seconds.max(0.0)).max(0.0);
    runtime.outer_transition_remaining =
        (runtime.outer_transition_remaining - delta_seconds.max(0.0)).max(0.0);
    let outer_transition = (stance == Stance::Standing && locomotion == LocomotionState::Grounded)
        .then(|| outer_transition_for(runtime, moving_smooth, turn))
        .flatten();
    if let Some(outer_transition) = outer_transition {
        runtime.outer_state = outer_transition.target();
        runtime.outer_transition_remaining = outer_transition.duration();
        if outer_transition == StandingOuterTransition::IdleToMove {
            runtime.detail = StandingDetail::Walk;
            runtime.detail_remaining = 0.0;
            runtime.detail_transition_request = None;
            runtime.detail_transition_request_remaining = 0.0;
            runtime.movement_state = StandingMovementState::Forward;
            runtime.movement_transition_source = None;
            runtime.movement_transition_request = None;
            runtime.movement_transition_remaining = 0.0;
        }
    }
    let state_index = runtime.outer_state.index();
    if state_index != 1.0 {
        runtime.detail_transition_request = None;
        runtime.detail_transition_request_remaining = 0.0;
        runtime.movement_transition_source = None;
        runtime.movement_transition_request = None;
        runtime.movement_transition_remaining = 0.0;
    }
    let detail_before = runtime.detail;
    let detail_remaining_before = runtime.detail_remaining;
    let detail = detail_for(
        runtime,
        gait,
        gait_running_amount,
        runtime.outer_transition_remaining <= f32::EPSILON,
        moving_smooth,
        pivot,
        delta_seconds,
    );
    if detail == StandingDetail::FirstPivot {
        runtime.pivot_pending = false;
    }
    let new_detail_transition = if state_index == 1.0 && detail != detail_before {
        match detail_before.transition_index(detail) {
            Some(index) => Some(index),
            None => {
                runtime.detail = detail_before;
                runtime.detail_remaining = detail_remaining_before;
                None
            }
        }
    } else {
        None
    };
    if let Some(index) = new_detail_transition {
        runtime.detail_transition_request = Some(index);
        runtime.detail_transition_request_remaining = StandingDetail::transition_duration(index);
    } else if runtime.detail_transition_request_remaining <= f32::EPSILON {
        runtime.detail_transition_request = None;
    }
    let movement_detail_transition_index = runtime.detail_transition_request.unwrap_or(-1.0);
    let desired_movement_state = StandingMovementState::from_direction(direction);
    let new_movement_transition = if state_index == 1.0
        && runtime.movement_transition_remaining <= f32::EPSILON
        && runtime.movement_state != desired_movement_state
    {
        match runtime.movement_state.next_transition_toward(
            desired_movement_state,
            hips_direction_lock_amount,
            feet_crossing_amount,
            runtime.movement_transition_remaining <= f32::EPSILON,
        ) {
            Some((next_state, index)) => {
                runtime.movement_transition_source = Some(runtime.movement_state);
                runtime.movement_state = next_state;
                runtime.movement_transition_remaining =
                    StandingMovementState::transition_duration(index);
                Some(index)
            }
            None => None,
        }
    } else {
        None
    };
    if let Some(index) = new_movement_transition {
        runtime.movement_transition_request = Some(index);
    } else if runtime.movement_transition_remaining <= f32::EPSILON {
        runtime.movement_transition_request = None;
    }
    let movement_transition_index = runtime.movement_transition_request.unwrap_or(-1.0);
    let yaw_angle = -view_space_direction_angle_from_radians(velocity_yaw, view_yaw);
    let target_yaw_offset = rotation_yaw_offset(runtime.movement_state.direction(), yaw_angle);
    let yaw_offset = runtime
        .movement_transition_source
        .zip(runtime.movement_transition_request)
        .filter(|_| runtime.movement_transition_remaining > f32::EPSILON)
        .map(|(source, index)| {
            let source_yaw_offset = rotation_yaw_offset(source.direction(), yaw_angle);
            let duration = StandingMovementState::transition_duration(index);
            let progress = 1.0 - runtime.movement_transition_remaining / duration;
            source_yaw_offset.lerp(
                target_yaw_offset,
                StandingMovementState::transition_alpha(index, progress),
            )
        })
        .unwrap_or(target_yaw_offset);
    let rotating = runtime.outer_state.is_rotate().then_some(turn).flatten();
    let stopping = runtime.outer_state == StandingOuterState::Stop;
    update_hips_direction(
        runtime,
        state_index == 1.0,
        runtime.movement_state.direction(),
        runtime.movement_state.index() as u8,
    );
    let (stop_left_backward_weight, stop_right_backward_weight) =
        stop_backward_weights(runtime.hips_direction);
    let stop_state_index = if stopping {
        stop_state_index_for_foot_planted_amount(foot_planted_amount)
    } else {
        0.0
    };
    let current_stop_state = stopping.then_some(stop_state_index as u8);
    let stop_transition_side =
        stop_transition_side(runtime.previous_stop_state, current_stop_state);
    runtime.previous_stop_state = current_stop_state;
    let (quick_stop_index, quick_stop_weight, quick_stop_play_rate) = quick_stop_values(transition);
    let turn_index = turn.map_or(0.0, |drive| {
        if drive.is_180 {
            if drive.lr <= 0.5 { 2.0 } else { 3.0 }
        } else if drive.lr <= 0.5 {
            0.0
        } else {
            1.0
        }
    });
    let turn_weight = turn_stack
        .filter(|_| !turn.is_some_and(|drive| drive.rotate_active))
        .map_or_else(
            || {
                turn.filter(|drive| !drive.rotate_active)
                    .map_or(0.0, |drive| drive.weight)
            },
            |stack| {
                stack
                    .entries
                    .iter()
                    .map(|entry| entry.weight())
                    .sum::<f32>()
                    .min(1.0)
            },
        );
    let turn_play_rate = turn.map_or(1.2, |drive| drive.play_rate.max(0.0));
    let rotation_play_rate = rotating.map_or(1.15, |drive| drive.rotate_rate.max(0.0));
    let rotate_left_loop = turn.is_some_and(|drive| drive.rotate_active && drive.rotate_lr <= 0.5);
    let rotate_right_loop = turn.is_some_and(|drive| drive.rotate_active && drive.rotate_lr > 0.5);
    let mut sequence_curves =
        standing_state_curve_overrides(state_index as u8, stop_state_index as u8);
    if let Some(rotate) = rotating {
        sequence_curves = sample_sequence_curves(
            animation_curves,
            rotate_curve_handle(curve_handles, rotate),
            rotate.rotate_clip_time,
        )
        .with_rotation_yaw_speed_scale(rotate.rotate_rate);
    }
    if !turn.is_some_and(|drive| drive.rotate_active) {
        sequence_curves = turn_montage_sequence_curves(
            sequence_curves,
            turn_stack,
            turn,
            animation_curves,
            curve_handles,
        );
    }
    if let Some(playback) = transition.filter(|state| state.active && state.weight > f32::EPSILON) {
        let transition_curves = sample_sequence_curves(
            animation_curves,
            transition_curve_handle(curve_handles, playback),
            playback.clip_time,
        );
        sequence_curves = sequence_curves.blend(transition_curves, playback.weight);
    }
    let (clip_index, looped, output_play_rate, transition_duration) = match state_index as u8 {
        0 => (0.0, 0.0, 1.0, 0.0),
        1 => (direction_index + 1.0, 1.0, play_rate, 0.2),
        2 => (direction_index + 1.0, 1.0, play_rate, 0.3),
        3 | 4 => (
            state_index - 3.0,
            turn.is_some_and(|drive| drive.rotate_active) as u8 as f32,
            rotation_play_rate,
            0.2,
        ),
        _ => (0.0, 0.0, 1.0, 0.0),
    };
    let movement_marker_phase = advance_standing_movement_marker_phase(
        runtime,
        state_index == 1.0 && stance == Stance::Standing && locomotion == LocomotionState::Grounded,
        gait,
        runtime.movement_state,
        stride_amount,
        gait_sprinting_amount,
        sprint_acceleration_amount,
        play_rate,
        delta_seconds,
    );
    runtime.previous_velocity = velocity;
    runtime.previous_gait = gait;
    runtime.previous_moving_smooth = moving_smooth;
    runtime.initialized = true;
    StandingGraphInputs {
        state_index,
        transition_index: outer_transition.map_or(-1.0, StandingOuterTransition::index),
        movement_state_index,
        movement_transition_index,
        movement_detail_state_index: runtime.detail.index(),
        movement_detail_transition_index,
        stop_state_index,
        direction_index,
        hips_direction: source_hips_direction_index(runtime.hips_direction),
        clip_index,
        looped,
        output_play_rate,
        transition_duration,
        pose_standing: 1.0,
        pose_moving: matches!(state_index as u8, 1 | 2) as u8 as f32,
        rotation_yaw_offset: if state_index == 1.0 { yaw_offset } else { 0.0 },
        velocity_blend: runtime.velocity_blend,
        stride_amount,
        walk_run_amount,
        play_rate,
        sprint_amount: gait_sprinting_amount,
        gait_is_sprinting,
        sprint_block_amount,
        sprint_acceleration_amount,
        acceleration_forward_amount: target_lean.x,
        sprint_play_rate: play_rate,
        movement_marker_phase,
        pivot_active: pivot,
        rotation_play_rate,
        rotate_left_loop,
        rotate_right_loop,
        rotate_clip_time: rotating.map_or(0.0, |drive| drive.rotate_clip_time),
        turn_index,
        turn_weight,
        turn_play_rate,
        quick_stop_index,
        quick_stop_weight,
        quick_stop_play_rate,
        lean_right_amount: runtime.lean.y,
        lean_forward_amount: runtime.lean.x,
        lean_weight: 1.0,
        unweighted_gait_running_amount,
        unweighted_gait_sprinting_amount,
        foot_left_lock: sequence_curves.foot_left_lock,
        foot_right_lock: sequence_curves.foot_right_lock,
        allow_transitions: sequence_curves.allow_transitions,
        rotation_yaw_speed: sequence_curves.rotation_yaw_speed,
        foot_planted_amount,
        feet_crossing_amount,
        moving_smooth,
        stop_left_backward_weight,
        stop_right_backward_weight,
        stop_transition_side,
        quick_stop_notify: outer_transition.is_some_and(StandingOuterTransition::starts_quick_stop),
    }
}

fn write_standing_inputs(player: &mut AnimationGraphPlayer, inputs: StandingGraphInputs) {
    for (key, value) in [
        ("als_standing_state_index", inputs.state_index),
        ("als_standing_transition_index", inputs.transition_index),
        (
            "als_standing_movement_state_index",
            inputs.movement_state_index,
        ),
        (
            "als_standing_movement_transition_index",
            inputs.movement_transition_index,
        ),
        (
            "als_standing_movement_detail_state_index",
            inputs.movement_detail_state_index,
        ),
        (
            "als_standing_movement_detail_transition_index",
            inputs.movement_detail_transition_index,
        ),
        ("als_standing_stop_state_index", inputs.stop_state_index),
        ("als_standing_direction_index", inputs.direction_index),
        ("als_standing_hips_direction", inputs.hips_direction),
        ("als_standing_clip_index", inputs.clip_index),
        ("als_standing_loop", inputs.looped),
        ("als_standing_output_play_rate", inputs.output_play_rate),
        (
            "als_standing_transition_duration",
            inputs.transition_duration,
        ),
        ("als_standing_curve_pose_standing", inputs.pose_standing),
        ("als_standing_curve_pose_moving", inputs.pose_moving),
        (
            "als_standing_curve_rotation_yaw_offset",
            inputs.rotation_yaw_offset,
        ),
        ("als_standing_curve_foot_left_lock", inputs.foot_left_lock),
        ("als_standing_curve_foot_right_lock", inputs.foot_right_lock),
        (
            "als_standing_curve_allow_transitions",
            inputs.allow_transitions,
        ),
        (
            "als_standing_curve_rotation_yaw_speed",
            inputs.rotation_yaw_speed,
        ),
        (
            "als_standing_velocity_forward_amount",
            inputs.velocity_blend.forward,
        ),
        (
            "als_standing_velocity_backward_amount",
            inputs.velocity_blend.backward,
        ),
        (
            "als_standing_velocity_right_amount",
            inputs.velocity_blend.right,
        ),
        (
            "als_standing_velocity_left_amount",
            inputs.velocity_blend.left,
        ),
        (
            "als_standing_stop_left_backward_weight",
            inputs.stop_left_backward_weight,
        ),
        (
            "als_standing_stop_right_backward_weight",
            inputs.stop_right_backward_weight,
        ),
        ("als_standing_stride_amount", inputs.stride_amount),
        ("als_standing_walk_run_amount", inputs.walk_run_amount),
        ("als_standing_play_rate", inputs.play_rate),
        ("als_standing_sprint_amount", inputs.sprint_amount),
        ("als_standing_gait_is_sprinting", inputs.gait_is_sprinting),
        (
            "als_standing_sprint_block_amount",
            inputs.sprint_block_amount,
        ),
        (
            "als_standing_sprint_acceleration_amount",
            inputs.sprint_acceleration_amount,
        ),
        ("als_standing_sprint_play_rate", inputs.sprint_play_rate),
        (
            "als_standing_movement_marker_phase",
            inputs.movement_marker_phase,
        ),
        ("als_movement_marker_phase", inputs.movement_marker_phase),
        ("als_standing_rotation_play_rate", inputs.rotation_play_rate),
        ("als_standing_rotate_clip_time", inputs.rotate_clip_time),
        ("als_standing_turn_index", inputs.turn_index),
        ("als_standing_turn_weight", inputs.turn_weight),
        ("als_standing_turn_play_rate", inputs.turn_play_rate),
        ("als_standing_quick_stop_index", inputs.quick_stop_index),
        ("als_standing_quick_stop_weight", inputs.quick_stop_weight),
        (
            "als_standing_quick_stop_play_rate",
            inputs.quick_stop_play_rate,
        ),
        ("als_standing_lean_right_amount", inputs.lean_right_amount),
        (
            "als_standing_lean_forward_amount",
            inputs.lean_forward_amount,
        ),
        ("als_standing_lean_weight", inputs.lean_weight),
        (
            "als_standing_unweighted_gait_running_amount",
            inputs.unweighted_gait_running_amount,
        ),
    ] {
        player.set_global_input_data(key, value);
    }
    for (key, value) in [
        ("grounded_state.hips_direction", inputs.hips_direction),
        ("standing_state.play_rate", inputs.play_rate),
        (
            "standing_source.acceleration_forward",
            inputs.acceleration_forward_amount,
        ),
        (
            "standing_state.sprint_acceleration_amount",
            inputs.sprint_acceleration_amount,
        ),
        (
            "standing_state.sprint_block_amount",
            inputs.sprint_block_amount,
        ),
        ("standing_state.stride_blend_amount", inputs.stride_amount),
        (
            "standing_state.walk_run_blend_amount",
            inputs.walk_run_amount,
        ),
        (
            "pose_state.unweighted_gait_running_amount",
            inputs.unweighted_gait_running_amount,
        ),
        ("lean_state.right_amount", inputs.lean_right_amount),
        ("lean_state.forward_amount", inputs.lean_forward_amount),
        ("rotate_in_place_state.play_rate", inputs.rotation_play_rate),
        ("turn_in_place_state.play_rate", inputs.turn_play_rate),
    ] {
        player.set_global_input_data(key, value);
    }
    for (key, value) in [
        (
            "rotate_in_place_state.b_rotating_left",
            inputs.rotate_left_loop,
        ),
        (
            "rotate_in_place_state.b_rotating_right",
            inputs.rotate_right_loop,
        ),
        ("standing_state.b_pivot_active", inputs.pivot_active),
    ] {
        player.set_global_input_data(key, value);
    }
    player.set_global_input_data("als_standing_rotate_left_loop", inputs.rotate_left_loop);
    player.set_global_input_data("als_standing_rotate_right_loop", inputs.rotate_right_loop);
}

fn generated_movement_state_index(label: &str) -> Option<u8> {
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

fn generated_fsm_telemetry(
    player: &AnimationGraphPlayer,
    asset_server: &AssetServer,
    graph_assets: &Assets<AnimationGraph>,
    state_machines: &Assets<StateMachine>,
    node_name: &str,
    state_index: fn(&str) -> Option<u8>,
) -> Option<(Option<u8>, bool, bool)> {
    let arena = player.get_context_arena()?;
    arena.iter_context_ids().find_map(|context_id| {
        let context = arena.get_context(context_id)?;
        if !is_generated_standing_graph(asset_server.get_path(context.get_graph_id())?.path()) {
            return None;
        }
        let graph = graph_assets.get(context.get_graph_id())?;
        let node = graph.nodes.values().find(|node| node.name == node_name)?;
        let fsm = node.try_inner_downcast_ref::<InterruptibleFsmNode>()?;
        let state_machine = state_machines.get(&fsm.fsm)?;
        if !state_machine
            .states
            .values()
            .any(|state| state_index(&state.label).is_some())
        {
            return None;
        }
        let state = context
            .node_states
            .get_all_upcoming_states::<InterruptibleFsmState>(node.id)
            .ok()?
            .find(|state| state.last_output_pose().is_some())?;
        let actual_state = match &state.state {
            LowLevelStateId::HlState(state) => state_machine
                .states
                .get(state)
                .and_then(|state| state_index(&state.label)),
            LowLevelStateId::HlTransition(_) => None,
        };
        let transition_active = matches!(state.state, LowLevelStateId::HlTransition(_));
        Some((
            actual_state,
            transition_active,
            state.last_output_pose().is_some(),
        ))
    })
}

fn generated_movement_detail_state_index(label: &str) -> Option<u8> {
    match label {
        "Walk" => Some(0),
        "Run" => Some(1),
        "First Pivot" => Some(2),
        "Second Pivot" => Some(3),
        "Run Start" => Some(4),
        "Run Start From Walk" => Some(5),
        _ => None,
    }
}

fn generated_standing_state_index(label: &str) -> Option<u8> {
    match label {
        "Idle" => Some(0),
        "Move" => Some(1),
        "Stop" => Some(2),
        "Rotate Left" => Some(3),
        "Rotate Right" => Some(4),
        _ => None,
    }
}

fn standing_dominant_branch(player: &AnimationGraphPlayer) -> (Option<u8>, Option<f32>) {
    let keys = [
        "als_standing_velocity_forward_amount",
        "als_standing_velocity_backward_amount",
        "als_standing_velocity_left_amount",
        "als_standing_velocity_right_amount",
    ];
    let mut total = 0.0;
    let mut dominant_index = None;
    let mut dominant_weight = 0.0;
    for (index, key) in keys.into_iter().enumerate() {
        let weight = player
            .get_global_input_data(key)
            .and_then(|value| value.as_f32().ok())
            .filter(|weight| weight.is_finite() && *weight > 0.0)
            .unwrap_or(0.0);
        total += weight;
        if weight > dominant_weight {
            dominant_index = Some(index as u8);
            dominant_weight = weight;
        }
    }
    if total <= f32::EPSILON {
        (None, None)
    } else {
        (dominant_index, Some(dominant_weight / total))
    }
}

fn capture_standing_graph_telemetry(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    avatars: Query<
        (
            &ChildOf,
            &AnimatedSceneInstance,
            Option<&StandingLayerRuntime>,
        ),
        With<AlsCharacterAvatar>,
    >,
    players: Query<&AnimationGraphPlayer>,
    graph_assets: Res<Assets<AnimationGraph>>,
    state_machines: Res<Assets<StateMachine>>,
    mut characters: Query<Option<&mut AlsStandingGraphTelemetry>, With<AlsCharacter>>,
) {
    for (parent, instance, runtime) in &avatars {
        let Ok(player) = players.get(instance.player_entity()) else {
            continue;
        };
        let actual = generated_fsm_telemetry(
            player,
            &asset_server,
            &graph_assets,
            &state_machines,
            "Movement States\nState Machine",
            generated_movement_state_index,
        );
        let detail = generated_fsm_telemetry(
            player,
            &asset_server,
            &graph_assets,
            &state_machines,
            "Movement Details States\nState Machine",
            generated_movement_detail_state_index,
        );
        let standing = generated_fsm_telemetry(
            player,
            &asset_server,
            &graph_assets,
            &state_machines,
            "Standing States\nState Machine",
            generated_standing_state_index,
        );
        let (dominant_detail_index, dominant_detail_weight) = standing_dominant_branch(player);
        let telemetry = AlsStandingGraphTelemetry {
            outer_state: runtime.map(|value| value.outer_state.index() as u8),
            outer_transition_active: runtime
                .map(|value| value.outer_transition_remaining > f32::EPSILON),
            detail_state: runtime.map(|value| value.detail.index() as u8),
            detail_transition_active: runtime.map(|value| {
                value.detail_remaining > f32::EPSILON
                    || value.detail_transition_request_remaining > f32::EPSILON
            }),
            actual_movement_state: actual.and_then(|value| value.0),
            actual_movement_transition_active: actual.map(|value| value.1),
            actual_movement_pose_ready: actual.map(|value| value.2),
            actual_detail_state: detail.and_then(|value| value.0),
            actual_detail_transition_active: detail.map(|value| value.1),
            actual_detail_pose_ready: detail.map(|value| value.2),
            actual_standing_state: standing.and_then(|value| value.0),
            actual_standing_transition_active: standing.map(|value| value.1),
            actual_standing_pose_ready: standing.map(|value| value.2),
            dominant_detail_index,
            dominant_detail_weight,
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

fn write_stop_leg_masks(player: &mut AnimationGraphPlayer, handles: &StandingCurveHandles) {
    let left = handles
        .stop_left_leg_mask
        .clone()
        .unwrap_or_else(BoneMask::none);
    let right = handles
        .stop_right_leg_mask
        .clone()
        .unwrap_or_else(BoneMask::none);
    player.set_global_input_data("als_standing_stop_left_leg_mask", DataValue::BoneMask(left));
    player.set_global_input_data(
        "als_standing_stop_right_leg_mask",
        DataValue::BoneMask(right),
    );
}

#[allow(clippy::type_complexity)]
fn standing_layer_writer(
    time: Res<Time>,
    mut commands: Commands,
    characters: Query<
        (
            &Transform,
            &AlsActualMotion,
            &AlsMovementParamsHandle,
            Option<&AlsGroundedDynamics>,
            &LocomotionState,
            &Stance,
            &Gait,
            &RotationMode,
            &AlsViewYaw,
            &AlsInputCommand,
            Option<&AlsInputOverride>,
            Option<&AlsMovementDirectionState>,
            Option<&AlsTransitionPlayback>,
            Option<&AlsPivotRequest>,
        ),
        With<AlsCharacter>,
    >,
    mut avatars: Query<
        (
            Entity,
            &ChildOf,
            &GlobalTransform,
            &AnimatedSceneInstance,
            Option<&mut StandingLayerRuntime>,
            Option<&AlsTurnDrive>,
            Option<&AlsTurnMontageStack>,
        ),
        With<AlsCharacterAvatar>,
    >,
    params: Res<Assets<AlsMovementParams>>,
    animation_curves: Res<Assets<AlsAnimCurves>>,
    mut curve_handles: ResMut<StandingCurveHandles>,
    skeletons: Res<Assets<Skeleton>>,
    mut players: Query<(&mut AnimationGraphPlayer, Option<&AlsRootAnimationContext>)>,
    mut event_clocks: Query<&mut AlsStandingEventClock>,
    mut quick_stop_notices: MessageWriter<AlsQuickStopNotify>,
) {
    let delta_seconds = time.delta_secs();
    update_stop_leg_masks(&mut curve_handles, &skeletons);
    for (entity, parent, avatar_transform, instance, current_runtime, turn, turn_stack) in
        &mut avatars
    {
        let Ok((
            transform,
            actual_motion,
            handle,
            dynamics,
            locomotion,
            stance,
            gait,
            rotation_mode,
            view_yaw,
            command,
            override_command,
            direction,
            transition,
            pivot_request,
        )) = characters.get(parent.parent())
        else {
            continue;
        };
        if *stance != Stance::Standing {
            if let Ok(mut clock) = event_clocks.get_mut(parent.parent()) {
                *clock = AlsStandingEventClock::default();
            }
            commands.entity(entity).remove::<StandingLayerRuntime>();
            if pivot_request.is_some() {
                commands.entity(parent.parent()).remove::<AlsPivotRequest>();
            }
            continue;
        }
        let Some(movement_params) = params.get(handle.id()) else {
            continue;
        };
        let Ok((mut player, context)) = players.get_mut(instance.player_entity()) else {
            continue;
        };
        let hips_direction_lock_amount = context
            .and_then(|state| state.f32("HipsDirectionLock"))
            .unwrap_or(0.0)
            .clamp(-1.0, 1.0);
        let gait_amount = context
            .and_then(|state| state.f32("PoseGait"))
            .unwrap_or(0.0)
            .clamp(0.0, 3.0);
        let gait_running_amount = (gait_amount - 1.0).clamp(0.0, 1.0);
        let gait_sprinting_amount = (gait_amount - 2.0).clamp(0.0, 1.0);
        let pose_grounded_amount = context
            .and_then(|state| state.f32("PoseGrounded"))
            .unwrap_or(0.0);
        let (unweighted_gait_running_amount, unweighted_gait_sprinting_amount) =
            unweighted_gait_amounts(gait_amount, pose_grounded_amount);
        let gait_is_sprinting = (*gait == Gait::Sprinting) as u8 as f32;
        let sprint_block_amount = context
            .and_then(|state| state.f32("SprintBlock"))
            .unwrap_or(0.0)
            .clamp(0.0, 1.0);
        let mut runtime = current_runtime.as_deref().cloned().unwrap_or_default();
        player.set_global_input_data("standing_source.pending_update", !runtime.initialized);
        player.set_global_input_data("standing_source.pivot_requested", pivot_request.is_some());
        player.set_global_input_data(
            "standing_source.pivot_activated",
            source_pivot_activates(pivot_request.is_some(), actual_motion.horizontal_speed),
        );
        let input = override_command.map_or(command, |value| &value.0);
        let inputs = update_standing_inputs(
            &mut runtime,
            delta_seconds,
            transform,
            animation_world_scale(avatar_transform),
            actual_motion,
            movement_params,
            dynamics,
            *locomotion,
            *stance,
            *gait,
            gait_running_amount,
            gait_sprinting_amount,
            unweighted_gait_running_amount,
            unweighted_gait_sprinting_amount,
            gait_is_sprinting,
            sprint_block_amount,
            pivot_request.is_some(),
            *rotation_mode,
            view_yaw.0,
            input,
            direction,
            transition,
            turn,
            turn_stack,
            hips_direction_lock_amount,
            &animation_curves,
            &curve_handles,
            context,
        );
        let active_sprint = inputs.state_index == 1.0 && inputs.sprint_amount > 0.5;
        let event_clock = AlsStandingEventClock {
            sprint_time: active_sprint
                .then(|| sprint_camera_animation_time(inputs.movement_marker_phase)),
            sprint_weight: if active_sprint {
                inputs.sprint_amount
            } else {
                0.0
            },
            sprint_acceleration_weight: if active_sprint {
                inputs.sprint_amount * inputs.sprint_acceleration_amount.clamp(0.0, 1.0)
            } else {
                0.0
            },
        };
        if let Ok(mut clock) = event_clocks.get_mut(parent.parent()) {
            *clock = event_clock;
        } else {
            commands.entity(parent.parent()).insert(event_clock);
        }
        if let Some(mut current_runtime) = current_runtime {
            *current_runtime = runtime;
        } else {
            commands.entity(entity).insert(runtime);
        }
        if pivot_request.is_some() {
            commands.entity(parent.parent()).remove::<AlsPivotRequest>();
        }
        if let Some(side) = inputs.stop_transition_side {
            commands
                .entity(parent.parent())
                .insert(AlsTransitionPlayback::stop(side));
        }
        if inputs.quick_stop_notify {
            quick_stop_notices.write(AlsQuickStopNotify {
                character: parent.parent(),
            });
        }
        write_standing_inputs(&mut player, inputs);
        write_stop_leg_masks(&mut player, &curve_handles);
        commands.entity(parent.parent()).insert(AlsFootLockInput {
            left_lock_amount: context
                .and_then(|state| state.f32("FootLeftLock"))
                .unwrap_or(0.0)
                .clamp(0.0, 1.0),
            right_lock_amount: context
                .and_then(|state| state.f32("FootRightLock"))
                .unwrap_or(0.0)
                .clamp(0.0, 1.0),
            allow_transitions: context
                .and_then(|state| state.f32("AllowTransitions"))
                .unwrap_or(0.0)
                >= 1.0 - 1e-4,
            moving_smooth: inputs.moving_smooth,
            foot_planted_amount: inputs.foot_planted_amount,
            feet_crossing_amount: inputs.feet_crossing_amount,
            ..Default::default()
        });
        commands
            .entity(parent.parent())
            .insert(AlsRotationAnimationState {
                yaw_offset_degrees: context
                    .and_then(|state| state.f32("RotationYawOffset"))
                    .unwrap_or(0.0),
                yaw_speed_degrees: context
                    .and_then(|state| state.f32("RotationYawSpeed"))
                    .unwrap_or(0.0),
            });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standing_source_boolean_inputs_keep_the_source_pin_types() {
        let mut player = AnimationGraphPlayer::new(default());
        write_standing_inputs(
            &mut player,
            StandingGraphInputs {
                pivot_active: true,
                rotate_left_loop: true,
                rotate_right_loop: false,
                ..default()
            },
        );
        for (key, expected) in [
            ("standing_state.b_pivot_active", true),
            ("rotate_in_place_state.b_rotating_left", true),
            ("rotate_in_place_state.b_rotating_right", false),
        ] {
            assert_eq!(
                player
                    .get_global_input_data(key)
                    .and_then(|value| value.as_bool().ok()),
                Some(expected),
                "{key}"
            );
        }
    }

    #[test]
    fn standing_source_height_scale_uses_the_runtime_up_axis() {
        let transform = GlobalTransform::from(Transform::from_scale(Vec3::new(2.0, 3.0, 4.0)));
        assert_eq!(animation_world_scale(&transform), 3.0);
    }

    #[test]
    fn source_moving_smooth_matches_input_and_speed_thresholds() {
        assert!(source_moving_smooth(true, VELOCITY_EPSILON));
        assert!(source_moving_smooth(
            false,
            MOVING_SMOOTH_SPEED_THRESHOLD + f32::EPSILON
        ));
        assert!(!source_moving_smooth(false, MOVING_SMOOTH_SPEED_THRESHOLD));
    }

    #[test]
    fn standing_direction_converts_target_yaw_to_source_sign() {
        assert_eq!(
            direction_for(
                -90.0,
                0.0,
                RotationMode::Aiming,
                Gait::Running,
                AlsMovementDirection::Forward,
            ),
            AlsMovementDirection::RightForward
        );
    }

    #[test]
    fn diagonal_velocity_blend_matches_source_normalization() {
        let blend = als_ext::horizontal_motion_from_world_velocity(
            Vec3::new(1.0, 0.0, -1.0),
            Quat::IDENTITY,
        )
        .velocity_blend;
        assert!((blend.forward - 0.5).abs() < 1e-6);
        assert!((blend.right - 0.5).abs() < 1e-6);
        assert_eq!(blend.backward, 0.0);
        assert_eq!(blend.left, 0.0);
    }

    #[test]
    fn standing_graph_telemetry_uses_active_multiway_weights() {
        let mut player = AnimationGraphPlayer::new(default());
        player.set_global_input_data("als_standing_velocity_forward_amount", 1.0);
        player.set_global_input_data("als_standing_velocity_backward_amount", f32::NAN);
        player.set_global_input_data("als_standing_velocity_left_amount", -2.0);
        player.set_global_input_data("als_standing_velocity_right_amount", 3.0);
        let (index, weight) = standing_dominant_branch(&player);
        assert_eq!(index, Some(3));
        assert!((weight.unwrap() - 0.75).abs() < 1e-6);
    }

    #[test]
    fn generated_movement_labels_match_source_indices() {
        assert!(is_generated_standing_graph(Path::new(
            "generated/ALS/Character/AnimationInstances/Stances/AB_Als_Standing/AnimGraph.animgraph.ron"
        )));
        assert!(!is_generated_standing_graph(Path::new(
            "generated/ALS/Character/AnimationInstances/Stances/AB_Als_Crouching/AnimGraph.animgraph.ron"
        )));
        assert_eq!(generated_movement_state_index("Move Forward"), Some(0));
        assert_eq!(
            generated_movement_state_index("Move Left Backward"),
            Some(5)
        );
        assert_eq!(generated_movement_state_index("Idle"), None);
        assert_eq!(generated_movement_detail_state_index("Walk"), Some(0));
        assert_eq!(
            generated_movement_detail_state_index("Run Start From Walk"),
            Some(5)
        );
        assert_eq!(generated_movement_detail_state_index("Idle"), None);
        assert_eq!(generated_standing_state_index("Idle"), Some(0));
        assert_eq!(generated_standing_state_index("Rotate Right"), Some(4));
        assert_eq!(generated_standing_state_index("Walk"), None);
    }

    #[test]
    fn movement_state_indices_match_the_source_state_machine_order() {
        assert_eq!(movement_state_index(AlsMovementDirection::Forward), 0.0);
        assert_eq!(movement_state_index(AlsMovementDirection::Backward), 1.0);
        assert_eq!(
            movement_state_index(AlsMovementDirection::RightForward),
            2.0
        );
        assert_eq!(
            movement_state_index(AlsMovementDirection::RightBackward),
            3.0
        );
        assert_eq!(movement_state_index(AlsMovementDirection::LeftForward), 4.0);
        assert_eq!(
            movement_state_index(AlsMovementDirection::LeftBackward),
            5.0
        );
    }

    #[test]
    fn stop_uses_the_last_relevant_hips_direction() {
        let mut runtime = StandingLayerRuntime::default();
        update_hips_direction(&mut runtime, true, AlsMovementDirection::LeftBackward, 5);
        assert_eq!(stop_backward_weights(runtime.hips_direction), (1.0, 0.0));

        update_hips_direction(&mut runtime, false, AlsMovementDirection::RightBackward, 3);
        assert_eq!(stop_backward_weights(runtime.hips_direction), (1.0, 0.0));

        update_hips_direction(&mut runtime, true, AlsMovementDirection::RightBackward, 3);
        assert_eq!(stop_backward_weights(runtime.hips_direction), (0.0, 1.0));
    }

    #[test]
    fn source_hips_direction_indices_match_the_authored_enum_order() {
        assert_eq!(
            source_hips_direction_index(AlsMovementDirection::Forward),
            0.0
        );
        assert_eq!(
            source_hips_direction_index(AlsMovementDirection::Backward),
            1.0
        );
        assert_eq!(
            source_hips_direction_index(AlsMovementDirection::LeftForward),
            2.0
        );
        assert_eq!(
            source_hips_direction_index(AlsMovementDirection::LeftBackward),
            3.0
        );
        assert_eq!(
            source_hips_direction_index(AlsMovementDirection::RightForward),
            4.0
        );
        assert_eq!(
            source_hips_direction_index(AlsMovementDirection::RightBackward),
            5.0
        );
    }

    #[test]
    fn source_velocity_and_lean_half_lives_are_exact() {
        assert!(
            (damper_exact_alpha(VELOCITY_BLEND_HALF_LIFE, VELOCITY_BLEND_HALF_LIFE) - 0.5).abs()
                < 1e-6
        );
        assert!((damper_exact_alpha(LEAN_HALF_LIFE, LEAN_HALF_LIFE) - 0.5).abs() < 1e-6);
    }

    #[test]
    fn standing_source_acceleration_clamps_before_planar_projection() {
        let velocity = Vec3::new(0.0, 3.0, -4.0);
        let amount = acceleration_amount(velocity, velocity, &Transform::default(), 1.0, 1.0);
        assert!(amount.distance(Vec2::new(0.8, 0.0)) < 1e-6, "{amount:?}");
    }

    #[test]
    fn standing_source_gait_division_uses_source_small_number() {
        assert_eq!(unweighted_gait_amounts(1.5e-7, 5e-8), (1.0, 1.0));
    }

    #[test]
    fn standing_source_stride_retains_the_source_key() {
        let value = standing_stride_amount(0.0, 0.583);
        assert!((value - 0.55).abs() < 1e-6, "{value}");
        assert!((standing_stride_amount(1.0, 1.66899994) - 0.45).abs() < 1e-6);
        assert_eq!(standing_stride_amount(0.5, 10.0), 1.0);
        assert_eq!(standing_stride_amount(0.5, -1.0), 0.2);
    }

    #[test]
    fn standing_source_yaw_offset_uses_runtime_rotation_sign() {
        let value = standing_rotation_yaw_offsets(-60.0).0;
        assert!((value + 60.0).abs() < 1e-6, "{value}");
        assert_eq!(standing_rotation_yaw_offsets(-420.0).0, -60.0);
        assert_eq!(standing_rotation_yaw_offsets(60.0).0, 60.0);
        assert_eq!(standing_rotation_yaw_offsets(-45.0), (-45.0, -45.0, 45.0, 0.0));
        assert_eq!(standing_rotation_yaw_offsets(45.0), (45.0, 45.0, 0.0, -45.0));
    }

    #[test]
    fn lean_normalizes_against_current_braking_dynamics() {
        let transform = Transform::default();
        let acceleration = Vec3::NEG_X * 5.0;
        let velocity = Vec3::X;
        let current = acceleration_amount(acceleration, velocity, &transform, 20.0, 5.0);
        let static_params = acceleration_amount(acceleration, velocity, &transform, 20.0, 15.0);
        let passed = (current.length() - 1.0).abs() < 1e-6
            && (static_params.length() - 1.0 / 3.0).abs() < 1e-6;
        let verdict = if passed { "PASS" } else { "FAIL" };
        println!(
            "[BF102.37][VERDICT] current_braking_lean={:.3} static_braking_lean={:.3} -> {verdict}",
            current.length(),
            static_params.length(),
        );
        assert!(passed);
    }

    #[test]
    fn standing_play_rate_matches_discrete_gait_lerps() {
        let params = AlsMovementParams::default();
        let rate = standing_play_rate(&params, 3.5, 1.0, 1.0, 0.0);
        assert!((rate - 1.0).abs() < 1e-6);
        let sprint = standing_play_rate(&params, 6.0, 1.0, 1.0, 1.0);
        assert!((sprint - 1.0).abs() < 1e-6);
    }

    #[test]
    fn standing_marker_phase_uses_the_source_blendspace_start() {
        let mut runtime = StandingLayerRuntime::default();
        let phase = advance_standing_movement_marker_phase(
            &mut runtime,
            true,
            Gait::Walking,
            StandingMovementState::Forward,
            1.0,
            0.0,
            0.0,
            1.0,
            0.0,
        );
        let leader =
            standing_movement_sync_leader(Gait::Walking, StandingMovementState::Forward, 1.0, 0.0);
        let time = als_ext::marker_phase_sample_time(
            phase,
            1.0,
            &MOVEMENT_PHASE_MARKERS,
            leader.duration,
            leader.markers,
            true,
        );
        assert!((time - 0.2266666).abs() < 1e-5);
    }

    #[test]
    fn standing_marker_phase_uses_the_highest_weighted_sprint_leader() {
        let normal = standing_movement_sync_leader(
            Gait::Sprinting,
            StandingMovementState::Forward,
            1.0,
            0.5,
        );
        let sprint = standing_movement_sync_leader(
            Gait::Sprinting,
            StandingMovementState::Forward,
            1.0,
            0.500001,
        );
        assert!(!normal.sprint);
        assert!(sprint.sprint);
        assert!((sprint.duration - 0.6).abs() < 1e-6);
    }

    #[test]
    fn sprint_camera_time_tracks_the_marker_synced_animation_rate() {
        let mut runtime = StandingLayerRuntime::default();
        let start_phase = advance_standing_movement_marker_phase(
            &mut runtime,
            true,
            Gait::Sprinting,
            StandingMovementState::Forward,
            1.0,
            1.0,
            0.0,
            1.0,
            0.0,
        );
        let half_rate_phase = advance_standing_movement_marker_phase(
            &mut runtime,
            true,
            Gait::Sprinting,
            StandingMovementState::Forward,
            1.0,
            1.0,
            0.0,
            0.5,
            0.1,
        );
        let full_rate_phase = advance_standing_movement_marker_phase(
            &mut runtime,
            true,
            Gait::Sprinting,
            StandingMovementState::Forward,
            1.0,
            1.0,
            0.0,
            1.5,
            0.1,
        );
        let start = sprint_camera_animation_time(start_phase);
        let half_rate = sprint_camera_animation_time(half_rate_phase);
        let full_rate = sprint_camera_animation_time(full_rate_phase);
        let passed = (start - SPRINT_START_TIME).abs() <= 1e-5
            && (half_rate - (SPRINT_START_TIME + 0.05)).abs() <= 1e-5
            && (full_rate - (SPRINT_START_TIME + 0.2)).abs() <= 1e-5;
        let verdict = if passed { "PASS" } else { "FAIL" };
        println!(
            "[BF102.53][VERDICT] start={start:.6} half_rate={half_rate:.6} full_rate={full_rate:.6} -> {verdict}",
        );
        assert!(passed);
    }

    #[test]
    fn standing_does_not_supply_generated_graph_clock_values() {
        let mut player = AnimationGraphPlayer::new(default());
        write_standing_inputs(
            &mut player,
            StandingGraphInputs {
                movement_marker_phase: 0.25,
                pivot_active: true,
                ..default()
            },
        );
        assert_eq!(
            player
                .get_global_input_data("als_movement_marker_phase")
                .and_then(|value| value.as_f32().ok()),
            Some(0.25)
        );
        assert_eq!(
            player
                .get_global_input_data("sync_groups.ab_als_standing.movement.phase")
                .and_then(|value| value.as_f32().ok()),
            None
        );
        assert_eq!(
            player
                .get_global_input_data("sync_groups.ab_als_standing.first_pivot.phase")
                .and_then(|value| value.as_f32().ok()),
            None
        );
        assert_eq!(
            player
                .get_global_input_data("standing_state.b_pivot_active")
                .and_then(|value| value.as_bool().ok()),
            Some(true)
        );
    }

    #[test]
    fn standing_marker_phase_preserves_the_marker_interval_across_direction_changes() {
        let mut runtime = StandingLayerRuntime::default();
        let phase = advance_standing_movement_marker_phase(
            &mut runtime,
            true,
            Gait::Running,
            StandingMovementState::Forward,
            1.0,
            0.0,
            0.0,
            1.0,
            0.0,
        );
        let forward =
            standing_movement_sync_leader(Gait::Running, StandingMovementState::Forward, 1.0, 0.0);
        let backward_phase = advance_standing_movement_marker_phase(
            &mut runtime,
            true,
            Gait::Running,
            StandingMovementState::RightBackward,
            1.0,
            0.0,
            0.0,
            1.0,
            0.0,
        );
        let backward = standing_movement_sync_leader(
            Gait::Running,
            StandingMovementState::RightBackward,
            1.0,
            0.0,
        );
        let forward_time = als_ext::marker_phase_sample_time(
            phase,
            1.0,
            &MOVEMENT_PHASE_MARKERS,
            forward.duration,
            forward.markers,
            true,
        );
        let backward_time = als_ext::marker_phase_sample_time(
            backward_phase,
            1.0,
            &MOVEMENT_PHASE_MARKERS,
            backward.duration,
            backward.markers,
            true,
        );
        let forward_marker =
            als_ext::marker_position_at_time(forward.markers, forward.duration, forward_time, true)
                .unwrap();
        let backward_marker = als_ext::marker_position_at_time(
            backward.markers,
            backward.duration,
            backward_time,
            true,
        )
        .unwrap();
        assert_eq!(forward_marker.previous, backward_marker.previous);
        assert_eq!(forward_marker.next, backward_marker.next);
        assert!((forward_marker.alpha - backward_marker.alpha).abs() < 1e-5);
    }

    #[test]
    fn animation_speed_divides_by_avatar_world_height_scale() {
        let avatar_transform =
            GlobalTransform::from(Transform::from_scale(Vec3::new(4.0, 2.0, 1.0)));
        let animation_speed = 6.0 / animation_world_scale(&avatar_transform);
        let passed = (animation_speed - 3.0).abs() < 1e-6;
        let verdict = if passed { "PASS" } else { "FAIL" };
        println!(
            "[BF102.38][VERDICT] character_local_scale=1.000 avatar_world_height_scale={:.3} animation_speed={animation_speed:.3} -> {verdict}",
            animation_world_scale(&avatar_transform),
        );
        assert!(passed);
    }

    #[test]
    fn run_start_windows_use_exported_clip_times() {
        assert!((RUN_START_DURATION - 0.72).abs() < 1e-6);
        assert!((RUN_START_FROM_WALK_DURATION - 0.68).abs() < 1e-6);
        assert!((PIVOT_DURATION - 0.6).abs() < 1e-6);
    }

    #[test]
    fn stop_substates_match_source_foot_planted_thresholds() {
        assert_eq!(stop_state_index_for_foot_planted_amount(0.6), 2.0);
        assert_eq!(stop_state_index_for_foot_planted_amount(0.1), 4.0);
        assert_eq!(stop_state_index_for_foot_planted_amount(0.0), 3.0);
        assert_eq!(stop_state_index_for_foot_planted_amount(-0.5), 1.0);
        assert_eq!(stop_state_index_for_foot_planted_amount(-0.1), 3.0);
    }

    #[test]
    fn plant_state_entry_requests_the_matching_source_transition_side() {
        assert_eq!(stop_transition_side(None, Some(4)), Some(1.0));
        assert_eq!(stop_transition_side(None, Some(3)), Some(0.0));
        assert_eq!(stop_transition_side(Some(4), Some(4)), None);
        assert_eq!(stop_transition_side(Some(3), None), None);
    }

    #[test]
    fn state_curve_overrides_match_the_compiled_source_nodes() {
        assert_eq!(
            standing_state_curve_overrides(0, 0),
            StandingSequenceCurves {
                foot_left_lock: 1.0,
                foot_right_lock: 1.0,
                allow_transitions: 1.0,
                rotation_yaw_speed: 0.0,
            }
        );
        assert_eq!(standing_state_curve_overrides(2, 2).foot_right_lock, 1.0);
        assert_eq!(standing_state_curve_overrides(2, 4).foot_right_lock, 1.0);
        assert_eq!(standing_state_curve_overrides(2, 1).foot_left_lock, 1.0);
        assert_eq!(standing_state_curve_overrides(2, 3).foot_left_lock, 1.0);
    }

    #[test]
    fn unweighted_gait_amounts_follow_the_source_grounded_division() {
        assert_eq!(unweighted_gait_amounts(1.5, 0.0), (0.5, 0.0));
        assert_eq!(unweighted_gait_amounts(1.5, 0.5), (1.0, 1.0));
    }

    #[test]
    fn rotation_yaw_speed_uses_the_source_scale_binding() {
        let curves = StandingSequenceCurves {
            rotation_yaw_speed: 0.5,
            ..Default::default()
        };
        assert_eq!(
            curves.with_rotation_yaw_speed_scale(1.2).rotation_yaw_speed,
            0.6
        );
        assert_eq!(
            curves.with_rotation_yaw_speed_scale(0.0).rotation_yaw_speed,
            0.0
        );
    }

    #[test]
    fn outer_transition_indices_match_the_source_state_machine_order() {
        let transitions = [
            StandingOuterTransition::IdleToMove,
            StandingOuterTransition::MoveToStop,
            StandingOuterTransition::StopToIdle,
            StandingOuterTransition::MoveToIdleQuickStop,
            StandingOuterTransition::IdleToRotateLeft,
            StandingOuterTransition::IdleToRotateRight,
            StandingOuterTransition::RotateLeftToIdleFinished,
            StandingOuterTransition::RotateRightToIdleFinished,
            StandingOuterTransition::RotateLeftToRotateRight,
            StandingOuterTransition::RotateRightToRotateLeft,
            StandingOuterTransition::RotateLeftToIdleMoving,
            StandingOuterTransition::RotateRightToIdleMoving,
        ];
        for (index, transition) in transitions.into_iter().enumerate() {
            assert_eq!(transition.index(), index as f32);
        }
    }

    #[test]
    fn move_to_stop_waits_for_full_weight_but_quick_stop_interrupts() {
        let mut runtime = StandingLayerRuntime {
            outer_state: StandingOuterState::Move,
            outer_transition_remaining: 0.1,
            ..default()
        };
        assert_eq!(
            outer_transition_for(&runtime, false, None),
            Some(StandingOuterTransition::MoveToIdleQuickStop)
        );
        runtime.outer_transition_remaining = 0.0;
        assert_eq!(
            outer_transition_for(&runtime, false, None),
            Some(StandingOuterTransition::MoveToStop)
        );
    }

    #[test]
    fn rotate_states_only_start_from_idle_and_exit_on_the_source_conditions() {
        let left = AlsTurnDrive {
            rotate_active: true,
            rotate_lr: 0.0,
            ..default()
        };
        let mut runtime = StandingLayerRuntime::default();
        assert_eq!(
            outer_transition_for(&runtime, false, Some(&left)),
            Some(StandingOuterTransition::IdleToRotateLeft)
        );
        runtime.outer_state = StandingOuterState::RotateLeft;
        runtime.outer_transition_remaining = 0.1;
        let right = AlsTurnDrive {
            rotate_active: true,
            rotate_lr: 1.0,
            ..default()
        };
        assert_eq!(
            outer_transition_for(&runtime, false, Some(&right)),
            Some(StandingOuterTransition::RotateLeftToRotateRight)
        );
        assert_eq!(
            outer_transition_for(&runtime, true, Some(&left)),
            Some(StandingOuterTransition::RotateLeftToIdleMoving)
        );
    }

    #[test]
    fn movement_detail_direct_indices_match_the_source_edges() {
        assert_eq!(
            StandingDetail::Walk.transition_index(StandingDetail::Run),
            Some(0.0)
        );
        assert_eq!(
            StandingDetail::Walk.transition_index(StandingDetail::RunStart),
            Some(5.0)
        );
        assert_eq!(
            StandingDetail::Run.transition_index(StandingDetail::FirstPivot),
            Some(9.0)
        );
        assert_eq!(
            StandingDetail::FirstPivot.transition_index(StandingDetail::SecondPivot),
            Some(1.0)
        );
        assert_eq!(
            StandingDetail::Walk.transition_index(StandingDetail::FirstPivot),
            None
        );
    }

    #[test]
    fn detail_run_to_walk_waits_for_the_source_running_amount_threshold() {
        let mut runtime = StandingLayerRuntime {
            detail: StandingDetail::Run,
            initialized: true,
            ..default()
        };
        assert_eq!(
            detail_for(&mut runtime, Gait::Walking, 0.2, true, true, false, 0.0),
            StandingDetail::Run
        );
        assert_eq!(
            detail_for(&mut runtime, Gait::Walking, 0.199, true, true, false, 0.0),
            StandingDetail::Walk
        );
    }

    #[test]
    fn detail_temporary_states_return_to_run_before_reselecting_gait() {
        let mut runtime = StandingLayerRuntime {
            detail: StandingDetail::RunStart,
            initialized: true,
            ..default()
        };
        assert_eq!(
            detail_for(&mut runtime, Gait::Walking, 0.0, true, true, false, 0.0),
            StandingDetail::Run
        );
        runtime.detail = StandingDetail::RunStartFromWalk;
        assert_eq!(
            detail_for(&mut runtime, Gait::Walking, 0.0, true, true, false, 0.0),
            StandingDetail::Run
        );
    }

    #[test]
    fn detail_run_start_begins_during_the_outer_move_blend() {
        let mut runtime = StandingLayerRuntime {
            detail: StandingDetail::Walk,
            initialized: true,
            ..default()
        };
        assert_eq!(
            detail_for(&mut runtime, Gait::Running, 1.0, false, true, false, 0.0),
            StandingDetail::RunStart
        );
    }

    #[test]
    fn movement_direct_indices_preserve_source_hips_and_crossing_rules() {
        let left_backward = StandingMovementState::LeftBackward;
        let left_forward = StandingMovementState::LeftForward;
        let right_backward = StandingMovementState::RightBackward;
        let right_forward = StandingMovementState::RightForward;

        assert_eq!(
            left_backward.transition_index(left_forward, 0.0, 0.0, true),
            Some(14.0)
        );
        assert_eq!(
            left_backward.transition_index(left_forward, 0.0, 0.0, false),
            Some(18.0)
        );
        assert_eq!(
            left_backward.transition_index(left_forward, -1.0, 0.0, false),
            Some(17.0)
        );
        assert_eq!(
            right_backward.transition_index(right_forward, 0.0, 0.0, true),
            Some(15.0)
        );
        assert_eq!(
            right_backward.transition_index(right_forward, 1.0, 0.0, false),
            Some(23.0)
        );
        assert_eq!(
            right_backward.transition_index(right_forward, 0.0, 0.1, true),
            Some(20.0)
        );
        assert_eq!(
            right_forward.transition_index(right_backward, -1.0, 0.0, false),
            Some(16.0)
        );
        assert_eq!(
            right_forward.transition_index(right_backward, 0.0, 0.1, false),
            Some(21.0)
        );
        assert_eq!(
            StandingMovementState::LeftForward.transition_index(
                StandingMovementState::LeftBackward,
                1.0,
                0.0,
                false,
            ),
            Some(22.0)
        );
        assert_eq!(
            StandingMovementState::LeftForward.transition_index(
                StandingMovementState::LeftBackward,
                0.0,
                0.1,
                false,
            ),
            Some(19.0)
        );
    }

    #[test]
    fn movement_routes_nonadjacent_directions_through_source_edges() {
        assert_eq!(
            StandingMovementState::Forward.next_transition_toward(
                StandingMovementState::RightBackward,
                0.0,
                0.0,
                true,
            ),
            Some((StandingMovementState::Backward, 0.0))
        );
        assert_eq!(
            StandingMovementState::LeftBackward.next_transition_toward(
                StandingMovementState::Forward,
                0.0,
                0.0,
                true,
            ),
            Some((StandingMovementState::Backward, 8.0))
        );
        assert_eq!(
            StandingMovementState::Backward.next_transition_toward(
                StandingMovementState::LeftForward,
                0.0,
                0.0,
                true,
            ),
            Some((StandingMovementState::Forward, 1.0))
        );
    }

    #[test]
    fn movement_yaw_curve_blend_uses_the_source_transition_modes() {
        assert!((StandingMovementState::transition_alpha(0.0, 0.25) - 0.15625).abs() < 1e-6);
        assert!((StandingMovementState::transition_alpha(14.0, 0.25) - 0.125).abs() < 1e-6);
    }

    #[test]
    fn pivot_notify_uses_the_source_speed_gate() {
        assert!(source_pivot_activates(true, 1.999));
        assert!(!source_pivot_activates(true, 2.0));
        assert!(!source_pivot_activates(false, 0.0));
        assert!(!source_pivot_activates(true, f32::NAN));
    }

    #[test]
    fn pivot_request_survives_the_run_start_window() {
        let mut runtime = StandingLayerRuntime {
            detail: StandingDetail::RunStart,
            pivot_pending: true,
            ..default()
        };
        let first_pivot = runtime.pivot_pending;
        let first = detail_for(
            &mut runtime,
            Gait::Running,
            1.0,
            true,
            true,
            first_pivot,
            RUN_START_DURATION,
        );
        assert_eq!(first, StandingDetail::Run);
        let second_pivot = runtime.pivot_pending;
        let second = detail_for(
            &mut runtime,
            Gait::Running,
            1.0,
            true,
            true,
            second_pivot,
            0.0,
        );
        assert_eq!(second, StandingDetail::FirstPivot);
    }
}
