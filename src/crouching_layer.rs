use als_core::character::{AlsCharacter, AlsMovementParamsHandle};
use std::sync::LazyLock;

use als_core::foot_ik::{
    AlsFootLockInput, AlsFootLockState, foot_lock_distance_squared, is_relevant_animation_weight,
};
use als_core::gait::stride_blend_amount_for_speed;
use als_core::input::AlsInputCommand;
use als_core::motion::{AlsInputOverride, AlsViewYaw};
use als_core::movement_direction::{
    AlsMovementDirection, AlsMovementDirectionState, update_direction,
};
use als_core::params::AlsMovementParams;
use als_core::state::{LocomotionState, RotationMode, Stance};
use als_ext::{
    AlphaBlendMode, AlsAnimCurves, CurveAsset, SyncMarker, SyncMarkerPhaseInput, SyncPhaseState,
    advance_marker_sync_phase, build_branch_bone_mask, transition_alpha,
};
use bevy::ecs::hierarchy::ChildOf;
use bevy::prelude::*;
use bevy_animation_graph::core::animated_scene::AnimatedSceneInstance;
use bevy_animation_graph::core::animation_graph_player::AnimationGraphPlayer;
use bevy_animation_graph::core::edge_data::{DataValue, bone_mask::BoneMask};
use bevy_animation_graph::core::plugin::AnimationGraphSet;
use bevy_animation_graph::core::skeleton::Skeleton;

use crate::actual_motion::AlsActualMotion;
use crate::character_assembly::AlsCharacterAvatar;
use crate::direction_layer::{velocity_yaw_deg, view_space_direction_angle_from_radians};
use crate::stop_link::{AlsTransitionKind, AlsTransitionPlayback};
use crate::turn_layer::{AlsTurnDrive, AlsTurnMontageStack, ROTATE_DUR};

const VELOCITY_EPSILON: f32 = 0.01;
const MOVING_SMOOTH_SPEED_THRESHOLD: f32 = 1.5;
const VELOCITY_BLEND_HALF_LIFE: f32 = 0.1;
const SOURCE_CROUCHING_STOP_STATE_ENTRY_FUNCTION: &str = "PlayStopTransitionAnimation";
const SOURCE_CROUCHING_TURN_STOP_FUNCTION: &str = "StopTransitionAndTurnInPlaceAnimations";
const LEAN_HALF_LIFE: f32 = 0.2;
const MIN_PLAY_RATE: f32 = 1e-4;
const ROTATE_COMPLETION_EPSILON: f32 = 1e-4;
const DYNAMIC_FOOT_LOCK_DISTANCE_METERS: f32 = 0.08;
const CROUCH_MOVEMENT_DURATION: f32 = 1.133333;

fn marker_set(entries: &[(&str, f32)]) -> Vec<SyncMarker> {
    entries
        .iter()
        .map(|(name, time)| SyncMarker {
            name: (*name).to_owned(),
            time: *time,
        })
        .collect()
}

static CROUCH_PHASE_MARKERS: LazyLock<Vec<SyncMarker>> =
    LazyLock::new(|| marker_set(&[("Left", 0.0), ("Right", 0.5)]));
static CROUCH_FORWARD_MARKERS: LazyLock<Vec<SyncMarker>> =
    LazyLock::new(|| marker_set(&[("Left", 0.091667), ("Right", 0.65)]));
static CROUCH_RIGHT_FORWARD_MARKERS: LazyLock<Vec<SyncMarker>> =
    LazyLock::new(|| marker_set(&[("Left", 0.016667), ("Right", 0.65)]));
static CROUCH_RIGHT_BACKWARD_MARKERS: LazyLock<Vec<SyncMarker>> =
    LazyLock::new(|| marker_set(&[("Right", 0.483333), ("Left", 1.041667)]));
static CROUCH_BACKWARD_MARKERS: LazyLock<Vec<SyncMarker>> =
    LazyLock::new(|| marker_set(&[("Right", 0.5), ("Left", 1.066667)]));
static CROUCH_LEFT_BACKWARD_MARKERS: LazyLock<Vec<SyncMarker>> =
    LazyLock::new(|| marker_set(&[("Right", 0.466667), ("Left", 1.041667)]));
static CROUCH_LEFT_FORWARD_MARKERS: LazyLock<Vec<SyncMarker>> =
    LazyLock::new(|| marker_set(&[("Left", 0.091667), ("Right", 0.591667)]));
const LEFT_LEG_BRANCHES: &[&str] = &["thigh_l", "ik_foot_l", "VB foot_l"];
const RIGHT_LEG_BRANCHES: &[&str] = &["thigh_r", "ik_foot_r", "VB foot_r"];
const STOP_LEG_BRANCHES: &[&str] = &[
    "thigh_l",
    "ik_foot_l",
    "VB foot_l",
    "thigh_r",
    "ik_foot_r",
    "VB foot_r",
];
const SKELETON_PATH: &str = "skeletons/SKM_Als.skn.ron";
const YAW_OFFSET_FORWARD_BACKWARD: &str =
    "curves/ALS/Data/AnimationInstance/Grounded/CF_Als_RotationYawOffset_ForwardBackward.curve.ron";
const YAW_OFFSET_LEFT: &str =
    "curves/ALS/Data/AnimationInstance/Grounded/CF_Als_RotationYawOffset_Left.curve.ron";
const YAW_OFFSET_RIGHT: &str =
    "curves/ALS/Data/AnimationInstance/Grounded/CF_Als_RotationYawOffset_Right.curve.ron";
const MOVEMENT_CURVE_ASSETS: [&str; 6] = [
    "animations/Grounded/Crouch/A_Als_Crouch_Walk_Forward.gltf#AlsCurves",
    "animations/Grounded/Crouch/A_Als_Crouch_Walk_Right_Forward.gltf#AlsCurves",
    "animations/Grounded/Crouch/A_Als_Crouch_Walk_Right_Backward.gltf#AlsCurves",
    "animations/Grounded/Crouch/A_Als_Crouch_Walk_Backward.gltf#AlsCurves",
    "animations/Grounded/Crouch/A_Als_Crouch_Walk_Left_Backward.gltf#AlsCurves",
    "animations/Grounded/Crouch/A_Als_Crouch_Walk_Left_Forward.gltf#AlsCurves",
];
const ROTATE_LEFT_CURVE_ASSET: &str =
    "animations/RotateInPlace/A_Als_Crouch_Rotate_90_Left.gltf#AlsCurves";
const ROTATE_RIGHT_CURVE_ASSET: &str =
    "animations/RotateInPlace/A_Als_Crouch_Rotate_90_Right.gltf#AlsCurves";
const TURN_LEFT_90_CURVE_ASSET: &str =
    "animations/TurnInPlace/A_Als_Crouch_Turn_90_Left.gltf#AlsCurves";
const TURN_RIGHT_90_CURVE_ASSET: &str =
    "animations/TurnInPlace/A_Als_Crouch_Turn_90_Right.gltf#AlsCurves";
const TURN_LEFT_180_CURVE_ASSET: &str =
    "animations/TurnInPlace/A_Als_Crouch_Turn_180_Left.gltf#AlsCurves";
const TURN_RIGHT_180_CURVE_ASSET: &str =
    "animations/TurnInPlace/A_Als_Crouch_Turn_180_Right.gltf#AlsCurves";
const TRANSITION_LEFT_CURVE_ASSET: &str =
    "animations/Transitions/A_Als_Crouch_Transition_Left.gltf#AlsCurves";
const TRANSITION_RIGHT_CURVE_ASSET: &str =
    "animations/Transitions/A_Als_Crouch_Transition_Right.gltf#AlsCurves";
const DYNAMIC_LEFT_CURVE_ASSET: &str =
    "animations/Transitions/A_Als_Crouch_DynamicTransition_Left.gltf#AlsCurves";
const DYNAMIC_RIGHT_CURVE_ASSET: &str =
    "animations/Transitions/A_Als_Crouch_DynamicTransition_Right.gltf#AlsCurves";

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AlsCrouchingLayerSet;

#[derive(Resource)]
struct CrouchingCurveHandles {
    forward_backward: Handle<CurveAsset>,
    left: Handle<CurveAsset>,
    right: Handle<CurveAsset>,
    movement: Vec<Handle<AlsAnimCurves>>,
    rotate_left: Handle<AlsAnimCurves>,
    rotate_right: Handle<AlsAnimCurves>,
    turn_left_90: Handle<AlsAnimCurves>,
    turn_right_90: Handle<AlsAnimCurves>,
    turn_left_180: Handle<AlsAnimCurves>,
    turn_right_180: Handle<AlsAnimCurves>,
    transition_left: Handle<AlsAnimCurves>,
    transition_right: Handle<AlsAnimCurves>,
    dynamic_left: Handle<AlsAnimCurves>,
    dynamic_right: Handle<AlsAnimCurves>,
    skeleton: Handle<Skeleton>,
    left_leg_mask: Option<BoneMask>,
    right_leg_mask: Option<BoneMask>,
    stop_legs_mask: Option<BoneMask>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct VelocityBlend {
    forward: f32,
    backward: f32,
    right: f32,
    left: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum CrouchingMovementState {
    #[default]
    Forward,
    Backward,
    RightForward,
    RightBackward,
    LeftForward,
    LeftBackward,
}

impl CrouchingMovementState {
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

    fn cache_directions(self) -> [AlsMovementDirection; 4] {
        match self {
            Self::Forward => [
                AlsMovementDirection::Forward,
                AlsMovementDirection::Backward,
                AlsMovementDirection::LeftForward,
                AlsMovementDirection::RightForward,
            ],
            Self::Backward => [
                AlsMovementDirection::Forward,
                AlsMovementDirection::Backward,
                AlsMovementDirection::LeftBackward,
                AlsMovementDirection::RightBackward,
            ],
            Self::RightForward | Self::LeftBackward => [
                AlsMovementDirection::Forward,
                AlsMovementDirection::Backward,
                AlsMovementDirection::LeftBackward,
                AlsMovementDirection::RightForward,
            ],
            Self::RightBackward | Self::LeftForward => [
                AlsMovementDirection::Forward,
                AlsMovementDirection::Backward,
                AlsMovementDirection::LeftForward,
                AlsMovementDirection::RightBackward,
            ],
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
            (Self::Forward, Self::RightForward) => Some(2.0),
            (Self::Forward, Self::LeftForward) => Some(3.0),
            (Self::Forward, Self::Backward) => Some(0.0),
            (Self::Backward, Self::RightBackward) => Some(4.0),
            (Self::Backward, Self::LeftBackward) => Some(5.0),
            (Self::Backward, Self::Forward) => Some(1.0),
            (Self::RightForward, Self::LeftBackward) => Some(11.0),
            (Self::RightForward, Self::RightBackward)
                if feet_crossing <= 0.0 && hips_lock <= -0.5 =>
            {
                Some(18.0)
            }
            (Self::RightForward, Self::RightBackward) => Some(17.0),
            (Self::RightForward, Self::Forward) => Some(6.0),
            (Self::RightBackward, Self::LeftForward) => Some(12.0),
            (Self::RightBackward, Self::RightForward) if feet_crossing <= 0.0 => {
                if source_full_weight && hips_lock.abs() < 0.5 {
                    Some(21.0)
                } else if hips_lock >= 0.5 {
                    Some(22.0)
                } else {
                    Some(16.0)
                }
            }
            (Self::RightBackward, Self::RightForward) => Some(16.0),
            (Self::RightBackward, Self::Backward) => Some(9.0),
            (Self::LeftForward, Self::RightBackward) => Some(10.0),
            (Self::LeftForward, Self::LeftBackward) if feet_crossing <= 0.0 && hips_lock >= 0.5 => {
                Some(19.0)
            }
            (Self::LeftForward, Self::LeftBackward) => Some(15.0),
            (Self::LeftForward, Self::Forward) => Some(7.0),
            (Self::LeftBackward, Self::RightForward) => Some(13.0),
            (Self::LeftBackward, Self::LeftForward) if feet_crossing <= 0.0 => {
                if source_full_weight && hips_lock.abs() < 0.5 {
                    Some(20.0)
                } else if hips_lock <= -0.5 {
                    Some(23.0)
                } else {
                    Some(14.0)
                }
            }
            (Self::LeftBackward, Self::LeftForward) => Some(14.0),
            (Self::LeftBackward, Self::Backward) => Some(8.0),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct CrouchingOuterTransitionProfile {
    source_static_contract: &'static str,
    duration: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct CrouchingMovementTransitionProfile {
    source_static_contract: &'static str,
    duration: f32,
    blend_mode: AlphaBlendMode,
}

include!("crouching_transition_profiles.rs");

fn crouching_movement_transition_profile(index: f32) -> CrouchingMovementTransitionProfile {
    let index = index as usize;
    CROUCHING_MOVEMENT_TRANSITION_PROFILES
        .get(index)
        .copied()
        .unwrap_or(CROUCHING_MOVEMENT_TRANSITION_PROFILES[0])
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum CrouchingOuterState {
    #[default]
    Idle,
    Move,
    RotateLeft,
    RotateRight,
    Stop,
}

impl CrouchingOuterState {
    fn index(self) -> f32 {
        match self {
            Self::Idle => 0.0,
            Self::Move => 1.0,
            Self::RotateLeft => 2.0,
            Self::RotateRight => 3.0,
            Self::Stop => 4.0,
        }
    }

    fn is_rotate(self) -> bool {
        matches!(self, Self::RotateLeft | Self::RotateRight)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CrouchingOuterTransition {
    IdleToMove,
    IdleToRotateRight,
    IdleToRotateLeft,
    MoveToStop,
    MoveToIdleQuickStop,
    RotateLeftToIdleMoving,
    RotateLeftToIdleFinished,
    RotateLeftToRotateRight,
    RotateRightToIdleMoving,
    RotateRightToIdleFinished,
    RotateRightToRotateLeft,
    StopToIdle,
}

impl CrouchingOuterTransition {
    fn index(self) -> f32 {
        match self {
            Self::IdleToMove => 8.0,
            Self::IdleToRotateRight => 3.0,
            Self::IdleToRotateLeft => 0.0,
            Self::MoveToStop => 10.0,
            Self::MoveToIdleQuickStop => 9.0,
            Self::RotateLeftToIdleMoving => 1.0,
            Self::RotateLeftToIdleFinished => 2.0,
            Self::RotateLeftToRotateRight => 6.0,
            Self::RotateRightToIdleMoving => 4.0,
            Self::RotateRightToIdleFinished => 5.0,
            Self::RotateRightToRotateLeft => 7.0,
            Self::StopToIdle => 11.0,
        }
    }

    fn target(self) -> CrouchingOuterState {
        match self {
            Self::IdleToMove => CrouchingOuterState::Move,
            Self::IdleToRotateRight | Self::RotateLeftToRotateRight => {
                CrouchingOuterState::RotateRight
            }
            Self::IdleToRotateLeft | Self::RotateRightToRotateLeft => {
                CrouchingOuterState::RotateLeft
            }
            Self::MoveToStop => CrouchingOuterState::Stop,
            Self::MoveToIdleQuickStop
            | Self::RotateLeftToIdleMoving
            | Self::RotateLeftToIdleFinished
            | Self::RotateRightToIdleMoving
            | Self::RotateRightToIdleFinished
            | Self::StopToIdle => CrouchingOuterState::Idle,
        }
    }

    fn duration(self) -> f32 {
        let profile = CROUCHING_OUTER_TRANSITION_PROFILES[self.index() as usize];
        debug_assert!(!profile.source_static_contract.is_empty());
        profile.duration
    }
}

fn source_crouching_state_lifecycle_action(
    transition: Option<CrouchingOuterTransition>,
) -> Option<&'static str> {
    match transition {
        Some(CrouchingOuterTransition::MoveToStop) => {
            Some(SOURCE_CROUCHING_STOP_STATE_ENTRY_FUNCTION)
        }
        Some(
            CrouchingOuterTransition::IdleToMove
            | CrouchingOuterTransition::IdleToRotateRight
            | CrouchingOuterTransition::IdleToRotateLeft,
        ) => Some(SOURCE_CROUCHING_TURN_STOP_FUNCTION),
        _ => None,
    }
}

#[derive(Component, Clone, Debug, Default, PartialEq)]
struct CrouchingLayerRuntime {
    initialized: bool,
    previous_velocity: Vec3,
    velocity_blend: VelocityBlend,
    lean: Vec2,
    movement_curve_time: f32,
    movement_marker_phase: SyncPhaseState,
    movement_state: CrouchingMovementState,
    movement_transition_source: Option<CrouchingMovementState>,
    movement_transition_request: Option<f32>,
    movement_transition_remaining: f32,
    outer_state: CrouchingOuterState,
    outer_transition_remaining: f32,
    dynamic_transition_delay_frames: u8,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct SequenceCurves {
    foot_left_lock: f32,
    foot_right_lock: f32,
    allow_transitions: f32,
    rotation_yaw_speed: f32,
}

impl SequenceCurves {
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

    fn scale_rotation_yaw_speed(mut self, rate: f32) -> Self {
        self.rotation_yaw_speed *= rate.max(0.0);
        self
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct CrouchingGraphInputs {
    state_index: f32,
    transition_index: f32,
    movement_state_index: f32,
    movement_transition_index: f32,
    direction_index: f32,
    clip_index: f32,
    looped: f32,
    output_play_rate: f32,
    transition_duration: f32,
    pose_crouching: f32,
    pose_moving: f32,
    rotation_yaw_offset: f32,
    velocity_blend: VelocityBlend,
    stride_amount: f32,
    play_rate: f32,
    lean_right_amount: f32,
    lean_forward_amount: f32,
    movement_marker_phase: f32,
    rotation_play_rate: f32,
    rotate_left_loop: bool,
    rotate_right_loop: bool,
    delta_seconds: f32,
    rotate_clip_time: f32,
    turn_index: f32,
    turn_weight: f32,
    turn_play_rate: f32,
    foot_left_lock: f32,
    foot_right_lock: f32,
    allow_transitions: f32,
    rotation_yaw_speed: f32,
    feet_crossing_amount: f32,
    moving_smooth: bool,
    transition_kind: f32,
    transition_slot_index: f32,
    transition_slot_weight: f32,
    transition_slot_play_rate: f32,
    transition_slot_start_time: f32,
    start_stop_transition: bool,
    stop_transition_and_turn: bool,
    dynamic_transition_index: Option<f32>,
}

#[derive(Component, Debug, Default, Clone, Copy, PartialEq)]
pub struct AlsCrouchingGraphTelemetry {
    pub outer_state: Option<u8>,
    pub outer_transition_active: Option<bool>,
    pub runtime_movement_state: Option<u8>,
    pub runtime_movement_transition_active: Option<bool>,
    pub graph_movement_state_input: Option<f32>,
    pub graph_stride_amount: Option<f32>,
    pub graph_play_rate: Option<f32>,
    pub graph_marker_phase: Option<f32>,
    pub graph_pose_moving: Option<f32>,
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
                .before(AnimationGraphSet::PrePhysics),
        )
        .add_systems(Startup, load_crouching_curve_assets)
        .add_systems(
            PostUpdate,
            crouching_layer_writer.in_set(AlsCrouchingLayerSet),
        )
        .add_systems(
            PostUpdate,
            capture_crouching_graph_telemetry.after(AnimationGraphSet::Final),
        );
    }
}

fn load_crouching_curve_assets(mut commands: Commands, assets: Res<AssetServer>) {
    commands.insert_resource(CrouchingCurveHandles {
        forward_backward: assets.load(YAW_OFFSET_FORWARD_BACKWARD),
        left: assets.load(YAW_OFFSET_LEFT),
        right: assets.load(YAW_OFFSET_RIGHT),
        movement: MOVEMENT_CURVE_ASSETS
            .into_iter()
            .map(|path| assets.load(path))
            .collect(),
        rotate_left: assets.load(ROTATE_LEFT_CURVE_ASSET),
        rotate_right: assets.load(ROTATE_RIGHT_CURVE_ASSET),
        turn_left_90: assets.load(TURN_LEFT_90_CURVE_ASSET),
        turn_right_90: assets.load(TURN_RIGHT_90_CURVE_ASSET),
        turn_left_180: assets.load(TURN_LEFT_180_CURVE_ASSET),
        turn_right_180: assets.load(TURN_RIGHT_180_CURVE_ASSET),
        transition_left: assets.load(TRANSITION_LEFT_CURVE_ASSET),
        transition_right: assets.load(TRANSITION_RIGHT_CURVE_ASSET),
        dynamic_left: assets.load(DYNAMIC_LEFT_CURVE_ASSET),
        dynamic_right: assets.load(DYNAMIC_RIGHT_CURVE_ASSET),
        skeleton: assets.load(SKELETON_PATH),
        left_leg_mask: None,
        right_leg_mask: None,
        stop_legs_mask: None,
    });
}

fn update_leg_masks(handles: &mut CrouchingCurveHandles, skeletons: &Assets<Skeleton>) {
    let Some(skeleton) = skeletons.get(&handles.skeleton) else {
        return;
    };
    if handles.left_leg_mask.is_none() {
        handles.left_leg_mask = Some(build_branch_bone_mask(skeleton, LEFT_LEG_BRANCHES, 1.0));
    }
    if handles.right_leg_mask.is_none() {
        handles.right_leg_mask = Some(build_branch_bone_mask(skeleton, RIGHT_LEG_BRANCHES, 1.0));
    }
    if handles.stop_legs_mask.is_none() {
        handles.stop_legs_mask = Some(build_branch_bone_mask(skeleton, STOP_LEG_BRANCHES, 1.0));
    }
}

fn damper_alpha(delta_seconds: f32, half_life: f32) -> f32 {
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

fn animation_scale(transform: &Transform) -> f32 {
    transform.scale.x.abs().max(f32::EPSILON)
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
    params: &AlsMovementParams,
) -> Vec2 {
    let maximum = if acceleration.dot(velocity) >= 0.0 {
        params.acceleration.max_acceleration
    } else {
        params.acceleration.braking_deceleration
    };
    if maximum <= f32::EPSILON {
        return Vec2::ZERO;
    }
    (local_horizontal(acceleration, transform) / maximum).clamp_length_max(1.0)
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

fn crouching_movement_markers(state: CrouchingMovementState) -> &'static [SyncMarker] {
    match state {
        CrouchingMovementState::Forward => &CROUCH_FORWARD_MARKERS,
        CrouchingMovementState::RightForward => &CROUCH_RIGHT_FORWARD_MARKERS,
        CrouchingMovementState::RightBackward => &CROUCH_RIGHT_BACKWARD_MARKERS,
        CrouchingMovementState::Backward => &CROUCH_BACKWARD_MARKERS,
        CrouchingMovementState::LeftBackward => &CROUCH_LEFT_BACKWARD_MARKERS,
        CrouchingMovementState::LeftForward => &CROUCH_LEFT_FORWARD_MARKERS,
    }
}

fn advance_crouching_movement_marker_phase(
    runtime: &mut CrouchingLayerRuntime,
    active: bool,
    state: CrouchingMovementState,
    play_rate: f32,
    delta_seconds: f32,
) -> f32 {
    advance_marker_sync_phase(
        &mut runtime.movement_marker_phase,
        SyncMarkerPhaseInput {
            active,
            reset: false,
            rate: play_rate,
            leader_duration: CROUCH_MOVEMENT_DURATION,
            leader_markers: crouching_movement_markers(state),
            phase_duration: 1.0,
            phase_markers: &CROUCH_PHASE_MARKERS,
            start_time: 0.0,
            looped: true,
            delta_seconds,
        },
    )
}

fn direction_for(
    velocity: Vec3,
    view_yaw: f32,
    rotation_mode: RotationMode,
    current: Option<&AlsMovementDirectionState>,
) -> AlsMovementDirection {
    if rotation_mode == RotationMode::VelocityDirection {
        AlsMovementDirection::Forward
    } else if let Some(current) = current {
        current.direction
    } else {
        update_direction(
            None,
            view_space_direction_angle_from_radians(velocity_yaw_deg(velocity), view_yaw),
        )
    }
}

fn yaw_offset_curve<'a>(
    direction: AlsMovementDirection,
    handles: &'a CrouchingCurveHandles,
) -> &'a Handle<CurveAsset> {
    match direction {
        AlsMovementDirection::Forward | AlsMovementDirection::Backward => &handles.forward_backward,
        AlsMovementDirection::RightForward | AlsMovementDirection::RightBackward => &handles.right,
        AlsMovementDirection::LeftForward | AlsMovementDirection::LeftBackward => &handles.left,
    }
}

fn sample_curve(
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

fn sample_movement_curve(
    handles: &CrouchingCurveHandles,
    assets: &Assets<AlsAnimCurves>,
    direction: AlsMovementDirection,
    time: f32,
    name: &str,
) -> f32 {
    handles
        .movement
        .get(direction_index(direction) as usize)
        .map_or(0.0, |handle| sample_curve(assets, handle, name, time))
}

fn sample_movement_state_curve(
    handles: &CrouchingCurveHandles,
    assets: &Assets<AlsAnimCurves>,
    state: CrouchingMovementState,
    blend: VelocityBlend,
    time: f32,
    name: &str,
) -> f32 {
    let weights = [blend.forward, blend.backward, blend.left, blend.right];
    let total = weights.iter().sum::<f32>();
    if total <= f32::EPSILON {
        return sample_movement_curve(handles, assets, state.direction(), time, name);
    }
    state
        .cache_directions()
        .into_iter()
        .zip(weights)
        .map(|(direction, weight)| {
            sample_movement_curve(handles, assets, direction, time, name) * weight
        })
        .sum::<f32>()
        / total
}

fn sample_sequence_curves(
    assets: &Assets<AlsAnimCurves>,
    handle: &Handle<AlsAnimCurves>,
    time: f32,
) -> SequenceCurves {
    SequenceCurves {
        foot_left_lock: sample_curve(assets, handle, "FootLeftLock", time).clamp(0.0, 1.0),
        foot_right_lock: sample_curve(assets, handle, "FootRightLock", time).clamp(0.0, 1.0),
        allow_transitions: sample_curve(assets, handle, "AllowTransitions", time),
        rotation_yaw_speed: sample_curve(assets, handle, "RotationYawSpeed", time),
    }
}

fn rotate_curve_handle<'a>(
    handles: &'a CrouchingCurveHandles,
    turn: &AlsTurnDrive,
) -> &'a Handle<AlsAnimCurves> {
    if turn.rotate_lr <= 0.5 {
        &handles.rotate_left
    } else {
        &handles.rotate_right
    }
}

fn turn_curve_handle<'a>(
    handles: &'a CrouchingCurveHandles,
    turn: &AlsTurnDrive,
) -> &'a Handle<AlsAnimCurves> {
    match (turn.is_180, turn.lr <= 0.5) {
        (false, true) => &handles.turn_left_90,
        (false, false) => &handles.turn_right_90,
        (true, true) => &handles.turn_left_180,
        (true, false) => &handles.turn_right_180,
    }
}

fn transition_curve_handle<'a>(
    handles: &'a CrouchingCurveHandles,
    transition: &AlsTransitionPlayback,
) -> &'a Handle<AlsAnimCurves> {
    match (transition.kind, transition.index <= 0.5) {
        (AlsTransitionKind::CrouchingDynamic, true) => &handles.dynamic_left,
        (AlsTransitionKind::CrouchingDynamic, false) => &handles.dynamic_right,
        (_, true) => &handles.transition_left,
        (_, false) => &handles.transition_right,
    }
}

fn rotate_finished(turn: Option<&AlsTurnDrive>) -> bool {
    turn.is_some_and(|turn| {
        !turn.rotate_active && turn.rotate_clip_time >= ROTATE_DUR - ROTATE_COMPLETION_EPSILON
    })
}

fn outer_transition_for(
    runtime: &CrouchingLayerRuntime,
    moving_smooth: bool,
    turn: Option<&AlsTurnDrive>,
) -> Option<CrouchingOuterTransition> {
    let rotate_side = turn
        .filter(|turn| turn.rotate_active)
        .map(|turn| turn.rotate_lr <= 0.5);
    match runtime.outer_state {
        CrouchingOuterState::Idle => {
            if moving_smooth {
                Some(CrouchingOuterTransition::IdleToMove)
            } else if rotate_side == Some(false) {
                Some(CrouchingOuterTransition::IdleToRotateRight)
            } else if rotate_side == Some(true) {
                Some(CrouchingOuterTransition::IdleToRotateLeft)
            } else {
                None
            }
        }
        CrouchingOuterState::Move => {
            if moving_smooth {
                None
            } else if runtime.outer_transition_remaining > f32::EPSILON {
                Some(CrouchingOuterTransition::MoveToIdleQuickStop)
            } else {
                Some(CrouchingOuterTransition::MoveToStop)
            }
        }
        CrouchingOuterState::RotateLeft => {
            if moving_smooth {
                Some(CrouchingOuterTransition::RotateLeftToIdleMoving)
            } else if rotate_side == Some(false) {
                Some(CrouchingOuterTransition::RotateLeftToRotateRight)
            } else if rotate_finished(turn) {
                Some(CrouchingOuterTransition::RotateLeftToIdleFinished)
            } else {
                None
            }
        }
        CrouchingOuterState::RotateRight => {
            if moving_smooth {
                Some(CrouchingOuterTransition::RotateRightToIdleMoving)
            } else if rotate_side == Some(true) {
                Some(CrouchingOuterTransition::RotateRightToRotateLeft)
            } else if rotate_finished(turn) {
                Some(CrouchingOuterTransition::RotateRightToIdleFinished)
            } else {
                None
            }
        }
        CrouchingOuterState::Stop => (runtime.outer_transition_remaining <= f32::EPSILON)
            .then_some(CrouchingOuterTransition::StopToIdle),
    }
}

fn transition_signal(transition: Option<&AlsTransitionPlayback>) -> (f32, f32, f32, f32, f32) {
    let Some(transition) =
        transition.filter(|transition| transition.active && transition.is_crouching())
    else {
        return (0.0, 0.0, 0.0, 1.0, 0.0);
    };
    let kind = match transition.kind {
        AlsTransitionKind::CrouchingStop => 1.0,
        AlsTransitionKind::CrouchingQuickStop => 2.0,
        AlsTransitionKind::CrouchingDynamic => 3.0,
        AlsTransitionKind::Stop | AlsTransitionKind::QuickStop => 0.0,
    };
    (
        kind,
        transition.index,
        transition.weight,
        transition.play_rate,
        transition.start_time,
    )
}

fn dynamic_transition_index(
    foot_lock: Option<&AlsFootLockState>,
    scale: f32,
    allow_transitions: bool,
) -> Option<f32> {
    if !allow_transitions || !foot_lock.is_some_and(|state| state.valid) {
        return None;
    }
    let foot_lock = foot_lock?;
    let threshold = DYNAMIC_FOOT_LOCK_DISTANCE_METERS * scale.max(f32::EPSILON);
    let threshold_squared = threshold * threshold;
    let left_allowed = is_relevant_animation_weight(foot_lock.left.lock_amount)
        && foot_lock_distance_squared(&foot_lock.left) > threshold_squared;
    let right_allowed = is_relevant_animation_weight(foot_lock.right.lock_amount)
        && foot_lock_distance_squared(&foot_lock.right) > threshold_squared;
    match (left_allowed, right_allowed) {
        (false, false) => None,
        (false, true) => Some(1.0),
        (true, false) => Some(0.0),
        (true, true) => (foot_lock_distance_squared(&foot_lock.left)
            < foot_lock_distance_squared(&foot_lock.right))
        .then_some(1.0)
        .or(Some(0.0)),
    }
}

#[allow(clippy::too_many_arguments)]
fn update_crouching_inputs(
    runtime: &mut CrouchingLayerRuntime,
    delta_seconds: f32,
    transform: &Transform,
    actual_motion: &AlsActualMotion,
    params: &AlsMovementParams,
    locomotion: LocomotionState,
    rotation_mode: RotationMode,
    view_yaw: f32,
    input: &AlsInputCommand,
    direction_state: Option<&AlsMovementDirectionState>,
    turn: Option<&AlsTurnDrive>,
    transition: Option<&AlsTransitionPlayback>,
    foot_lock: Option<&AlsFootLockState>,
    hips_lock: f32,
    curves: &Assets<CurveAsset>,
    animation_curves: &Assets<AlsAnimCurves>,
    handles: &CrouchingCurveHandles,
) -> CrouchingGraphInputs {
    let velocity = actual_motion.world_velocity;
    let horizontal_speed = actual_motion.horizontal_speed;
    let has_input = input.move_dir.length_squared() > f32::EPSILON;
    let has_velocity = horizontal_speed >= VELOCITY_EPSILON;
    let moving_smooth =
        (has_input && has_velocity) || horizontal_speed > MOVING_SMOOTH_SPEED_THRESHOLD;
    let acceleration = if runtime.initialized && delta_seconds > f32::EPSILON {
        (velocity - runtime.previous_velocity) / delta_seconds
    } else {
        Vec3::ZERO
    };
    let blend_target = VelocityBlend {
        forward: actual_motion.velocity_blend.forward,
        backward: actual_motion.velocity_blend.backward,
        right: actual_motion.velocity_blend.right,
        left: actual_motion.velocity_blend.left,
    };
    let lean_target = acceleration_amount(acceleration, velocity, transform, params);
    if runtime.initialized {
        runtime.velocity_blend = damp_velocity_blend(
            runtime.velocity_blend,
            blend_target,
            damper_alpha(delta_seconds, VELOCITY_BLEND_HALF_LIFE),
        );
        runtime.lean = runtime
            .lean
            .lerp(lean_target, damper_alpha(delta_seconds, LEAN_HALF_LIFE));
    } else {
        runtime.velocity_blend = blend_target;
        runtime.lean = lean_target;
    }

    let animation_speed = horizontal_speed / animation_scale(transform);
    let stride_amount = stride_blend_amount_for_speed(0.0, animation_speed);
    let play_rate = (animation_speed
        / (params.crouch.animated_crouch_speed.max(f32::EPSILON)
            * stride_amount.max(f32::EPSILON)))
    .clamp(MIN_PLAY_RATE, 2.0);
    let direction = direction_for(velocity, view_yaw, rotation_mode, direction_state);
    let desired_movement_state = CrouchingMovementState::from_direction(direction);
    if moving_smooth {
        runtime.movement_curve_time += play_rate * delta_seconds.max(0.0);
    }
    let feet_crossing_amount = sample_movement_state_curve(
        handles,
        animation_curves,
        runtime.movement_state,
        runtime.velocity_blend,
        runtime.movement_curve_time,
        "FeetCrossing",
    )
    .clamp(0.0, 1.0);
    runtime.movement_transition_remaining =
        (runtime.movement_transition_remaining - delta_seconds.max(0.0)).max(0.0);
    if runtime.movement_transition_remaining <= f32::EPSILON {
        runtime.movement_transition_source = None;
        runtime.movement_transition_request = None;
    }
    runtime.outer_transition_remaining =
        (runtime.outer_transition_remaining - delta_seconds.max(0.0)).max(0.0);
    let outer_transition = (locomotion == LocomotionState::Grounded)
        .then(|| outer_transition_for(runtime, moving_smooth, turn))
        .flatten();
    if let Some(outer_transition) = outer_transition {
        runtime.outer_state = outer_transition.target();
        runtime.outer_transition_remaining = outer_transition.duration();
        if outer_transition == CrouchingOuterTransition::IdleToMove {
            runtime.movement_state = CrouchingMovementState::Forward;
            runtime.movement_transition_source = None;
            runtime.movement_transition_request = None;
            runtime.movement_transition_remaining = 0.0;
        }
    }
    let lifecycle_action = source_crouching_state_lifecycle_action(outer_transition);
    let new_movement_transition = if runtime.outer_state == CrouchingOuterState::Move
        && runtime.movement_state != desired_movement_state
    {
        runtime.movement_state.transition_index(
            desired_movement_state,
            hips_lock,
            feet_crossing_amount,
            runtime.movement_transition_remaining <= f32::EPSILON,
        )
    } else {
        None
    };
    if let Some(index) = new_movement_transition {
        let profile = crouching_movement_transition_profile(index);
        runtime.movement_transition_source = Some(runtime.movement_state);
        runtime.movement_state = desired_movement_state;
        runtime.movement_transition_request = Some(index);
        runtime.movement_transition_remaining = profile.duration;
    }
    let movement_transition_index = runtime.movement_transition_request.unwrap_or(-1.0);
    let velocity_yaw = if horizontal_speed >= VELOCITY_EPSILON {
        velocity_yaw_deg(velocity)
    } else {
        view_yaw.to_degrees()
    };
    let yaw_angle = view_space_direction_angle_from_radians(velocity_yaw, view_yaw);
    let target_yaw_offset = curves
        .get(yaw_offset_curve(
            runtime.movement_state.direction(),
            handles,
        ))
        .map(|curve| curve.sample(yaw_angle))
        .unwrap_or(0.0);
    let rotation_yaw_offset = runtime
        .movement_transition_source
        .filter(|_| runtime.movement_transition_remaining > f32::EPSILON)
        .map(|source| {
            let source_yaw_offset = curves
                .get(yaw_offset_curve(source.direction(), handles))
                .map(|curve| curve.sample(yaw_angle))
                .unwrap_or(0.0);
            let profile = runtime
                .movement_transition_request
                .map(crouching_movement_transition_profile)
                .unwrap_or(CROUCHING_MOVEMENT_TRANSITION_PROFILES[0]);
            let progress = 1.0 - runtime.movement_transition_remaining / profile.duration;
            source_yaw_offset.lerp(
                target_yaw_offset,
                transition_alpha(progress, profile.blend_mode, &[]),
            )
        })
        .unwrap_or(target_yaw_offset);

    let state_index = runtime.outer_state.index();
    let rotating = runtime.outer_state.is_rotate().then_some(turn).flatten();
    let turn_weight = turn
        .filter(|turn| !turn.rotate_active)
        .map_or(0.0, |turn| turn.weight);
    let turn_index = turn.map_or(0.0, |turn| match (turn.is_180, turn.lr <= 0.5) {
        (false, true) => 0.0,
        (false, false) => 1.0,
        (true, true) => 2.0,
        (true, false) => 3.0,
    });
    let turn_play_rate = turn.map_or(1.2, |turn| turn.play_rate.max(0.0));
    let rotation_play_rate = rotating.map_or(1.15, |turn| turn.rotate_rate.max(0.0));
    let rotate_left_loop = turn.is_some_and(|turn| turn.rotate_active && turn.rotate_lr <= 0.5);
    let rotate_right_loop = turn.is_some_and(|turn| turn.rotate_active && turn.rotate_lr > 0.5);
    let mut sequence_curves = match runtime.outer_state {
        CrouchingOuterState::Idle => SequenceCurves {
            foot_left_lock: 1.0,
            foot_right_lock: 1.0,
            allow_transitions: 1.0,
            rotation_yaw_speed: 0.0,
        },
        CrouchingOuterState::Stop => SequenceCurves {
            foot_left_lock: 1.0,
            foot_right_lock: 1.0,
            allow_transitions: 0.0,
            rotation_yaw_speed: 0.0,
        },
        _ => SequenceCurves::default(),
    };
    if let Some(turn) = rotating {
        sequence_curves = sample_sequence_curves(
            animation_curves,
            rotate_curve_handle(handles, turn),
            turn.rotate_clip_time,
        )
        .scale_rotation_yaw_speed(turn.rotate_rate);
    }
    if let Some(turn) = turn.filter(|turn| !turn.rotate_active && turn.weight > f32::EPSILON) {
        sequence_curves = sequence_curves.blend(
            sample_sequence_curves(
                animation_curves,
                turn_curve_handle(handles, turn),
                turn.clip_time,
            )
            .scale_rotation_yaw_speed(turn.play_rate),
            turn.weight,
        );
    }
    if let Some(transition) =
        transition.filter(|transition| transition.active && transition.is_crouching())
    {
        sequence_curves = sequence_curves.blend(
            sample_sequence_curves(
                animation_curves,
                transition_curve_handle(handles, transition),
                transition.clip_time,
            ),
            transition.weight,
        );
    }
    if runtime.dynamic_transition_delay_frames > 0 {
        runtime.dynamic_transition_delay_frames -= 1;
    }
    let dynamic_transition_index = (runtime.outer_state == CrouchingOuterState::Idle
        && runtime.dynamic_transition_delay_frames == 0
        && transition.is_none_or(|transition| !transition.active || !transition.is_crouching()))
    .then(|| {
        dynamic_transition_index(
            foot_lock,
            animation_scale(transform),
            sequence_curves.allow_transitions >= 1.0 - 1e-4,
        )
    })
    .flatten();
    if dynamic_transition_index.is_some() {
        runtime.dynamic_transition_delay_frames = 2;
    }
    let (
        transition_kind,
        transition_slot_index,
        transition_slot_weight,
        transition_slot_play_rate,
        transition_slot_start_time,
    ) = transition_signal(transition);
    let (clip_index, looped, output_play_rate, transition_duration) = match runtime.outer_state {
        CrouchingOuterState::Idle => (0.0, 1.0, 1.0, 0.0),
        CrouchingOuterState::Move => (direction_index(direction) + 1.0, 1.0, play_rate, 0.2),
        CrouchingOuterState::RotateLeft => (
            7.0,
            turn.is_some_and(|turn| turn.rotate_active) as u8 as f32,
            rotation_play_rate,
            0.2,
        ),
        CrouchingOuterState::RotateRight => (
            8.0,
            turn.is_some_and(|turn| turn.rotate_active) as u8 as f32,
            rotation_play_rate,
            0.2,
        ),
        CrouchingOuterState::Stop => (9.0, 1.0, play_rate, 0.1),
    };
    let movement_marker_phase = advance_crouching_movement_marker_phase(
        runtime,
        runtime.outer_state == CrouchingOuterState::Move && locomotion == LocomotionState::Grounded,
        runtime.movement_state,
        play_rate,
        delta_seconds,
    );
    runtime.previous_velocity = velocity;
    runtime.initialized = true;
    CrouchingGraphInputs {
        state_index,
        transition_index: outer_transition.map_or(-1.0, CrouchingOuterTransition::index),
        movement_state_index: runtime.movement_state.index(),
        movement_transition_index,
        direction_index: direction_index(direction),
        clip_index,
        looped,
        output_play_rate,
        transition_duration,
        pose_crouching: 1.0,
        pose_moving: matches!(
            runtime.outer_state,
            CrouchingOuterState::Move | CrouchingOuterState::Stop
        ) as u8 as f32,
        rotation_yaw_offset: (runtime.outer_state == CrouchingOuterState::Move) as u8 as f32
            * rotation_yaw_offset,
        velocity_blend: runtime.velocity_blend,
        stride_amount,
        play_rate,
        lean_right_amount: runtime.lean.y,
        lean_forward_amount: runtime.lean.x,
        movement_marker_phase,
        rotation_play_rate,
        rotate_left_loop,
        rotate_right_loop,
        delta_seconds,
        rotate_clip_time: rotating.map_or(0.0, |turn| turn.rotate_clip_time),
        turn_index,
        turn_weight,
        turn_play_rate,
        foot_left_lock: sequence_curves.foot_left_lock,
        foot_right_lock: sequence_curves.foot_right_lock,
        allow_transitions: sequence_curves.allow_transitions,
        rotation_yaw_speed: sequence_curves.rotation_yaw_speed,
        feet_crossing_amount,
        moving_smooth,
        transition_kind,
        transition_slot_index,
        transition_slot_weight,
        transition_slot_play_rate,
        transition_slot_start_time,
        start_stop_transition: lifecycle_action == Some(SOURCE_CROUCHING_STOP_STATE_ENTRY_FUNCTION),
        stop_transition_and_turn: lifecycle_action == Some(SOURCE_CROUCHING_TURN_STOP_FUNCTION),
        dynamic_transition_index,
    }
}

fn write_crouching_inputs(player: &mut AnimationGraphPlayer, inputs: CrouchingGraphInputs) {
    for (key, value) in [
        ("als_crouching_state_index", inputs.state_index),
        ("als_crouching_transition_index", inputs.transition_index),
        (
            "als_crouching_movement_state_index",
            inputs.movement_state_index,
        ),
        (
            "als_crouching_movement_transition_index",
            inputs.movement_transition_index,
        ),
        ("als_crouching_direction_index", inputs.direction_index),
        ("als_crouching_clip_index", inputs.clip_index),
        ("als_crouching_loop", inputs.looped),
        ("als_crouching_output_play_rate", inputs.output_play_rate),
        (
            "als_crouching_transition_duration",
            inputs.transition_duration,
        ),
        ("als_crouching_curve_pose_crouching", inputs.pose_crouching),
        ("als_crouching_curve_pose_moving", inputs.pose_moving),
        (
            "als_crouching_curve_rotation_yaw_offset",
            inputs.rotation_yaw_offset,
        ),
        ("als_crouching_curve_foot_left_lock", inputs.foot_left_lock),
        (
            "als_crouching_curve_foot_right_lock",
            inputs.foot_right_lock,
        ),
        (
            "als_crouching_curve_allow_transitions",
            inputs.allow_transitions,
        ),
        (
            "als_crouching_curve_rotation_yaw_speed",
            inputs.rotation_yaw_speed,
        ),
        (
            "als_crouching_velocity_forward_amount",
            inputs.velocity_blend.forward,
        ),
        (
            "als_crouching_velocity_backward_amount",
            inputs.velocity_blend.backward,
        ),
        (
            "als_crouching_velocity_right_amount",
            inputs.velocity_blend.right,
        ),
        (
            "als_crouching_velocity_left_amount",
            inputs.velocity_blend.left,
        ),
        ("als_crouching_stride_amount", inputs.stride_amount),
        ("als_crouching_play_rate", inputs.play_rate),
        ("als_crouching_source_pose_frame", 0.0),
        ("als_crouching_lean_right_amount", inputs.lean_right_amount),
        (
            "als_crouching_lean_forward_amount",
            inputs.lean_forward_amount,
        ),
        (
            "als_crouching_movement_marker_phase",
            inputs.movement_marker_phase,
        ),
        ("als_movement_marker_phase", inputs.movement_marker_phase),
        (
            "als_crouching_rotation_play_rate",
            inputs.rotation_play_rate,
        ),
        ("als_crouching_delta_seconds", inputs.delta_seconds),
        ("als_crouching_rotate_clip_time", inputs.rotate_clip_time),
        ("als_crouching_turn_index", inputs.turn_index),
        ("als_crouching_turn_weight", inputs.turn_weight),
        ("als_crouching_turn_play_rate", inputs.turn_play_rate),
        ("als_crouching_transition_kind", inputs.transition_kind),
        (
            "als_crouching_transition_slot_index",
            inputs.transition_slot_index,
        ),
        (
            "als_crouching_transition_slot_weight",
            inputs.transition_slot_weight,
        ),
        (
            "als_crouching_transition_slot_play_rate",
            inputs.transition_slot_play_rate,
        ),
        (
            "als_crouching_transition_slot_start_time",
            inputs.transition_slot_start_time,
        ),
    ] {
        player.set_global_input_data(key, value);
    }
    player.set_global_input_data("als_crouching_rotate_left_loop", inputs.rotate_left_loop);
    player.set_global_input_data("als_crouching_rotate_right_loop", inputs.rotate_right_loop);
}

fn write_leg_masks(player: &mut AnimationGraphPlayer, handles: &CrouchingCurveHandles) {
    player.set_global_input_data(
        "als_crouching_stop_left_leg_mask",
        DataValue::BoneMask(handles.left_leg_mask.clone().unwrap_or_else(BoneMask::none)),
    );
    player.set_global_input_data(
        "als_crouching_stop_right_leg_mask",
        DataValue::BoneMask(
            handles
                .right_leg_mask
                .clone()
                .unwrap_or_else(BoneMask::none),
        ),
    );
    player.set_global_input_data(
        "als_crouching_stop_legs_mask",
        DataValue::BoneMask(
            handles
                .stop_legs_mask
                .clone()
                .unwrap_or_else(BoneMask::none),
        ),
    );
}

fn crouching_dominant_velocity(player: &AnimationGraphPlayer) -> (Option<u8>, Option<f32>) {
    let keys = [
        "als_crouching_velocity_forward_amount",
        "als_crouching_velocity_backward_amount",
        "als_crouching_velocity_left_amount",
        "als_crouching_velocity_right_amount",
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

fn capture_crouching_graph_telemetry(
    mut commands: Commands,
    avatars: Query<
        (
            &ChildOf,
            &AnimatedSceneInstance,
            Option<&CrouchingLayerRuntime>,
        ),
        With<AlsCharacterAvatar>,
    >,
    players: Query<&AnimationGraphPlayer>,
    mut characters: Query<Option<&mut AlsCrouchingGraphTelemetry>, With<AlsCharacter>>,
) {
    for (parent, instance, runtime) in &avatars {
        let Ok(player) = players.get(instance.player_entity()) else {
            continue;
        };
        let (dominant_velocity_index, dominant_velocity_weight) =
            crouching_dominant_velocity(player);
        let telemetry = AlsCrouchingGraphTelemetry {
            outer_state: runtime.map(|value| value.outer_state.index() as u8),
            outer_transition_active: runtime
                .map(|value| value.outer_transition_remaining > f32::EPSILON),
            runtime_movement_state: runtime.map(|value| value.movement_state.index() as u8),
            runtime_movement_transition_active: runtime
                .map(|value| value.movement_transition_remaining > f32::EPSILON),
            graph_movement_state_input: player
                .get_global_input_data("als_crouching_movement_state_index")
                .and_then(|value| value.as_f32().ok()),
            graph_stride_amount: player
                .get_global_input_data("als_crouching_stride_amount")
                .and_then(|value| value.as_f32().ok()),
            graph_play_rate: player
                .get_global_input_data("als_crouching_play_rate")
                .and_then(|value| value.as_f32().ok()),
            graph_marker_phase: player
                .get_global_input_data("als_crouching_movement_marker_phase")
                .and_then(|value| value.as_f32().ok()),
            graph_pose_moving: player
                .get_global_input_data("als_crouching_curve_pose_moving")
                .and_then(|value| value.as_f32().ok()),
            dominant_velocity_index,
            dominant_velocity_weight,
            lean_right_amount: player
                .get_global_input_data("als_crouching_lean_right_amount")
                .and_then(|value| value.as_f32().ok()),
            lean_forward_amount: player
                .get_global_input_data("als_crouching_lean_forward_amount")
                .and_then(|value| value.as_f32().ok()),
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

#[allow(clippy::type_complexity)]
fn crouching_layer_writer(
    time: Res<Time>,
    mut commands: Commands,
    mut characters: Query<
        (
            &Transform,
            &AlsActualMotion,
            &AlsMovementParamsHandle,
            &LocomotionState,
            &Stance,
            &RotationMode,
            &AlsViewYaw,
            &AlsInputCommand,
            Option<&AlsInputOverride>,
            Option<&AlsMovementDirectionState>,
            Option<&mut AlsTransitionPlayback>,
            Option<&AlsFootLockState>,
        ),
        With<AlsCharacter>,
    >,
    mut avatars: Query<
        (
            Entity,
            &ChildOf,
            &AnimatedSceneInstance,
            Option<&mut CrouchingLayerRuntime>,
            Option<&AlsTurnDrive>,
            Option<&mut AlsTurnMontageStack>,
        ),
        With<AlsCharacterAvatar>,
    >,
    params: Res<Assets<AlsMovementParams>>,
    curves: Res<Assets<CurveAsset>>,
    animation_curves: Res<Assets<AlsAnimCurves>>,
    mut curve_handles: ResMut<CrouchingCurveHandles>,
    skeletons: Res<Assets<Skeleton>>,
    mut players: Query<&mut AnimationGraphPlayer>,
) {
    update_leg_masks(&mut curve_handles, &skeletons);
    let delta_seconds = time.delta_secs();
    for (avatar, parent, instance, current_runtime, turn, mut turn_stack) in &mut avatars {
        let Ok((
            transform,
            actual_motion,
            handle,
            locomotion,
            stance,
            rotation_mode,
            view_yaw,
            command,
            override_command,
            direction_state,
            mut transition,
            foot_lock,
        )) = characters.get_mut(parent.parent())
        else {
            continue;
        };
        if *stance != Stance::Crouching {
            if let Some(turn_stack) = turn_stack.as_deref_mut() {
                turn_stack.request_stop();
            }
            if let Some(transition) = transition.as_deref_mut() {
                transition.request_stop();
            }
            commands.entity(avatar).remove::<CrouchingLayerRuntime>();
            continue;
        }
        let Some(movement_params) = params.get(handle.id()) else {
            continue;
        };
        let Ok(mut player) = players.get_mut(instance.player_entity()) else {
            continue;
        };
        let hips_lock = player
            .get_global_input_data("als_crouching_hips_direction_lock_amount")
            .and_then(|value| value.as_f32().ok())
            .unwrap_or(0.0)
            .clamp(-1.0, 1.0);
        let mut runtime = current_runtime.as_deref().cloned().unwrap_or_default();
        let inputs = update_crouching_inputs(
            &mut runtime,
            delta_seconds,
            transform,
            actual_motion,
            movement_params,
            *locomotion,
            *rotation_mode,
            view_yaw.0,
            override_command.map_or(command, |value| &value.0),
            direction_state,
            turn,
            transition.as_deref(),
            foot_lock,
            hips_lock,
            &curves,
            &animation_curves,
            &curve_handles,
        );
        if let Some(mut current_runtime) = current_runtime {
            *current_runtime = runtime;
        } else {
            commands.entity(avatar).insert(runtime);
        }
        if inputs.stop_transition_and_turn {
            if let Some(turn_stack) = turn_stack.as_deref_mut() {
                turn_stack.request_stop();
            }
            if let Some(transition) = transition.as_deref_mut() {
                transition.request_stop();
            }
        }
        if inputs.start_stop_transition {
            commands
                .entity(parent.parent())
                .insert(AlsTransitionPlayback::crouching_stop_right());
        }
        if let Some(index) = inputs.dynamic_transition_index {
            commands
                .entity(parent.parent())
                .insert(AlsTransitionPlayback::crouching_dynamic(index));
        }
        write_crouching_inputs(&mut player, inputs);
        write_leg_masks(&mut player, &curve_handles);
        commands.entity(parent.parent()).insert(AlsFootLockInput {
            left_lock_amount: inputs.foot_left_lock,
            right_lock_amount: inputs.foot_right_lock,
            allow_transitions: inputs.allow_transitions >= 1.0 - 1e-4,
            moving_smooth: inputs.moving_smooth,
            feet_crossing_amount: inputs.feet_crossing_amount,
            ..Default::default()
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crouching_direction_uses_the_source_view_yaw_sign() {
        assert_eq!(
            direction_for(
                Vec3::NEG_X,
                (-45.0_f32).to_radians(),
                RotationMode::Aiming,
                None,
            ),
            AlsMovementDirection::LeftBackward
        );
    }

    #[test]
    fn source_crouching_state_indices_match_baked_state_machine() {
        assert_eq!(CrouchingOuterState::Idle.index(), 0.0);
        assert_eq!(CrouchingOuterState::Move.index(), 1.0);
        assert_eq!(CrouchingOuterState::RotateLeft.index(), 2.0);
        assert_eq!(CrouchingOuterState::RotateRight.index(), 3.0);
        assert_eq!(CrouchingOuterState::Stop.index(), 4.0);
    }

    #[test]
    fn source_crouching_movement_state_indices_match_baked_state_machine() {
        assert_eq!(CrouchingMovementState::Forward.index(), 0.0);
        assert_eq!(CrouchingMovementState::Backward.index(), 1.0);
        assert_eq!(CrouchingMovementState::RightForward.index(), 2.0);
        assert_eq!(CrouchingMovementState::RightBackward.index(), 3.0);
        assert_eq!(CrouchingMovementState::LeftForward.index(), 4.0);
        assert_eq!(CrouchingMovementState::LeftBackward.index(), 5.0);
    }

    #[test]
    fn source_crouching_direction_transition_is_seven_tenths_second() {
        assert!(
            CROUCHING_MOVEMENT_TRANSITION_PROFILES
                .iter()
                .all(|profile| (profile.duration - 0.7).abs() < f32::EPSILON)
        );
    }

    #[test]
    fn source_crouching_transition_indices_follow_serialized_source_order() {
        let movement_cases = [
            (
                CrouchingMovementState::Forward,
                CrouchingMovementState::RightForward,
                0.0,
                1.0,
                true,
                2.0,
            ),
            (
                CrouchingMovementState::Forward,
                CrouchingMovementState::LeftForward,
                0.0,
                1.0,
                true,
                3.0,
            ),
            (
                CrouchingMovementState::Forward,
                CrouchingMovementState::Backward,
                0.0,
                1.0,
                true,
                0.0,
            ),
            (
                CrouchingMovementState::Backward,
                CrouchingMovementState::RightBackward,
                0.0,
                1.0,
                true,
                4.0,
            ),
            (
                CrouchingMovementState::Backward,
                CrouchingMovementState::LeftBackward,
                0.0,
                1.0,
                true,
                5.0,
            ),
            (
                CrouchingMovementState::Backward,
                CrouchingMovementState::Forward,
                0.0,
                1.0,
                true,
                1.0,
            ),
            (
                CrouchingMovementState::RightForward,
                CrouchingMovementState::LeftBackward,
                0.0,
                1.0,
                true,
                11.0,
            ),
            (
                CrouchingMovementState::RightForward,
                CrouchingMovementState::RightBackward,
                -0.5,
                0.0,
                true,
                18.0,
            ),
            (
                CrouchingMovementState::RightForward,
                CrouchingMovementState::RightBackward,
                0.0,
                1.0,
                true,
                17.0,
            ),
            (
                CrouchingMovementState::RightForward,
                CrouchingMovementState::Forward,
                0.0,
                1.0,
                true,
                6.0,
            ),
            (
                CrouchingMovementState::RightBackward,
                CrouchingMovementState::LeftForward,
                0.0,
                1.0,
                true,
                12.0,
            ),
            (
                CrouchingMovementState::RightBackward,
                CrouchingMovementState::RightForward,
                0.0,
                0.0,
                true,
                21.0,
            ),
            (
                CrouchingMovementState::RightBackward,
                CrouchingMovementState::RightForward,
                0.5,
                0.0,
                false,
                22.0,
            ),
            (
                CrouchingMovementState::RightBackward,
                CrouchingMovementState::RightForward,
                0.0,
                0.0,
                false,
                16.0,
            ),
            (
                CrouchingMovementState::RightBackward,
                CrouchingMovementState::Backward,
                0.0,
                1.0,
                true,
                9.0,
            ),
            (
                CrouchingMovementState::LeftForward,
                CrouchingMovementState::RightBackward,
                0.0,
                1.0,
                true,
                10.0,
            ),
            (
                CrouchingMovementState::LeftForward,
                CrouchingMovementState::LeftBackward,
                0.5,
                0.0,
                true,
                19.0,
            ),
            (
                CrouchingMovementState::LeftForward,
                CrouchingMovementState::LeftBackward,
                0.0,
                1.0,
                true,
                15.0,
            ),
            (
                CrouchingMovementState::LeftForward,
                CrouchingMovementState::Forward,
                0.0,
                1.0,
                true,
                7.0,
            ),
            (
                CrouchingMovementState::LeftBackward,
                CrouchingMovementState::RightForward,
                0.0,
                1.0,
                true,
                13.0,
            ),
            (
                CrouchingMovementState::LeftBackward,
                CrouchingMovementState::LeftForward,
                0.0,
                0.0,
                true,
                20.0,
            ),
            (
                CrouchingMovementState::LeftBackward,
                CrouchingMovementState::LeftForward,
                -0.5,
                0.0,
                false,
                23.0,
            ),
            (
                CrouchingMovementState::LeftBackward,
                CrouchingMovementState::LeftForward,
                0.0,
                0.0,
                false,
                14.0,
            ),
            (
                CrouchingMovementState::LeftBackward,
                CrouchingMovementState::Backward,
                0.0,
                1.0,
                true,
                8.0,
            ),
        ];
        for (source, target, hips_lock, feet_crossing, full_weight, expected) in movement_cases {
            assert_eq!(
                source.transition_index(target, hips_lock, feet_crossing, full_weight),
                Some(expected),
            );
        }
        let outer_cases = [
            (CrouchingOuterTransition::IdleToMove, 8.0),
            (CrouchingOuterTransition::IdleToRotateRight, 3.0),
            (CrouchingOuterTransition::IdleToRotateLeft, 0.0),
            (CrouchingOuterTransition::MoveToStop, 10.0),
            (CrouchingOuterTransition::MoveToIdleQuickStop, 9.0),
            (CrouchingOuterTransition::RotateLeftToIdleMoving, 1.0),
            (CrouchingOuterTransition::RotateLeftToIdleFinished, 2.0),
            (CrouchingOuterTransition::RotateLeftToRotateRight, 6.0),
            (CrouchingOuterTransition::RotateRightToIdleMoving, 4.0),
            (CrouchingOuterTransition::RotateRightToIdleFinished, 5.0),
            (CrouchingOuterTransition::RotateRightToRotateLeft, 7.0),
            (CrouchingOuterTransition::StopToIdle, 11.0),
        ];
        for (transition, expected) in outer_cases {
            assert_eq!(transition.index(), expected);
        }
    }

    #[test]
    fn source_crouching_transition_driver_assets_use_the_same_order() {
        let movement = include_str!(
            "../assets/graphs/ab_als/stances/crouching/movement_states_runtime.animgraph.ron"
        );
        let outer = include_str!(
            "../assets/graphs/ab_als/stances/crouching/crouching_states_runtime.animgraph.ron"
        );
        let movement_ids = [
            "103", "106", "101", "102", "104", "105", "109", "118", "123", "114", "116", "107",
            "111", "120", "121", "117", "112", "108", "110", "119", "122", "113", "115", "124",
        ];
        let outer_ids = [
            "103", "106", "107", "102", "109", "110", "108", "111", "101", "105", "104", "112",
        ];
        for (text, prefix, identifiers) in [
            (
                movement,
                "16200000-0000-4000-8000-000000000",
                movement_ids.as_slice(),
            ),
            (
                outer,
                "16100000-0000-4000-8000-000000000",
                outer_ids.as_slice(),
            ),
        ] {
            let offsets = identifiers
                .iter()
                .map(|identifier| {
                    text.find(&format!("{prefix}{identifier}"))
                        .expect("source transition identifier must exist")
                })
                .collect::<Vec<_>>();
            assert!(offsets.windows(2).all(|window| window[0] < window[1]));
        }
    }

    #[test]
    fn crouching_marker_phase_uses_the_source_zero_start_time() {
        let mut runtime = CrouchingLayerRuntime::default();
        let phase = advance_crouching_movement_marker_phase(
            &mut runtime,
            true,
            CrouchingMovementState::Forward,
            1.0,
            0.0,
        );
        let sample_time = als_ext::marker_phase_sample_time(
            phase,
            1.0,
            &CROUCH_PHASE_MARKERS,
            CROUCH_MOVEMENT_DURATION,
            crouching_movement_markers(CrouchingMovementState::Forward),
            true,
        );
        assert!(sample_time.abs() < 1e-5);
    }

    #[test]
    fn crouching_marker_phase_preserves_the_foot_interval_across_directions() {
        let mut runtime = CrouchingLayerRuntime::default();
        let phase = advance_crouching_movement_marker_phase(
            &mut runtime,
            true,
            CrouchingMovementState::Forward,
            1.0,
            0.2,
        );
        let right_phase = advance_crouching_movement_marker_phase(
            &mut runtime,
            true,
            CrouchingMovementState::RightBackward,
            1.0,
            0.0,
        );
        let forward_time = als_ext::marker_phase_sample_time(
            phase,
            1.0,
            &CROUCH_PHASE_MARKERS,
            CROUCH_MOVEMENT_DURATION,
            crouching_movement_markers(CrouchingMovementState::Forward),
            true,
        );
        let right_time = als_ext::marker_phase_sample_time(
            right_phase,
            1.0,
            &CROUCH_PHASE_MARKERS,
            CROUCH_MOVEMENT_DURATION,
            crouching_movement_markers(CrouchingMovementState::RightBackward),
            true,
        );
        let forward_marker = als_ext::marker_position_at_time(
            crouching_movement_markers(CrouchingMovementState::Forward),
            CROUCH_MOVEMENT_DURATION,
            forward_time,
            true,
        )
        .unwrap();
        let right_marker = als_ext::marker_position_at_time(
            crouching_movement_markers(CrouchingMovementState::RightBackward),
            CROUCH_MOVEMENT_DURATION,
            right_time,
            true,
        )
        .unwrap();
        assert_eq!(forward_marker.previous, right_marker.previous);
        assert_eq!(forward_marker.next, right_marker.next);
        assert!((forward_marker.alpha - right_marker.alpha).abs() < 1e-5);
    }

    #[test]
    fn crouching_writes_the_canonical_movement_phase_for_linked_overlays() {
        let mut player = AnimationGraphPlayer::new(default());
        write_crouching_inputs(
            &mut player,
            CrouchingGraphInputs {
                movement_marker_phase: 0.25,
                ..default()
            },
        );
        assert_eq!(
            player
                .get_global_input_data("als_movement_marker_phase")
                .and_then(|value| value.as_f32().ok()),
            Some(0.25)
        );
    }

    #[test]
    fn dynamic_transition_selects_the_larger_lock_distance() {
        let mut state = AlsFootLockState {
            valid: true,
            ..default()
        };
        state.left.lock_amount = 1.0;
        state.left.target_location = Vec3::new(0.2, 0.0, 0.0);
        state.left.lock_location = Vec3::ZERO;
        state.right.lock_amount = 1.0;
        state.right.target_location = Vec3::new(0.4, 0.0, 0.0);
        state.right.lock_location = Vec3::ZERO;
        assert_eq!(dynamic_transition_index(Some(&state), 1.0, true), Some(1.0));
    }

    #[test]
    fn source_crouching_state_result_lifecycle_actions_match_the_authored_roots() {
        assert_eq!(
            source_crouching_state_lifecycle_action(Some(CrouchingOuterTransition::MoveToStop)),
            Some(SOURCE_CROUCHING_STOP_STATE_ENTRY_FUNCTION)
        );
        for transition in [
            CrouchingOuterTransition::IdleToMove,
            CrouchingOuterTransition::IdleToRotateLeft,
            CrouchingOuterTransition::IdleToRotateRight,
        ] {
            assert_eq!(
                source_crouching_state_lifecycle_action(Some(transition)),
                Some(SOURCE_CROUCHING_TURN_STOP_FUNCTION)
            );
        }
        assert_eq!(source_crouching_state_lifecycle_action(None), None);
    }
}
