use std::num::NonZeroU8;
use std::time::{Duration, Instant};

use als_camera::{AlsCameraDebugInfo, AlsCameraRig, AlsCameraSet, AlsCameraShakeRequest};
use als_core::character::{AlsCharacter, AlsMovementParamsHandle};
use als_core::dynamics::AlsMovementBaseDynamicsState;
use als_core::foot_ik::{AlsFootLockState, AlsFootProbe};
use als_core::gait::{AlsDesiredGait, AlsMaxAllowedGait};
use als_core::input_request::{AlsCameraLeftShoulder, AlsFirstPersonView};
use als_core::mantle::{AlsMantleControlSettings, AlsMantleState};
use als_core::motion::{
    AlsAimingYawConstraint, AlsCrouchTransaction, AlsInputOverride, AlsRotationAnimationState,
    AlsRotationResponseSet, AlsRotationResponseState, AlsViewYaw,
};
use als_core::movement_direction::{AlsMovementDirection, AlsMovementDirectionState};
use als_core::orientation::world_forward_to_yaw;
use als_core::params::AlsMovementParams;
use als_core::roll::AlsRollState;
use als_core::start_stop::{StartStopPhase, StartStopState};
use als_core::state::{Gait, LocomotionState, OverlayState, RotationMode, Stance};
use als_core::transition::{AlsDesiredStance, AlsStateRequest};
use als_core::view::AlsViewPitch;
use als_ext::{
    ALS_LAYER_CURVE_NAMES, RagdollCollisionMember, RagdollPhysicsProfile,
    RagdollPhysicsProfileApplied,
};
use avian3d::prelude::{Collider, CollisionLayers, LinearVelocity, Position, RigidBody, Rotation};
use bevy::ecs::message::{MessageReader, MessageWriter};
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::time::{Real, Virtual};
use bevy_animation_graph::core::animated_scene::AnimatedSceneInstance;
use bevy_animation_graph::core::animation_graph_player::{AnimationGraphPlayer, AnimationSource};
use bevy_animation_graph::core::edge_data::DataValue;
use bevy_animation_graph::core::plugin::AnimationGraphSet;
use bevy_animation_graph::core::ragdoll::definition::BodyLabel;
use bevy_tnua::TnuaToggle;
use serde::Serialize;

use crate::actual_motion::AlsActualMotion;
use crate::animation_context_projection::AlsRootAnimationContext;
use crate::animation_events::{
    AlsAnimationEvent, AlsNotifyEarlyBlendOut, AlsNotifyLocomotionAction, AlsNotifyRootMotionScale,
};
use crate::animation_visual_validation::AnimationVisualValidationProgress;
use crate::character_assembly::{AlsCharacterAssembly, AlsCharacterAvatar};
use crate::control_rig_apply::{
    AlsControlRigConstruction, AlsControlRigInput, AlsControlRigSet, AlsControlRigStageTrace,
    AlsControlRigState,
};
use crate::crouching_layer::{AlsCrouchingGraphTelemetry, AlsCrouchingTelemetrySet};
use crate::debug_display::AlsDebugDisplayStats;
use crate::foot_ik_apply::{
    AlsFootIkApplyTelemetry, AlsFootIkSpaceState, AlsFootIkTelemetry, AlsFootIkTelemetryEnabled,
    AlsFootIkWeightOverride, AlsFootLockTeleportTelemetry, FootIkApplySet,
};
use crate::game_mode::AlsPlayerAdmissionComplete;
use crate::generated_transition_notify::GeneratedTransitionNotifyTelemetry;
use crate::grounded_layer::AlsGroundedEntryMode;
use crate::grounded_layer::AlsGroundedGraphTelemetry;
use crate::head_layer::AlsHeadState;
use crate::layering_layer::AlsLayeringState;
use crate::overlay_mode_menu::OverlayMenuState;
use crate::overlay_object::{AlsOverlayObject, AlsOverlayObjectState};
use crate::ragdoll_link::{
    AlsRagdollGroundFollow, AlsRagdollJointDrive, RAGDOLL_ANGULAR_STIFFNESS,
};
use crate::roll_drive::AlsRollSlotRuntime;
use crate::show_debug_hud::{
    AlsShowDebugHudRoot, AlsShowDebugHudState, DebugCategory, apply_show_debug_key,
};
use crate::stance_layer::AlsStanceBlend;
use crate::standing_layer::{AlsPivotNotify, AlsStandingGraphTelemetry};
use crate::stop_link::{
    AlsRootTransitionSlotRuntime, AlsTransitionKind, AlsTransitionPlayback,
    RIGHT_TRANSITION_START_TIME, STOP_TRANSITION_BLEND_IN, STOP_TRANSITION_BLEND_OUT,
    STOP_TRANSITION_PLAY_RATE,
};
use crate::turn_layer::{
    AlsStandingTurnSlotRuntime, AlsTurnDrive, AlsTurnMontageStack, AlsTurnSlotBridgeSet,
    TURN_DUR_90,
};
use crate::{
    air_jump_layer::AlsJumpVariantState,
    als_hud::{AlsHudDebugSection, AlsHudInputSection, AlsHudRoot, AlsHudState},
    gait_layer::AlsGaitBlend,
};

const GATE_TIMEOUT: Duration = Duration::from_secs(180);
const CROUCH_CLEARANCE_TIMEOUT: f32 = 90.0;
const DIRECTION_CASE_SECONDS: f32 = 3.0;
const DIRECTION_CASE_SETTLE_SECONDS: f32 = 1.6;
const STATE_MACHINE_TURN_FIRST_START: f32 = 0.5;
const STATE_MACHINE_TURN_FIRST_END: f32 = 1.75;
const STATE_MACHINE_TURN_SECOND_START: f32 = 4.0;
const STATE_MACHINE_TURN_SECOND_END: f32 = 5.25;
const STATE_MACHINE_TURN_END: f32 = STATE_MACHINE_TURN_SECOND_END + TURN_DUR_90 + 1.0;
const DIRECTION_CASE_COUNT: usize = 11;
const CROUCH_DIRECTION_CASE_SECONDS: f32 = 3.0;
const CROUCH_DIRECTION_CASE_SETTLE_SECONDS: f32 = 1.6;
const CROUCH_DIRECTION_CASE_COUNT: usize = 8;
const CROUCH_STATIONARY_TIMEOUT_SECONDS: f32 = 40.0;
const CROUCH_STATIONARY_STABLE_FRAMES: u8 = 3;
const CROUCH_ROTATE_SOURCE_MIN_YAW_CONVERGENCE_RADIANS: f32 = 1.4;
const CROUCH_TURN_SOURCE_MIN_YAW_CONVERGENCE_RADIANS: f32 = 1.4;
const DIRECTION_BRANCH_FORWARD: u8 = 1 << 0;
const DIRECTION_BRANCH_BACKWARD: u8 = 1 << 1;
const DIRECTION_BRANCH_LEFT: u8 = 1 << 2;
const DIRECTION_BRANCH_RIGHT: u8 = 1 << 3;
const FOOT_IK_BASELINE_MASK: u8 = 0b1_1111;
const FOOT_IK_WEIGHT_MASK: u8 = 0b11_1111;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CrouchStationaryValidation {
    RotateRight,
    RotateLeft,
    TurnRight,
    TurnLeft,
}

impl CrouchStationaryValidation {
    fn is_left(self) -> bool {
        matches!(self, Self::RotateLeft | Self::TurnLeft)
    }

    fn is_turn(self) -> bool {
        matches!(self, Self::TurnRight | Self::TurnLeft)
    }

    fn side_index(self) -> usize {
        self.is_left() as usize
    }
}

fn next_crouch_stationary_validation(
    coverage: &IntegrationCoverage,
) -> Option<CrouchStationaryValidation> {
    if coverage.crouch_turn_completion_mask & (1 << 0) == 0 {
        Some(CrouchStationaryValidation::TurnRight)
    } else if coverage.crouch_turn_completion_mask & (1 << 1) == 0 {
        Some(CrouchStationaryValidation::TurnLeft)
    } else if coverage.crouch_rotate_completion_mask & (1 << 0) == 0 {
        Some(CrouchStationaryValidation::RotateRight)
    } else if coverage.crouch_rotate_completion_mask & (1 << 1) == 0 {
        Some(CrouchStationaryValidation::RotateLeft)
    } else {
        None
    }
}

fn generated_stop_quick_input_step(
    input_started: bool,
    input_released: bool,
    telemetry: Option<&AlsStandingGraphTelemetry>,
) -> (bool, bool, bool) {
    let input_started = input_started
        || telemetry.is_some_and(|telemetry| {
            telemetry.actual_standing_state == Some(0)
                && telemetry.actual_standing_pose_ready == Some(true)
        });
    let input_released = input_released
        || (input_started
            && telemetry.is_some_and(|telemetry| {
                telemetry.actual_standing_transition_active == Some(true)
            }));
    (
        input_started,
        input_released,
        input_started && !input_released,
    )
}

#[derive(Resource, Default)]
pub struct ManualPlaygroundStartupReady {
    pub passed: bool,
}

#[derive(Resource, Clone, Debug, Default)]
pub struct ManualAnimationWindowEvidence {
    pub label: Option<&'static str>,
    pub index: Option<usize>,
    pub settled: bool,
    pub capture_source_window: bool,
    pub requires_pre_rig_motion: bool,
    pub requires_final_pose_motion: bool,
    pub requires_non_reference_root_pose: bool,
    pub requires_source_pose_chain: bool,
    pub requires_actor_yaw_motion: bool,
    pub requires_actor_yaw_convergence: bool,
    pub expected_actor_yaw_delta_sign: Option<i8>,
    pub expected_actor_yaw_baseline: Option<f32>,
    pub expected_animation_variant: Option<&'static str>,
    pub source_node: Option<&'static str>,
    pub source_state: Option<&'static str>,
    pub source_pose_chain: Vec<ManualAnimationPoseBoundary>,
    pub minimum_pre_rig_translation_span: f32,
    pub minimum_pre_rig_rotation_span_radians: f32,
    pub minimum_final_translation_span: f32,
    pub minimum_final_rotation_span_radians: f32,
    pub minimum_actor_yaw_delta_radians: f32,
    pub minimum_actor_yaw_convergence_radians: f32,
    pub minimum_observation_seconds: f32,
    pub source_sync_group: Option<&'static str>,
    pub sync_phase_global: Option<&'static str>,
    pub canonical_sync_phase: Option<f32>,
    pub maximum_sync_phase_distance: f32,
    pub canonical_clip_time: Option<f32>,
    pub maximum_clip_time_distance: f32,
    pub direction_signature_group: Option<&'static str>,
    pub required_direction_signature_members: usize,
    pub minimum_direction_signature_rotation_radians: f32,
    pub expected_direction: Option<&'static str>,
    pub observed_direction: Option<&'static str>,
    pub expected_runtime_state: Option<u8>,
    pub observed_runtime_state: Option<u8>,
    pub expected_parent_runtime_state: Option<u8>,
    pub expected_terminal_parent_runtime_state: Option<u8>,
    pub expected_terminal_parent_transition_active: Option<bool>,
    pub expected_terminal_requested_parent_transition_active: Option<bool>,
    pub observed_parent_runtime_state: Option<u8>,
    pub observed_parent_transition_active: Option<bool>,
    pub observed_requested_parent_runtime_state: Option<u8>,
    pub observed_requested_parent_transition_active: Option<bool>,
    pub observed_actor_yaw: Option<f32>,
    pub observed_view_yaw: Option<f32>,
    pub observed_animation_yaw_speed_degrees: Option<f32>,
    pub observed_animation_curve_direct_yaw_speed_degrees: Option<f32>,
    pub observed_animation_clip_time: Option<f32>,
    pub observed_animation_weight: Option<f32>,
    pub observed_animation_play_rate: Option<f32>,
    pub observed_animation_curve_stack_weight: Option<f32>,
    pub observed_animation_curve_stack_entries: Vec<ManualTurnStackEntryEvidence>,
    pub observed_animation_variant: Option<&'static str>,
    pub expected_lifecycle_stages: &'static [&'static str],
    pub observed_lifecycle_stages: &'static [&'static str],
}

#[derive(Clone, Debug, Serialize)]
pub struct ManualTurnStackEntryEvidence {
    pub animation_variant: &'static str,
    pub elapsed_seconds: f32,
    pub weight: f32,
    pub play_rate: f32,
    pub sampling_ready: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct ManualAnimationPoseBoundary {
    pub label: &'static str,
    pub graph: &'static str,
    pub node: &'static str,
    pub output: &'static str,
    pub minimum_translation_span: f32,
    pub minimum_rotation_span_radians: f32,
}

#[derive(Resource, Clone, Debug, Default)]
pub struct ManualRuntimePhaseEvidence {
    pub scenario: String,
    pub phase: String,
    pub elapsed_seconds: f32,
    pub temporal_rebase_epoch: u64,
    pub locomotion: String,
    pub gait: String,
    pub stance: String,
    pub rotation_mode: String,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct ManualIntegrationGateResult {
    pub name: String,
    pub covered: bool,
    pub detail: String,
}

#[derive(Resource, Clone, Debug, Default, Serialize, PartialEq, Eq)]
pub struct ManualIntegrationGateReport {
    pub scenario: String,
    pub timed_out: bool,
    pub completed: bool,
    pub passed: Option<bool>,
    pub results: Vec<ManualIntegrationGateResult>,
}

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ManualIntegrationObservationSet;

#[derive(SystemParam)]
struct ManualObservationResources<'w> {
    state: ResMut<'w, ManualIntegrationState>,
    animation_window: ResMut<'w, ManualAnimationWindowEvidence>,
    gate_report: ResMut<'w, ManualIntegrationGateReport>,
}

pub struct ManualPlaygroundIntegrationPlugin;

impl Plugin for ManualPlaygroundIntegrationPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ManualPlaygroundStartupReady>()
            .init_resource::<ManualIntegrationState>()
            .init_resource::<ManualAnimationWindowEvidence>()
            .init_resource::<ManualRuntimePhaseEvidence>()
            .init_resource::<ManualIntegrationGateReport>()
            .add_systems(Update, initialize_manual_integration)
            .add_systems(
                PreUpdate,
                drive_manual_integration
                    .after(bevy_enhanced_input::prelude::EnhancedInputSystems::Apply),
            )
            .add_systems(
                PreUpdate,
                drive_crouch_direction_reference_frame
                    .after(drive_manual_integration)
                    .after(bevy_enhanced_input::prelude::EnhancedInputSystems::Apply),
            )
            .add_systems(
                PostUpdate,
                drive_control_rig_view_offset
                    .after(AnimationGraphSet::Final)
                    .before(AlsControlRigSet),
            )
            .add_systems(
                PostUpdate,
                prepare_get_up_physics_fixture
                    .after(als_ext::RagdollAngularDriveSet)
                    .before(avian3d::prelude::PhysicsSystems::First),
            )
            .add_systems(
                PostUpdate,
                observe_manual_integration
                    .after(AlsCameraSet)
                    .after(AlsCrouchingTelemetrySet)
                    .after(AlsTurnSlotBridgeSet)
                    .after(FootIkApplySet::Telemetry)
                    .after(AlsRotationResponseSet)
                    .in_set(ManualIntegrationObservationSet),
            );
    }
}

#[derive(Component)]
struct ManualIntegrationPlatform;

#[derive(Component)]
struct ManualFootIkTerrain;

fn drive_control_rig_view_offset(
    state: Res<ManualIntegrationState>,
    mut characters: Query<(&Transform, &mut AlsViewYaw), With<AlsCharacter>>,
) {
    if state.scenario != IntegrationScenario::ControlRig
        || state.phase != IntegrationPhase::ControlRig
    {
        return;
    }
    let Some(character) = state.character else {
        return;
    };
    let Ok((transform, mut view_yaw)) = characters.get_mut(character) else {
        return;
    };
    let forward = transform.rotation * Vec3::NEG_Z;
    view_yaw.0 = f32::atan2(forward.x, -forward.z) + 60.0_f32.to_radians();
}

fn drive_crouch_direction_reference_frame(
    fixture: Option<Res<ManualIntegrationFixture>>,
    state: Res<ManualIntegrationState>,
    mut characters: Query<
        (&mut Transform, Option<&mut Rotation>, &mut AlsViewYaw),
        With<AlsCharacter>,
    >,
) {
    if state.phase != IntegrationPhase::CrouchDirectionRegression {
        return;
    }
    if !crouch_direction_reference_frame_active(state.phase_elapsed()) {
        return;
    }
    let Some(fixture) = fixture else {
        return;
    };
    let Some(character) = state.character else {
        return;
    };
    let Ok((mut transform, rotation, mut view_yaw)) = characters.get_mut(character) else {
        return;
    };
    let rotation_value = Quat::from_rotation_y(fixture.facing_yaw);
    transform.rotation = rotation_value;
    if let Some(mut rotation) = rotation {
        rotation.0 = rotation_value;
    }
    view_yaw.0 = fixture.facing_yaw;
}

fn crouch_direction_reference_frame_active(elapsed: f32) -> bool {
    elapsed < CROUCH_DIRECTION_CASE_COUNT as f32 * CROUCH_DIRECTION_CASE_SECONDS
}

#[derive(Resource)]
struct ManualIntegrationFixture {
    platform: Entity,
    platform_home: Vec3,
    locomotion_start: Vec3,
    mantle_start: Vec3,
    roll_start: Vec3,
    facing_yaw: f32,
    obstacle_mesh: Handle<Mesh>,
    obstacle_material: Handle<StandardMaterial>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum IntegrationPhase {
    #[default]
    Await,
    DirectionRegression,
    Sprint,
    FootIkNoHit,
    FootIkWeightRamp,
    FootIkTeleport,
    FootIkTerrain,
    ControlRig,
    DefaultOverlay,
    StateMachineGrounded,
    StateMachineAir,
    StateMachineRotate,
    GeneratedTransitionNotify,
    Locomotion,
    CrouchRecovery,
    CrouchClearance,
    CrouchDirectionRegression,
    Turn,
    Mantle,
    Platform,
    Roll,
    Ragdoll,
    Camera,
    Interface,
    Notify,
    Finish,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum CrouchClearanceStage {
    #[default]
    AwaitStanding,
    Cycling,
    PrepareCanopyCrouch,
    StabilizeCanopyCrouch,
    ArmCanopy,
    AwaitCanopyReady,
    RequestStanding,
    AwaitBlocked,
    MoveAway,
    AwaitRecovery,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum IntegrationScenario {
    Base,
    Grounded,
    StandingDirection,
    Crouching,
    ControlRig,
    DefaultOverlay,
    StateMachine,
    GeneratedTransitionNotify,
    FootIk,
    Sprint,
    Ragdoll,
    GetUp,
    Camera,
    Interface,
    Notify,
}

impl IntegrationScenario {
    fn from_environment() -> Self {
        match std::env::var("ALS_MANUAL_INTEGRATION_CASE").ok().as_deref() {
            Some("grounded") => Self::Grounded,
            Some("standing_direction") => Self::StandingDirection,
            Some("crouching") => Self::Crouching,
            Some("control_rig") => Self::ControlRig,
            Some("default_overlay") => Self::DefaultOverlay,
            Some("state_machine") => Self::StateMachine,
            Some("transition_notify") => Self::GeneratedTransitionNotify,
            Some("foot_ik") => Self::FootIk,
            Some("sprint") => Self::Sprint,
            Some("ragdoll") => Self::Ragdoll,
            Some("get_up") => Self::GetUp,
            Some("camera") => Self::Camera,
            Some("interface") => Self::Interface,
            Some("notify") => Self::Notify,
            _ => Self::Base,
        }
    }
}

fn initial_phase(scenario: IntegrationScenario) -> IntegrationPhase {
    match scenario {
        IntegrationScenario::Base
        | IntegrationScenario::StandingDirection
        | IntegrationScenario::StateMachine
        | IntegrationScenario::FootIk => IntegrationPhase::DirectionRegression,
        IntegrationScenario::GeneratedTransitionNotify => {
            IntegrationPhase::GeneratedTransitionNotify
        }
        IntegrationScenario::Grounded => IntegrationPhase::Roll,
        IntegrationScenario::Sprint => IntegrationPhase::Sprint,
        IntegrationScenario::ControlRig => IntegrationPhase::ControlRig,
        IntegrationScenario::DefaultOverlay => IntegrationPhase::DefaultOverlay,
        IntegrationScenario::Crouching => IntegrationPhase::CrouchDirectionRegression,
        IntegrationScenario::Ragdoll | IntegrationScenario::GetUp => IntegrationPhase::Ragdoll,
        IntegrationScenario::Camera => IntegrationPhase::Camera,
        IntegrationScenario::Interface => IntegrationPhase::Interface,
        IntegrationScenario::Notify => IntegrationPhase::Notify,
    }
}

#[derive(Clone, Copy)]
struct DirectionRegressionCase {
    name: &'static str,
    move_dir: Vec2,
    desired_gait: Gait,
    expected_allowed_gait: Gait,
    expected_actual_gait: Gait,
    sprint_held: bool,
    first_person: bool,
    rotation_mode: RotationMode,
    yaw_offset: f32,
    branch_mask: u8,
    branch_from_actual_local: bool,
    expected_movement_state: Option<AlsMovementDirection>,
    source_state: Option<&'static str>,
    direction_signature_group: Option<&'static str>,
}

#[derive(Clone, Copy)]
struct CrouchDirectionRegressionCase {
    name: &'static str,
    move_dir: Vec2,
    expected_direction: AlsMovementDirection,
}

fn direction_regression_case(index: usize) -> Option<DirectionRegressionCase> {
    let running = |name, move_dir, branch_mask| DirectionRegressionCase {
        name,
        move_dir,
        desired_gait: Gait::Running,
        expected_allowed_gait: Gait::Running,
        expected_actual_gait: Gait::Running,
        sprint_held: false,
        first_person: false,
        rotation_mode: RotationMode::Aiming,
        yaw_offset: 0.0,
        branch_mask,
        branch_from_actual_local: false,
        expected_movement_state: None,
        source_state: None,
        direction_signature_group: None,
    };
    match index {
        0 => Some(DirectionRegressionCase {
            name: "walk_forward",
            move_dir: Vec2::Y,
            desired_gait: Gait::Walking,
            expected_allowed_gait: Gait::Walking,
            expected_actual_gait: Gait::Walking,
            sprint_held: false,
            first_person: false,
            rotation_mode: RotationMode::Aiming,
            yaw_offset: 0.0,
            branch_mask: DIRECTION_BRANCH_FORWARD,
            branch_from_actual_local: false,
            expected_movement_state: Some(AlsMovementDirection::Forward),
            source_state: Some("Move Forward"),
            direction_signature_group: Some("AB_Als_Standing/Movement States"),
        }),
        1 => Some(DirectionRegressionCase {
            expected_movement_state: Some(AlsMovementDirection::Backward),
            source_state: Some("Move Backward"),
            direction_signature_group: Some("AB_Als_Standing/Movement States"),
            ..running("run_backward", Vec2::NEG_Y, DIRECTION_BRANCH_BACKWARD)
        }),
        2 => Some(DirectionRegressionCase {
            expected_movement_state: Some(AlsMovementDirection::RightBackward),
            source_state: Some("Move Right Backward"),
            direction_signature_group: Some("AB_Als_Standing/Movement States"),
            ..running("run_right_from_backward", Vec2::X, DIRECTION_BRANCH_RIGHT)
        }),
        3 => Some(DirectionRegressionCase {
            expected_movement_state: Some(AlsMovementDirection::LeftForward),
            source_state: Some("Move Left Forward"),
            direction_signature_group: Some("AB_Als_Standing/Movement States"),
            ..running(
                "run_left_from_right_backward",
                Vec2::NEG_X,
                DIRECTION_BRANCH_LEFT,
            )
        }),
        4 => Some(DirectionRegressionCase {
            expected_movement_state: Some(AlsMovementDirection::Forward),
            source_state: Some("Move Forward"),
            direction_signature_group: None,
            ..running(
                "run_forward_from_left_forward",
                Vec2::Y,
                DIRECTION_BRANCH_FORWARD,
            )
        }),
        5 => Some(DirectionRegressionCase {
            expected_movement_state: Some(AlsMovementDirection::Backward),
            source_state: Some("Move Backward"),
            direction_signature_group: None,
            ..running(
                "run_backward_from_forward",
                Vec2::NEG_Y,
                DIRECTION_BRANCH_BACKWARD,
            )
        }),
        6 => Some(DirectionRegressionCase {
            expected_movement_state: Some(AlsMovementDirection::LeftForward),
            source_state: Some("Move Left Backward to Move Left Forward"),
            direction_signature_group: None,
            ..running(
                "run_left_forward_from_backward",
                Vec2::NEG_X,
                DIRECTION_BRANCH_LEFT,
            )
        }),
        7 => Some(DirectionRegressionCase {
            expected_movement_state: Some(AlsMovementDirection::RightForward),
            source_state: Some("Move Right Forward"),
            direction_signature_group: Some("AB_Als_Standing/Movement States"),
            ..running(
                "run_right_from_left_backward",
                Vec2::X,
                DIRECTION_BRANCH_RIGHT,
            )
        }),
        8 => Some(DirectionRegressionCase {
            name: "sprint_allowed_forward",
            move_dir: Vec2::Y,
            desired_gait: Gait::Sprinting,
            expected_allowed_gait: Gait::Sprinting,
            expected_actual_gait: Gait::Sprinting,
            sprint_held: true,
            first_person: false,
            rotation_mode: RotationMode::VelocityDirection,
            yaw_offset: 0.0,
            branch_mask: DIRECTION_BRANCH_FORWARD,
            branch_from_actual_local: false,
            expected_movement_state: Some(AlsMovementDirection::Forward),
            source_state: Some("Move Forward"),
            direction_signature_group: None,
        }),
        9 => Some(DirectionRegressionCase {
            name: "sprint_rejected_first_person_lateral",
            move_dir: Vec2::X,
            desired_gait: Gait::Sprinting,
            expected_allowed_gait: Gait::Running,
            expected_actual_gait: Gait::Running,
            sprint_held: true,
            first_person: true,
            rotation_mode: RotationMode::ViewDirection,
            yaw_offset: 0.0,
            branch_mask: DIRECTION_BRANCH_RIGHT,
            branch_from_actual_local: false,
            expected_movement_state: None,
            source_state: None,
            direction_signature_group: None,
        }),
        10 => Some(DirectionRegressionCase {
            name: "run_forward_rotated_90",
            move_dir: Vec2::Y,
            desired_gait: Gait::Running,
            expected_allowed_gait: Gait::Running,
            expected_actual_gait: Gait::Running,
            sprint_held: false,
            first_person: false,
            rotation_mode: RotationMode::Aiming,
            yaw_offset: std::f32::consts::FRAC_PI_2,
            branch_mask: 0,
            branch_from_actual_local: true,
            expected_movement_state: None,
            source_state: None,
            direction_signature_group: None,
        }),
        _ => None,
    }
}

fn crouch_direction_regression_case(index: usize) -> Option<CrouchDirectionRegressionCase> {
    match index {
        0 => Some(CrouchDirectionRegressionCase {
            name: "crouch_forward",
            move_dir: Vec2::Y,
            expected_direction: AlsMovementDirection::Forward,
        }),
        1 => Some(CrouchDirectionRegressionCase {
            name: "crouch_right_forward",
            move_dir: Vec2::X,
            expected_direction: AlsMovementDirection::RightForward,
        }),
        2 => Some(CrouchDirectionRegressionCase {
            name: "crouch_right_backward",
            move_dir: Vec2::new(1.0, -1.0).normalize(),
            expected_direction: AlsMovementDirection::RightBackward,
        }),
        3 => Some(CrouchDirectionRegressionCase {
            name: "crouch_backward",
            move_dir: Vec2::NEG_Y,
            expected_direction: AlsMovementDirection::Backward,
        }),
        4 => Some(CrouchDirectionRegressionCase {
            name: "crouch_left_backward",
            move_dir: Vec2::new(-1.0, -1.0).normalize(),
            expected_direction: AlsMovementDirection::LeftBackward,
        }),
        5 => Some(CrouchDirectionRegressionCase {
            name: "crouch_left_forward",
            move_dir: Vec2::NEG_X,
            expected_direction: AlsMovementDirection::LeftForward,
        }),
        6 => Some(CrouchDirectionRegressionCase {
            name: "crouch_forward_repeat",
            move_dir: Vec2::Y,
            expected_direction: AlsMovementDirection::Forward,
        }),
        7 => Some(CrouchDirectionRegressionCase {
            name: "crouch_right_forward_repeat",
            move_dir: Vec2::X,
            expected_direction: AlsMovementDirection::RightForward,
        }),
        _ => None,
    }
}

fn crouch_direction_source_node(direction: AlsMovementDirection) -> &'static str {
    match direction {
        AlsMovementDirection::Forward => "AB_Als_Crouching/A_Als_Crouch_Walk_Forward",
        AlsMovementDirection::Backward => "AB_Als_Crouching/A_Als_Crouch_Walk_Backward",
        AlsMovementDirection::RightForward => "AB_Als_Crouching/A_Als_Crouch_Walk_Right_Forward",
        AlsMovementDirection::RightBackward => "AB_Als_Crouching/A_Als_Crouch_Walk_Right_Backward",
        AlsMovementDirection::LeftForward => "AB_Als_Crouching/A_Als_Crouch_Walk_Left_Forward",
        AlsMovementDirection::LeftBackward => "AB_Als_Crouching/A_Als_Crouch_Walk_Left_Backward",
    }
}

fn crouch_direction_graph_node(direction: AlsMovementDirection) -> &'static str {
    match direction {
        AlsMovementDirection::Forward => {
            "A_Als_Crouch_Walk_Forward\nSequence Player\nSync group Movement"
        }
        AlsMovementDirection::Backward => {
            "A_Als_Crouch_Walk_Backward\nSequence Player\nSync group Movement"
        }
        AlsMovementDirection::RightForward => {
            "A_Als_Crouch_Walk_Right_Forward\nSequence Player\nSync group Movement"
        }
        AlsMovementDirection::RightBackward => {
            "A_Als_Crouch_Walk_Right_Backward\nSequence Player\nSync group Movement"
        }
        AlsMovementDirection::LeftForward => {
            "A_Als_Crouch_Walk_Left_Forward\nSequence Player\nSync group Movement"
        }
        AlsMovementDirection::LeftBackward => {
            "A_Als_Crouch_Walk_Left_Backward\nSequence Player\nSync group Movement"
        }
    }
}

fn crouch_direction_cache_node(direction: AlsMovementDirection) -> &'static str {
    match direction {
        AlsMovementDirection::Forward => "Save cached pose 'Move Forward'",
        AlsMovementDirection::Backward => "Save cached pose 'Move Backward'",
        AlsMovementDirection::RightForward => "Save cached pose 'Move Right Forward'",
        AlsMovementDirection::RightBackward => "Save cached pose 'Move Right Backward'",
        AlsMovementDirection::LeftForward => "Save cached pose 'Move Left Forward'",
        AlsMovementDirection::LeftBackward => "Save cached pose 'Move Left Backward'",
    }
}

fn generated_standing_source_pose_chain() -> Vec<ManualAnimationPoseBoundary> {
    vec![
        ManualAnimationPoseBoundary {
            label: "standing_movement_cache",
            graph: "generated/ALS/Character/AnimationInstances/Stances/AB_Als_Standing/AnimGraph.animgraph.ron",
            node: "Save cached pose 'Movement'",
            output: "__als_source_cached_pose",
            minimum_translation_span: 0.0,
            minimum_rotation_span_radians: 0.05,
        },
        ManualAnimationPoseBoundary {
            label: "standing_movement_details_fsm",
            graph: "generated/ALS/Character/AnimationInstances/Stances/AB_Als_Standing/AnimGraph.animgraph.ron",
            node: "Movement Details States\nState Machine",
            output: "Pose",
            minimum_translation_span: 0.0,
            minimum_rotation_span_radians: 0.05,
        },
        ManualAnimationPoseBoundary {
            label: "standing_movement_states_fsm",
            graph: "generated/ALS/Character/AnimationInstances/Stances/AB_Als_Standing/AnimGraph.animgraph.ron",
            node: "Movement States\nState Machine",
            output: "Pose",
            minimum_translation_span: 0.0,
            minimum_rotation_span_radians: 0.05,
        },
        ManualAnimationPoseBoundary {
            label: "standing_states_fsm",
            graph: "generated/ALS/Character/AnimationInstances/Stances/AB_Als_Standing/AnimGraph.animgraph.ron",
            node: "Standing States\nState Machine",
            output: "Pose",
            minimum_translation_span: 0.0,
            minimum_rotation_span_radians: 0.05,
        },
        ManualAnimationPoseBoundary {
            label: "standing_stop_states_fsm",
            graph: "generated/ALS/Character/AnimationInstances/Stances/AB_Als_Standing/AnimGraph/SM6_Standing_States/S4_Stop.animgraph.ron",
            node: "Stop States\nState Machine",
            output: "Pose",
            minimum_translation_span: 0.0,
            minimum_rotation_span_radians: 0.05,
        },
        ManualAnimationPoseBoundary {
            label: "standing_apply_additive",
            graph: "generated/ALS/Character/AnimationInstances/Stances/AB_Als_Standing/AnimGraph.animgraph.ron",
            node: "Apply Additive",
            output: "Pose",
            minimum_translation_span: 0.0,
            minimum_rotation_span_radians: 0.05,
        },
        ManualAnimationPoseBoundary {
            label: "standing_refresh_rotate_in_place",
            graph: "generated/ALS/Character/AnimationInstances/Stances/AB_Als_Standing/AnimGraph.animgraph.ron",
            node: "RefreshRotateInPlace\r\nOn Update",
            output: "Pose",
            minimum_translation_span: 0.0,
            minimum_rotation_span_radians: 0.05,
        },
        ManualAnimationPoseBoundary {
            label: "standing_graph_output",
            graph: "generated/ALS/Character/AnimationInstances/Stances/AB_Als_Standing/AnimGraph.animgraph.ron",
            node: "Output Pose\nAnimGraph",
            output: "Pose",
            minimum_translation_span: 0.0,
            minimum_rotation_span_radians: 0.05,
        },
        ManualAnimationPoseBoundary {
            label: "grounded_standing_cache",
            graph: "generated/ALS/Character/AnimationInstances/AB_Als_Grounded/AnimGraph.animgraph.ron",
            node: "Save cached pose 'Standing'",
            output: "__als_source_cached_pose",
            minimum_translation_span: 0.0,
            minimum_rotation_span_radians: 0.05,
        },
        ManualAnimationPoseBoundary {
            label: "grounded_fsm",
            graph: "generated/ALS/Character/AnimationInstances/AB_Als_Grounded/AnimGraph.animgraph.ron",
            node: "Grounded States\nState Machine",
            output: "Pose",
            minimum_translation_span: 0.0,
            minimum_rotation_span_radians: 0.05,
        },
    ]
}

fn generated_stop_quick_source_pose_chain() -> Vec<ManualAnimationPoseBoundary> {
    let mut chain = generated_standing_source_pose_chain()
        .into_iter()
        .filter(|boundary| {
            matches!(
                boundary.label,
                "standing_states_fsm"
                    | "standing_graph_output"
                    | "grounded_standing_cache"
                    | "grounded_fsm"
            )
        })
        .collect::<Vec<_>>();
    for boundary in &mut chain {
        boundary.minimum_translation_span = 0.0;
        boundary.minimum_rotation_span_radians = 0.0;
    }
    chain
}

fn state_machine_grounded_window(elapsed: f32) -> ManualAnimationWindowEvidence {
    let moving = (1.0..2.4).contains(&elapsed) || elapsed >= 3.5;
    ManualAnimationWindowEvidence {
        label: Some("state_machine_grounded"),
        settled: moving,
        capture_source_window: moving,
        requires_pre_rig_motion: true,
        requires_final_pose_motion: true,
        requires_non_reference_root_pose: true,
        requires_source_pose_chain: true,
        source_node: Some("AB_Als_Standing/Standing States"),
        source_pose_chain: generated_standing_source_pose_chain(),
        source_sync_group: Some("Movement"),
        sync_phase_global: Some("sync_groups.ab_als_standing.movement.phase"),
        ..default()
    }
}

fn crouch_direction_source_pose_chain(
    direction: AlsMovementDirection,
) -> Vec<ManualAnimationPoseBoundary> {
    vec![
        ManualAnimationPoseBoundary {
            label: "sequence_player",
            graph: "graphs/ab_als/stances/crouching.animgraph.ron",
            node: crouch_direction_graph_node(direction),
            output: "Pose",
            minimum_translation_span: 0.0,
            minimum_rotation_span_radians: 0.05,
        },
        ManualAnimationPoseBoundary {
            label: "cache_producer",
            graph: "graphs/ab_als/stances/crouching.animgraph.ron",
            node: crouch_direction_cache_node(direction),
            output: "__als_source_cached_pose",
            minimum_translation_span: 0.0,
            minimum_rotation_span_radians: 0.05,
        },
        ManualAnimationPoseBoundary {
            label: "cache_consumer",
            graph: "graphs/ab_als/stances/crouching.animgraph.ron",
            node: "Movement States\nState Machine",
            output: "Pose",
            minimum_translation_span: 0.0,
            minimum_rotation_span_radians: 0.05,
        },
        ManualAnimationPoseBoundary {
            label: "movement_fsm",
            graph: "graphs/ab_als/stances/crouching/movement_states_runtime.animgraph.ron",
            node: "crouching movement states runtime",
            output: "pose",
            minimum_translation_span: 0.0,
            minimum_rotation_span_radians: 0.05,
        },
        ManualAnimationPoseBoundary {
            label: "outer_fsm",
            graph: "graphs/ab_als/stances/crouching/crouching_states_runtime.animgraph.ron",
            node: "crouching states runtime",
            output: "pose",
            minimum_translation_span: 0.0,
            minimum_rotation_span_radians: 0.05,
        },
        ManualAnimationPoseBoundary {
            label: "parent_graph",
            graph: "graphs/ab_als/stances/crouching.animgraph.ron",
            node: "Crouching States\nState Machine",
            output: "Pose",
            minimum_translation_span: 0.0,
            minimum_rotation_span_radians: 0.05,
        },
        ManualAnimationPoseBoundary {
            label: "root_entry",
            graph: "graphs/ab_als/stances/crouching_runtime_entry.animgraph.ron",
            node: "crouching source graph runtime entry",
            output: "pose",
            minimum_translation_span: 0.0,
            minimum_rotation_span_radians: 0.05,
        },
    ]
}

fn crouch_rotate_source_pose_chain(left: bool) -> Vec<ManualAnimationPoseBoundary> {
    let graph = if left {
        "graphs/ab_als/stances/crouching/state_rotate_left.animgraph.ron"
    } else {
        "graphs/ab_als/stances/crouching/state_rotate_right.animgraph.ron"
    };
    let sequence = if left {
        "A_Als_Crouch_Rotate_90_Left\nSequence Player"
    } else {
        "A_Als_Crouch_Rotate_90_Right\nSequence Player"
    };
    vec![
        ManualAnimationPoseBoundary {
            label: "sequence_player",
            graph,
            node: sequence,
            output: "Pose",
            minimum_translation_span: 0.0,
            minimum_rotation_span_radians: 0.05,
        },
        ManualAnimationPoseBoundary {
            label: "curve_output",
            graph,
            node: "Modify Curve",
            output: "Pose",
            minimum_translation_span: 0.0,
            minimum_rotation_span_radians: 0.05,
        },
        ManualAnimationPoseBoundary {
            label: "outer_fsm",
            graph: "graphs/ab_als/stances/crouching/crouching_states_runtime.animgraph.ron",
            node: "crouching states runtime",
            output: "pose",
            minimum_translation_span: 0.0,
            minimum_rotation_span_radians: 0.05,
        },
        ManualAnimationPoseBoundary {
            label: "parent_graph",
            graph: "graphs/ab_als/stances/crouching.animgraph.ron",
            node: "Crouching States\nState Machine",
            output: "Pose",
            minimum_translation_span: 0.0,
            minimum_rotation_span_radians: 0.05,
        },
        ManualAnimationPoseBoundary {
            label: "root_entry",
            graph: "graphs/ab_als/stances/crouching_runtime_entry.animgraph.ron",
            node: "crouching source graph runtime entry",
            output: "pose",
            minimum_translation_span: 0.0,
            minimum_rotation_span_radians: 0.05,
        },
    ]
}

fn crouch_turn_source_pose_chain() -> Vec<ManualAnimationPoseBoundary> {
    vec![
        ManualAnimationPoseBoundary {
            label: "slot",
            graph: "graphs/ab_als/stances/crouching/state_idle.animgraph.ron",
            node: "Slot 'TurnInPlaceCrouching'\nGroup 'Grounded'",
            output: "Pose",
            minimum_translation_span: 0.0,
            minimum_rotation_span_radians: 0.05,
        },
        ManualAnimationPoseBoundary {
            label: "refresh_turn",
            graph: "graphs/ab_als/stances/crouching/state_idle.animgraph.ron",
            node: "RefreshTurnInPlace\nOn Update",
            output: "Pose",
            minimum_translation_span: 0.0,
            minimum_rotation_span_radians: 0.05,
        },
        ManualAnimationPoseBoundary {
            label: "outer_fsm",
            graph: "graphs/ab_als/stances/crouching/crouching_states_runtime.animgraph.ron",
            node: "crouching states runtime",
            output: "pose",
            minimum_translation_span: 0.0,
            minimum_rotation_span_radians: 0.05,
        },
        ManualAnimationPoseBoundary {
            label: "parent_graph",
            graph: "graphs/ab_als/stances/crouching.animgraph.ron",
            node: "Crouching States\nState Machine",
            output: "Pose",
            minimum_translation_span: 0.0,
            minimum_rotation_span_radians: 0.05,
        },
        ManualAnimationPoseBoundary {
            label: "root_entry",
            graph: "graphs/ab_als/stances/crouching_runtime_entry.animgraph.ron",
            node: "crouching source graph runtime entry",
            output: "pose",
            minimum_translation_span: 0.0,
            minimum_rotation_span_radians: 0.05,
        },
    ]
}

fn crouch_rotate_window(
    left: bool,
    active: bool,
    observed_lifecycle_stages: &'static [&'static str],
) -> ManualAnimationWindowEvidence {
    ManualAnimationWindowEvidence {
        label: Some(if left {
            "crouch_rotate_left"
        } else {
            "crouch_rotate_right"
        }),
        index: None,
        settled: true,
        capture_source_window: active || !observed_lifecycle_stages.is_empty(),
        requires_pre_rig_motion: active,
        requires_final_pose_motion: active,
        requires_non_reference_root_pose: active,
        requires_source_pose_chain: active,
        requires_actor_yaw_motion: active,
        requires_actor_yaw_convergence: active,
        expected_actor_yaw_delta_sign: Some(if left { 1 } else { -1 }),
        expected_actor_yaw_baseline: None,
        expected_animation_variant: Some(crouch_stationary_animation_variant(left, false)),
        source_node: Some(if left {
            "AB_Als_Crouching/A_Als_Crouch_Rotate_90_Left"
        } else {
            "AB_Als_Crouching/A_Als_Crouch_Rotate_90_Right"
        }),
        source_state: Some(if left { "Rotate Left" } else { "Rotate Right" }),
        source_pose_chain: active
            .then(|| crouch_rotate_source_pose_chain(left))
            .unwrap_or_default(),
        minimum_pre_rig_translation_span: 0.0,
        minimum_pre_rig_rotation_span_radians: if active { 0.05 } else { 0.0 },
        minimum_final_translation_span: 0.0,
        minimum_final_rotation_span_radians: if active { 0.05 } else { 0.0 },
        minimum_actor_yaw_delta_radians: if active {
            CROUCH_ROTATE_SOURCE_MIN_YAW_CONVERGENCE_RADIANS
        } else {
            0.0
        },
        minimum_actor_yaw_convergence_radians: if active {
            CROUCH_ROTATE_SOURCE_MIN_YAW_CONVERGENCE_RADIANS
        } else {
            0.0
        },
        minimum_observation_seconds: 0.0,
        source_sync_group: None,
        sync_phase_global: None,
        canonical_sync_phase: None,
        maximum_sync_phase_distance: 0.0,
        canonical_clip_time: None,
        maximum_clip_time_distance: 0.0,
        direction_signature_group: None,
        required_direction_signature_members: 0,
        minimum_direction_signature_rotation_radians: 0.0,
        expected_direction: None,
        observed_direction: None,
        expected_runtime_state: None,
        observed_runtime_state: None,
        expected_parent_runtime_state: Some(if left { 2 } else { 3 }),
        expected_terminal_parent_runtime_state: Some(0),
        expected_terminal_parent_transition_active: Some(false),
        expected_terminal_requested_parent_transition_active: Some(true),
        observed_parent_runtime_state: None,
        observed_parent_transition_active: None,
        observed_requested_parent_runtime_state: None,
        observed_requested_parent_transition_active: None,
        observed_actor_yaw: None,
        observed_view_yaw: None,
        observed_animation_yaw_speed_degrees: None,
        observed_animation_curve_direct_yaw_speed_degrees: None,
        observed_animation_clip_time: None,
        observed_animation_weight: None,
        observed_animation_play_rate: None,
        observed_animation_curve_stack_weight: None,
        observed_animation_curve_stack_entries: Vec::new(),
        observed_animation_variant: None,
        expected_lifecycle_stages: &["sequence_completed", "settled"],
        observed_lifecycle_stages,
    }
}

fn crouch_turn_window(
    left: bool,
    active: bool,
    observed_lifecycle_stages: &'static [&'static str],
) -> ManualAnimationWindowEvidence {
    ManualAnimationWindowEvidence {
        label: Some(if left {
            "crouch_turn_left"
        } else {
            "crouch_turn_right"
        }),
        index: None,
        settled: true,
        capture_source_window: active || !observed_lifecycle_stages.is_empty(),
        requires_pre_rig_motion: active,
        requires_final_pose_motion: active,
        requires_non_reference_root_pose: active,
        requires_source_pose_chain: active,
        requires_actor_yaw_motion: active,
        requires_actor_yaw_convergence: active,
        expected_actor_yaw_delta_sign: Some(if left { 1 } else { -1 }),
        expected_actor_yaw_baseline: None,
        expected_animation_variant: Some(crouch_stationary_animation_variant(left, true)),
        source_node: Some("AB_Als_Crouching/Slot 'TurnInPlaceCrouching'"),
        source_state: Some("Idle"),
        source_pose_chain: active
            .then(crouch_turn_source_pose_chain)
            .unwrap_or_default(),
        minimum_pre_rig_translation_span: 0.0,
        minimum_pre_rig_rotation_span_radians: if active { 0.05 } else { 0.0 },
        minimum_final_translation_span: 0.0,
        minimum_final_rotation_span_radians: if active { 0.05 } else { 0.0 },
        minimum_actor_yaw_delta_radians: if active {
            CROUCH_TURN_SOURCE_MIN_YAW_CONVERGENCE_RADIANS
        } else {
            0.0
        },
        minimum_actor_yaw_convergence_radians: if active {
            CROUCH_TURN_SOURCE_MIN_YAW_CONVERGENCE_RADIANS
        } else {
            0.0
        },
        minimum_observation_seconds: 0.0,
        source_sync_group: None,
        sync_phase_global: None,
        canonical_sync_phase: None,
        maximum_sync_phase_distance: 0.0,
        canonical_clip_time: None,
        maximum_clip_time_distance: 0.0,
        direction_signature_group: None,
        required_direction_signature_members: 0,
        minimum_direction_signature_rotation_radians: 0.0,
        expected_direction: None,
        observed_direction: None,
        expected_runtime_state: None,
        observed_runtime_state: None,
        expected_parent_runtime_state: Some(0),
        expected_terminal_parent_runtime_state: Some(0),
        expected_terminal_parent_transition_active: Some(false),
        expected_terminal_requested_parent_transition_active: Some(false),
        observed_parent_runtime_state: None,
        observed_parent_transition_active: None,
        observed_requested_parent_runtime_state: None,
        observed_requested_parent_transition_active: None,
        observed_actor_yaw: None,
        observed_view_yaw: None,
        observed_animation_yaw_speed_degrees: None,
        observed_animation_curve_direct_yaw_speed_degrees: None,
        observed_animation_clip_time: None,
        observed_animation_weight: None,
        observed_animation_play_rate: None,
        observed_animation_curve_stack_weight: None,
        observed_animation_curve_stack_entries: Vec::new(),
        observed_animation_variant: None,
        expected_lifecycle_stages: &[
            "slot_active",
            "natural_completion",
            "inertialization",
            "settled",
        ],
        observed_lifecycle_stages,
    }
}

fn crouch_stationary_animation_variant(left: bool, turn: bool) -> &'static str {
    match (turn, left) {
        (false, true) => "A_Als_Crouch_Rotate_90_Left",
        (false, false) => "A_Als_Crouch_Rotate_90_Right",
        (true, true) => "A_Als_Crouch_Turn_90_Left",
        (true, false) => "A_Als_Crouch_Turn_90_Right",
    }
}

fn crouch_turn_stack_animation_variant(left: bool, is_180: bool) -> &'static str {
    match (is_180, left) {
        (false, true) => "A_Als_Crouch_Turn_90_Left",
        (false, false) => "A_Als_Crouch_Turn_90_Right",
        (true, true) => "A_Als_Crouch_Turn_180_Left",
        (true, false) => "A_Als_Crouch_Turn_180_Right",
    }
}

fn record_animation_window_trace(
    window: &mut ManualAnimationWindowEvidence,
    transform: &Transform,
    view_yaw: f32,
    rotation_animation: Option<&AlsRotationAnimationState>,
    crouching_graph: Option<&AlsCrouchingGraphTelemetry>,
    turn_drive: Option<&AlsTurnDrive>,
    turn_stack: Option<&AlsTurnMontageStack>,
    crouching: bool,
    animation_variant: Option<&'static str>,
) {
    window.observed_actor_yaw = Some(world_forward_to_yaw(transform.rotation * Vec3::NEG_Z));
    window.observed_view_yaw = Some(view_yaw);
    window.observed_animation_yaw_speed_degrees =
        rotation_animation.map(|value| value.yaw_speed_degrees);
    window.observed_animation_curve_direct_yaw_speed_degrees =
        crouching_graph.and_then(|value| value.graph_turn_curve_direct_yaw_speed_target);
    window.observed_animation_clip_time = turn_drive.map(|drive| {
        if drive.rotate_active || drive.rotate_finishing {
            drive.rotate_clip_time
        } else {
            drive.clip_time
        }
    });
    window.observed_animation_weight = turn_drive.map(|drive| {
        if drive.rotate_active || drive.rotate_finishing {
            1.0
        } else {
            drive.weight
        }
    });
    window.observed_animation_play_rate = turn_drive.map(|drive| {
        if drive.rotate_active || drive.rotate_finishing {
            drive.rotate_rate
        } else {
            drive.play_rate
        }
    });
    window.observed_animation_curve_stack_weight = turn_stack.map(|stack| {
        stack
            .entries
            .iter()
            .filter(|entry| entry.crouching == crouching)
            .map(|entry| entry.weight())
            .sum()
    });
    window.observed_animation_curve_stack_entries = turn_stack
        .map(|stack| {
            stack
                .entries
                .iter()
                .filter(|entry| entry.crouching == crouching)
                .map(|entry| ManualTurnStackEntryEvidence {
                    animation_variant: crouch_turn_stack_animation_variant(
                        entry.lr <= 0.5,
                        entry.is_180,
                    ),
                    elapsed_seconds: entry.elapsed,
                    weight: entry.weight(),
                    play_rate: entry.play_rate,
                    sampling_ready: entry.sampling_ready,
                })
                .collect()
        })
        .unwrap_or_default();
    window.observed_animation_variant = animation_variant;
}

fn crouch_stationary_source_precondition(
    motion: Option<&AlsActualMotion>,
    telemetry: Option<&AlsCrouchingGraphTelemetry>,
    drive: Option<&AlsTurnDrive>,
    slot: Option<&AlsStandingTurnSlotRuntime>,
) -> bool {
    motion.is_some_and(|motion| motion.horizontal_speed <= crate::turn_layer::TURN_GROUND_SPEED_EPS)
        && telemetry.is_some_and(|telemetry| {
            telemetry.actual_outer_state == Some(0)
                && telemetry.actual_outer_transition_active == Some(false)
                && telemetry.actual_outer_pose_ready == Some(true)
        })
        && drive.is_none_or(|drive| {
            !drive.rotate_active
                && !drive.rotate_finishing
                && !drive.playing
                && drive.weight <= 0.05
        })
        && slot.is_none_or(|slot| !slot.active && slot.weight <= 0.05)
}

fn source_rotate_window_active(drive: Option<&AlsTurnDrive>) -> bool {
    drive.is_some_and(|drive| drive.rotate_active || drive.rotate_finishing)
}

fn source_rotate_window_completed(active: bool, cycle_completed: bool, stationary: bool) -> bool {
    active && cycle_completed && stationary
}

fn advance_crouch_stationary_stability(frames: u8, stable: bool) -> u8 {
    if stable {
        frames
            .saturating_add(1)
            .min(CROUCH_STATIONARY_STABLE_FRAMES)
    } else {
        0
    }
}

fn crouch_direction_regression_requires_stationary_turn_phase(
    elapsed: f32,
    direction_duration: f32,
) -> bool {
    elapsed >= direction_duration
}

fn crouch_stationary_validation_holds_aim(validation: Option<CrouchStationaryValidation>) -> bool {
    validation.is_some_and(|validation| !validation.is_turn())
}

fn source_stationary_target_yaw(base_yaw: f32, left: bool) -> f32 {
    base_yaw
        + if left {
            90.0_f32.to_radians()
        } else {
            -90.0_f32.to_radians()
        }
}

fn movement_direction_fsm_index(direction: AlsMovementDirection) -> u8 {
    match direction {
        AlsMovementDirection::Forward => 0,
        AlsMovementDirection::Backward => 1,
        AlsMovementDirection::RightForward => 2,
        AlsMovementDirection::RightBackward => 3,
        AlsMovementDirection::LeftForward => 4,
        AlsMovementDirection::LeftBackward => 5,
    }
}

fn movement_direction_from_fsm_index(index: u8) -> Option<AlsMovementDirection> {
    match index {
        0 => Some(AlsMovementDirection::Forward),
        1 => Some(AlsMovementDirection::Backward),
        2 => Some(AlsMovementDirection::RightForward),
        3 => Some(AlsMovementDirection::RightBackward),
        4 => Some(AlsMovementDirection::LeftForward),
        5 => Some(AlsMovementDirection::LeftBackward),
        _ => None,
    }
}

fn movement_direction_name(direction: AlsMovementDirection) -> &'static str {
    match direction {
        AlsMovementDirection::Forward => "Forward",
        AlsMovementDirection::Backward => "Backward",
        AlsMovementDirection::RightForward => "RightForward",
        AlsMovementDirection::RightBackward => "RightBackward",
        AlsMovementDirection::LeftForward => "LeftForward",
        AlsMovementDirection::LeftBackward => "LeftBackward",
    }
}

fn direction_branch_matches(index: Option<u8>, mask: u8) -> bool {
    index.is_some_and(|index| index < 8 && mask & (1 << index) != 0)
}

fn actual_velocity_branch_mask(motion: &AlsActualMotion) -> u8 {
    let weights = [
        motion.velocity_blend.forward,
        motion.velocity_blend.backward,
        motion.velocity_blend.left,
        motion.velocity_blend.right,
    ];
    let maximum = weights.into_iter().fold(0.0_f32, f32::max);
    if maximum <= f32::EPSILON {
        return 0;
    }
    weights
        .into_iter()
        .enumerate()
        .filter(|(_, weight)| *weight >= maximum - 1e-4)
        .fold(0, |mask, (index, _)| mask | (1 << index))
}

fn foot_ik_baseline_bit(index: usize) -> Option<u8> {
    match index {
        0 => Some(1 << 0),
        1 => Some(1 << 1),
        2 => Some(1 << 2),
        3 => Some(1 << 3),
        10 => Some(1 << 4),
        _ => None,
    }
}

#[derive(Default)]
struct IntegrationCoverage {
    assembly: bool,
    graph: bool,
    locomotion: bool,
    backward: bool,
    sideways: bool,
    standing_graph: bool,
    default_overlay_idle: bool,
    default_overlay_moving: bool,
    default_overlay_crouching: bool,
    default_overlay_air: bool,
    default_overlay_curves: bool,
    direction_cases: [bool; DIRECTION_CASE_COUNT],
    crouch_transaction: bool,
    crouch_graph: bool,
    crouch_air_forced_standing: bool,
    crouch_air_landed: bool,
    crouch_graph_recovered: bool,
    crouch_cycles: bool,
    crouch_canopy_visible: bool,
    crouch_canopy_blocked: bool,
    crouch_canopy_recovered: bool,
    crouch_canopy_cleaned: bool,
    crouch_moving: bool,
    crouching: bool,
    crouch_direction_cases: [bool; CROUCH_DIRECTION_CASE_COUNT],
    crouch_direction_diagnostics: [bool; CROUCH_DIRECTION_CASE_COUNT],
    crouch_direction_state_mask: u8,
    crouch_direction_lean: bool,
    crouch_rotate_mask: u8,
    crouch_rotate_source_completion_mask: u8,
    crouch_rotate_completion_mask: u8,
    crouch_turn_active_mask: u8,
    crouch_turn_completion_mask: u8,
    sprinting: bool,
    airborne: bool,
    landed: bool,
    overlay: bool,
    overlay_interface_binding: bool,
    overlay_interface_fallback: bool,
    overlay_interface_reentry: bool,
    gait_layer: bool,
    stance_layer: bool,
    jump_variant: bool,
    turn_in_place: bool,
    layering: bool,
    overlay_object: bool,
    start_stop: bool,
    view_head: bool,
    control_rig: bool,
    control_rig_stages: bool,
    control_rig_input: bool,
    control_rig_foot_targets: bool,
    mantle_started: bool,
    mantle_completed: bool,
    movement_base: bool,
    foot_probe: bool,
    foot_lock: bool,
    foot_ik_space_baseline: bool,
    foot_ik_no_hit_recovered: bool,
    foot_ik_weight_ramp: bool,
    foot_ik_teleport: bool,
    foot_ik_slope: bool,
    foot_ik_step: bool,
    foot_ik_crouching: bool,
    foot_ik_jump_air: bool,
    foot_ik_landed: bool,
    foot_ik_roll: bool,
    foot_ik_mantle: bool,
    foot_ik_terrain_cleaned: bool,
    sprint_curve_relay: bool,
    roll_started: bool,
    roll_slot: bool,
    grounded_roll_exit_transition: bool,
    ragdoll_entered: bool,
    ragdoll_pa_profile: bool,
    ragdoll_follow: bool,
    ragdoll_joint_drive: bool,
    ragdoll_recovered: bool,
    ragdoll_camera_pivot: bool,
    get_up_front: bool,
    get_up_back: bool,
    get_up_input_locked: [bool; 2],
    get_up_input_released: [bool; 2],
    camera_rig: bool,
    camera_collision: bool,
    camera_recovery: bool,
    camera_first_person: bool,
    camera_shoulder: bool,
    camera_shake: bool,
    camera_unscaled_time: bool,
    hud_layout: bool,
    hud_toggle: bool,
    hud_slomo: bool,
    overlay_menu: bool,
    show_debug: bool,
    debug_draw: bool,
    notify_sprint: bool,
    notify_sprint_acceleration: bool,
    notify_roll: bool,
    notify_marker: bool,
    notify_grounded_entry: bool,
    notify_root_scale: bool,
    notify_rolling_action: bool,
    notify_mantling_action: bool,
    notify_early_blend_out: bool,
    p1_ground: bool,
    p1_stance: bool,
    p1_gait: bool,
    p1_air: bool,
    p1_turn: bool,
    generated_transition_stop_quick: bool,
    turn_slot: bool,
    turn_inertialization: bool,
}

#[derive(Resource)]
struct ManualIntegrationState {
    character: Option<Entity>,
    avatar: Option<Entity>,
    camera: Option<Entity>,
    spawn_translation: Option<Vec3>,
    spawn_yaw: Option<f32>,
    scenario: IntegrationScenario,
    phase: IntegrationPhase,
    phase_started_seconds: f64,
    game_time_seconds: f64,
    deadline: Instant,
    entered: bool,
    initialized: bool,
    camera_obstacles: [Option<Entity>; 2],
    camera_obstacle_removed: bool,
    camera_blocked_ratio: Option<f32>,
    shake_requested: bool,
    dilation_active: bool,
    initial_view_yaw: Option<f32>,
    initial_view_pitch: Option<f32>,
    mantle_target_y: Option<f32>,
    mantle_was_active: bool,
    ragdoll_seen: bool,
    ragdoll_exit_requested: bool,
    ragdoll_frames: u32,
    get_up_case: usize,
    get_up_fixture_seeded: bool,
    ragdoll_grounded_at: Option<f64>,
    ragdoll_recovered_at: Option<f64>,
    ragdoll_current_joint_anchor_error_m: f32,
    ragdoll_current_joint_anchor_label: Option<String>,
    ragdoll_max_joint_anchor_error_m: f32,
    ragdoll_max_joint_anchor_label: Option<String>,
    ragdoll_handoff_locked_joint_anchor_error_m: f32,
    ragdoll_handoff_locked_joint_anchor_label: Option<String>,
    ragdoll_handoff_pose_error_m: f32,
    ragdoll_handoff_rotation_error_radians: f32,
    camera_scenario_armed: bool,
    camera_last_probe_secs: f32,
    camera_view_mode_requested: bool,
    camera_shoulder_requested: bool,
    interface_debug_armed: bool,
    interface_hud_hidden_seen: bool,
    interface_hud_restored_seen: bool,
    interface_slomo_seen: bool,
    interface_slomo_restored_seen: bool,
    interface_menu_open_seen: bool,
    interface_menu_closed_seen: bool,
    interface_overlay_rifle_seen: bool,
    interface_overlay_default_after_rifle_seen: bool,
    interface_overlay_reentry_seen: bool,
    notify_roll_armed: bool,
    notify_mantle_armed: bool,
    direction_case_index: usize,
    crouch_direction_case_index: usize,
    crouch_stationary_validation: Option<CrouchStationaryValidation>,
    temporal_rebase_epoch: u64,
    crouch_stationary_precondition_ready: [bool; 2],
    crouch_stationary_precondition_frames: [u8; 2],
    crouch_stationary_base_yaw: [Option<f32>; 2],
    crouch_turn_epoch_baseline: [Option<f32>; 2],
    foot_ik_baseline_mask: u8,
    foot_ik_no_hit_logged: bool,
    foot_ik_weight_mask: u8,
    foot_ik_teleport_requested: bool,
    foot_ik_teleport_baseline: Option<(u32, u32, u32)>,
    foot_ik_terrain_air_seen: bool,
    crouch_recovery_jump_requested: bool,
    crouch_recovery_air_crouch_requested: bool,
    crouch_recovery_standing_requested: bool,
    crouch_clearance_stage: CrouchClearanceStage,
    crouch_clearance_cycles: u16,
    crouch_clearance_last_actual: Stance,
    crouch_canopy: Option<Entity>,
    crouch_canopy_center: Option<Vec3>,
    crouch_canopy_prepare_at: Option<f64>,
    crouch_canopy_armed_at: Option<f64>,
    foot_ik_slope: Option<Entity>,
    foot_ik_step: Option<Entity>,
    p1_pivot_sent: bool,
    p1_ground_mask: u8,
    p1_gait_mask: u8,
    p1_air_mask: u8,
    p1_turn_mask: u8,
    p1_stance_mask: u8,
    generated_stop_quick_input_started: bool,
    generated_stop_quick_input_released: bool,
    grounded_roll_exit_diagnostic: String,
    timed_out: bool,
    final_logged: bool,
    exit_requested: bool,
    coverage: IntegrationCoverage,
}

impl Default for ManualIntegrationState {
    fn default() -> Self {
        let now = Instant::now();
        Self {
            character: None,
            avatar: None,
            camera: None,
            spawn_translation: None,
            spawn_yaw: None,
            scenario: IntegrationScenario::from_environment(),
            phase: IntegrationPhase::Await,
            phase_started_seconds: 0.0,
            game_time_seconds: 0.0,
            deadline: now + GATE_TIMEOUT,
            entered: false,
            initialized: false,
            camera_obstacles: [None, None],
            camera_obstacle_removed: false,
            camera_blocked_ratio: None,
            shake_requested: false,
            dilation_active: false,
            initial_view_yaw: None,
            initial_view_pitch: None,
            mantle_target_y: None,
            mantle_was_active: false,
            ragdoll_seen: false,
            ragdoll_exit_requested: false,
            ragdoll_frames: 0,
            get_up_case: 0,
            get_up_fixture_seeded: false,
            ragdoll_grounded_at: None,
            ragdoll_recovered_at: None,
            ragdoll_current_joint_anchor_error_m: 0.0,
            ragdoll_current_joint_anchor_label: None,
            ragdoll_max_joint_anchor_error_m: 0.0,
            ragdoll_max_joint_anchor_label: None,
            ragdoll_handoff_locked_joint_anchor_error_m: 0.0,
            ragdoll_handoff_locked_joint_anchor_label: None,
            ragdoll_handoff_pose_error_m: 0.0,
            ragdoll_handoff_rotation_error_radians: 0.0,
            camera_scenario_armed: false,
            camera_last_probe_secs: -1.0,
            camera_view_mode_requested: false,
            camera_shoulder_requested: false,
            interface_debug_armed: false,
            interface_hud_hidden_seen: false,
            interface_hud_restored_seen: false,
            interface_slomo_seen: false,
            interface_slomo_restored_seen: false,
            interface_menu_open_seen: false,
            interface_menu_closed_seen: false,
            interface_overlay_rifle_seen: false,
            interface_overlay_default_after_rifle_seen: false,
            interface_overlay_reentry_seen: false,
            notify_roll_armed: false,
            notify_mantle_armed: false,
            direction_case_index: usize::MAX,
            crouch_direction_case_index: usize::MAX,
            crouch_stationary_validation: None,
            temporal_rebase_epoch: 0,
            crouch_stationary_precondition_ready: [false; 2],
            crouch_stationary_precondition_frames: [0; 2],
            crouch_stationary_base_yaw: [None; 2],
            crouch_turn_epoch_baseline: [None; 2],
            foot_ik_baseline_mask: 0,
            foot_ik_no_hit_logged: false,
            foot_ik_weight_mask: 0,
            foot_ik_teleport_requested: false,
            foot_ik_teleport_baseline: None,
            foot_ik_terrain_air_seen: false,
            crouch_recovery_jump_requested: false,
            crouch_recovery_air_crouch_requested: false,
            crouch_recovery_standing_requested: false,
            crouch_clearance_stage: CrouchClearanceStage::default(),
            crouch_clearance_cycles: 0,
            crouch_clearance_last_actual: Stance::Standing,
            crouch_canopy: None,
            crouch_canopy_center: None,
            crouch_canopy_prepare_at: None,
            crouch_canopy_armed_at: None,
            foot_ik_slope: None,
            foot_ik_step: None,
            p1_pivot_sent: false,
            p1_ground_mask: 0,
            p1_gait_mask: 0,
            p1_air_mask: 0,
            p1_turn_mask: 0,
            p1_stance_mask: 0,
            generated_stop_quick_input_started: false,
            generated_stop_quick_input_released: false,
            grounded_roll_exit_diagnostic: "not observed".to_owned(),
            timed_out: false,
            final_logged: false,
            exit_requested: false,
            coverage: IntegrationCoverage::default(),
        }
    }
}

impl ManualIntegrationState {
    fn phase_elapsed(&self) -> f32 {
        (self.game_time_seconds - self.phase_started_seconds).max(0.0) as f32
    }
}

#[derive(SystemParam)]
struct InterfaceProbe<'w, 's> {
    virtual_time: Res<'w, Time<Virtual>>,
    visual_progress: Option<Res<'w, AnimationVisualValidationProgress>>,
    hud_state: Res<'w, AlsHudState>,
    menu_state: Res<'w, OverlayMenuState>,
    show_debug: Res<'w, AlsShowDebugHudState>,
    debug_stats: Res<'w, AlsDebugDisplayStats>,
    overlay_props: Query<'w, 's, (), With<AlsOverlayObject>>,
    hud_roots: Query<'w, 's, (), With<AlsHudRoot>>,
    hud_debug_nodes: Query<'w, 's, &'static Node, With<AlsHudDebugSection>>,
    hud_input_nodes: Query<'w, 's, &'static Node, With<AlsHudInputSection>>,
    show_debug_roots: Query<'w, 's, (), With<AlsShowDebugHudRoot>>,
}

#[derive(SystemParam)]
struct MotionProbe<'w, 's> {
    standing: Query<'w, 's, &'static AlsStandingGraphTelemetry, With<AlsCharacter>>,
    crouching_graph: Query<'w, 's, &'static AlsCrouchingGraphTelemetry, With<AlsCharacter>>,
    grounded: Query<'w, 's, &'static AlsGroundedGraphTelemetry, With<AlsCharacter>>,
    crouch: Query<'w, 's, &'static AlsCrouchTransaction, With<AlsCharacter>>,
    actual: Query<'w, 's, &'static AlsActualMotion, With<AlsCharacter>>,
    gait: Query<'w, 's, (&'static AlsDesiredGait, &'static AlsMaxAllowedGait), With<AlsCharacter>>,
    rotation_animation:
        Query<'w, 's, Option<&'static AlsRotationAnimationState>, With<AlsCharacter>>,
    generated_transition_notify:
        Query<'w, 's, Option<&'static GeneratedTransitionNotifyTelemetry>, With<AlsCharacter>>,
}

#[derive(SystemParam)]
struct RagdollPaProbe<'w, 's> {
    labels: Query<'w, 's, &'static BodyLabel>,
    profiles: Query<
        'w,
        's,
        (
            &'static RagdollPhysicsProfileApplied,
            &'static RagdollPhysicsProfile,
        ),
    >,
    collision_members: Query<'w, 's, &'static RagdollCollisionMember>,
    body_modes: Query<'w, 's, &'static RigidBody>,
    get_ups: Query<
        'w,
        's,
        (
            Option<&'static crate::get_up::AlsGetUpState>,
            Has<als_core::motion::AlsMovementInputBlocked>,
        ),
        With<AlsCharacter>,
    >,
    camera_sockets: Query<
        'w,
        's,
        (
            &'static als_camera::AlsCameraSocketLocations,
            &'static GlobalTransform,
            &'static Collider,
        ),
        With<AlsCharacter>,
    >,
}

fn get_up_fixture_rotation(facing_up: bool, yaw: f32) -> Quat {
    Quat::from_rotation_y(yaw)
        * Quat::from_rotation_x(if facing_up {
            -std::f32::consts::FRAC_PI_2
        } else {
            std::f32::consts::FRAC_PI_2
        })
}

fn prepare_get_up_physics_fixture(
    mut state: ResMut<ManualIntegrationState>,
    mut phase: ResMut<ManualRuntimePhaseEvidence>,
    characters: Query<&LocomotionState, With<AlsCharacter>>,
    avatars: Query<(&AlsCharacterAvatar, &AnimatedSceneInstance)>,
    players: Query<&AnimationGraphPlayer>,
    labels: Query<&BodyLabel>,
    mut bodies: Query<
        (
            &mut Position,
            &mut Rotation,
            &mut LinearVelocity,
            &mut avian3d::prelude::AngularVelocity,
            &RigidBody,
        ),
        Without<AlsCharacter>,
    >,
) {
    if state.scenario != IntegrationScenario::GetUp
        || state.get_up_fixture_seeded
        || state.phase != IntegrationPhase::Ragdoll
    {
        return;
    }
    let Some(character) = state.character else {
        return;
    };
    if characters.get(character) != Ok(&LocomotionState::Ragdolling) {
        return;
    }
    let Some(spawned) = avatars
        .iter()
        .find(|(avatar, _)| avatar.character == character)
        .and_then(|(_, instance)| players.get(instance.player_entity()).ok())
        .and_then(AnimationGraphPlayer::get_spawned_ragdoll)
    else {
        return;
    };
    let Some(pelvis) = spawned
        .bodies
        .values()
        .find(|entity| labels.get(**entity).is_ok_and(|label| label.0 == "pelvis"))
    else {
        return;
    };
    let Ok((position, rotation, _, _, _)) = bodies.get(*pelvis) else {
        return;
    };
    let pivot = position.0;
    let facing_up = state.get_up_case == 1;
    let delta = get_up_fixture_rotation(facing_up, state.spawn_yaw.unwrap_or_default())
        * rotation.0.inverse();
    for entity in spawned.bodies.values() {
        let Ok((mut position, mut rotation, mut velocity, mut angular_velocity, body)) =
            bodies.get_mut(*entity)
        else {
            continue;
        };
        if *body != RigidBody::Dynamic {
            continue;
        }
        position.0 = pivot + delta * (position.0 - pivot);
        rotation.0 = (delta * rotation.0).normalize();
        velocity.0 = Vec3::ZERO;
        angular_velocity.0 = Vec3::ZERO;
    }
    state.get_up_fixture_seeded = true;
    rebase_temporal_fixture(
        &mut state,
        &mut phase,
        if facing_up {
            "get_up_back_initial_orientation"
        } else {
            "get_up_front_initial_orientation"
        },
    );
}

fn spawn_manual_integration_fixture(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    origin: Vec3,
    facing_yaw: f32,
) {
    let facing = Quat::from_rotation_y(facing_yaw);
    let forward = facing * Vec3::NEG_Z;
    let right = facing * Vec3::X;
    let ground_y = origin.y - 0.92;
    let ledge_center = origin + forward * 6.0 + Vec3::Y * (ground_y + 0.4 - origin.y);
    let mantle_start = origin - forward * 1.5;
    let platform_home =
        origin + right * 9.0 + forward * 2.0 + Vec3::Y * (ground_y + 0.25 - origin.y);
    let locomotion_start = origin - right * 8.0 - forward * 2.0;
    let roll_start = origin - right * 8.0 + forward * 7.0;
    let ledge_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.12, 0.48, 0.76),
        perceptual_roughness: 0.7,
        ..default()
    });
    let platform_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.95, 0.48, 0.08),
        perceptual_roughness: 0.6,
        ..default()
    });
    let obstacle_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.75, 0.08, 0.18),
        perceptual_roughness: 0.5,
        ..default()
    });
    commands.spawn((
        Name::new("manual_integration_mantle_ledge"),
        RigidBody::Static,
        Collider::cuboid(2.4, 0.8, 1.2),
        Transform::from_translation(ledge_center),
        Mesh3d(meshes.add(Mesh::from(Cuboid::new(4.8, 1.6, 2.4)))),
        MeshMaterial3d(ledge_material),
    ));
    let platform = commands
        .spawn((
            Name::new("manual_integration_platform"),
            ManualIntegrationPlatform,
            RigidBody::Kinematic,
            Collider::cuboid(3.0, 0.25, 3.0),
            LinearVelocity(Vec3::X * 1.5),
            Transform::from_translation(platform_home),
            Mesh3d(meshes.add(Mesh::from(Cuboid::new(6.0, 0.5, 6.0)))),
            MeshMaterial3d(platform_material),
        ))
        .id();
    commands.insert_resource(ManualIntegrationFixture {
        platform,
        platform_home,
        locomotion_start,
        mantle_start,
        roll_start,
        facing_yaw,
        obstacle_mesh: meshes.add(Mesh::from(Cuboid::new(1.6, 1.6, 1.6))),
        obstacle_material,
    });
    info!(
        "[MANUAL-INTEGRATION] visible fixture attached to playground origin=({:.2},{:.2},{:.2})",
        origin.x, origin.y, origin.z
    );
}

fn initialize_manual_integration(
    mut commands: Commands,
    mut state: ResMut<ManualIntegrationState>,
    virtual_time: Res<Time<Virtual>>,
    mut foot_ik_telemetry_enabled: ResMut<AlsFootIkTelemetryEnabled>,
    mut foot_ik_weight_override: ResMut<AlsFootIkWeightOverride>,
    startup: Res<ManualPlaygroundStartupReady>,
    admissions: Query<&AlsPlayerAdmissionComplete>,
    assemblies: Query<&AlsCharacterAssembly>,
    avatars: Query<&AnimatedSceneInstance, With<AlsCharacterAvatar>>,
    players: Query<&AnimationGraphPlayer>,
    cameras: Query<(&AlsCameraRig, &AlsCameraDebugInfo)>,
    toggles: Query<&TnuaToggle, With<AlsCharacter>>,
    view: Query<(&AlsViewYaw, &AlsViewPitch), With<AlsCharacter>>,
    transforms: Query<&Transform, With<AlsCharacter>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    if state.initialized || !startup.passed {
        return;
    }
    state.game_time_seconds = virtual_time.elapsed_secs_f64();
    let Ok(admission) = admissions.single() else {
        return;
    };
    let character = admission.character;
    let Ok(assembly) = assemblies.get(character) else {
        return;
    };
    let Ok(instance) = avatars.get(assembly.avatar) else {
        return;
    };
    let Ok(player) = players.get(instance.player_entity()) else {
        return;
    };
    let Some(camera) = assembly.camera else {
        return;
    };
    let Ok((_rig, debug)) = cameras.get(camera) else {
        return;
    };
    let Ok(toggle) = toggles.get(character) else {
        return;
    };
    let Ok((view_yaw, view_pitch)) = view.get(character) else {
        return;
    };
    let Ok(transform) = transforms.get(character) else {
        return;
    };
    let graph_ready = matches!(player.get_animation_source(), AnimationSource::Graph(_))
        && player.get_error().is_none();
    if !graph_ready || !debug.valid || *toggle != TnuaToggle::Enabled {
        return;
    }
    commands
        .entity(character)
        .insert(AlsInputOverride::default());
    let spawn_yaw = transform.rotation.to_euler(EulerRot::YXZ).0;
    spawn_manual_integration_fixture(
        &mut commands,
        &mut meshes,
        &mut materials,
        transform.translation,
        spawn_yaw,
    );
    state.character = Some(character);
    state.avatar = Some(assembly.avatar);
    state.camera = Some(camera);
    state.spawn_translation = Some(transform.translation);
    state.spawn_yaw = Some(spawn_yaw);
    state.initial_view_yaw = Some(view_yaw.0);
    state.initial_view_pitch = Some(view_pitch.0);
    state.initialized = true;
    foot_ik_telemetry_enabled.0 = state.scenario == IntegrationScenario::FootIk;
    foot_ik_weight_override.left = None;
    foot_ik_weight_override.right = None;
    state.coverage.assembly = true;
    state.coverage.graph = true;
    let first_phase = initial_phase(state.scenario);
    begin_phase(&mut state, first_phase);
    info!(
        "[MANUAL-INTEGRATION][EXECUTED] runtime_assembly scenario={:?} character={character:?} avatar={:?} camera={camera:?} playable_plugin=AlsPlayableCharacterPlugin runtime_plugin=AlsCharacterRuntimePlugin graph=Graph tnua=Enabled camera_valid=true",
        state.scenario, assembly.avatar,
    );
}

#[allow(clippy::type_complexity)]
fn drive_manual_integration(
    mut commands: Commands,
    fixture: Option<Res<ManualIntegrationFixture>>,
    mut state: ResMut<ManualIntegrationState>,
    mut animation_window: ResMut<ManualAnimationWindowEvidence>,
    mut phase_evidence: ResMut<ManualRuntimePhaseEvidence>,
    mut virtual_time: ResMut<Time<Virtual>>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut show_debug: ResMut<AlsShowDebugHudState>,
    mut shake_requests: MessageWriter<AlsCameraShakeRequest>,
    mut pivot_notices: MessageWriter<AlsPivotNotify>,
    mut foot_ik_weight_override: ResMut<AlsFootIkWeightOverride>,
    params_assets: Res<Assets<AlsMovementParams>>,
    mut characters: Query<
        (
            &mut AlsInputOverride,
            &mut Transform,
            &mut Position,
            Option<&mut Rotation>,
            &mut LinearVelocity,
            &mut AlsStateRequest,
            &Stance,
            &LocomotionState,
            &mut Gait,
            &mut OverlayState,
            &mut RotationMode,
            &mut AlsViewYaw,
            &mut AlsViewPitch,
            Option<&AlsRagdollGroundFollow>,
            Option<&AlsStandingGraphTelemetry>,
        ),
        (With<AlsCharacter>, Without<ManualIntegrationPlatform>),
    >,
    mut crouch_state: Query<
        (
            &AlsMovementParamsHandle,
            &AlsCrouchTransaction,
            Option<&mut AlsRotationResponseState>,
            &mut AlsAimingYawConstraint,
            Option<&mut AlsRotationAnimationState>,
        ),
        With<AlsCharacter>,
    >,
    mut platforms: Query<
        (&mut Transform, Option<&mut Position>, &mut LinearVelocity),
        (With<ManualIntegrationPlatform>, Without<AlsCharacter>),
    >,
    cameras: Query<&AlsCameraDebugInfo, With<AlsCameraRig>>,
) {
    if !state.initialized || state.final_logged {
        return;
    }
    state.game_time_seconds = virtual_time.elapsed_secs_f64();
    phase_evidence.scenario = format!("{:?}", state.scenario);
    phase_evidence.phase = format!("{:?}", state.phase);
    phase_evidence.elapsed_seconds = state.phase_elapsed();
    phase_evidence.temporal_rebase_epoch = state.temporal_rebase_epoch;
    let Some(fixture) = fixture else {
        return;
    };
    if Instant::now() >= state.deadline {
        cleanup_crouch_canopy(&mut commands, &mut state);
        cleanup_foot_ik_terrain(&mut commands, &mut state);
        state.timed_out = true;
        virtual_time.set_relative_speed(1.0);
        begin_phase(&mut state, IntegrationPhase::Finish);
        return;
    }
    let Some(character) = state.character else {
        return;
    };
    let Ok((
        mut input,
        mut transform,
        mut position,
        mut rotation,
        mut velocity,
        mut request,
        stance,
        locomotion,
        mut gait,
        mut overlay,
        mut rotation_mode,
        mut view_yaw,
        mut view_pitch,
        ground_follow,
        standing_graph,
    )) = characters.get_mut(character)
    else {
        return;
    };
    let Ok((
        params_handle,
        transaction,
        rotation_response,
        mut aiming_yaw_constraint,
        rotation_animation,
    )) = crouch_state.get_mut(character)
    else {
        return;
    };
    let elapsed = state.phase_elapsed();
    let entering = std::mem::take(&mut state.entered);
    phase_evidence.locomotion = format!("{:?}", locomotion);
    phase_evidence.gait = format!("{:?}", gait);
    phase_evidence.stance = format!("{:?}", stance);
    phase_evidence.rotation_mode = format!("{:?}", rotation_mode);
    input.0.reset();
    if !matches!(
        state.phase,
        IntegrationPhase::DirectionRegression | IntegrationPhase::CrouchDirectionRegression
    ) {
        *animation_window = ManualAnimationWindowEvidence::default();
    }

    match state.phase {
        IntegrationPhase::Await => {}
        IntegrationPhase::DirectionRegression => {
            if entering {
                state.direction_case_index = usize::MAX;
                state.coverage.direction_cases = [false; DIRECTION_CASE_COUNT];
                state.foot_ik_baseline_mask = 0;
                state.coverage.foot_ik_space_baseline = false;
                state.coverage.foot_ik_no_hit_recovered = false;
                state.coverage.foot_ik_weight_ramp = false;
                state.foot_ik_weight_mask = 0;
                if state.scenario == IntegrationScenario::StateMachine {
                    state.coverage.p1_ground = false;
                    state.coverage.p1_stance = false;
                    state.coverage.p1_gait = false;
                    state.coverage.p1_air = false;
                    state.coverage.p1_turn = false;
                    state.coverage.generated_transition_stop_quick = false;
                    state.coverage.turn_slot = false;
                    state.coverage.turn_inertialization = false;
                    state.p1_pivot_sent = false;
                    state.p1_ground_mask = 0;
                    state.p1_gait_mask = 0;
                    state.p1_air_mask = 0;
                    state.p1_turn_mask = 0;
                    state.p1_stance_mask = 0;
                }
            }
            let case_index = (elapsed / DIRECTION_CASE_SECONDS) as usize;
            let Some(case) = direction_regression_case(case_index) else {
                *animation_window = ManualAnimationWindowEvidence::default();
                commands.entity(character).remove::<AlsFirstPersonView>();
                if state.scenario == IntegrationScenario::FootIk {
                    begin_phase(&mut state, IntegrationPhase::FootIkNoHit);
                } else if state.scenario == IntegrationScenario::StateMachine {
                    begin_phase(&mut state, IntegrationPhase::StateMachineGrounded);
                } else if state.scenario == IntegrationScenario::StandingDirection {
                    begin_phase(&mut state, IntegrationPhase::Finish);
                } else {
                    begin_phase(&mut state, IntegrationPhase::Locomotion);
                }
                return;
            };
            *animation_window = ManualAnimationWindowEvidence {
                label: Some(case.name),
                index: Some(case_index),
                settled: elapsed
                    >= case_index as f32 * DIRECTION_CASE_SECONDS + DIRECTION_CASE_SETTLE_SECONDS,
                capture_source_window: true,
                requires_pre_rig_motion: true,
                requires_final_pose_motion: true,
                requires_non_reference_root_pose: true,
                requires_source_pose_chain: true,
                requires_actor_yaw_motion: false,
                requires_actor_yaw_convergence: false,
                expected_actor_yaw_delta_sign: None,
                expected_actor_yaw_baseline: None,
                expected_animation_variant: None,
                source_node: Some("AB_Als_Standing/Movement States"),
                source_state: case.source_state,
                source_pose_chain: generated_standing_source_pose_chain(),
                minimum_pre_rig_translation_span: 0.0,
                minimum_pre_rig_rotation_span_radians: 0.0,
                minimum_final_translation_span: 0.0,
                minimum_final_rotation_span_radians: 0.0,
                minimum_actor_yaw_delta_radians: 0.0,
                minimum_actor_yaw_convergence_radians: 0.0,
                minimum_observation_seconds: 0.0,
                source_sync_group: case.direction_signature_group.map(|_| "Movement"),
                sync_phase_global: case
                    .direction_signature_group
                    .map(|_| "sync_groups.ab_als_standing.movement.phase"),
                canonical_sync_phase: case.direction_signature_group.map(|_| 0.5),
                canonical_clip_time: None,
                maximum_clip_time_distance: 0.0,
                maximum_sync_phase_distance: if case.direction_signature_group.is_some() {
                    0.12
                } else {
                    0.0
                },
                direction_signature_group: case.direction_signature_group,
                required_direction_signature_members: if case.direction_signature_group.is_some() {
                    5
                } else {
                    0
                },
                minimum_direction_signature_rotation_radians: if case
                    .direction_signature_group
                    .is_some()
                {
                    0.01
                } else {
                    0.0
                },
                expected_direction: case.expected_movement_state.map(movement_direction_name),
                observed_direction: None,
                expected_runtime_state: case
                    .expected_movement_state
                    .map(movement_direction_fsm_index),
                observed_runtime_state: None,
                expected_parent_runtime_state: None,
                expected_terminal_parent_runtime_state: None,
                expected_terminal_parent_transition_active: None,
                expected_terminal_requested_parent_transition_active: None,
                observed_parent_runtime_state: None,
                observed_parent_transition_active: None,
                observed_requested_parent_runtime_state: None,
                observed_requested_parent_transition_active: None,
                observed_actor_yaw: None,
                observed_view_yaw: None,
                observed_animation_yaw_speed_degrees: None,
                observed_animation_curve_direct_yaw_speed_degrees: None,
                observed_animation_clip_time: None,
                observed_animation_weight: None,
                observed_animation_play_rate: None,
                observed_animation_curve_stack_weight: None,
                observed_animation_curve_stack_entries: Vec::new(),
                observed_animation_variant: None,
                expected_lifecycle_stages: &[],
                observed_lifecycle_stages: &[],
            };
            if state.direction_case_index != case_index {
                let yaw = fixture.facing_yaw + case.yaw_offset;
                if entering || case.yaw_offset.abs() > f32::EPSILON {
                    reset_character_without_rotation(
                        &mut transform,
                        &mut position,
                        &mut velocity,
                        &mut view_yaw,
                        &mut view_pitch,
                        fixture.locomotion_start,
                        yaw,
                    );
                    rebase_temporal_fixture(&mut state, &mut phase_evidence, "direction_fixture");
                }
                *overlay = OverlayState::Default;
                request.locomotion_state = Some(LocomotionState::Grounded);
                request.stance = Some(Stance::Standing);
                request.gait = Some(case.desired_gait);
                request.rotation_mode = Some(case.rotation_mode);
                if case.first_person {
                    commands.entity(character).insert(AlsFirstPersonView);
                } else {
                    commands.entity(character).remove::<AlsFirstPersonView>();
                }
                state.direction_case_index = case_index;
                info!(
                    "[MANUAL-INTEGRATION][DIRECTION][START] case={} input=({:.3},{:.3}) desired={:?} first_person={} yaw_offset={:.1}",
                    case.name,
                    case.move_dir.x,
                    case.move_dir.y,
                    case.desired_gait,
                    case.first_person,
                    case.yaw_offset.to_degrees(),
                );
            }
            input.0.move_dir = case.move_dir;
            input.0.sprint_held = case.sprint_held;
            *rotation_mode = case.rotation_mode;
            request.locomotion_state = Some(LocomotionState::Grounded);
            request.stance = Some(Stance::Standing);
            request.gait = Some(case.desired_gait);
            request.rotation_mode = Some(case.rotation_mode);
        }
        IntegrationPhase::Sprint => {
            if entering {
                reset_character(
                    &mut transform,
                    &mut position,
                    rotation,
                    &mut velocity,
                    &mut view_yaw,
                    &mut view_pitch,
                    fixture.locomotion_start,
                    fixture.facing_yaw,
                );
                *gait = Gait::Sprinting;
                *overlay = OverlayState::Default;
                *rotation_mode = RotationMode::VelocityDirection;
                request.locomotion_state = Some(LocomotionState::Grounded);
                request.stance = Some(Stance::Standing);
                request.gait = Some(Gait::Sprinting);
                request.rotation_mode = Some(RotationMode::VelocityDirection);
                state.coverage.sprint_curve_relay = false;
                info!("[MANUAL-INTEGRATION][SPRINT] source-curve relay probe started");
            }
            input.0.move_dir = Vec2::Y;
            input.0.sprint_held = true;
            request.locomotion_state = Some(LocomotionState::Grounded);
            request.stance = Some(Stance::Standing);
            request.gait = Some(Gait::Sprinting);
            request.rotation_mode = Some(RotationMode::VelocityDirection);
            if elapsed >= 10.0 {
                begin_phase(&mut state, IntegrationPhase::Finish);
            }
        }
        IntegrationPhase::FootIkNoHit => {
            if entering {
                state.foot_ik_no_hit_logged = false;
                reset_character_without_rotation(
                    &mut transform,
                    &mut position,
                    &mut velocity,
                    &mut view_yaw,
                    &mut view_pitch,
                    fixture.locomotion_start,
                    fixture.facing_yaw,
                );
                *gait = Gait::Running;
                *overlay = OverlayState::Default;
                *rotation_mode = RotationMode::ViewDirection;
                request.locomotion_state = Some(LocomotionState::Grounded);
                request.stance = Some(Stance::Standing);
                request.gait = Some(Gait::Running);
                request.rotation_mode = Some(RotationMode::ViewDirection);
                commands.entity(character).insert(AlsMantleControlSettings {
                    allow_mantling: false,
                    auto_start_in_air: false,
                });
            }
            input.0.jump_pressed = (0.5..0.7).contains(&elapsed);
            input.0.jump_held = (0.5..1.0).contains(&elapsed);
            if elapsed >= 6.0 {
                commands
                    .entity(character)
                    .remove::<AlsMantleControlSettings>();
                begin_phase(&mut state, IntegrationPhase::FootIkWeightRamp);
            }
        }
        IntegrationPhase::FootIkWeightRamp => {
            if entering {
                reset_character(
                    &mut transform,
                    &mut position,
                    rotation,
                    &mut velocity,
                    &mut view_yaw,
                    &mut view_pitch,
                    fixture.locomotion_start,
                    fixture.facing_yaw,
                );
                *gait = Gait::Running;
                *overlay = OverlayState::Default;
                *rotation_mode = RotationMode::ViewDirection;
                request.locomotion_state = Some(LocomotionState::Grounded);
                request.stance = Some(Stance::Standing);
                request.gait = Some(Gait::Running);
                request.rotation_mode = Some(RotationMode::ViewDirection);
            }
            let target_weight = if elapsed < 0.75 {
                0.0
            } else if elapsed < 1.5 {
                0.25
            } else if elapsed < 2.25 {
                0.5
            } else if elapsed < 3.0 {
                0.75
            } else if elapsed < 3.75 {
                1.0
            } else {
                0.0
            };
            foot_ik_weight_override.left = Some(target_weight);
            foot_ik_weight_override.right = Some(target_weight);
            if elapsed >= 4.75 {
                foot_ik_weight_override.left = None;
                foot_ik_weight_override.right = None;
                begin_phase(&mut state, IntegrationPhase::FootIkTeleport);
            }
        }
        IntegrationPhase::FootIkTeleport => {
            if entering {
                reset_character(
                    &mut transform,
                    &mut position,
                    rotation,
                    &mut velocity,
                    &mut view_yaw,
                    &mut view_pitch,
                    fixture.locomotion_start,
                    fixture.facing_yaw,
                );
                *gait = Gait::Running;
                *overlay = OverlayState::Default;
                *rotation_mode = RotationMode::ViewDirection;
                request.locomotion_state = Some(LocomotionState::Grounded);
                request.stance = Some(Stance::Standing);
                request.gait = Some(Gait::Running);
                request.rotation_mode = Some(RotationMode::ViewDirection);
                state.foot_ik_teleport_requested = false;
                state.foot_ik_teleport_baseline = None;
                state.coverage.foot_ik_teleport = false;
            }
            input.0.move_dir = Vec2::Y;
            if elapsed >= 1.0 && !state.foot_ik_teleport_requested {
                let right = Quat::from_rotation_y(fixture.facing_yaw) * Vec3::X;
                let destination = transform.translation + right * 0.6;
                reset_character_without_rotation(
                    &mut transform,
                    &mut position,
                    &mut velocity,
                    &mut view_yaw,
                    &mut view_pitch,
                    destination,
                    fixture.facing_yaw,
                );
                state.foot_ik_teleport_requested = true;
                info!(
                    "[MANUAL-INTEGRATION][FOOT-IK-TELEPORT][REQUEST] distance_m=0.600 destination=({:.3},{:.3},{:.3})",
                    destination.x, destination.y, destination.z,
                );
            }
            if elapsed >= 3.0 {
                begin_phase(&mut state, IntegrationPhase::FootIkTerrain);
            }
        }
        IntegrationPhase::FootIkTerrain => {
            if entering {
                cleanup_foot_ik_terrain(&mut commands, &mut state);
                reset_character(
                    &mut transform,
                    &mut position,
                    rotation,
                    &mut velocity,
                    &mut view_yaw,
                    &mut view_pitch,
                    fixture.locomotion_start,
                    fixture.facing_yaw,
                );
                *gait = Gait::Walking;
                *overlay = OverlayState::Default;
                *rotation_mode = RotationMode::ViewDirection;
                request.locomotion_state = Some(LocomotionState::Grounded);
                request.stance = Some(Stance::Standing);
                request.gait = Some(Gait::Walking);
                request.rotation_mode = Some(RotationMode::ViewDirection);
                state.foot_ik_terrain_air_seen = false;
                state.coverage.foot_ik_slope = false;
                state.coverage.foot_ik_step = false;
                state.coverage.foot_ik_crouching = false;
                state.coverage.foot_ik_jump_air = false;
                state.coverage.foot_ik_landed = false;
                state.coverage.foot_ik_roll = false;
                state.coverage.foot_ik_terrain_cleaned = false;
                let forward = Quat::from_rotation_y(fixture.facing_yaw) * Vec3::NEG_Z;
                let ground_y = fixture.locomotion_start.y - 0.92;
                let slope = commands
                    .spawn((
                        Name::new("manual_foot_ik_slope"),
                        ManualFootIkTerrain,
                        RigidBody::Static,
                        Collider::cuboid(3.0, 0.12, 2.5),
                        Transform::from_translation(
                            fixture.locomotion_start
                                + forward * 0.8
                                + Vec3::Y * (ground_y + 0.20 - fixture.locomotion_start.y),
                        )
                        .with_rotation(Quat::from_rotation_x(-18.0_f32.to_radians()))
                        .with_scale(Vec3::new(3.75, 0.18, 3.125)),
                        Mesh3d(fixture.obstacle_mesh.clone()),
                        MeshMaterial3d(fixture.obstacle_material.clone()),
                    ))
                    .id();
                let step = commands
                    .spawn((
                        Name::new("manual_foot_ik_step"),
                        ManualFootIkTerrain,
                        RigidBody::Static,
                        Collider::cuboid(1.6, 0.20, 1.5),
                        Transform::from_translation(
                            fixture.locomotion_start
                                + forward * 3.5
                                + Vec3::Y * (ground_y + 0.20 - fixture.locomotion_start.y),
                        )
                        .with_scale(Vec3::new(2.0, 0.25, 1.875)),
                        Mesh3d(fixture.obstacle_mesh.clone()),
                        MeshMaterial3d(fixture.obstacle_material.clone()),
                    ))
                    .id();
                state.foot_ik_slope = Some(slope);
                state.foot_ik_step = Some(step);
                commands.entity(character).insert(AlsMantleControlSettings {
                    allow_mantling: false,
                    auto_start_in_air: false,
                });
                info!(
                    "[MANUAL-INTEGRATION][FOOT-IK-TERRAIN] slope={slope:?} step={step:?} slope_center=({:.3},{:.3},{:.3}) step_center=({:.3},{:.3},{:.3})",
                    fixture.locomotion_start.x + forward.x * 0.8,
                    ground_y + 0.20,
                    fixture.locomotion_start.z + forward.z * 0.8,
                    fixture.locomotion_start.x + forward.x * 3.5,
                    ground_y + 0.20,
                    fixture.locomotion_start.z + forward.z * 3.5,
                );
            }
            if (1.0..3.0).contains(&elapsed) || (3.2..4.2).contains(&elapsed) {
                input.0.move_dir = Vec2::Y;
            }
            input.0.crouch_pressed = (3.0..3.2).contains(&elapsed) || (4.2..4.4).contains(&elapsed);
            input.0.jump_pressed = (5.0..5.2).contains(&elapsed);
            input.0.jump_held = (5.0..5.6).contains(&elapsed);
            input.0.roll_pressed = (8.0..8.2).contains(&elapsed);
            if elapsed >= 10.0 {
                commands
                    .entity(character)
                    .remove::<AlsMantleControlSettings>();
                cleanup_foot_ik_terrain(&mut commands, &mut state);
                begin_phase(&mut state, IntegrationPhase::Mantle);
            }
        }
        IntegrationPhase::ControlRig => {
            if entering {
                reset_character(
                    &mut transform,
                    &mut position,
                    rotation,
                    &mut velocity,
                    &mut view_yaw,
                    &mut view_pitch,
                    fixture.locomotion_start,
                    fixture.facing_yaw,
                );
                *gait = Gait::Running;
                *overlay = OverlayState::Rifle;
                *rotation_mode = RotationMode::Aiming;
                request.locomotion_state = Some(LocomotionState::Grounded);
                request.stance = Some(Stance::Standing);
                request.rotation_mode = Some(RotationMode::Aiming);
                request.overlay_state = Some(OverlayState::Rifle);
            }
            input.0.move_dir = Vec2::new(0.65, 1.0).normalize();
            input.0.aim_held = true;
            view_yaw.0 = fixture.facing_yaw + 35.0_f32.to_radians();
            request.rotation_mode = Some(RotationMode::Aiming);
            request.overlay_state = Some(OverlayState::Rifle);
            if elapsed >= 6.0 {
                begin_phase(&mut state, IntegrationPhase::Finish);
            }
        }
        IntegrationPhase::DefaultOverlay => {
            if entering {
                reset_character(
                    &mut transform,
                    &mut position,
                    rotation,
                    &mut velocity,
                    &mut view_yaw,
                    &mut view_pitch,
                    fixture.locomotion_start,
                    fixture.facing_yaw,
                );
                *gait = Gait::Walking;
                *overlay = OverlayState::Default;
                *rotation_mode = RotationMode::Aiming;
            }
            request.overlay_state = Some(OverlayState::Default);
            request.rotation_mode = Some(RotationMode::Aiming);
            request.gait = Some(Gait::Walking);
            if elapsed < 1.2 {
                request.locomotion_state = Some(LocomotionState::Grounded);
                request.stance = Some(Stance::Standing);
            } else if elapsed < 2.8 {
                request.locomotion_state = Some(LocomotionState::Grounded);
                request.stance = Some(Stance::Standing);
                input.0.move_dir = Vec2::Y;
            } else if elapsed < 4.4 {
                request.locomotion_state = Some(LocomotionState::Grounded);
                request.stance = Some(Stance::Crouching);
                input.0.move_dir = Vec2::Y;
            } else {
                request.stance = Some(Stance::Standing);
                input.0.jump_pressed = (4.7..4.9).contains(&elapsed);
                input.0.jump_held = (4.7..5.4).contains(&elapsed);
            }
            if elapsed >= 8.0 {
                begin_phase(&mut state, IntegrationPhase::Finish);
            }
        }
        IntegrationPhase::StateMachineGrounded => {
            *animation_window = state_machine_grounded_window(elapsed);
            if entering {
                reset_character(
                    &mut transform,
                    &mut position,
                    rotation,
                    &mut velocity,
                    &mut view_yaw,
                    &mut view_pitch,
                    fixture.locomotion_start,
                    fixture.facing_yaw,
                );
                *gait = Gait::Running;
                *overlay = OverlayState::Default;
                *rotation_mode = RotationMode::Aiming;
                request.locomotion_state = Some(LocomotionState::Grounded);
                request.stance = Some(Stance::Standing);
                request.gait = Some(Gait::Running);
                request.rotation_mode = Some(RotationMode::Aiming);
                rebase_temporal_fixture(&mut state, &mut phase_evidence, "state_machine_grounded");
                info!("[MANUAL-INTEGRATION][P1][GROUND] sequence=idle_move_stop_pivot start");
            }
            let requested_gait = if (3.2..3.6).contains(&elapsed) {
                Gait::Walking
            } else {
                Gait::Running
            };
            request.locomotion_state = Some(LocomotionState::Grounded);
            request.stance = Some(Stance::Standing);
            request.gait = Some(requested_gait);
            request.rotation_mode = Some(RotationMode::Aiming);
            if (0.75..2.4).contains(&elapsed) {
                input.0.move_dir = Vec2::Y;
            } else if elapsed >= 3.2 {
                input.0.move_dir = Vec2::NEG_Y;
                let horizontal_speed = Vec2::new(velocity.0.x, velocity.0.z).length();
                if !state.p1_pivot_sent && (0.1..2.0).contains(&horizontal_speed) {
                    pivot_notices.write(AlsPivotNotify { character });
                    state.p1_pivot_sent = true;
                    info!(
                        "[MANUAL-INTEGRATION][P1][GROUND] pivot_notice speed={horizontal_speed:.3}"
                    );
                }
            } else {
                input.0.move_dir = Vec2::ZERO;
            }
            if elapsed >= 5.8 {
                begin_phase(&mut state, IntegrationPhase::StateMachineAir);
            }
        }
        IntegrationPhase::GeneratedTransitionNotify => {
            if entering {
                state.generated_stop_quick_input_started = false;
                state.generated_stop_quick_input_released = false;
            }
            let (input_started, input_released, should_move) = generated_stop_quick_input_step(
                state.generated_stop_quick_input_started,
                state.generated_stop_quick_input_released,
                standing_graph,
            );
            state.generated_stop_quick_input_started = input_started;
            state.generated_stop_quick_input_released = input_released;
            let stopping = input_released;
            *animation_window = ManualAnimationWindowEvidence {
                label: Some("generated_transition_stop_quick"),
                settled: stopping,
                capture_source_window: stopping,
                requires_pre_rig_motion: true,
                requires_final_pose_motion: true,
                requires_non_reference_root_pose: true,
                requires_source_pose_chain: true,
                source_node: Some("AB_Als_Standing/Standing States"),
                source_state: Some("Move to Idle"),
                source_pose_chain: generated_stop_quick_source_pose_chain(),
                ..default()
            };
            if entering {
                reset_character(
                    &mut transform,
                    &mut position,
                    rotation,
                    &mut velocity,
                    &mut view_yaw,
                    &mut view_pitch,
                    fixture.locomotion_start,
                    fixture.facing_yaw,
                );
                *gait = Gait::Running;
                *overlay = OverlayState::Default;
                *rotation_mode = RotationMode::Aiming;
                request.locomotion_state = Some(LocomotionState::Grounded);
                request.stance = Some(Stance::Standing);
                request.gait = Some(Gait::Running);
                request.rotation_mode = Some(RotationMode::Aiming);
                state.coverage.generated_transition_stop_quick = false;
                rebase_temporal_fixture(
                    &mut state,
                    &mut phase_evidence,
                    "generated_transition_notify",
                );
            }
            input.0.move_dir = if should_move { Vec2::Y } else { Vec2::ZERO };
            if state.coverage.generated_transition_stop_quick {
                begin_phase(&mut state, IntegrationPhase::Finish);
            } else if elapsed >= 7.0 {
                state.timed_out = true;
                begin_phase(&mut state, IntegrationPhase::Finish);
            }
        }
        IntegrationPhase::StateMachineAir => {
            if entering {
                reset_character(
                    &mut transform,
                    &mut position,
                    rotation,
                    &mut velocity,
                    &mut view_yaw,
                    &mut view_pitch,
                    fixture.locomotion_start,
                    fixture.facing_yaw,
                );
                *gait = Gait::Running;
                *overlay = OverlayState::Default;
                *rotation_mode = RotationMode::ViewDirection;
                request.locomotion_state = Some(LocomotionState::Grounded);
                request.stance = Some(Stance::Standing);
                request.gait = Some(Gait::Running);
                request.rotation_mode = Some(RotationMode::ViewDirection);
                rebase_temporal_fixture(&mut state, &mut phase_evidence, "state_machine_air");
                commands.entity(character).insert(AlsMantleControlSettings {
                    allow_mantling: false,
                    auto_start_in_air: false,
                });
                info!("[MANUAL-INTEGRATION][P1][AIR] standing_jump start");
            }
            input.0.jump_pressed = (0.5..0.7).contains(&elapsed);
            input.0.jump_held = (0.5..1.0).contains(&elapsed);
            if elapsed >= 5.0 {
                commands
                    .entity(character)
                    .remove::<AlsMantleControlSettings>();
                begin_phase(&mut state, IntegrationPhase::CrouchRecovery);
            }
        }
        IntegrationPhase::Locomotion => {
            if entering {
                reset_character(
                    &mut transform,
                    &mut position,
                    rotation,
                    &mut velocity,
                    &mut view_yaw,
                    &mut view_pitch,
                    fixture.locomotion_start,
                    fixture.facing_yaw,
                );
                *gait = Gait::Running;
                *overlay = OverlayState::Default;
                *rotation_mode = RotationMode::ViewDirection;
                request.locomotion_state = Some(LocomotionState::Grounded);
                request.stance = Some(Stance::Standing);
                state.crouch_recovery_air_crouch_requested = false;
                state.crouch_recovery_standing_requested = false;
                state.coverage.crouch_air_forced_standing = false;
                state.coverage.crouch_air_landed = false;
                state.coverage.crouch_graph_recovered = false;
            }
            if (1.0..2.5).contains(&elapsed) {
                input.0.move_dir = Vec2::NEG_Y;
            } else if (2.5..4.0).contains(&elapsed) {
                input.0.move_dir = Vec2::X;
            } else if (4.0..5.5).contains(&elapsed) {
                input.0.move_dir = Vec2::Y;
            } else if (5.5..7.0).contains(&elapsed) {
                input.0.move_dir = Vec2::Y;
                input.0.sprint_held = true;
            } else if (7.0..8.5).contains(&elapsed) {
                input.0.move_dir = Vec2::Y;
                input.0.crouch_pressed = elapsed < 7.2;
            } else if (8.5..10.0).contains(&elapsed) {
                input.0.move_dir = Vec2::Y;
                input.0.crouch_pressed = elapsed < 8.7;
            } else if (10.0..12.0).contains(&elapsed) {
                input.0.move_dir = Vec2::Y;
                input.0.jump_pressed = elapsed < 10.2;
                input.0.jump_held = elapsed < 10.6;
            } else if (12.0..14.0).contains(&elapsed) {
                request.overlay_state = Some(OverlayState::Rifle);
            } else if (14.0..15.0).contains(&elapsed) {
                request.overlay_state = Some(OverlayState::Default);
            }
            if (0.5..1.5).contains(&elapsed) {
                input.0.look_delta = Vec2::new(6.0, -3.0);
                input.0.aim_held = true;
            }
            if elapsed >= 15.0 {
                begin_phase(&mut state, IntegrationPhase::CrouchRecovery);
            }
        }
        IntegrationPhase::CrouchRecovery => {
            if entering {
                let standing_center = state.spawn_translation.unwrap_or(fixture.locomotion_start);
                let recovery_start = params_assets
                    .get(params_handle.id())
                    .map(|params| {
                        stance_center_from_standing_center(params, standing_center, *stance)
                    })
                    .unwrap_or(standing_center);
                reset_character(
                    &mut transform,
                    &mut position,
                    rotation,
                    &mut velocity,
                    &mut view_yaw,
                    &mut view_pitch,
                    recovery_start,
                    fixture.facing_yaw,
                );
                *gait = Gait::Running;
                *overlay = OverlayState::Default;
                *rotation_mode = RotationMode::ViewDirection;
                request.locomotion_state = Some(LocomotionState::Grounded);
                request.stance = Some(Stance::Standing);
                rebase_temporal_fixture(&mut state, &mut phase_evidence, "crouch_recovery");
                commands.entity(character).insert(AlsMantleControlSettings {
                    allow_mantling: false,
                    auto_start_in_air: false,
                });
                state.crouch_recovery_jump_requested = false;
                state.crouch_recovery_air_crouch_requested = false;
                state.crouch_recovery_standing_requested = false;
                state.coverage.crouch_graph = false;
                state.coverage.crouch_air_forced_standing = false;
                state.coverage.crouch_air_landed = false;
                state.coverage.crouch_graph_recovered = false;
            }
            if !state.crouch_recovery_jump_requested
                && elapsed >= 1.0
                && *stance == Stance::Standing
                && *locomotion == LocomotionState::Grounded
            {
                input.0.jump_pressed = true;
                input.0.jump_held = true;
                state.crouch_recovery_jump_requested = true;
            } else if state.coverage.crouch_air_landed && !state.crouch_recovery_standing_requested
            {
                input.0.crouch_pressed = true;
                state.crouch_recovery_standing_requested = true;
            } else if state.coverage.crouch_graph_recovered {
                commands
                    .entity(character)
                    .remove::<AlsMantleControlSettings>();
                if state.scenario == IntegrationScenario::StateMachine {
                    begin_phase(&mut state, IntegrationPhase::Turn);
                } else {
                    begin_phase(&mut state, IntegrationPhase::CrouchClearance);
                }
            }
            if elapsed >= 12.0 {
                commands
                    .entity(character)
                    .remove::<AlsMantleControlSettings>();
                state.timed_out = true;
                begin_phase(&mut state, IntegrationPhase::Finish);
            }
        }
        IntegrationPhase::CrouchClearance => {
            if entering {
                cleanup_crouch_canopy(&mut commands, &mut state);
                let standing_center = state.spawn_translation.unwrap_or(fixture.locomotion_start);
                let clearance_start = params_assets
                    .get(params_handle.id())
                    .map(|params| {
                        stance_center_from_standing_center(params, standing_center, *stance)
                    })
                    .unwrap_or(standing_center);
                reset_character(
                    &mut transform,
                    &mut position,
                    rotation,
                    &mut velocity,
                    &mut view_yaw,
                    &mut view_pitch,
                    clearance_start,
                    fixture.facing_yaw,
                );
                *gait = Gait::Running;
                *overlay = OverlayState::Default;
                *rotation_mode = RotationMode::ViewDirection;
                request.locomotion_state = Some(LocomotionState::Grounded);
                request.stance = Some(Stance::Standing);
                rebase_temporal_fixture(&mut state, &mut phase_evidence, "crouch_clearance");
                state.crouch_clearance_stage = CrouchClearanceStage::AwaitStanding;
                state.crouch_clearance_cycles = 0;
                state.crouch_clearance_last_actual = Stance::Standing;
                state.crouch_canopy_prepare_at = None;
                state.crouch_canopy_armed_at = None;
                state.coverage.crouch_cycles = false;
                state.coverage.crouch_canopy_visible = false;
                state.coverage.crouch_canopy_blocked = false;
                state.coverage.crouch_canopy_recovered = false;
                state.coverage.crouch_canopy_cleaned = false;
                state.coverage.crouch_moving = false;
            }
            match state.crouch_clearance_stage {
                CrouchClearanceStage::AwaitStanding => {
                    if *stance == Stance::Standing
                        && *locomotion == LocomotionState::Grounded
                        && !transaction.headroom_blocked
                    {
                        state.crouch_clearance_last_actual = *stance;
                        state.crouch_clearance_stage = CrouchClearanceStage::Cycling;
                        info!(
                            "[MANUAL-INTEGRATION][CROUCH-CLEARANCE-READY] desired={:?} actual={:?} half={:.3}",
                            transaction.desired_stance, stance, transaction.collider_half_height,
                        );
                    }
                }
                CrouchClearanceStage::Cycling => {
                    if *stance == Stance::Standing {
                        if state.crouch_clearance_last_actual == Stance::Crouching {
                            state.crouch_clearance_cycles =
                                state.crouch_clearance_cycles.saturating_add(1);
                            if state.crouch_clearance_cycles % 25 == 0
                                && state.crouch_clearance_cycles < 100
                            {
                                info!(
                                    "[MANUAL-INTEGRATION][CROUCH-CYCLES] pairs={} actual={:?} half={:.3}",
                                    state.crouch_clearance_cycles,
                                    stance,
                                    transaction.collider_half_height,
                                );
                            }
                        }
                        state.crouch_clearance_last_actual = Stance::Standing;
                        if state.crouch_clearance_cycles >= 100 {
                            state.coverage.crouch_cycles = true;
                            state.crouch_clearance_stage =
                                CrouchClearanceStage::PrepareCanopyCrouch;
                            info!(
                                "[MANUAL-INTEGRATION][CROUCH-CYCLES] pairs={} actual={:?} half={:.3}",
                                state.crouch_clearance_cycles,
                                stance,
                                transaction.collider_half_height,
                            );
                        } else {
                            request.stance = Some(Stance::Crouching);
                        }
                    } else {
                        state.crouch_clearance_last_actual = Stance::Crouching;
                        request.stance = Some(Stance::Standing);
                    }
                }
                CrouchClearanceStage::PrepareCanopyCrouch => {
                    let standing_center =
                        state.spawn_translation.unwrap_or(fixture.locomotion_start);
                    let canopy_start = params_assets
                        .get(params_handle.id())
                        .map(|params| {
                            stance_center_from_standing_center(params, standing_center, *stance)
                        })
                        .unwrap_or(standing_center);
                    reset_character_without_rotation(
                        &mut transform,
                        &mut position,
                        &mut velocity,
                        &mut view_yaw,
                        &mut view_pitch,
                        canopy_start,
                        fixture.facing_yaw,
                    );
                    *gait = Gait::Walking;
                    *rotation_mode = RotationMode::ViewDirection;
                    request.locomotion_state = Some(LocomotionState::Grounded);
                    request.stance = Some(Stance::Crouching);
                    request.gait = Some(Gait::Walking);
                    request.rotation_mode = Some(RotationMode::ViewDirection);
                    state.crouch_canopy_prepare_at = Some(state.game_time_seconds);
                    state.crouch_clearance_stage = CrouchClearanceStage::StabilizeCanopyCrouch;
                    info!(
                        "[MANUAL-INTEGRATION][CROUCH-CANOPY-STABILIZE] reset=({:.3},{:.3},{:.3})",
                        canopy_start.x, canopy_start.y, canopy_start.z,
                    );
                }
                CrouchClearanceStage::StabilizeCanopyCrouch => {
                    request.locomotion_state = Some(LocomotionState::Grounded);
                    request.stance = Some(Stance::Crouching);
                    request.gait = Some(Gait::Walking);
                    request.rotation_mode = Some(RotationMode::ViewDirection);
                    if *stance == Stance::Crouching
                        && *locomotion == LocomotionState::Grounded
                        && !transaction.headroom_blocked
                        && state
                            .crouch_canopy_prepare_at
                            .is_some_and(|at| state.game_time_seconds - at >= 0.5)
                    {
                        state.crouch_clearance_stage = CrouchClearanceStage::ArmCanopy;
                    }
                }
                CrouchClearanceStage::ArmCanopy => {
                    if let Some(params) = params_assets.get(params_handle.id()) {
                        let crouched_half_height = params.crouch.crouched_capsule_half_height;
                        let base_y = transform.translation.y - crouched_half_height;
                        let canopy_bottom = base_y + crouched_half_height * 2.0 + 0.08;
                        let canopy_center = Vec3::new(
                            transform.translation.x,
                            canopy_bottom + 0.08,
                            transform.translation.z,
                        );
                        let canopy = commands
                            .spawn((
                                Name::new("manual_integration_crouch_canopy"),
                                RigidBody::Static,
                                Collider::cuboid(1.2, 0.08, 1.2),
                                Transform::from_translation(canopy_center)
                                    .with_scale(Vec3::new(1.5, 0.1, 1.5)),
                                Mesh3d(fixture.obstacle_mesh.clone()),
                                MeshMaterial3d(fixture.obstacle_material.clone()),
                            ))
                            .id();
                        state.crouch_canopy = Some(canopy);
                        state.crouch_canopy_center = Some(canopy_center);
                        state.crouch_canopy_armed_at = Some(state.game_time_seconds);
                        state.coverage.crouch_canopy_visible = true;
                        state.crouch_clearance_stage = CrouchClearanceStage::AwaitCanopyReady;
                        info!(
                            "[MANUAL-INTEGRATION][CROUCH-CANOPY] center=({:.3},{:.3},{:.3}) bottom={:.3}",
                            canopy_center.x, canopy_center.y, canopy_center.z, canopy_bottom,
                        );
                    }
                }
                CrouchClearanceStage::AwaitCanopyReady => {
                    if state
                        .crouch_canopy_armed_at
                        .is_some_and(|at| state.game_time_seconds - at >= 0.5)
                    {
                        state.crouch_clearance_stage = CrouchClearanceStage::RequestStanding;
                    }
                }
                CrouchClearanceStage::RequestStanding => {
                    if *stance == Stance::Crouching {
                        request.stance = Some(Stance::Standing);
                        state.crouch_clearance_stage = CrouchClearanceStage::AwaitBlocked;
                    }
                }
                CrouchClearanceStage::AwaitBlocked => {
                    if state.coverage.crouch_canopy_blocked {
                        state.crouch_clearance_stage = CrouchClearanceStage::MoveAway;
                    }
                }
                CrouchClearanceStage::MoveAway => {
                    input.0.move_dir = Vec2::X;
                    if state.crouch_canopy_center.is_some_and(|canopy_center| {
                        let offset = transform.translation - canopy_center;
                        Vec2::new(offset.x, offset.z).length() > 1.65
                    }) {
                        state.crouch_clearance_stage = CrouchClearanceStage::AwaitRecovery;
                    }
                }
                CrouchClearanceStage::AwaitRecovery => {
                    if state.coverage.crouch_canopy_recovered {
                        cleanup_crouch_canopy(&mut commands, &mut state);
                        begin_phase(&mut state, IntegrationPhase::CrouchDirectionRegression);
                    }
                }
            }
            if elapsed >= CROUCH_CLEARANCE_TIMEOUT {
                cleanup_crouch_canopy(&mut commands, &mut state);
                state.timed_out = true;
                begin_phase(&mut state, IntegrationPhase::Finish);
            }
        }
        IntegrationPhase::CrouchDirectionRegression => {
            if entering {
                let crouch_start = state.spawn_translation.unwrap_or(fixture.locomotion_start);
                reset_character_with_rotation(
                    &mut transform,
                    &mut position,
                    rotation.as_deref_mut(),
                    &mut velocity,
                    &mut view_yaw,
                    &mut view_pitch,
                    crouch_start,
                    fixture.facing_yaw,
                );
                *gait = Gait::Running;
                *overlay = OverlayState::Default;
                *rotation_mode = RotationMode::Aiming;
                request.locomotion_state = Some(LocomotionState::Grounded);
                request.stance = Some(Stance::Crouching);
                request.gait = Some(Gait::Running);
                request.rotation_mode = Some(RotationMode::Aiming);
                state.crouch_direction_case_index = usize::MAX;
                state.coverage.crouch_direction_cases = [false; CROUCH_DIRECTION_CASE_COUNT];
                state.coverage.crouch_direction_diagnostics = [false; CROUCH_DIRECTION_CASE_COUNT];
                state.coverage.crouch_direction_state_mask = 0;
                state.coverage.crouch_direction_lean = false;
                state.coverage.crouch_rotate_mask = 0;
                state.coverage.crouch_rotate_source_completion_mask = 0;
                state.coverage.crouch_rotate_completion_mask = 0;
                state.coverage.crouch_turn_active_mask = 0;
                state.coverage.crouch_turn_completion_mask = 0;
                state.crouch_stationary_validation = None;
                state.crouch_stationary_precondition_ready = [false; 2];
                state.crouch_stationary_precondition_frames = [0; 2];
                state.crouch_stationary_base_yaw = [None; 2];
                state.crouch_turn_epoch_baseline = [None; 2];
                info!("[MANUAL-INTEGRATION][CROUCH-DIRECTION] start");
            }
            let direction_duration =
                CROUCH_DIRECTION_CASE_COUNT as f32 * CROUCH_DIRECTION_CASE_SECONDS;
            request.locomotion_state = Some(LocomotionState::Grounded);
            request.stance = Some(Stance::Crouching);
            request.gait = Some(Gait::Running);
            if crouch_direction_regression_requires_stationary_turn_phase(
                elapsed,
                direction_duration,
            ) {
                input.0.move_dir = Vec2::ZERO;
                velocity.0 = Vec3::ZERO;
            }
            input.0.aim_held = false;
            if elapsed < direction_duration {
                let case_index = (elapsed / CROUCH_DIRECTION_CASE_SECONDS) as usize;
                let Some(case) = crouch_direction_regression_case(case_index) else {
                    return;
                };
                *animation_window = ManualAnimationWindowEvidence {
                    label: Some(case.name),
                    index: Some(case_index),
                    settled: elapsed
                        >= case_index as f32 * CROUCH_DIRECTION_CASE_SECONDS
                            + CROUCH_DIRECTION_CASE_SETTLE_SECONDS,
                    capture_source_window: true,
                    requires_pre_rig_motion: true,
                    requires_final_pose_motion: true,
                    requires_non_reference_root_pose: true,
                    requires_source_pose_chain: true,
                    requires_actor_yaw_motion: false,
                    requires_actor_yaw_convergence: false,
                    expected_actor_yaw_delta_sign: None,
                    expected_actor_yaw_baseline: None,
                    expected_animation_variant: None,
                    source_node: Some(crouch_direction_source_node(case.expected_direction)),
                    source_state: None,
                    source_pose_chain: crouch_direction_source_pose_chain(case.expected_direction),
                    minimum_pre_rig_translation_span: 0.0,
                    minimum_pre_rig_rotation_span_radians: 0.05,
                    minimum_final_translation_span: 0.0,
                    minimum_final_rotation_span_radians: 0.05,
                    minimum_actor_yaw_delta_radians: 0.0,
                    minimum_actor_yaw_convergence_radians: 0.0,
                    minimum_observation_seconds: 0.8,
                    source_sync_group: None,
                    sync_phase_global: None,
                    canonical_sync_phase: None,
                    maximum_sync_phase_distance: 0.0,
                    canonical_clip_time: None,
                    maximum_clip_time_distance: 0.0,
                    direction_signature_group: None,
                    required_direction_signature_members: 0,
                    minimum_direction_signature_rotation_radians: 0.0,
                    expected_direction: Some(movement_direction_name(case.expected_direction)),
                    observed_direction: None,
                    expected_runtime_state: Some(movement_direction_fsm_index(
                        case.expected_direction,
                    )),
                    observed_runtime_state: None,
                    expected_parent_runtime_state: Some(1),
                    expected_terminal_parent_runtime_state: None,
                    expected_terminal_parent_transition_active: None,
                    expected_terminal_requested_parent_transition_active: None,
                    observed_parent_runtime_state: None,
                    observed_parent_transition_active: None,
                    observed_requested_parent_runtime_state: None,
                    observed_requested_parent_transition_active: None,
                    observed_actor_yaw: None,
                    observed_view_yaw: None,
                    observed_animation_yaw_speed_degrees: None,
                    observed_animation_curve_direct_yaw_speed_degrees: None,
                    observed_animation_clip_time: None,
                    observed_animation_weight: None,
                    observed_animation_play_rate: None,
                    observed_animation_curve_stack_weight: None,
                    observed_animation_curve_stack_entries: Vec::new(),
                    observed_animation_variant: None,
                    expected_lifecycle_stages: &[],
                    observed_lifecycle_stages: &[],
                };
                if state.crouch_direction_case_index != case_index {
                    state.crouch_direction_case_index = case_index;
                    let crouch_start = state.spawn_translation.unwrap_or(fixture.locomotion_start);
                    reset_character_with_rotation(
                        &mut transform,
                        &mut position,
                        rotation.as_deref_mut(),
                        &mut velocity,
                        &mut view_yaw,
                        &mut view_pitch,
                        crouch_start,
                        fixture.facing_yaw,
                    );
                    if let Some(avatar) = state.avatar {
                        commands
                            .entity(avatar)
                            .remove::<AlsMovementDirectionState>();
                    }
                    info!(
                        "[MANUAL-INTEGRATION][CROUCH-DIRECTION][START] case={} input=({:.3},{:.3}) expected={:?}",
                        case.name, case.move_dir.x, case.move_dir.y, case.expected_direction,
                    );
                }
                input.0.move_dir = case.move_dir;
                *rotation_mode = RotationMode::Aiming;
                request.rotation_mode = Some(RotationMode::Aiming);
                view_yaw.0 = fixture.facing_yaw;
            } else if let Some(validation) = next_crouch_stationary_validation(&state.coverage) {
                if state.crouch_stationary_validation != Some(validation) {
                    state.crouch_stationary_validation = Some(validation);
                    state.crouch_stationary_precondition_ready[validation.side_index()] = false;
                    state.crouch_stationary_precondition_frames[validation.side_index()] = 0;
                    state.crouch_stationary_base_yaw[validation.side_index()] = None;
                    state.crouch_turn_epoch_baseline[validation.side_index()] = None;
                    state.temporal_rebase_epoch = state.temporal_rebase_epoch.wrapping_add(1);
                    phase_evidence.temporal_rebase_epoch = state.temporal_rebase_epoch;
                    info!(
                        "[MANUAL-INTEGRATION][TEMPORAL-REBASE] epoch={} validation={validation:?}",
                        state.temporal_rebase_epoch
                    );
                    reinitialize_fixture_rotation_response(
                        rotation_response,
                        &mut aiming_yaw_constraint,
                        rotation_animation,
                    );
                    reset_character_with_rotation(
                        &mut transform,
                        &mut position,
                        rotation.as_deref_mut(),
                        &mut velocity,
                        &mut view_yaw,
                        &mut view_pitch,
                        state.spawn_translation.unwrap_or(fixture.locomotion_start),
                        fixture.facing_yaw,
                    );
                    info!(
                        "[MANUAL-INTEGRATION][CROUCH-STATIONARY][START] validation={validation:?}"
                    );
                }
                let target_yaw =
                    if !state.crouch_stationary_precondition_ready[validation.side_index()] {
                        fixture.facing_yaw
                    } else {
                        state.crouch_stationary_base_yaw[validation.side_index()]
                            .map(|base_yaw| {
                                source_stationary_target_yaw(base_yaw, validation.is_left())
                            })
                            .unwrap_or(fixture.facing_yaw)
                    };
                input.0.move_dir = Vec2::ZERO;
                velocity.0 = Vec3::ZERO;
                view_yaw.0 = target_yaw;
                if validation.is_turn() {
                    input.0.aim_held = crouch_stationary_validation_holds_aim(Some(validation));
                    *rotation_mode = RotationMode::ViewDirection;
                    request.rotation_mode = Some(RotationMode::ViewDirection);
                    *animation_window = crouch_turn_window(validation.is_left(), false, &[]);
                    animation_window.expected_actor_yaw_baseline =
                        state.crouch_stationary_base_yaw[validation.side_index()];
                } else {
                    input.0.aim_held = true;
                    *rotation_mode = RotationMode::Aiming;
                    request.rotation_mode = Some(RotationMode::Aiming);
                    *animation_window = crouch_rotate_window(
                        validation.is_left(),
                        state.crouch_stationary_precondition_ready[validation.side_index()],
                        &[],
                    );
                    animation_window.expected_actor_yaw_baseline =
                        state.crouch_stationary_base_yaw[validation.side_index()];
                }
            } else {
                *animation_window = ManualAnimationWindowEvidence::default();
                state.crouch_stationary_validation = None;
                request.stance = Some(Stance::Standing);
                if state.scenario == IntegrationScenario::Crouching {
                    begin_phase(&mut state, IntegrationPhase::Finish);
                } else {
                    begin_phase(&mut state, IntegrationPhase::Turn);
                }
            }
            if elapsed >= direction_duration + CROUCH_STATIONARY_TIMEOUT_SECONDS {
                state.timed_out = true;
                begin_phase(&mut state, IntegrationPhase::Finish);
            }
        }
        IntegrationPhase::Turn => {
            if entering {
                let turn_start = state.spawn_translation.unwrap_or(transform.translation);
                let turn_yaw = state.spawn_yaw.unwrap_or(fixture.facing_yaw);
                reset_character(
                    &mut transform,
                    &mut position,
                    rotation,
                    &mut velocity,
                    &mut view_yaw,
                    &mut view_pitch,
                    turn_start,
                    turn_yaw,
                );
                *gait = Gait::Walking;
                *overlay = OverlayState::Default;
                *rotation_mode = RotationMode::ViewDirection;
                request.locomotion_state = Some(LocomotionState::Grounded);
                request.stance = Some(Stance::Standing);
                rebase_temporal_fixture(&mut state, &mut phase_evidence, "turn_fixture");
            }
            let body_yaw = world_forward_to_yaw(transform.rotation * Vec3::NEG_Z);
            if state.scenario == IntegrationScenario::StateMachine {
                if let Some(target_yaw) = state_machine_turn_target_yaw(elapsed, body_yaw) {
                    view_yaw.0 = target_yaw;
                }
                if state_machine_turn_phase_complete(elapsed) {
                    begin_phase(&mut state, IntegrationPhase::StateMachineRotate);
                }
            } else {
                if elapsed >= 0.5 {
                    view_yaw.0 = body_yaw + 90.0f32.to_radians();
                }
                if elapsed >= 2.5 {
                    begin_phase(&mut state, IntegrationPhase::Mantle);
                }
            }
        }
        IntegrationPhase::StateMachineRotate => {
            if entering {
                reset_character(
                    &mut transform,
                    &mut position,
                    rotation,
                    &mut velocity,
                    &mut view_yaw,
                    &mut view_pitch,
                    fixture.locomotion_start,
                    fixture.facing_yaw,
                );
                *gait = Gait::Walking;
                *overlay = OverlayState::Default;
                *rotation_mode = RotationMode::Aiming;
                request.locomotion_state = Some(LocomotionState::Grounded);
                request.stance = Some(Stance::Standing);
                request.gait = Some(Gait::Walking);
                rebase_temporal_fixture(&mut state, &mut phase_evidence, "state_machine_rotate");
                info!("[MANUAL-INTEGRATION][P1][TURN] rotate_modes start");
            }
            let body_yaw = fixture.facing_yaw;
            transform.rotation = Quat::from_rotation_y(body_yaw);
            request.locomotion_state = Some(LocomotionState::Grounded);
            request.stance = Some(Stance::Standing);
            request.gait = Some(Gait::Walking);
            if elapsed < 1.3 {
                commands.entity(character).remove::<AlsFirstPersonView>();
                *rotation_mode = RotationMode::Aiming;
                request.rotation_mode = Some(RotationMode::Aiming);
                view_yaw.0 = body_yaw - 90.0f32.to_radians();
            } else if elapsed < 2.6 {
                commands.entity(character).remove::<AlsFirstPersonView>();
                *rotation_mode = RotationMode::Aiming;
                request.rotation_mode = Some(RotationMode::Aiming);
                view_yaw.0 = body_yaw + 90.0f32.to_radians();
            } else if elapsed < 3.9 {
                commands.entity(character).insert(AlsFirstPersonView);
                *rotation_mode = RotationMode::ViewDirection;
                request.rotation_mode = Some(RotationMode::ViewDirection);
                view_yaw.0 = body_yaw - 90.0f32.to_radians();
            } else if elapsed < 5.2 {
                commands.entity(character).insert(AlsFirstPersonView);
                *rotation_mode = RotationMode::ViewDirection;
                request.rotation_mode = Some(RotationMode::ViewDirection);
                view_yaw.0 = body_yaw + 90.0f32.to_radians();
            } else {
                commands.entity(character).remove::<AlsFirstPersonView>();
                begin_phase(&mut state, IntegrationPhase::Finish);
            }
        }
        IntegrationPhase::Mantle => {
            if entering {
                reset_character(
                    &mut transform,
                    &mut position,
                    rotation,
                    &mut velocity,
                    &mut view_yaw,
                    &mut view_pitch,
                    fixture.mantle_start,
                    fixture.facing_yaw,
                );
                *gait = Gait::Running;
                *overlay = OverlayState::Default;
                *rotation_mode = RotationMode::ViewDirection;
                request.locomotion_state = Some(LocomotionState::Grounded);
                request.stance = Some(Stance::Standing);
            }
            let forward = Quat::from_rotation_y(fixture.facing_yaw) * Vec3::NEG_Z;
            let progress = (transform.translation - fixture.mantle_start).dot(forward);
            input.0.move_dir = Vec2::Y;
            input.0.jump_pressed = (5.85..7.1).contains(&progress) && elapsed < 7.0;
            if elapsed >= 10.0 {
                if state.scenario == IntegrationScenario::FootIk {
                    begin_phase(&mut state, IntegrationPhase::Roll);
                } else {
                    begin_phase(&mut state, IntegrationPhase::Platform);
                }
            }
        }
        IntegrationPhase::Platform => {
            if entering {
                if let Ok((mut platform_transform, platform_position, mut platform_velocity)) =
                    platforms.get_mut(fixture.platform)
                {
                    let platform_location = fixture.platform_home;
                    platform_transform.translation = platform_location;
                    if let Some(mut platform_position) = platform_position {
                        platform_position.0 = platform_location;
                    }
                    platform_velocity.0 = Vec3::X * 1.5;
                }
                reset_character(
                    &mut transform,
                    &mut position,
                    rotation,
                    &mut velocity,
                    &mut view_yaw,
                    &mut view_pitch,
                    fixture.platform_home + Vec3::Y * 1.17,
                    fixture.facing_yaw,
                );
                *gait = Gait::Running;
                *overlay = OverlayState::Default;
                request.locomotion_state = Some(LocomotionState::Grounded);
            }
            if elapsed >= 6.0 {
                begin_phase(&mut state, IntegrationPhase::Roll);
            }
        }
        IntegrationPhase::Roll => {
            if entering {
                reset_character(
                    &mut transform,
                    &mut position,
                    rotation,
                    &mut velocity,
                    &mut view_yaw,
                    &mut view_pitch,
                    fixture.roll_start,
                    fixture.facing_yaw,
                );
                *gait = Gait::Running;
                *overlay = OverlayState::Default;
                request.locomotion_state = Some(LocomotionState::Grounded);
                request.stance = Some(Stance::Standing);
            }
            input.0.move_dir = Vec2::Y;
            input.0.roll_pressed = (1.0..1.2).contains(&elapsed);
            if state.scenario == IntegrationScenario::Grounded {
                *animation_window = ManualAnimationWindowEvidence {
                    label: Some("grounded_roll_exit_standing"),
                    settled: elapsed >= 3.0,
                    capture_source_window: true,
                    requires_pre_rig_motion: true,
                    requires_final_pose_motion: true,
                    requires_non_reference_root_pose: true,
                    source_node: Some("AB_Als_Grounded/Grounded States"),
                    source_state: Some("Roll"),
                    expected_runtime_state: Some(1),
                    ..default()
                };
            }
            if elapsed >= 5.5 {
                begin_phase(&mut state, IntegrationPhase::Finish);
            }
        }
        IntegrationPhase::Ragdoll => {
            if entering {
                state.ragdoll_seen = false;
                state.ragdoll_exit_requested = false;
                state.ragdoll_frames = 0;
                state.get_up_fixture_seeded = false;
                state.ragdoll_grounded_at = None;
                state.ragdoll_recovered_at = None;
                state.coverage.ragdoll_entered = false;
                state.coverage.ragdoll_pa_profile = false;
                state.coverage.ragdoll_follow = false;
                state.coverage.ragdoll_joint_drive = false;
                state.coverage.ragdoll_recovered = false;
                let spawn_translation = state.spawn_translation.unwrap_or(transform.translation);
                let spawn_yaw = state.spawn_yaw.unwrap_or(fixture.facing_yaw);
                reset_character(
                    &mut transform,
                    &mut position,
                    rotation,
                    &mut velocity,
                    &mut view_yaw,
                    &mut view_pitch,
                    spawn_translation,
                    spawn_yaw,
                );
                *gait = Gait::Running;
                *overlay = OverlayState::Default;
                request.locomotion_state = Some(LocomotionState::Grounded);
                request.stance = Some(Stance::Standing);
                if state.scenario == IntegrationScenario::GetUp {
                    rebase_temporal_fixture(
                        &mut state,
                        &mut phase_evidence,
                        "get_up_initial_position",
                    );
                }
            }
            let ready_to_exit = elapsed
                >= if state.scenario == IntegrationScenario::GetUp {
                    0.9
                } else {
                    2.0
                }
                && state.coverage.ragdoll_follow
                && state.coverage.ragdoll_joint_drive
                && ground_follow.is_some_and(|follow| follow.grounded);
            if state.ragdoll_exit_requested {
            } else if state.scenario == IntegrationScenario::GetUp && elapsed < 0.3 {
                request.locomotion_state = Some(LocomotionState::Grounded);
            } else if elapsed < 1.0
                && !matches!(
                    state.scenario,
                    IntegrationScenario::Ragdoll | IntegrationScenario::GetUp
                )
            {
                input.0.move_dir = Vec2::Y;
            } else if !ready_to_exit {
                request.locomotion_state = Some(LocomotionState::Ragdolling);
            } else {
                request.ragdoll_exit = true;
                state.ragdoll_exit_requested = true;
            }
            if state
                .ragdoll_recovered_at
                .is_some_and(|at| state.game_time_seconds - at >= 1.5)
                && elapsed >= 3.0
            {
                if state.scenario == IntegrationScenario::GetUp && state.get_up_case == 0 {
                    state.get_up_case = 1;
                    begin_phase(&mut state, IntegrationPhase::Ragdoll);
                } else {
                    begin_phase(&mut state, IntegrationPhase::Finish);
                }
            } else if elapsed >= 20.0 {
                state.timed_out = true;
                begin_phase(&mut state, IntegrationPhase::Finish);
            }
        }
        IntegrationPhase::Camera => {
            if entering {
                state.coverage.camera_collision = false;
                state.coverage.camera_recovery = false;
                state.camera_obstacles = [None, None];
                state.camera_obstacle_removed = false;
                state.camera_blocked_ratio = None;
                state.camera_scenario_armed = false;
                state.camera_last_probe_secs = -1.0;
                state.camera_view_mode_requested = false;
                state.camera_shoulder_requested = false;
                let camera_spawn = state.spawn_translation.unwrap_or(transform.translation);
                let camera_yaw = state.spawn_yaw.unwrap_or(fixture.facing_yaw);
                reset_character(
                    &mut transform,
                    &mut position,
                    rotation,
                    &mut velocity,
                    &mut view_yaw,
                    &mut view_pitch,
                    camera_spawn,
                    camera_yaw,
                );
                *gait = Gait::Running;
                *overlay = OverlayState::Default;
                *rotation_mode = RotationMode::ViewDirection;
            }
            if elapsed < 1.0 {
                input.0.look_delta = Vec2::new(8.0, -4.0);
            }
            if !state.camera_shoulder_requested {
                input.0.switch_shoulder_pressed = true;
                state.camera_shoulder_requested = true;
            }
            if elapsed >= 2.0 && !state.camera_view_mode_requested {
                input.0.view_mode_pressed = true;
                state.camera_view_mode_requested = true;
            }
            if (3.0..4.0).contains(&elapsed) {
                input.0.look_delta = Vec2::new(2.0, 5.0);
                if !state.dilation_active {
                    virtual_time.set_relative_speed(0.2);
                    state.dilation_active = true;
                }
            } else if state.dilation_active {
                virtual_time.set_relative_speed(1.0);
                state.dilation_active = false;
            }
            if elapsed >= 4.0 && !state.shake_requested {
                shake_requests.write(AlsCameraShakeRequest::sprint(character, 1.0));
                state.shake_requested = true;
            }
            if elapsed >= 0.5
                && state.camera_obstacles.iter().all(Option::is_none)
                && !state.camera_obstacle_removed
            {
                if let Some(camera) = state.camera
                    && let Ok(debug) = cameras.get(camera)
                    && debug.valid
                    && debug.trace_start.distance(debug.trace_end) > 0.5
                {
                    let owner_direction =
                        (transform.translation - debug.trace_start).normalize_or_zero();
                    let owner_direction = if owner_direction.length_squared() > 1e-6 {
                        owner_direction
                    } else {
                        Vec3::Y
                    };
                    let trace_direction = (debug.trace_end - debug.trace_start).normalize_or_zero();
                    let lateral = trace_direction.cross(owner_direction).normalize_or_zero();
                    let lateral = if lateral.length_squared() > 1e-6 {
                        lateral
                    } else {
                        Vec3::X
                    };
                    let directions = [
                        owner_direction,
                        (owner_direction + lateral * 0.35).normalize_or_zero(),
                    ];
                    for (index, direction) in directions.into_iter().enumerate() {
                        let direction = if direction.length_squared() > 1e-6 {
                            direction
                        } else {
                            owner_direction
                        };
                        let center = debug.trace_start - direction * 0.1;
                        state.camera_obstacles[index] = Some(
                            commands
                                .spawn((
                                    Name::new(format!(
                                        "manual_integration_camera_obstacle_{index}"
                                    )),
                                    RigidBody::Static,
                                    CollisionLayers::from_bits(0b0010, 0),
                                    Collider::cuboid(0.05, 0.6, 0.6),
                                    Transform::from_translation(center)
                                        .with_rotation(Quat::from_rotation_arc(Vec3::X, direction)),
                                    Mesh3d(fixture.obstacle_mesh.clone()),
                                    MeshMaterial3d(fixture.obstacle_material.clone()),
                                ))
                                .id(),
                        );
                    }
                    state.camera_scenario_armed = true;
                    info!("[MANUAL-INTEGRATION] camera MTD pair armed");
                }
            }
            if state.coverage.camera_collision && !state.camera_obstacle_removed {
                for obstacle in state.camera_obstacles.iter_mut().flatten() {
                    commands.entity(*obstacle).despawn();
                }
                state.camera_obstacles = [None, None];
                state.camera_obstacle_removed = true;
                info!("[MANUAL-INTEGRATION] camera MTD pair removed after collision probe");
            }
            if elapsed >= 9.0 {
                virtual_time.set_relative_speed(1.0);
                state.dilation_active = false;
                begin_phase(&mut state, IntegrationPhase::Finish);
            }
        }
        IntegrationPhase::Interface => {
            if entering {
                state.interface_debug_armed = false;
                state.interface_hud_hidden_seen = false;
                state.interface_hud_restored_seen = false;
                state.interface_slomo_seen = false;
                state.interface_slomo_restored_seen = false;
                state.interface_menu_open_seen = false;
                state.interface_menu_closed_seen = false;
                state.interface_overlay_rifle_seen = false;
                state.interface_overlay_default_after_rifle_seen = false;
                state.interface_overlay_reentry_seen = false;
                state.coverage.hud_layout = false;
                state.coverage.hud_toggle = false;
                state.coverage.hud_slomo = false;
                state.coverage.overlay_menu = false;
                state.coverage.overlay_interface_binding = false;
                state.coverage.overlay_interface_fallback = false;
                state.coverage.overlay_interface_reentry = false;
                state.coverage.show_debug = false;
                state.coverage.debug_draw = false;
                for category in DebugCategory::ALL {
                    apply_show_debug_key(
                        &mut show_debug,
                        true,
                        match category {
                            DebugCategory::Curves => KeyCode::Digit1,
                            DebugCategory::State => KeyCode::Digit2,
                            DebugCategory::Shapes => KeyCode::Digit3,
                            DebugCategory::Traces => KeyCode::Digit4,
                            DebugCategory::Mantling => KeyCode::Digit5,
                            DebugCategory::CameraCurves => KeyCode::Digit6,
                            DebugCategory::CameraShapes => KeyCode::Digit7,
                            DebugCategory::CameraTraces => KeyCode::Digit8,
                        },
                    );
                }
                state.interface_debug_armed = true;
                *overlay = OverlayState::Default;
                request.overlay_state = Some(OverlayState::Default);
                info!(
                    "[MANUAL-INTEGRATION][INTERFACE] HUD, overlay menu, and show-debug probes armed"
                );
            }
            drive_key_interval(&mut keys, KeyCode::Tab, elapsed, 0.25, 0.55);
            drive_key_interval(&mut keys, KeyCode::Tab, elapsed, 0.95, 1.25);
            drive_key_interval(&mut keys, KeyCode::KeyZ, elapsed, 1.65, 1.95);
            drive_key_interval(&mut keys, KeyCode::KeyZ, elapsed, 2.25, 2.55);
            drive_key_interval(&mut keys, KeyCode::KeyQ, elapsed, 2.9, 3.9);
            request.overlay_state = Some(
                if (0.6..1.4).contains(&elapsed) || (2.6..3.4).contains(&elapsed) {
                    OverlayState::Rifle
                } else {
                    OverlayState::Default
                },
            );
            if elapsed >= 5.0 {
                keys.release(KeyCode::Tab);
                keys.release(KeyCode::KeyZ);
                keys.release(KeyCode::KeyQ);
                virtual_time.set_relative_speed(1.0);
                begin_phase(&mut state, IntegrationPhase::Finish);
            }
        }
        IntegrationPhase::Notify => {
            if entering {
                state.notify_roll_armed = false;
                state.notify_mantle_armed = false;
                state.coverage.notify_sprint = false;
                state.coverage.notify_sprint_acceleration = false;
                state.coverage.notify_roll = false;
                state.coverage.notify_marker = false;
                state.coverage.notify_grounded_entry = false;
                state.coverage.notify_root_scale = false;
                state.coverage.notify_rolling_action = false;
                state.coverage.notify_mantling_action = false;
                state.coverage.notify_early_blend_out = false;
                reset_character(
                    &mut transform,
                    &mut position,
                    rotation,
                    &mut velocity,
                    &mut view_yaw,
                    &mut view_pitch,
                    fixture.locomotion_start,
                    fixture.facing_yaw,
                );
                *gait = Gait::Running;
                *overlay = OverlayState::Default;
                *rotation_mode = RotationMode::ViewDirection;
                request.locomotion_state = Some(LocomotionState::Grounded);
                request.stance = Some(Stance::Standing);
            }
            if elapsed < 3.0 {
                input.0.move_dir = Vec2::Y;
                input.0.sprint_held = true;
            } else if elapsed < 6.0 {
                if !state.notify_roll_armed {
                    reset_character_without_rotation(
                        &mut transform,
                        &mut position,
                        &mut velocity,
                        &mut view_yaw,
                        &mut view_pitch,
                        fixture.roll_start,
                        fixture.facing_yaw,
                    );
                    *gait = Gait::Running;
                    request.locomotion_state = Some(LocomotionState::Grounded);
                    request.stance = Some(Stance::Standing);
                    state.notify_roll_armed = true;
                }
                input.0.move_dir = Vec2::Y;
                input.0.roll_pressed = (3.5..3.8).contains(&elapsed);
            } else {
                if !state.notify_mantle_armed {
                    reset_character_without_rotation(
                        &mut transform,
                        &mut position,
                        &mut velocity,
                        &mut view_yaw,
                        &mut view_pitch,
                        fixture.mantle_start,
                        fixture.facing_yaw,
                    );
                    *gait = Gait::Running;
                    request.locomotion_state = Some(LocomotionState::Grounded);
                    request.stance = Some(Stance::Standing);
                    state.notify_mantle_armed = true;
                }
                let forward = Quat::from_rotation_y(fixture.facing_yaw) * Vec3::NEG_Z;
                let progress = (transform.translation - fixture.mantle_start).dot(forward);
                input.0.move_dir = Vec2::Y;
                input.0.jump_pressed = (5.85..7.1).contains(&progress) && elapsed < 13.0;
            }
            if elapsed >= 17.0 {
                begin_phase(&mut state, IntegrationPhase::Finish);
            }
        }
        IntegrationPhase::Finish => {}
    }
}

fn reset_character(
    transform: &mut Transform,
    position: &mut Position,
    mut rotation: Option<Mut<Rotation>>,
    velocity: &mut LinearVelocity,
    view_yaw: &mut AlsViewYaw,
    view_pitch: &mut AlsViewPitch,
    translation: Vec3,
    yaw: f32,
) {
    reset_character_with_rotation(
        transform,
        position,
        rotation.as_deref_mut(),
        velocity,
        view_yaw,
        view_pitch,
        translation,
        yaw,
    );
}

fn rebase_temporal_fixture(
    state: &mut ManualIntegrationState,
    phase_evidence: &mut ManualRuntimePhaseEvidence,
    label: &str,
) {
    state.temporal_rebase_epoch = state.temporal_rebase_epoch.wrapping_add(1);
    phase_evidence.temporal_rebase_epoch = state.temporal_rebase_epoch;
    info!(
        "[MANUAL-INTEGRATION][TEMPORAL-REBASE] epoch={} fixture={label}",
        state.temporal_rebase_epoch
    );
}

fn reset_fixture_rotation_response_state(
    rotation_response: &mut AlsRotationResponseState,
    aiming_yaw_constraint: &mut AlsAimingYawConstraint,
    rotation_animation: Option<&mut AlsRotationAnimationState>,
) {
    *rotation_response = AlsRotationResponseState::default();
    *aiming_yaw_constraint = AlsAimingYawConstraint::default();
    if let Some(rotation_animation) = rotation_animation {
        *rotation_animation = AlsRotationAnimationState::default();
    }
}

fn reinitialize_fixture_rotation_response(
    mut rotation_response: Option<Mut<AlsRotationResponseState>>,
    aiming_yaw_constraint: &mut AlsAimingYawConstraint,
    mut rotation_animation: Option<Mut<AlsRotationAnimationState>>,
) {
    if let Some(rotation_response) = rotation_response.as_deref_mut() {
        reset_fixture_rotation_response_state(
            rotation_response,
            aiming_yaw_constraint,
            rotation_animation.as_deref_mut(),
        );
    } else {
        *aiming_yaw_constraint = AlsAimingYawConstraint::default();
        if let Some(rotation_animation) = rotation_animation.as_deref_mut() {
            *rotation_animation = AlsRotationAnimationState::default();
        }
    }
}

fn reset_character_with_rotation(
    transform: &mut Transform,
    position: &mut Position,
    rotation: Option<&mut Rotation>,
    velocity: &mut LinearVelocity,
    view_yaw: &mut AlsViewYaw,
    view_pitch: &mut AlsViewPitch,
    translation: Vec3,
    yaw: f32,
) {
    let rotation_value = Quat::from_rotation_y(yaw);
    transform.translation = translation;
    transform.rotation = rotation_value;
    position.0 = translation;
    if let Some(rotation) = rotation {
        rotation.0 = rotation_value;
    }
    velocity.0 = Vec3::ZERO;
    view_yaw.0 = yaw;
    view_pitch.0 = 0.0;
}

fn reset_character_without_rotation(
    transform: &mut Transform,
    position: &mut Position,
    velocity: &mut LinearVelocity,
    view_yaw: &mut AlsViewYaw,
    view_pitch: &mut AlsViewPitch,
    translation: Vec3,
    yaw: f32,
) {
    transform.translation = translation;
    transform.rotation = Quat::from_rotation_y(yaw);
    position.0 = translation;
    velocity.0 = Vec3::ZERO;
    view_yaw.0 = yaw;
    view_pitch.0 = 0.0;
}

fn stance_center_from_standing_center(
    params: &AlsMovementParams,
    standing_center: Vec3,
    stance: Stance,
) -> Vec3 {
    let target_half_height = match stance {
        Stance::Standing => params.crouch.standing_capsule_half_height,
        Stance::Crouching => params.crouch.crouched_capsule_half_height,
    };
    standing_center + Vec3::Y * (target_half_height - params.crouch.standing_capsule_half_height)
}

fn drive_key_interval(
    keys: &mut ButtonInput<KeyCode>,
    key: KeyCode,
    elapsed: f32,
    start: f32,
    end: f32,
) {
    if (start..end).contains(&elapsed) {
        keys.press(key);
    } else if elapsed >= end {
        keys.release(key);
    }
}

fn begin_phase(state: &mut ManualIntegrationState, phase: IntegrationPhase) {
    state.phase = phase;
    state.phase_started_seconds = state.game_time_seconds;
    state.entered = true;
    info!("[MANUAL-INTEGRATION][PHASE] {phase:?}");
}

fn state_machine_turn_target_yaw(elapsed: f32, body_yaw: f32) -> Option<f32> {
    if (STATE_MACHINE_TURN_FIRST_START..STATE_MACHINE_TURN_FIRST_END).contains(&elapsed) {
        Some(body_yaw + 90.0f32.to_radians())
    } else if (STATE_MACHINE_TURN_SECOND_START..STATE_MACHINE_TURN_SECOND_END).contains(&elapsed) {
        Some(body_yaw - 90.0f32.to_radians())
    } else {
        Some(body_yaw)
    }
}

fn state_machine_turn_phase_complete(elapsed: f32) -> bool {
    elapsed >= STATE_MACHINE_TURN_END
}

fn cleanup_crouch_canopy(commands: &mut Commands, state: &mut ManualIntegrationState) {
    if let Some(entity) = state.crouch_canopy.take() {
        commands.entity(entity).despawn();
        state.coverage.crouch_canopy_cleaned = true;
        info!("[MANUAL-INTEGRATION][CROUCH-CANOPY] cleaned entity={entity:?}");
    }
    state.crouch_canopy_center = None;
    state.crouch_canopy_armed_at = None;
}

fn cleanup_foot_ik_terrain(commands: &mut Commands, state: &mut ManualIntegrationState) {
    let mut cleaned = false;
    if let Some(entity) = state.foot_ik_slope.take() {
        commands.entity(entity).despawn();
        cleaned = true;
    }
    if let Some(entity) = state.foot_ik_step.take() {
        commands.entity(entity).despawn();
        cleaned = true;
    }
    if cleaned {
        state.coverage.foot_ik_terrain_cleaned = true;
        info!("[MANUAL-INTEGRATION][FOOT-IK-TERRAIN] cleaned");
    }
}

fn log_foot_ik_stage(stage: &str, spaces: &AlsFootIkSpaceState, apply: &AlsFootIkApplyTelemetry) {
    info!(
        "[MANUAL-INTEGRATION][FOOT-IK-STAGE] stage={stage} left_target=({:.3},{:.3},{:.3}) left_armature=({:.3},{:.3},{:.3}) left_parent=({:.3},{:.3},{:.3}) left_bone=({:.3},{:.3},{:.3}) left_hip=({:.3},{:.3},{:.3}) left_knee=({:.3},{:.3},{:.3}) left_ankle=({:.3},{:.3},{:.3}) left_errors=({:.6},{:.6},{:.6}) left_pole={:.6} left_weight={:.3} right_target=({:.3},{:.3},{:.3}) right_armature=({:.3},{:.3},{:.3}) right_parent=({:.3},{:.3},{:.3}) right_bone=({:.3},{:.3},{:.3}) right_hip=({:.3},{:.3},{:.3}) right_knee=({:.3},{:.3},{:.3}) right_ankle=({:.3},{:.3},{:.3}) right_errors=({:.6},{:.6},{:.6}) right_pole={:.6} right_weight={:.3}",
        spaces.left.resolved_target_world.x,
        spaces.left.resolved_target_world.y,
        spaces.left.resolved_target_world.z,
        spaces.left.target_armature_local.x,
        spaces.left.target_armature_local.y,
        spaces.left.target_armature_local.z,
        spaces.left.target_parent_local.x,
        spaces.left.target_parent_local.y,
        spaces.left.target_parent_local.z,
        spaces.left.target_bone_local.x,
        spaces.left.target_bone_local.y,
        spaces.left.target_bone_local.z,
        apply.left.hip_world.x,
        apply.left.hip_world.y,
        apply.left.hip_world.z,
        apply.left.knee_world.x,
        apply.left.knee_world.y,
        apply.left.knee_world.z,
        apply.left.ankle_world.x,
        apply.left.ankle_world.y,
        apply.left.ankle_world.z,
        apply.left.hip_error,
        apply.left.knee_error,
        apply.left.effector_error,
        apply.left.pole_sign,
        apply.left.applied_weight,
        spaces.right.resolved_target_world.x,
        spaces.right.resolved_target_world.y,
        spaces.right.resolved_target_world.z,
        spaces.right.target_armature_local.x,
        spaces.right.target_armature_local.y,
        spaces.right.target_armature_local.z,
        spaces.right.target_parent_local.x,
        spaces.right.target_parent_local.y,
        spaces.right.target_parent_local.z,
        spaces.right.target_bone_local.x,
        spaces.right.target_bone_local.y,
        spaces.right.target_bone_local.z,
        apply.right.hip_world.x,
        apply.right.hip_world.y,
        apply.right.hip_world.z,
        apply.right.knee_world.x,
        apply.right.knee_world.y,
        apply.right.knee_world.z,
        apply.right.ankle_world.x,
        apply.right.ankle_world.y,
        apply.right.ankle_world.z,
        apply.right.hip_error,
        apply.right.knee_error,
        apply.right.effector_error,
        apply.right.pole_sign,
        apply.right.applied_weight,
    );
}

fn graph_f32(player: &AnimationGraphPlayer, key: &str) -> Option<f32> {
    match player.get_input_data(key.to_owned()) {
        Some(DataValue::F32(value)) if value.is_finite() => Some(value),
        _ => None,
    }
}

fn graph_global_f32(player: &AnimationGraphPlayer, key: &str) -> Option<f32> {
    match player.get_global_input_data(key) {
        Some(DataValue::F32(value)) if value.is_finite() => Some(*value),
        _ => None,
    }
}

fn default_overlay_curve_value(name: &str) -> f32 {
    match name {
        "HandLeftIk" | "HandRightIk" | "HipsDirectionLock" | "LayerHandLeft" | "LayerHandRight"
        | "PoseAiming" => 0.0,
        _ => 1.0,
    }
}

fn default_overlay_curves_ready(context: Option<&AlsRootAnimationContext>) -> bool {
    ALS_LAYER_CURVE_NAMES.iter().all(|name| {
        context
            .and_then(|context| context.f32(name))
            .is_some_and(|value| (value - default_overlay_curve_value(name)).abs() < 1e-4)
    })
}

fn grounded_roll_exit_transition_matches_source(
    phase: IntegrationPhase,
    roll: Option<&AlsRollState>,
    playback: Option<&AlsTransitionPlayback>,
    slot: Option<&AlsRootTransitionSlotRuntime>,
    grounded: Option<&AlsGroundedGraphTelemetry>,
) -> bool {
    phase == IntegrationPhase::Roll
        && roll.is_some_and(|roll| !roll.active)
        && playback.is_some_and(|playback| {
            playback.active
                && playback.kind == AlsTransitionKind::StandingTransitionRight
                && (playback.blend_in - STOP_TRANSITION_BLEND_IN).abs() <= 1e-6
                && (playback.blend_out - STOP_TRANSITION_BLEND_OUT).abs() <= 1e-6
                && (playback.play_rate - STOP_TRANSITION_PLAY_RATE).abs() <= 1e-6
                && (playback.start_time - RIGHT_TRANSITION_START_TIME).abs() <= 1e-6
        })
        && slot.is_some_and(|slot| slot.active && slot.pose_ready && slot.weight > 1e-4)
        && grounded.is_some_and(|grounded| {
            grounded.actual_state == Some(1)
                && grounded.transition_active == Some(false)
                && grounded.pose_ready == Some(true)
        })
}

fn grounded_roll_exit_diagnostic(
    stance: Stance,
    desired_stance: AlsDesiredStance,
    roll: Option<&AlsRollState>,
    grounded: Option<&AlsGroundedGraphTelemetry>,
    playback: Option<&AlsTransitionPlayback>,
    slot: Option<&AlsRootTransitionSlotRuntime>,
) -> String {
    let roll = roll.map_or_else(
        || "missing".to_owned(),
        |roll| {
            format!(
                "active={} time={:.6} stance_before={:?}",
                roll.active, roll.time, roll.stance_before_roll
            )
        },
    );
    let grounded = grounded.map_or_else(
        || "missing".to_owned(),
        |grounded| {
            format!(
                "state={:?} transition={:?} pose_ready={:?}",
                grounded.actual_state, grounded.transition_active, grounded.pose_ready
            )
        },
    );
    let playback = playback.map_or_else(
        || "missing".to_owned(),
        |playback| {
            format!(
                "active={} kind={:?} weight={:.6} blend_in={:.6} blend_out={:.6} play_rate={:.6} start_time={:.6}",
                playback.active,
                playback.kind,
                playback.weight,
                playback.blend_in,
                playback.blend_out,
                playback.play_rate,
                playback.start_time,
            )
        },
    );
    let slot = slot.map_or_else(
        || "missing".to_owned(),
        |slot| {
            format!(
                "active={} pose_ready={} weight={:.6}",
                slot.active, slot.pose_ready, slot.weight
            )
        },
    );
    format!(
        "stance={stance:?}; desired_stance={:?}; roll={roll}; grounded={grounded}; transition={playback}; slot={slot}",
        desired_stance.0
    )
}

fn manual_gate_exit_ready(
    visual_progress: Option<&AnimationVisualValidationProgress>,
    timed_out: bool,
) -> bool {
    timed_out || visual_progress.map_or(true, |progress| progress.evidence_ready)
}

#[allow(clippy::type_complexity)]
fn observe_manual_integration(
    fixture: Option<Res<ManualIntegrationFixture>>,
    mut observation: ManualObservationResources,
    real_time: Res<Time<Real>>,
    mut animation_events: MessageReader<AlsAnimationEvent>,
    interface: InterfaceProbe,
    core: Query<
        (
            &Transform,
            &LinearVelocity,
            &Stance,
            &Gait,
            &LocomotionState,
            &OverlayState,
            &AlsViewYaw,
            &AlsViewPitch,
            Has<AlsFirstPersonView>,
            Has<AlsCameraLeftShoulder>,
            Option<&StartStopState>,
            Option<&AlsControlRigState>,
            Option<&AlsHeadState>,
            Option<&AlsLayeringState>,
            Option<&AlsOverlayObjectState>,
        ),
        With<AlsCharacter>,
    >,
    mut state_requests: Query<&mut AlsStateRequest, With<AlsCharacter>>,
    control_rig_runtime: Query<
        (
            &AlsControlRigConstruction,
            &AlsControlRigInput,
            &AlsControlRigStageTrace,
        ),
        With<AlsCharacter>,
    >,
    motion_probe: MotionProbe,
    foot_ik_data: Query<
        (
            &AlsFootIkTelemetry,
            &AlsFootIkSpaceState,
            &AlsFootIkApplyTelemetry,
            Option<&AlsFootLockTeleportTelemetry>,
        ),
        With<AlsCharacter>,
    >,
    action: Query<
        (
            Option<&AlsMantleState>,
            Option<&AlsRollState>,
            Option<&AlsRollSlotRuntime>,
            Option<&AlsFootProbe>,
            Option<&AlsFootLockState>,
            Option<&AlsMovementBaseDynamicsState>,
            Option<&AlsRagdollGroundFollow>,
            Option<&AlsRagdollJointDrive>,
            Option<&AlsNotifyLocomotionAction>,
            Option<&AlsNotifyRootMotionScale>,
            Option<&AlsNotifyEarlyBlendOut>,
            Option<&AlsGroundedEntryMode>,
            Option<&AlsTransitionPlayback>,
            Option<&AlsRootTransitionSlotRuntime>,
            Option<&AlsDesiredStance>,
        ),
        With<AlsCharacter>,
    >,
    avatars: Query<(
        &AlsCharacterAvatar,
        &AnimatedSceneInstance,
        Option<&AlsGaitBlend>,
        Option<&AlsStanceBlend>,
        Option<&AlsJumpVariantState>,
        Option<&AlsTurnDrive>,
        Option<&AlsTurnMontageStack>,
        Option<&AlsStandingTurnSlotRuntime>,
        Option<&AlsMovementDirectionState>,
    )>,
    players: Query<(&AnimationGraphPlayer, Option<&AlsRootAnimationContext>)>,
    ragdoll_probe: RagdollPaProbe,
    cameras: Query<(&Transform, &AlsCameraDebugInfo), With<AlsCameraRig>>,
    mut exit: MessageWriter<AppExit>,
) {
    let (state, mut animation_window, gate_report) = (
        &mut observation.state,
        &mut observation.animation_window,
        &mut observation.gate_report,
    );
    if !state.initialized {
        return;
    }
    state.game_time_seconds = interface.virtual_time.elapsed_secs_f64();
    let Some(fixture) = fixture else {
        return;
    };
    let Some(character) = state.character else {
        return;
    };
    let Ok((
        transform,
        velocity,
        stance,
        gait,
        locomotion,
        overlay,
        view_yaw,
        view_pitch,
        first_person,
        left_shoulder,
        start_stop,
        control_rig,
        head,
        layering,
        overlay_object,
    )) = core.get(character)
    else {
        return;
    };
    let rotation_animation = motion_probe
        .rotation_animation
        .get(character)
        .ok()
        .flatten();
    let standing_graph = motion_probe.standing.get(character).ok();
    let crouching_graph = motion_probe.crouching_graph.get(character).ok();
    let grounded_graph = motion_probe.grounded.get(character).ok();
    let crouch_transaction = motion_probe.crouch.get(character).ok();
    if matches!(
        state.scenario,
        IntegrationScenario::Base | IntegrationScenario::StateMachine
    ) && state.phase == IntegrationPhase::CrouchRecovery
        && state.phase_elapsed() >= 1.0
        && state.crouch_recovery_jump_requested
        && !state.crouch_recovery_air_crouch_requested
        && *locomotion == LocomotionState::Air
        && let Ok(mut request) = state_requests.get_mut(character)
    {
        request.stance = Some(Stance::Crouching);
        state.crouch_recovery_air_crouch_requested = true;
        info!(
            "[MANUAL-INTEGRATION][CROUCH-AIR-REQUEST] requested=Crouching actual_locomotion={:?}",
            locomotion,
        );
    }
    let Ok((
        mantle,
        roll,
        roll_slot,
        foot_probe,
        foot_lock,
        movement_base,
        ragdoll_follow,
        joint_drive,
        notify_action,
        notify_root_scale,
        notify_early_blend_out,
        grounded_entry,
        transition_playback,
        root_transition_slot,
        desired_stance,
    )) = action.get(character)
    else {
        return;
    };
    let Some((
        _,
        instance,
        gait_blend,
        stance_blend,
        jump_variant,
        turn_drive,
        turn_stack,
        turn_slot,
        movement_direction,
    )) = avatars
        .iter()
        .find(|(avatar, _, _, _, _, _, _, _, _)| avatar.character == character)
    else {
        return;
    };
    let Ok((player, context)) = players.get(instance.player_entity()) else {
        return;
    };
    let speed = Vec2::new(velocity.0.x, velocity.0.z).length();
    let actual_motion = motion_probe.actual.get(character).ok();
    let gait_state = motion_probe.gait.get(character).ok();
    let direction = graph_f32(player, "direction_angle");
    let jump_phase = graph_global_f32(player, "als_locomotion_jump_phase");
    let land_phase = graph_global_f32(player, "als_locomotion_land_phase");
    let overlay_index = graph_global_f32(player, "als_root_overlay_index");
    let source_ready = matches!(player.get_animation_source(), AnimationSource::Graph(_))
        && player.get_error().is_none();
    record_animation_window_trace(
        &mut animation_window,
        transform,
        view_yaw.0,
        rotation_animation,
        crouching_graph,
        turn_drive,
        turn_stack,
        *stance == Stance::Crouching,
        None,
    );
    if matches!(
        state.phase,
        IntegrationPhase::DirectionRegression | IntegrationPhase::StateMachineGrounded
    ) {
        animation_window.observed_runtime_state =
            standing_graph.and_then(|value| value.actual_movement_state);
        animation_window.observed_direction = animation_window
            .observed_runtime_state
            .and_then(movement_direction_from_fsm_index)
            .map(movement_direction_name);
        animation_window.observed_parent_runtime_state =
            standing_graph.and_then(|value| value.actual_standing_state);
        animation_window.observed_parent_transition_active =
            standing_graph.and_then(|value| value.actual_standing_transition_active);
    } else if state.phase == IntegrationPhase::CrouchDirectionRegression {
        animation_window.observed_runtime_state =
            crouching_graph.and_then(|value| value.runtime_movement_state);
        animation_window.observed_direction = animation_window
            .observed_runtime_state
            .and_then(movement_direction_from_fsm_index)
            .map(movement_direction_name);
        animation_window.observed_parent_runtime_state =
            crouching_graph.and_then(|value| value.actual_outer_state);
        animation_window.observed_parent_transition_active =
            crouching_graph.and_then(|value| value.actual_outer_transition_active);
        animation_window.observed_requested_parent_runtime_state =
            crouching_graph.and_then(|value| value.requested_outer_state);
        animation_window.observed_requested_parent_transition_active =
            crouching_graph.and_then(|value| value.requested_outer_transition_active);
    } else if state.scenario == IntegrationScenario::Grounded
        && state.phase == IntegrationPhase::Roll
    {
        animation_window.observed_runtime_state =
            grounded_graph.and_then(|value| value.actual_state);
    }
    if !source_ready {
        state.coverage.graph = false;
    }
    if state.scenario == IntegrationScenario::DefaultOverlay {
        let default_active = *overlay == OverlayState::Default
            && overlay_index.is_some_and(|value| value.abs() < 1e-4)
            && source_ready;
        let curves_ready = default_active && default_overlay_curves_ready(context);
        let ground_prediction = graph_global_f32(player, "als_overlay_ground_prediction_amount");
        let idle_ready = curves_ready
            && *stance == Stance::Standing
            && *locomotion == LocomotionState::Grounded
            && speed <= 0.1;
        let moving_ready = curves_ready
            && *stance == Stance::Standing
            && *locomotion == LocomotionState::Grounded
            && speed > 0.5;
        let crouching_ready = curves_ready && *stance == Stance::Crouching && speed > 0.1;
        let air_ready = curves_ready
            && *locomotion == LocomotionState::Air
            && ground_prediction.is_some_and(f32::is_finite);
        if idle_ready && !state.coverage.default_overlay_idle {
            info!("[MANUAL-INTEGRATION][DEFAULT-OVERLAY] state=grounded_idle curves=true -> PASS");
        }
        if moving_ready && !state.coverage.default_overlay_moving {
            info!("[MANUAL-INTEGRATION][DEFAULT-OVERLAY] state=grounded_walk curves=true -> PASS");
        }
        if crouching_ready && !state.coverage.default_overlay_crouching {
            info!("[MANUAL-INTEGRATION][DEFAULT-OVERLAY] state=crouching curves=true -> PASS");
        }
        if air_ready && !state.coverage.default_overlay_air {
            info!(
                "[MANUAL-INTEGRATION][DEFAULT-OVERLAY] state=air ground_prediction={:.6} curves=true -> PASS",
                ground_prediction.unwrap_or_default(),
            );
        }
        state.coverage.default_overlay_idle |= idle_ready;
        state.coverage.default_overlay_moving |= moving_ready;
        state.coverage.default_overlay_crouching |= crouching_ready;
        state.coverage.default_overlay_air |= air_ready;
        state.coverage.default_overlay_curves |= curves_ready;
    }
    let standing_graph_live = standing_graph.is_some_and(|telemetry| {
        (telemetry.actual_movement_state.is_some()
            || telemetry.actual_movement_transition_active == Some(true))
            && telemetry.actual_movement_pose_ready == Some(true)
            && telemetry.dominant_detail_index.is_some()
            && telemetry
                .dominant_detail_weight
                .is_some_and(|weight| (0.0..=1.0).contains(&weight))
    });
    if *stance == Stance::Standing && speed > 0.5 && source_ready && standing_graph_live {
        if !state.coverage.standing_graph
            && let Some(telemetry) = standing_graph
        {
            info!(
                "[MANUAL-INTEGRATION][STANDING-GRAPH] fsm_state={:?} transition_active={} pose_ready={} dominant_branch={:?} dominant_weight={:.3}",
                telemetry.actual_movement_state,
                telemetry.actual_movement_transition_active.unwrap_or(false),
                telemetry.actual_movement_pose_ready.unwrap_or(false),
                telemetry.dominant_detail_index,
                telemetry.dominant_detail_weight.unwrap_or_default(),
            );
        }
        state.coverage.standing_graph = true;
    }
    if state.phase == IntegrationPhase::CrouchDirectionRegression {
        let elapsed = state.phase_elapsed();
        let case_index = (elapsed / CROUCH_DIRECTION_CASE_SECONDS) as usize;
        if case_index < CROUCH_DIRECTION_CASE_COUNT
            && !state.coverage.crouch_direction_cases[case_index]
            && !state.coverage.crouch_direction_diagnostics[case_index]
            && elapsed
                >= case_index as f32 * CROUCH_DIRECTION_CASE_SECONDS
                    + CROUCH_DIRECTION_CASE_SETTLE_SECONDS
            && let Some(case) = crouch_direction_regression_case(case_index)
        {
            state.coverage.crouch_direction_diagnostics[case_index] = true;
            info!(
                "[MANUAL-INTEGRATION][CROUCH-DIRECTION][OBSERVE] case={} stance={:?} speed={:.3} source={} runtime={:?} stride={:?} rate={:?} marker={:?} pose_moving={:?} movement_direction={:?} velocity={:?}",
                case.name,
                stance,
                speed,
                source_ready,
                crouching_graph,
                crouching_graph.and_then(|value| value.graph_stride_amount),
                crouching_graph.and_then(|value| value.graph_play_rate),
                crouching_graph.and_then(|value| value.graph_marker_phase),
                crouching_graph.and_then(|value| value.graph_pose_moving),
                movement_direction.map(|value| value.direction),
                actual_motion.map(|value| value.local_velocity),
            );
        }
        if case_index < CROUCH_DIRECTION_CASE_COUNT
            && !state.coverage.crouch_direction_cases[case_index]
            && elapsed
                >= case_index as f32 * CROUCH_DIRECTION_CASE_SECONDS
                    + CROUCH_DIRECTION_CASE_SETTLE_SECONDS
            && let Some(case) = crouch_direction_regression_case(case_index)
            && let (Some(motion), Some(telemetry)) = (actual_motion, crouching_graph)
            && let Some(state_index) = telemetry.runtime_movement_state
            && state_index < 6
        {
            let expected_world =
                als_core::motion::view_relative_move_direction(case.move_dir, view_yaw.0);
            let expected_local = Vec2::new(
                expected_world.dot(transform.rotation * Vec3::NEG_Z),
                expected_world.dot(transform.rotation * Vec3::X),
            )
            .normalize_or_zero();
            let world_alignment = motion
                .world_velocity
                .normalize_or_zero()
                .dot(expected_world);
            let local_alignment = motion
                .local_velocity
                .normalize_or_zero()
                .dot(expected_local);
            let graph_matches = telemetry.actual_outer_state == Some(1)
                && telemetry.actual_outer_transition_active == Some(false)
                && telemetry.actual_outer_pose_ready == Some(true)
                && telemetry.runtime_movement_transition_active == Some(false)
                && state_index == movement_direction_fsm_index(case.expected_direction)
                && telemetry
                    .graph_movement_state_input
                    .is_some_and(|value| (value - state_index as f32).abs() < 1e-4)
                && movement_direction
                    .is_some_and(|value| value.direction == case.expected_direction);
            if source_ready
                && motion.horizontal_speed > 0.45
                && world_alignment >= 0.75
                && local_alignment >= 0.75
                && graph_matches
            {
                state.coverage.crouch_direction_cases[case_index] = true;
                state.coverage.crouch_direction_state_mask |= 1 << state_index;
                info!(
                    "[MANUAL-INTEGRATION][CROUCH-DIRECTION] case={} expected={:?} state={:?} graph_input={:?} branch={:?} velocity=({:.3},{:.3}) world_align={:.3} local_align={:.3}",
                    case.name,
                    case.expected_direction,
                    telemetry.runtime_movement_state,
                    telemetry.graph_movement_state_input,
                    movement_direction.map(|value| value.direction),
                    motion.local_velocity.x,
                    motion.local_velocity.y,
                    world_alignment,
                    local_alignment,
                );
            }
        }
        if let Some(telemetry) = crouching_graph {
            state.coverage.crouch_direction_lean |= telemetry
                .lean_right_amount
                .zip(telemetry.lean_forward_amount)
                .is_some_and(|(right, forward)| {
                    right.is_finite()
                        && forward.is_finite()
                        && right.abs().max(forward.abs()) > 1e-4
                });
        }
        if let Some(validation) = state.crouch_stationary_validation {
            let expected_parent_state = if validation.is_turn() {
                0
            } else if validation.is_left() {
                2
            } else {
                3
            };
            let side_index = validation.side_index();
            let source_stationary_precondition = crouch_stationary_source_precondition(
                actual_motion,
                crouching_graph,
                turn_drive,
                turn_slot,
            );
            state.crouch_stationary_precondition_frames[side_index] =
                advance_crouch_stationary_stability(
                    state.crouch_stationary_precondition_frames[side_index],
                    source_stationary_precondition,
                );
            if state.crouch_stationary_precondition_frames[side_index]
                >= CROUCH_STATIONARY_STABLE_FRAMES
                && !state.crouch_stationary_precondition_ready[side_index]
            {
                state.crouch_stationary_precondition_ready[side_index] = true;
                state.crouch_stationary_base_yaw[side_index] =
                    Some(world_forward_to_yaw(transform.rotation * Vec3::NEG_Z));
                info!(
                    "[MANUAL-INTEGRATION][CROUCH-STATIONARY-READY] side={} speed={:.4} outer={:?}",
                    if validation.is_left() {
                        "left"
                    } else {
                        "right"
                    },
                    actual_motion
                        .map(|motion| motion.horizontal_speed)
                        .unwrap_or_default(),
                    crouching_graph.and_then(|telemetry| telemetry.actual_outer_state),
                );
            }
            if validation.is_turn() {
                let (active, natural_completion, inertialization, settled) =
                    if let (Some(telemetry), Some(drive)) = (crouching_graph, turn_drive) {
                        let turn_precondition_ready =
                            state.crouch_stationary_precondition_ready[side_index];
                        let baseline = state.crouch_turn_epoch_baseline[side_index]
                            .get_or_insert(drive.inertialization_request_epoch);
                        let side_matches = (drive.lr <= 0.5) == validation.is_left();
                        let curve_driven = rotation_animation
                            .is_some_and(|state| state.yaw_speed_degrees.abs() > 1e-3);
                        let slot_active = turn_slot.is_some_and(|slot| {
                            slot.active && slot.pose_ready && slot.weight > 0.05
                        });
                        let active = turn_precondition_ready
                            && telemetry.actual_outer_state == Some(0)
                            && telemetry.actual_outer_transition_active == Some(false)
                            && drive.playing
                            && drive.weight > 0.05
                            && side_matches
                            && curve_driven
                            && slot_active;
                        let natural_completion =
                            drive.inertialization_request_epoch > *baseline + f32::EPSILON;
                        let inertialization = natural_completion
                            && turn_slot.is_some_and(|slot| {
                                slot.inertialization_request_epoch > *baseline + f32::EPSILON
                            });
                        let settled = inertialization
                            && !drive.playing
                            && drive.weight <= 0.05
                            && turn_slot.is_some_and(|slot| !slot.active && slot.weight <= 0.05);
                        (active, natural_completion, inertialization, settled)
                    } else {
                        (false, false, false, false)
                    };
                let lifecycle = match (active, natural_completion, inertialization, settled) {
                    (true, _, _, _) => &["slot_active"][..],
                    (_, true, true, true) => {
                        &["natural_completion", "inertialization", "settled"][..]
                    }
                    (_, true, true, false) => &["natural_completion", "inertialization"][..],
                    (_, true, false, _) => &["natural_completion"][..],
                    (_, false, false, true) => &["settled"][..],
                    _ => &[],
                };
                **animation_window = crouch_turn_window(validation.is_left(), active, lifecycle);
                animation_window.expected_actor_yaw_baseline =
                    state.crouch_stationary_base_yaw[side_index];
                animation_window.observed_parent_runtime_state =
                    crouching_graph.and_then(|value| value.actual_outer_state);
                animation_window.observed_parent_transition_active =
                    crouching_graph.and_then(|value| value.actual_outer_transition_active);
                animation_window.observed_requested_parent_runtime_state =
                    crouching_graph.and_then(|value| value.requested_outer_state);
                animation_window.observed_requested_parent_transition_active =
                    crouching_graph.and_then(|value| value.requested_outer_transition_active);
                record_animation_window_trace(
                    &mut animation_window,
                    transform,
                    view_yaw.0,
                    rotation_animation,
                    crouching_graph,
                    turn_drive,
                    turn_stack,
                    true,
                    turn_drive
                        .map(|drive| crouch_stationary_animation_variant(drive.lr <= 0.5, true)),
                );
                if active {
                    state.coverage.crouch_turn_active_mask |= 1 << side_index;
                }
                if natural_completion && inertialization && settled {
                    state.coverage.crouch_turn_completion_mask |= 1 << side_index;
                }
            } else {
                let active = state.crouch_stationary_precondition_ready[side_index]
                    && crouching_graph.is_some_and(|telemetry| {
                        telemetry.actual_outer_state == Some(expected_parent_state)
                    })
                    && source_rotate_window_active(turn_drive)
                    && turn_drive
                        .is_some_and(|drive| (drive.rotate_lr <= 0.5) == validation.is_left())
                    && rotation_animation.is_some_and(|state| state.yaw_speed_degrees.abs() > 1e-3);
                if active {
                    state.coverage.crouch_rotate_mask |= 1 << validation.side_index();
                }
                if turn_drive.is_some_and(|drive| drive.rotate_completion_event) {
                    state.coverage.crouch_rotate_source_completion_mask |= 1 << side_index;
                }
                let source_completed =
                    state.coverage.crouch_rotate_source_completion_mask & (1 << side_index) != 0;
                let settled = source_rotate_window_completed(
                    state.coverage.crouch_rotate_mask & (1 << side_index) != 0,
                    source_completed,
                    source_stationary_precondition,
                );
                if settled {
                    state.coverage.crouch_rotate_completion_mask |= 1 << side_index;
                }
                let lifecycle = match (active, source_completed, settled) {
                    (true, _, _) => &[][..],
                    (_, true, true) => &["sequence_completed", "settled"][..],
                    (_, true, false) => &["sequence_completed"][..],
                    _ => &[],
                };
                **animation_window = crouch_rotate_window(validation.is_left(), active, lifecycle);
                animation_window.expected_actor_yaw_baseline =
                    state.crouch_stationary_base_yaw[side_index];
                animation_window.observed_parent_runtime_state =
                    crouching_graph.and_then(|value| value.actual_outer_state);
                animation_window.observed_parent_transition_active =
                    crouching_graph.and_then(|value| value.actual_outer_transition_active);
                animation_window.observed_requested_parent_runtime_state =
                    crouching_graph.and_then(|value| value.requested_outer_state);
                animation_window.observed_requested_parent_transition_active =
                    crouching_graph.and_then(|value| value.requested_outer_transition_active);
                record_animation_window_trace(
                    &mut animation_window,
                    transform,
                    view_yaw.0,
                    rotation_animation,
                    crouching_graph,
                    turn_drive,
                    turn_stack,
                    true,
                    turn_drive.map(|drive| {
                        crouch_stationary_animation_variant(drive.rotate_lr <= 0.5, false)
                    }),
                );
            }
        }
    }
    if state.phase == IntegrationPhase::CrouchRecovery
        && let Some(transaction) = crouch_transaction
        && transaction.actual_stance == Stance::Crouching
        && transaction.controller_crouch_active
        && (transaction.headroom_blocked || transaction.collider_half_height < 0.9)
    {
        if !state.coverage.crouch_transaction {
            info!(
                "[MANUAL-INTEGRATION][CROUCH] desired={:?} actual={:?} phase={:?} retries={} half={:.3} radius={:.3} controller={} blocked={}",
                transaction.desired_stance,
                transaction.actual_stance,
                transaction.phase,
                transaction.blocked_retry_count,
                transaction.collider_half_height,
                transaction.collider_radius,
                transaction.controller_crouch_active,
                transaction.headroom_blocked,
            );
        }
        state.coverage.crouch_transaction = true;
    }
    if let (Some(transaction), Some(telemetry)) = (crouch_transaction, grounded_graph)
        && transaction.actual_stance == Stance::Crouching
        && telemetry.actual_state == Some(2)
        && telemetry.transition_active == Some(false)
        && telemetry.pose_ready == Some(true)
    {
        if !state.coverage.crouch_graph {
            info!(
                "[MANUAL-INTEGRATION][CROUCH-GRAPH] desired={:?} actual={:?} forced_standing={} state={} transition_active={} pose_ready={}",
                transaction.desired_stance,
                transaction.actual_stance,
                transaction.forced_standing,
                telemetry.actual_state.unwrap_or_default(),
                telemetry.transition_active.unwrap_or(false),
                telemetry.pose_ready.unwrap_or(false),
            );
        }
        state.coverage.crouch_graph = true;
    }
    if let Some(transaction) = crouch_transaction
        && *locomotion == LocomotionState::Air
        && transaction.desired_stance == Stance::Crouching
        && transaction.actual_stance == Stance::Standing
        && transaction.forced_standing
        && !transaction.controller_crouch_active
    {
        if !state.coverage.crouch_air_forced_standing {
            info!(
                "[MANUAL-INTEGRATION][CROUCH-AIR] desired={:?} actual={:?} phase={:?} controller={} forced_standing={}",
                transaction.desired_stance,
                transaction.actual_stance,
                transaction.phase,
                transaction.controller_crouch_active,
                transaction.forced_standing,
            );
        }
        state.coverage.crouch_air_forced_standing = true;
    }
    if state.phase == IntegrationPhase::CrouchRecovery
        && state.coverage.crouch_air_forced_standing
        && let (Some(transaction), Some(telemetry)) = (crouch_transaction, grounded_graph)
        && *locomotion == LocomotionState::Grounded
        && transaction.desired_stance == Stance::Crouching
        && transaction.actual_stance == Stance::Crouching
        && telemetry.actual_state == Some(2)
        && telemetry.transition_active == Some(false)
        && telemetry.pose_ready == Some(true)
    {
        if !state.coverage.crouch_air_landed {
            info!(
                "[MANUAL-INTEGRATION][CROUCH-LAND] desired={:?} actual={:?} state={} controller={} forced_standing={}",
                transaction.desired_stance,
                transaction.actual_stance,
                telemetry.actual_state.unwrap_or_default(),
                transaction.controller_crouch_active,
                transaction.forced_standing,
            );
        }
        state.coverage.crouch_air_landed = true;
    }
    if state.phase == IntegrationPhase::CrouchRecovery
        && state.crouch_recovery_standing_requested
        && let (Some(transaction), Some(telemetry)) = (crouch_transaction, grounded_graph)
        && transaction.desired_stance == Stance::Standing
        && transaction.actual_stance == Stance::Standing
        && !transaction.controller_crouch_active
        && telemetry.actual_state == Some(1)
        && telemetry.transition_active == Some(false)
        && telemetry.pose_ready == Some(true)
    {
        if !state.coverage.crouch_graph_recovered {
            info!(
                "[MANUAL-INTEGRATION][CROUCH-RECOVERED] desired={:?} actual={:?} state={} controller={}",
                transaction.desired_stance,
                transaction.actual_stance,
                telemetry.actual_state.unwrap_or_default(),
                transaction.controller_crouch_active,
            );
        }
        state.coverage.crouch_graph_recovered = true;
    }
    if state.phase == IntegrationPhase::CrouchClearance
        && *stance == Stance::Crouching
        && speed > 0.1
    {
        if !state.coverage.crouch_moving {
            info!(
                "[MANUAL-INTEGRATION][CROUCH-MOVING] speed={:.3} desired={:?} actual={:?}",
                speed,
                crouch_transaction.map(|transaction| transaction.desired_stance),
                stance,
            );
        }
        state.coverage.crouch_moving = true;
    }
    if state.phase == IntegrationPhase::CrouchClearance
        && state.crouch_canopy.is_some()
        && let (Some(transaction), Some(telemetry)) = (crouch_transaction, grounded_graph)
        && transaction.desired_stance == Stance::Standing
        && transaction.actual_stance == Stance::Crouching
        && transaction.headroom_blocked
        && transaction.blocked_retry_count >= 2
        && telemetry.actual_state == Some(2)
        && telemetry.transition_active == Some(false)
        && telemetry.pose_ready == Some(true)
    {
        if !state.coverage.crouch_canopy_blocked {
            info!(
                "[MANUAL-INTEGRATION][CROUCH-CANOPY-BLOCKED] desired={:?} actual={:?} retries={} half={:.3} state={} controller={}",
                transaction.desired_stance,
                transaction.actual_stance,
                transaction.blocked_retry_count,
                transaction.collider_half_height,
                telemetry.actual_state.unwrap_or_default(),
                transaction.controller_crouch_active,
            );
        }
        state.coverage.crouch_canopy_blocked = true;
    }
    if state.phase == IntegrationPhase::CrouchClearance
        && state.coverage.crouch_canopy_blocked
        && let (Some(transaction), Some(telemetry)) = (crouch_transaction, grounded_graph)
        && transaction.desired_stance == Stance::Standing
        && transaction.actual_stance == Stance::Standing
        && !transaction.headroom_blocked
        && !transaction.controller_crouch_active
        && telemetry.actual_state == Some(1)
        && telemetry.transition_active == Some(false)
        && telemetry.pose_ready == Some(true)
    {
        if !state.coverage.crouch_canopy_recovered {
            info!(
                "[MANUAL-INTEGRATION][CROUCH-CANOPY-RECOVERED] desired={:?} actual={:?} retries={} half={:.3} state={}",
                transaction.desired_stance,
                transaction.actual_stance,
                transaction.blocked_retry_count,
                transaction.collider_half_height,
                telemetry.actual_state.unwrap_or_default(),
            );
        }
        state.coverage.crouch_canopy_recovered = true;
    }
    if state.phase == IntegrationPhase::DirectionRegression
        && state.direction_case_index < DIRECTION_CASE_COUNT
        && !state.coverage.direction_cases[state.direction_case_index]
        && state.phase_elapsed()
            >= state.direction_case_index as f32 * DIRECTION_CASE_SECONDS
                + DIRECTION_CASE_SETTLE_SECONDS
        && let Some(case) = direction_regression_case(state.direction_case_index)
        && let (Some(motion), Some((desired, allowed)), Some(telemetry), Some(direction)) = (
            actual_motion,
            gait_state,
            standing_graph,
            movement_direction,
        )
    {
        let expected_world =
            als_core::motion::view_relative_move_direction(case.move_dir, view_yaw.0);
        let expected_local = Vec2::new(
            expected_world.dot(transform.rotation * Vec3::NEG_Z),
            expected_world.dot(transform.rotation * Vec3::X),
        )
        .normalize_or_zero();
        let world_alignment = motion
            .world_velocity
            .normalize_or_zero()
            .dot(expected_world);
        let local_alignment = motion
            .local_velocity
            .normalize_or_zero()
            .dot(expected_local);
        let branch_mask = if case.branch_from_actual_local {
            actual_velocity_branch_mask(motion)
        } else {
            case.branch_mask
        };
        let source_state_matches = case.expected_movement_state.is_none_or(|expected| {
            telemetry.actual_movement_state == Some(movement_direction_fsm_index(expected))
        });
        let target_state_matches = case.expected_movement_state.is_some()
            || telemetry.actual_movement_state
                == Some(movement_direction_fsm_index(direction.direction));
        let graph_matches = source_state_matches
            && target_state_matches
            && telemetry.actual_movement_transition_active == Some(false)
            && telemetry.actual_movement_pose_ready == Some(true)
            && direction_branch_matches(telemetry.dominant_detail_index, branch_mask)
            && telemetry
                .dominant_detail_weight
                .is_some_and(|weight| weight >= 0.4);
        let gait_matches = desired.0 == case.desired_gait
            && allowed.0 == case.expected_allowed_gait
            && *gait == case.expected_actual_gait
            && first_person == case.first_person;
        if source_ready
            && motion.horizontal_speed > 0.75
            && world_alignment >= 0.8
            && local_alignment >= 0.8
            && gait_matches
            && graph_matches
        {
            let case_index = state.direction_case_index;
            state.coverage.direction_cases[case_index] = true;
            info!(
                "[MANUAL-INTEGRATION][DIRECTION] case={} source_state={:?} expected_direction={:?} world=({:.3},{:.3}) local=({:.3},{:.3}) direction={:?} gait=({:?},{:?},{:?}) fsm={} branch={} branch_mask={} weight={:.3} world_align={:.3} local_align={:.3}",
                case.name,
                case.source_state,
                case.expected_movement_state,
                motion.world_velocity.x,
                motion.world_velocity.z,
                motion.local_velocity.x,
                motion.local_velocity.y,
                direction.direction,
                desired.0,
                allowed.0,
                gait,
                telemetry.actual_movement_state.unwrap_or_default(),
                telemetry.dominant_detail_index.unwrap_or_default(),
                branch_mask,
                telemetry.dominant_detail_weight.unwrap_or_default(),
                world_alignment,
                local_alignment,
            );
        }
    }
    for event in animation_events
        .read()
        .filter(|event| event.character == character)
    {
        if state.scenario != IntegrationScenario::Notify {
            continue;
        }
        match event.class_name.as_str() {
            "AlsAnimNotify_CameraShake"
                if event.source_asset.contains("A_Als_Sprint_Acceleration") =>
            {
                state.coverage.notify_sprint_acceleration = true;
            }
            "AlsAnimNotify_CameraShake" if event.source_asset.contains("A_Als_Sprint") => {
                state.coverage.notify_sprint = true;
            }
            "AlsAnimNotify_CameraShake" if event.source_asset.contains("A_Als_Roll") => {
                state.coverage.notify_roll = true;
            }
            "AuthoredSyncMarker" => {
                state.coverage.notify_marker = true;
            }
            "AlsAnimNotify_SetGroundedEntryMode" => {
                state.coverage.notify_grounded_entry = true;
            }
            "AlsAnimNotifyState_SetRootMotionScale" => {
                state.coverage.notify_root_scale = true;
            }
            "AlsAnimNotifyState_SetLocomotionAction" => {
                if event.phase == als_ext::EventPhase::Active {
                    match event.parameters.get("LocomotionAction") {
                        Some(value)
                            if value
                                .get("__tagged__")
                                .and_then(|value| value.get("TagName"))
                                .and_then(serde_json::Value::as_str)
                                == Some("Als.LocomotionAction.Rolling") =>
                        {
                            state.coverage.notify_rolling_action = true;
                        }
                        Some(value)
                            if value
                                .get("__tagged__")
                                .and_then(|value| value.get("TagName"))
                                .and_then(serde_json::Value::as_str)
                                == Some("Als.LocomotionAction.Mantling") =>
                        {
                            state.coverage.notify_mantling_action = true;
                        }
                        _ => {}
                    }
                }
            }
            "AlsAnimNotifyState_EarlyBlendOut" => {
                state.coverage.notify_early_blend_out = true;
            }
            _ => {}
        }
        info!(
            "[MANUAL-INTEGRATION][NOTIFY] source={} class={} name={} phase={:?} time={:.6} weight={:.3}",
            event.source_asset, event.class_name, event.name, event.phase, event.time, event.weight,
        );
    }
    if state.scenario == IntegrationScenario::Notify {
        state.coverage.notify_rolling_action |=
            notify_action.is_some_and(|action| *action == AlsNotifyLocomotionAction::Rolling);
        state.coverage.notify_mantling_action |=
            notify_action.is_some_and(|action| *action == AlsNotifyLocomotionAction::Mantling);
        state.coverage.notify_root_scale |=
            notify_root_scale.is_some_and(|scale| (scale.0 - 1.3).abs() < 1e-4);
        state.coverage.notify_early_blend_out |= notify_early_blend_out
            .is_some_and(|state| state.active && state.blend_out_duration > 0.0);
        state.coverage.notify_grounded_entry |=
            grounded_entry.is_some_and(|mode| *mode == AlsGroundedEntryMode::FromRoll);
    }
    if state.scenario == IntegrationScenario::Sprint && state.phase == IntegrationPhase::Sprint {
        let standing_global = graph_global_f32(player, "als_standing_curve_pose_moving");
        let final_pose_moving = context.and_then(|value| value.f32("PoseMoving"));
        let final_left_ik = context.and_then(|value| value.f32("FootLeftIk"));
        let final_right_ik = context.and_then(|value| value.f32("FootRightIk"));
        let applied = foot_ik_data
            .get(character)
            .ok()
            .map(|(_, _, apply, _)| (apply.left.applied_weight, apply.right.applied_weight));
        let relay = source_ready
            && *gait == Gait::Sprinting
            && speed > 0.1
            && standing_global.is_some_and(|value| value >= 0.999)
            && final_pose_moving.is_some_and(|value| value >= 0.999)
            && final_left_ik.is_some_and(|value| value >= 0.999)
            && final_right_ik.is_some_and(|value| value >= 0.999)
            && applied.is_some_and(|(left, right)| left >= 0.999 && right >= 0.999);
        if relay && !state.coverage.sprint_curve_relay {
            info!(
                "[MANUAL-INTEGRATION][SPRINT-CURVE-RELAY] global={:.3} final={:.3} left_ik={:.3} right_ik={:.3} applied=({:.3},{:.3}) -> PASS",
                standing_global.unwrap_or_default(),
                final_pose_moving.unwrap_or_default(),
                final_left_ik.unwrap_or_default(),
                final_right_ik.unwrap_or_default(),
                applied.map_or(0.0, |value| value.0),
                applied.map_or(0.0, |value| value.1),
            );
        }
        state.coverage.sprint_curve_relay |= relay;
    }
    state.coverage.locomotion |= speed > 0.25 && direction.is_some();
    if speed > 0.5 && direction.is_some_and(|value| value.abs() > 150.0) {
        state.coverage.backward = true;
    }
    if speed > 0.5 && direction.is_some_and(|value| (60.0..120.0).contains(&value.abs())) {
        state.coverage.sideways = true;
    }
    state.coverage.crouching |= *stance == Stance::Crouching && speed > 0.1;
    state.coverage.sprinting |= *gait == Gait::Sprinting && speed > 0.1;
    if *locomotion == LocomotionState::Air && jump_phase.is_some() {
        state.coverage.airborne = true;
    }
    if state.coverage.airborne
        && *locomotion == LocomotionState::Grounded
        && land_phase.is_some_and(|value| value > 1e-4)
    {
        state.coverage.landed = true;
    }
    state.coverage.overlay |= *overlay == OverlayState::Rifle && overlay_index.is_some();
    state.coverage.gait_layer |=
        gait_blend.is_some_and(|blend| blend.weights.sprint_w > 1e-4) && *gait == Gait::Sprinting;
    state.coverage.stance_layer |=
        stance_blend.is_some_and(|blend| blend.weight > 1e-4) && *stance == Stance::Crouching;
    state.coverage.jump_variant |=
        jump_variant.is_some_and(|variant| variant.airborne) && *locomotion == LocomotionState::Air;
    state.coverage.turn_in_place |= turn_drive.is_some_and(|drive| {
        (drive.playing && drive.weight > 1e-4)
            || (drive.rotate_active && drive.rotate_weight > 1e-4)
    });
    state.coverage.layering |= *stance == Stance::Crouching
        && layering.is_some()
        && graph_global_f32(player, "als_layering_crouching_amount")
            .is_some_and(|value| value > 1e-4);
    state.coverage.overlay_object |= overlay_object.is_some_and(|object| {
        object.visible
            && object.applied_overlay == Some(OverlayState::Rifle)
            && object
                .prop
                .is_some_and(|prop| interface.overlay_props.get(prop).is_ok())
    });
    state.coverage.start_stop |= start_stop.is_some_and(|value| {
        matches!(
            value.phase,
            StartStopPhase::Started(_) | StartStopPhase::Stopping(_)
        )
    });
    if matches!(
        state.scenario,
        IntegrationScenario::StateMachine | IntegrationScenario::GeneratedTransitionNotify
    ) {
        state.coverage.generated_transition_stop_quick |= motion_probe
            .generated_transition_notify
            .get(character)
            .ok()
            .flatten()
            .is_some_and(|telemetry| telemetry.stop_quick_count > 0);
        if let Some(telemetry) = standing_graph {
            let previous_mask = state.p1_ground_mask;
            if source_ready && telemetry.outer_state == Some(0) {
                state.p1_ground_mask |= 1 << 0;
            }
            if source_ready
                && telemetry.outer_state == Some(1)
                && telemetry.actual_movement_state.is_some()
                && speed > 0.5
            {
                state.p1_ground_mask |= 1 << 1;
            }
            if source_ready && telemetry.outer_state == Some(2) {
                state.p1_ground_mask |= 1 << 2;
            }
            if source_ready
                && telemetry.outer_state == Some(1)
                && matches!(telemetry.detail_state, Some(2) | Some(5))
            {
                state.p1_ground_mask |= 1 << 3;
            }
            if state.p1_ground_mask != previous_mask {
                info!(
                    "[MANUAL-INTEGRATION][P1][GROUND] mask={:04b} outer={:?} outer_transition={:?} detail={:?} detail_transition={:?} movement={:?} weight={:.3} speed={speed:.3}",
                    state.p1_ground_mask,
                    telemetry.outer_state,
                    telemetry.outer_transition_active,
                    telemetry.detail_state,
                    telemetry.detail_transition_active,
                    telemetry.actual_movement_state,
                    telemetry.dominant_detail_weight.unwrap_or_default(),
                );
            }
        }
        state.coverage.p1_ground = state.p1_ground_mask == 0b1111;

        if let Some((desired, allowed)) = gait_state {
            let previous_mask = state.p1_gait_mask;
            if desired.0 == Gait::Walking && allowed.0 == Gait::Walking && *gait == Gait::Walking {
                state.p1_gait_mask |= 1 << 0;
            }
            if desired.0 == Gait::Running && allowed.0 == Gait::Running && *gait == Gait::Running {
                state.p1_gait_mask |= 1 << 1;
            }
            if desired.0 == Gait::Sprinting
                && allowed.0 == Gait::Sprinting
                && *gait == Gait::Sprinting
            {
                state.p1_gait_mask |= 1 << 2;
            }
            if desired.0 == Gait::Sprinting && allowed.0 == Gait::Running && *gait == Gait::Running
            {
                state.p1_gait_mask |= 1 << 3;
            }
            if state.p1_gait_mask != previous_mask {
                let (local_x, local_y) = actual_motion
                    .map(|motion| (motion.local_velocity.x, motion.local_velocity.y))
                    .unwrap_or_default();
                info!(
                    "[MANUAL-INTEGRATION][P1][GAIT] mask={:04b} desired={:?} allowed={:?} actual={:?} local=({local_x:.3},{local_y:.3}) speed={speed:.3}",
                    state.p1_gait_mask, desired.0, allowed.0, gait,
                );
            }
        }
        state.coverage.p1_gait = state.p1_gait_mask == 0b1111;

        if let (Some(transaction), Some(telemetry)) = (crouch_transaction, grounded_graph) {
            let previous_mask = state.p1_stance_mask;
            if transaction.actual_stance == Stance::Crouching
                && telemetry.actual_state == Some(2)
                && telemetry.pose_ready == Some(true)
            {
                state.p1_stance_mask |= 1 << 0;
            }
            if state.p1_stance_mask & 1 != 0
                && transaction.actual_stance == Stance::Standing
                && telemetry.actual_state == Some(1)
                && telemetry.pose_ready == Some(true)
            {
                state.p1_stance_mask |= 1 << 1;
            }
            if state.p1_stance_mask != previous_mask {
                info!(
                    "[MANUAL-INTEGRATION][P1][STANCE] mask={:02b} desired={:?} actual={:?} phase={:?} collider={:.3} grounded={:?}",
                    state.p1_stance_mask,
                    transaction.desired_stance,
                    transaction.actual_stance,
                    transaction.phase,
                    transaction.collider_half_height,
                    telemetry.actual_state,
                );
            }
        }
        state.coverage.p1_stance = state.p1_stance_mask == 0b11;

        let previous_air_mask = state.p1_air_mask;
        if *locomotion == LocomotionState::Air
            && jump_variant.is_some_and(|variant| variant.airborne)
        {
            state.p1_air_mask |= 1 << 0;
        }
        if *locomotion == LocomotionState::Air && velocity.0.y < -0.05 {
            state.p1_air_mask |= 1 << 1;
        }
        if state.p1_air_mask & 0b11 == 0b11
            && *locomotion == LocomotionState::Grounded
            && land_phase.is_some_and(|value| value > 1e-4)
        {
            state.p1_air_mask |= 1 << 2;
        }
        if state.p1_air_mask != previous_air_mask {
            info!(
                "[MANUAL-INTEGRATION][P1][AIR] mask={:03b} locomotion={:?} vertical={:.3} jump={:?} land={:?}",
                state.p1_air_mask,
                locomotion,
                velocity.0.y,
                jump_variant.map(|variant| variant.airborne),
                land_phase,
            );
        }
        state.coverage.p1_air = state.p1_air_mask == 0b111;

        if let Some(drive) = turn_drive {
            let previous_mask = state.p1_turn_mask;
            if drive.playing && drive.weight > 1e-4 {
                if drive.lr <= 0.5 {
                    state.p1_turn_mask |= 1 << 0;
                } else {
                    state.p1_turn_mask |= 1 << 1;
                }
            }
            if drive.rotate_active && drive.rotate_weight > 1e-4 {
                if drive.rotate_lr <= 0.5 {
                    state.p1_turn_mask |= 1 << 2;
                } else {
                    state.p1_turn_mask |= 1 << 3;
                }
            }
            if state.p1_turn_mask != previous_mask {
                info!(
                    "[MANUAL-INTEGRATION][P1][TURN] mask={:04b} turn=(playing={},lr={:.1},weight={:.3}) slot=(active={},ready={},weight={:.3},inertia_epoch={:.0}) rotate=(active={},lr={:.1},weight={:.3},rate={:.3}) first_person={}",
                    state.p1_turn_mask,
                    drive.playing,
                    drive.lr,
                    drive.weight,
                    turn_slot.is_some_and(|slot| slot.active),
                    turn_slot.is_some_and(|slot| slot.pose_ready),
                    turn_slot.map_or(0.0, |slot| slot.weight),
                    turn_slot.map_or(0.0, |slot| slot.inertialization_request_epoch),
                    drive.rotate_active,
                    drive.rotate_lr,
                    drive.rotate_weight,
                    drive.rotate_rate,
                    first_person,
                );
            }
        }
        state.coverage.p1_turn = state.p1_turn_mask == 0b1111;
        let turn_slot_ready =
            turn_slot.is_some_and(|slot| slot.active && slot.pose_ready && slot.weight > 0.05);
        if turn_slot_ready && !state.coverage.turn_slot {
            warn!(
                "[MANUAL-INTEGRATION][P1][TURN-SLOT] active=true pose_ready=true weight={:.3} -> PASS",
                turn_slot.map_or(0.0, |slot| slot.weight)
            );
        }
        state.coverage.turn_slot |= turn_slot_ready;
        let turn_inertialized =
            turn_slot.is_some_and(|slot| slot.inertialization_request_epoch >= 1.0);
        if turn_inertialized && !state.coverage.turn_inertialization {
            warn!(
                "[MANUAL-INTEGRATION][P1][TURN-INERTIALIZATION] epoch={:.0} -> PASS",
                turn_slot.map_or(0.0, |slot| slot.inertialization_request_epoch)
            );
        }
        state.coverage.turn_inertialization |= turn_inertialized;
    }
    state.coverage.view_head |= state
        .initial_view_yaw
        .is_some_and(|initial| (view_yaw.0 - initial).abs() > 0.01)
        && state
            .initial_view_pitch
            .is_some_and(|initial| (view_pitch.0 - initial).abs() > 0.01)
        && head.is_some_and(|value| value.pitch_angle.abs() > 0.01 || value.yaw_angle.abs() > 0.01);
    let control_rig_ready = control_rig.is_some_and(|value| {
        value.spine_rotation_allowed
            && value.spine_amount.abs() > 1e-4
            && (state.scenario != IntegrationScenario::ControlRig
                || value.final_yaw_angle.abs() > 1e-4)
    });
    if control_rig_ready && !state.coverage.control_rig {
        if let Some(value) = control_rig {
            info!(
                "[MANUAL-INTEGRATION][CR-ALS-SPINE] allowed={} amount={:.6} final_yaw_degrees={:.6} -> PASS",
                value.spine_rotation_allowed, value.spine_amount, value.final_yaw_angle,
            );
        }
    }
    state.coverage.control_rig |= control_rig_ready;
    if let Ok((construction, input, trace)) = control_rig_runtime.get(character) {
        let input_ready = input.use_hand_ik_bones
            && input.use_foot_ik_bones
            && input.velocity_blend_forward_amount.is_finite()
            && input.velocity_blend_backward_amount.is_finite()
            && input.spine_yaw_angle_degrees.is_finite()
            && input.pelvis_offset_amount.is_finite()
            && input.foot_left_location_armature.is_finite()
            && input.foot_right_location_armature.is_finite()
            && input.foot_left_rotation_armature.is_finite()
            && input.foot_right_rotation_armature.is_finite()
            && (state.scenario != IntegrationScenario::ControlRig
                || input.velocity_blend_forward_amount.abs() > 1e-3
                || input.velocity_blend_backward_amount.abs() > 1e-3
                || input.spine_yaw_angle_degrees.abs() > 1e-3);
        let stage_ready = trace.construction_complete
            && trace.initialization_count == 1
            && trace.stage_mask == AlsControlRigStageTrace::COMPLETE
            && trace.completed_frames > 0;
        let foot_targets_ready = foot_ik_data
            .get(character)
            .is_ok_and(|(_, space, apply, _)| {
                construction.foot_height_m.is_finite()
                    && construction.foot_height_m > 0.0
                    && construction.leg_length_m.is_finite()
                    && construction.leg_length_m > 0.0
                    && space.left.target_armature_local.is_finite()
                    && space.right.target_armature_local.is_finite()
                    && space.left.normal_armature_local.is_finite()
                    && space.right.normal_armature_local.is_finite()
                    && apply.left.target_world.is_finite()
                    && apply.right.target_world.is_finite()
            });
        if stage_ready && !state.coverage.control_rig_stages {
            info!(
                "[MANUAL-INTEGRATION][CR-ALS-STAGES] construction_count={} runtime_mask={:07b} completed_frames={} -> PASS",
                trace.initialization_count, trace.stage_mask, trace.completed_frames,
            );
        }
        if input_ready && !state.coverage.control_rig_input {
            info!(
                "[MANUAL-INTEGRATION][CR-ALS-INPUT] forward={:.3} backward={:.3} spine_yaw={:.3} pelvis={:.3} left=({:.3},{:.3},{:.3}) right=({:.3},{:.3},{:.3}) -> PASS",
                input.velocity_blend_forward_amount,
                input.velocity_blend_backward_amount,
                input.spine_yaw_angle_degrees,
                input.pelvis_offset_amount,
                input.foot_left_location_armature.x,
                input.foot_left_location_armature.y,
                input.foot_left_location_armature.z,
                input.foot_right_location_armature.x,
                input.foot_right_location_armature.y,
                input.foot_right_location_armature.z,
            );
        }
        if foot_targets_ready && !state.coverage.control_rig_foot_targets {
            info!(
                "[MANUAL-INTEGRATION][CR-ALS-TARGET-TRANSFORM] foot_height_m={:.9} leg_length_m={:.9} -> PASS",
                construction.foot_height_m, construction.leg_length_m,
            );
        }
        state.coverage.control_rig_stages |= stage_ready;
        state.coverage.control_rig_input |= input_ready;
        state.coverage.control_rig_foot_targets |= foot_targets_ready;
    }

    if let Some(mantle) = mantle {
        if mantle.active {
            state.coverage.mantle_started = true;
            state.mantle_target_y = Some(mantle.target.y);
        }
        if state.mantle_was_active
            && !mantle.active
            && state
                .mantle_target_y
                .is_some_and(|target_y| transform.translation.y >= target_y - 0.3)
        {
            state.coverage.mantle_completed = true;
        }
        state.mantle_was_active = mantle.active;
    }
    if let Some(probe) = foot_probe {
        state.coverage.foot_probe |= probe.left.is_some() && probe.right.is_some();
    }
    if let Some(lock) = foot_lock {
        state.coverage.foot_lock |= lock.valid;
    }
    if state.scenario == IntegrationScenario::FootIk
        && state.phase == IntegrationPhase::DirectionRegression
        && let Some(bit) = foot_ik_baseline_bit(state.direction_case_index)
        && state.phase_elapsed()
            >= state.direction_case_index as f32 * DIRECTION_CASE_SECONDS
                + DIRECTION_CASE_SETTLE_SECONDS
                + 0.95
        && let Ok((telemetry, space, apply, _)) = foot_ik_data.get(character)
        && telemetry.left.valid
        && telemetry.right.valid
        && space.left.valid
        && space.right.valid
        && apply.left.valid
        && apply.right.valid
        && telemetry.left.target_world.is_finite()
        && telemetry.right.target_world.is_finite()
        && telemetry.left.target_armature_local.is_finite()
        && telemetry.right.target_armature_local.is_finite()
        && telemetry.left.target_parent_local.is_finite()
        && telemetry.right.target_parent_local.is_finite()
        && telemetry.left.reference_chain_error.is_finite()
        && telemetry.right.reference_chain_error.is_finite()
        && telemetry.left.pole_sign.is_finite()
        && telemetry.right.pole_sign.is_finite()
    {
        if state.foot_ik_baseline_mask & bit == 0
            && let Some(case) = direction_regression_case(state.direction_case_index)
        {
            info!(
                "[MANUAL-INTEGRATION][FOOT-IK-BASELINE] case={} left_world=({:.3},{:.3},{:.3}) left_armature=({:.3},{:.3},{:.3}) left_parent=({:.3},{:.3},{:.3}) left_error={:.6} left_pole={:.6} left_weights=({:.3},{:.3}) right_world=({:.3},{:.3},{:.3}) right_armature=({:.3},{:.3},{:.3}) right_parent=({:.3},{:.3},{:.3}) right_error={:.6} right_pole={:.6} right_weights=({:.3},{:.3})",
                case.name,
                telemetry.left.target_world.x,
                telemetry.left.target_world.y,
                telemetry.left.target_world.z,
                telemetry.left.target_armature_local.x,
                telemetry.left.target_armature_local.y,
                telemetry.left.target_armature_local.z,
                telemetry.left.target_parent_local.x,
                telemetry.left.target_parent_local.y,
                telemetry.left.target_parent_local.z,
                telemetry.left.reference_chain_error,
                telemetry.left.pole_sign,
                telemetry.left.ik_weight,
                telemetry.left.pin_weight,
                telemetry.right.target_world.x,
                telemetry.right.target_world.y,
                telemetry.right.target_world.z,
                telemetry.right.target_armature_local.x,
                telemetry.right.target_armature_local.y,
                telemetry.right.target_armature_local.z,
                telemetry.right.target_parent_local.x,
                telemetry.right.target_parent_local.y,
                telemetry.right.target_parent_local.z,
                telemetry.right.reference_chain_error,
                telemetry.right.pole_sign,
                telemetry.right.ik_weight,
                telemetry.right.pin_weight,
            );
            info!(
                "[MANUAL-INTEGRATION][FOOT-IK-SPACE] case={} left_trace=({:.3},{:.3},{:.3}) left_hit=({:.3},{:.3},{:.3}) left_raw={:.6} left_normal_world=({:.3},{:.3},{:.3}) left_normal_armature=({:.3},{:.3},{:.3}) right_trace=({:.3},{:.3},{:.3}) right_hit=({:.3},{:.3},{:.3}) right_raw={:.6} right_normal_world=({:.3},{:.3},{:.3}) right_normal_armature=({:.3},{:.3},{:.3})",
                case.name,
                space.left.trace_target_world.x,
                space.left.trace_target_world.y,
                space.left.trace_target_world.z,
                space.left.hit_world.x,
                space.left.hit_world.y,
                space.left.hit_world.z,
                space.left.raw_offset,
                space.left.normal_world.x,
                space.left.normal_world.y,
                space.left.normal_world.z,
                space.left.normal_armature_local.x,
                space.left.normal_armature_local.y,
                space.left.normal_armature_local.z,
                space.right.trace_target_world.x,
                space.right.trace_target_world.y,
                space.right.trace_target_world.z,
                space.right.hit_world.x,
                space.right.hit_world.y,
                space.right.hit_world.z,
                space.right.raw_offset,
                space.right.normal_world.x,
                space.right.normal_world.y,
                space.right.normal_world.z,
                space.right.normal_armature_local.x,
                space.right.normal_armature_local.y,
                space.right.normal_armature_local.z,
            );
            info!(
                "[MANUAL-INTEGRATION][FOOT-IK-APPLY] case={} left_error={:.6} left_adjust={:.6} left_pole={:.6} left_weight={:.3} right_error={:.6} right_adjust={:.6} right_pole={:.6} right_weight={:.3}",
                case.name,
                apply.left.effector_error,
                space
                    .left
                    .target_world
                    .distance(space.left.resolved_target_world),
                apply.left.pole_sign,
                apply.left.applied_weight,
                apply.right.effector_error,
                space
                    .right
                    .target_world
                    .distance(space.right.resolved_target_world),
                apply.right.pole_sign,
                apply.right.applied_weight,
            );
        }
        state.foot_ik_baseline_mask |= bit;
        state.coverage.foot_ik_space_baseline =
            state.foot_ik_baseline_mask == FOOT_IK_BASELINE_MASK;
    }
    if state.scenario == IntegrationScenario::FootIk
        && state.phase == IntegrationPhase::FootIkNoHit
        && !state.foot_ik_no_hit_logged
        && state.phase_elapsed() >= 0.9
        && let Ok((_, space, _, _)) = foot_ik_data.get(character)
    {
        let left_delta = (space.left.target_armature_local
            - space.left.solver_reference_armature_local)
            .length();
        let right_delta = (space.right.target_armature_local
            - space.right.solver_reference_armature_local)
            .length();
        info!(
            "[MANUAL-INTEGRATION][FOOT-IK-NO-HIT-PROBE] locomotion={:?} left_hit={} right_hit={} left_raw={:.6} right_raw={:.6} left_delta={left_delta:.6} right_delta={right_delta:.6}",
            locomotion,
            space.left.has_ground_hit,
            space.right.has_ground_hit,
            space.left.raw_offset,
            space.right.raw_offset,
        );
        state.foot_ik_no_hit_logged = true;
    }
    if state.scenario == IntegrationScenario::FootIk
        && state.phase == IntegrationPhase::FootIkNoHit
        && state.phase_elapsed() >= 0.8
        && *locomotion == LocomotionState::Air
        && let Ok((_, space, _, _)) = foot_ik_data.get(character)
        && space.left.valid
        && space.right.valid
        && !space.left.has_ground_hit
        && !space.right.has_ground_hit
        && space.left.raw_offset.abs() <= 1e-6
        && space.right.raw_offset.abs() <= 1e-6
    {
        let left_delta = (space.left.target_armature_local
            - space.left.solver_reference_armature_local)
            .length();
        let right_delta = (space.right.target_armature_local
            - space.right.solver_reference_armature_local)
            .length();
        if left_delta <= 0.03 && right_delta <= 0.03 {
            if !state.coverage.foot_ik_no_hit_recovered {
                info!(
                    "[MANUAL-INTEGRATION][FOOT-IK-NO-HIT] left_delta={left_delta:.6} right_delta={right_delta:.6} left_target=({:.3},{:.3},{:.3}) right_target=({:.3},{:.3},{:.3})",
                    space.left.target_world.x,
                    space.left.target_world.y,
                    space.left.target_world.z,
                    space.right.target_world.x,
                    space.right.target_world.y,
                    space.right.target_world.z,
                );
            }
            state.coverage.foot_ik_no_hit_recovered = true;
        }
    }
    if state.scenario == IntegrationScenario::FootIk
        && state.phase == IntegrationPhase::FootIkWeightRamp
        && let Ok((_, _, apply, _)) = foot_ik_data.get(character)
    {
        let elapsed = state.phase_elapsed();
        let sample = if elapsed >= 0.5 && elapsed < 0.75 {
            Some((1 << 0, "off"))
        } else if elapsed >= 1.15 && elapsed < 1.5 {
            Some((1 << 1, "quarter"))
        } else if elapsed >= 1.9 && elapsed < 2.25 {
            Some((1 << 2, "half"))
        } else if elapsed >= 2.65 && elapsed < 3.0 {
            Some((1 << 3, "three_quarters"))
        } else if elapsed >= 3.4 && elapsed < 3.75 {
            Some((1 << 4, "full"))
        } else if elapsed >= 4.35 && elapsed < 4.7 {
            Some((1 << 5, "released"))
        } else {
            None
        };
        if let Some((bit, label)) = sample {
            let weights = (apply.left.applied_weight, apply.right.applied_weight);
            let expected = match bit {
                1 => weights.0 <= 0.05 && weights.1 <= 0.05,
                2 => (0.10..=0.40).contains(&weights.0) && (0.10..=0.40).contains(&weights.1),
                4 => (0.35..=0.65).contains(&weights.0) && (0.35..=0.65).contains(&weights.1),
                8 => (0.60..=0.90).contains(&weights.0) && (0.60..=0.90).contains(&weights.1),
                16 => weights.0 >= 0.95 && weights.1 >= 0.95,
                _ => weights.0 <= 0.05 && weights.1 <= 0.05,
            };
            if expected && state.foot_ik_weight_mask & bit == 0 {
                info!(
                    "[MANUAL-INTEGRATION][FOOT-IK-WEIGHT] phase={label} left={:.3} right={:.3} left_error={:.6} right_error={:.6}",
                    weights.0, weights.1, apply.left.effector_error, apply.right.effector_error,
                );
                state.foot_ik_weight_mask |= bit;
                state.coverage.foot_ik_weight_ramp =
                    state.foot_ik_weight_mask == FOOT_IK_WEIGHT_MASK;
            }
        }
    }
    if state.scenario == IntegrationScenario::FootIk
        && state.phase == IntegrationPhase::FootIkTeleport
        && let Ok((_, spaces, apply, Some(teleport))) = foot_ik_data.get(character)
    {
        if !state.foot_ik_teleport_requested {
            state.foot_ik_teleport_baseline = Some((
                teleport.detected_count,
                teleport.left_rebase_count,
                teleport.right_rebase_count,
            ));
        }
        if state.foot_ik_teleport_requested
            && let Some((detected_before, left_before, right_before)) =
                state.foot_ik_teleport_baseline
            && teleport.detected_count > detected_before
            && teleport.left_rebase_count > left_before
            && teleport.right_rebase_count > right_before
            && spaces.left.valid
            && spaces.right.valid
            && apply.left.valid
            && apply.right.valid
            && !state.coverage.foot_ik_teleport
        {
            info!(
                "[MANUAL-INTEGRATION][FOOT-IK-TELEPORT] delta_m={:.6} detected={} left_rebases={} right_rebases={} left_knee=({:.3},{:.3},{:.3}) right_knee=({:.3},{:.3},{:.3}) left_error={:.6} right_error={:.6}",
                teleport.last_translation_delta_m,
                teleport.detected_count,
                teleport.left_rebase_count,
                teleport.right_rebase_count,
                apply.left.knee_world.x,
                apply.left.knee_world.y,
                apply.left.knee_world.z,
                apply.right.knee_world.x,
                apply.right.knee_world.y,
                apply.right.knee_world.z,
                apply.left.effector_error,
                apply.right.effector_error,
            );
            state.coverage.foot_ik_teleport = true;
        }
    }
    if state.scenario == IntegrationScenario::FootIk
        && let Ok((_, spaces, apply, _)) = foot_ik_data.get(character)
        && spaces.left.valid
        && spaces.right.valid
        && apply.left.valid
        && apply.right.valid
    {
        if state.phase == IntegrationPhase::FootIkTerrain {
            let stable = apply.left.applied_weight >= 0.99
                && apply.right.applied_weight >= 0.99
                && apply.left.effector_error <= 0.02
                && apply.right.effector_error <= 0.02;
            if stable
                && (spaces.left.has_ground_hit || spaces.right.has_ground_hit)
                && (spaces.left.normal_world.y < 0.995 || spaces.right.normal_world.y < 0.995)
                && !state.coverage.foot_ik_slope
            {
                log_foot_ik_stage("slope", spaces, apply);
                state.coverage.foot_ik_slope = true;
            }
            if stable
                && state.phase_elapsed() >= 3.2
                && ((spaces.left.has_ground_hit
                    && spaces.left.hit_world.y > fixture.locomotion_start.y - 0.82)
                    || (spaces.right.has_ground_hit
                        && spaces.right.hit_world.y > fixture.locomotion_start.y - 0.82))
                && !state.coverage.foot_ik_step
            {
                log_foot_ik_stage("step", spaces, apply);
                state.coverage.foot_ik_step = true;
            }
            if stable && *stance == Stance::Crouching && !state.coverage.foot_ik_crouching {
                log_foot_ik_stage("crouching", spaces, apply);
                state.coverage.foot_ik_crouching = true;
            }
            if *locomotion == LocomotionState::Air {
                state.foot_ik_terrain_air_seen = true;
                if stable && !state.coverage.foot_ik_jump_air {
                    log_foot_ik_stage("jump_air", spaces, apply);
                    state.coverage.foot_ik_jump_air = true;
                }
            }
            if state.foot_ik_terrain_air_seen
                && *locomotion == LocomotionState::Grounded
                && stable
                && !state.coverage.foot_ik_landed
            {
                log_foot_ik_stage("landed", spaces, apply);
                state.coverage.foot_ik_landed = true;
            }
            if roll.is_some_and(|roll| roll.active) && stable && !state.coverage.foot_ik_roll {
                log_foot_ik_stage("roll", spaces, apply);
                state.coverage.foot_ik_roll = true;
            }
        }
        if state.phase == IntegrationPhase::Mantle
            && mantle.is_some_and(|mantle| mantle.active)
            && apply.left.applied_weight >= 0.99
            && apply.right.applied_weight >= 0.99
            && apply.left.effector_error <= 0.02
            && apply.right.effector_error <= 0.02
            && !state.coverage.foot_ik_mantle
        {
            log_foot_ik_stage("mantle", spaces, apply);
            state.coverage.foot_ik_mantle = true;
        }
        if state.phase == IntegrationPhase::Roll
            && roll.is_some_and(|roll| roll.active)
            && apply.left.applied_weight >= 0.99
            && apply.right.applied_weight >= 0.99
            && apply.left.effector_error <= 0.02
            && apply.right.effector_error <= 0.02
            && !state.coverage.foot_ik_roll
        {
            log_foot_ik_stage("roll", spaces, apply);
            state.coverage.foot_ik_roll = true;
        }
    }
    if let Some(base) = movement_base {
        state.coverage.movement_base |= base.entity == Some(fixture.platform)
            && base.support_velocity.length() > 0.25
            && base.relative_velocity.length() < 0.75;
    }
    if let Some(roll) = roll {
        state.coverage.roll_started |= roll.active;
    }
    if let Some(slot) = roll_slot {
        state.coverage.roll_slot |= slot.active
            && slot.pose_ready
            && slot.weight > 0.2
            && slot.root_motion_delta.length() > 1e-5;
    }
    if state.scenario == IntegrationScenario::Grounded && state.phase == IntegrationPhase::Roll {
        state.grounded_roll_exit_diagnostic = grounded_roll_exit_diagnostic(
            *stance,
            desired_stance.copied().unwrap_or(AlsDesiredStance(*stance)),
            roll,
            grounded_graph,
            transition_playback,
            root_transition_slot,
        );
    }
    if grounded_roll_exit_transition_matches_source(
        state.phase,
        roll,
        transition_playback,
        root_transition_slot,
        grounded_graph,
    ) && !state.coverage.grounded_roll_exit_transition
    {
        let playback = transition_playback.unwrap();
        info!(
            "[MANUAL-INTEGRATION][GROUNDED-ROLL-EXIT] stance=Standing kind={:?} blend_in={:.3} blend_out={:.3} play_rate={:.3} start_time={:.3} slot_weight={:.3} -> PASS",
            playback.kind,
            playback.blend_in,
            playback.blend_out,
            playback.play_rate,
            playback.start_time,
            root_transition_slot.unwrap().weight,
        );
        state.coverage.grounded_roll_exit_transition = true;
    }
    if let Some(spawned) = player.get_spawned_ragdoll() {
        let profile = ragdoll_probe.profiles.get(instance.player_entity()).ok();
        if let Some((applied, source_profile)) = profile {
            state.ragdoll_current_joint_anchor_error_m = applied.current_joint_anchor_error_m;
            state.ragdoll_current_joint_anchor_label = applied
                .current_joint_anchor_profile_index
                .and_then(|index| source_profile.joints.get(index as usize))
                .map(|joint| joint.label.clone());
            state.ragdoll_max_joint_anchor_error_m = applied.max_joint_anchor_error_m;
            state.ragdoll_max_joint_anchor_label = applied
                .max_joint_anchor_profile_index
                .and_then(|index| source_profile.joints.get(index as usize))
                .map(|joint| joint.label.clone());
            state.ragdoll_handoff_locked_joint_anchor_error_m =
                applied.handoff_locked_joint_anchor_error_m;
            state.ragdoll_handoff_locked_joint_anchor_label = applied
                .handoff_locked_joint_anchor_profile_index
                .and_then(|index| source_profile.joints.get(index as usize))
                .map(|joint| joint.label.clone());
            state.ragdoll_handoff_pose_error_m = applied.handoff_animated_body_position_error_m;
            state.ragdoll_handoff_rotation_error_radians =
                applied.handoff_animated_body_rotation_error_radians;
        }
        let profile_valid = profile.is_some_and(|(applied, _)| {
            applied.body_count == 21
                && applied.collider_count == 22
                && applied.joint_count == 20
                && applied.source_joint_count == 19
                && applied.collision_disabled_pair_count == 49
                && applied.handoff_animated_body_count == applied.body_count
                && applied.handoff_animated_body_position_error_m <= 1e-4
                && applied.handoff_animated_body_rotation_error_radians <= 1e-4
        }) && spawned.colliders.values().all(|collider| {
            ragdoll_probe
                .collision_members
                .get(*collider)
                .is_ok_and(|member| {
                    member.group == spawned.root && member.disabled_pairs.len() == 49
                })
        });
        let mut dynamic = 0usize;
        let mut root_kinematic = false;
        for body in spawned.bodies.values() {
            if ragdoll_probe
                .labels
                .get(*body)
                .is_ok_and(|label| label.0 == "root")
            {
                root_kinematic = ragdoll_probe
                    .body_modes
                    .get(*body)
                    .is_ok_and(|mode| *mode == RigidBody::Kinematic);
            } else if ragdoll_probe
                .body_modes
                .get(*body)
                .is_ok_and(|mode| *mode == RigidBody::Dynamic)
            {
                dynamic += 1;
            }
        }
        state.coverage.ragdoll_pa_profile |= profile_valid
            && (*locomotion != LocomotionState::Ragdolling || (dynamic == 20 && root_kinematic));
    }
    if *locomotion == LocomotionState::Ragdolling {
        state.coverage.ragdoll_entered = true;
        state.ragdoll_seen = true;
        state.ragdoll_frames = state.ragdoll_frames.saturating_add(1);
        if let Ok((sockets, actor, collider)) = ragdoll_probe.camera_sockets.get(character) {
            use avian3d::prelude::SimpleCollider;
            let bounds = collider.aabb(actor.translation(), actor.rotation());
            let bottom = actor.translation() - Vec3::Y * bounds.size().y * 0.5;
            state.coverage.ragdoll_camera_pivot |= sockets.valid
                && sockets.root.distance(bottom) < 1e-4
                && cameras.iter().any(|(_, camera)| {
                    camera.pivot_target.distance((bottom + sockets.head) * 0.5) < 1e-4
                });
        }
    }
    if let Some(follow) = ragdoll_follow {
        if state.ragdoll_seen && follow.grounded {
            state.coverage.ragdoll_follow = true;
            if state.ragdoll_grounded_at.is_none() {
                state.ragdoll_grounded_at = Some(state.game_time_seconds);
                info!("[MANUAL-INTEGRATION] ragdoll ground follow stabilized");
            }
        } else if state.ragdoll_seen {
            state.ragdoll_grounded_at = None;
        }
    }
    if let Some(drive) = joint_drive {
        state.coverage.ragdoll_joint_drive |= state.ragdoll_seen
            && drive.target_joint_count == 19
            && (drive.angular_stiffness - drive.speed_amount * RAGDOLL_ANGULAR_STIFFNESS).abs()
                <= 1e-3
            && drive.angular_damping.abs() <= 1e-6
            && drive.angular_force_limit.abs() <= 1e-6;
    }
    let ragdoll_recovered = state.ragdoll_seen
        && state.ragdoll_exit_requested
        && *locomotion == LocomotionState::Grounded
        && velocity.0.length() < 0.5;
    if ragdoll_recovered {
        state.coverage.ragdoll_recovered = true;
        if state.ragdoll_recovered_at.is_none() {
            state.ragdoll_recovered_at = Some(state.game_time_seconds);
            info!(
                "[MANUAL-INTEGRATION] ragdoll recovered position=({:.3},{:.3},{:.3}) velocity={:.3}",
                transform.translation.x,
                transform.translation.y,
                transform.translation.z,
                velocity.0.length()
            );
        }
    }
    if state.phase == IntegrationPhase::Ragdoll {
        if let Ok((Some(get_up), blocked)) = ragdoll_probe.get_ups.get(character) {
            let expected_facing_up = if state.scenario == IntegrationScenario::GetUp {
                state.get_up_case == 1
            } else {
                get_up.facing_up
            };
            let facing_matches = get_up.facing_up == expected_facing_up;
            let active = facing_matches
                && get_up.pose_ready
                && graph_global_f32(player, "post_locomotion.weight")
                    .is_some_and(|weight| weight > 0.0);
            if active {
                state.coverage.get_up_front |= !get_up.facing_up;
                state.coverage.get_up_back |= get_up.facing_up;
                state.coverage.get_up_input_locked[usize::from(get_up.facing_up)] |=
                    blocked && notify_action == Some(&AlsNotifyLocomotionAction::GettingUp);
                state.coverage.get_up_input_released[usize::from(get_up.facing_up)] |=
                    !blocked && notify_action == Some(&AlsNotifyLocomotionAction::None);
            }
            let variant = if get_up.facing_up {
                "GetUpBack"
            } else {
                "GetUpFront"
            };
            **animation_window = ManualAnimationWindowEvidence {
                label: Some(if get_up.facing_up {
                    "get_up_back"
                } else {
                    "get_up_front"
                }),
                index: Some(usize::from(get_up.facing_up)),
                settled: active && (0.4..=0.8).contains(&get_up.clip_time),
                capture_source_window: true,
                requires_pre_rig_motion: true,
                requires_final_pose_motion: true,
                requires_non_reference_root_pose: true,
                source_node: Some("AB_Als/Slot 'PostLocomotion'"),
                source_state: Some("GettingUp"),
                expected_animation_variant: Some(if expected_facing_up {
                    "GetUpBack"
                } else {
                    "GetUpFront"
                }),
                observed_animation_variant: Some(variant),
                observed_animation_clip_time: Some(get_up.clip_time),
                observed_animation_weight: Some(get_up.weight),
                observed_animation_play_rate: Some(
                    crate::get_up::get_up_settings(get_up.facing_up).play_rate,
                ),
                direction_signature_group: (state.scenario == IntegrationScenario::GetUp)
                    .then_some("PostLocomotion GetUp"),
                required_direction_signature_members: 2,
                minimum_direction_signature_rotation_radians: 0.25,
                expected_direction: Some(if expected_facing_up { "Back" } else { "Front" }),
                observed_direction: Some(if get_up.facing_up { "Back" } else { "Front" }),
                canonical_clip_time: Some(0.6),
                maximum_clip_time_distance: 0.1,
                minimum_observation_seconds: 0.1,
                ..default()
            };
        }
    }

    if state.scenario == IntegrationScenario::Interface {
        let hud_nodes_hidden = !interface.hud_debug_nodes.is_empty()
            && !interface.hud_input_nodes.is_empty()
            && interface
                .hud_debug_nodes
                .iter()
                .all(|node| node.display == Display::None)
            && interface
                .hud_input_nodes
                .iter()
                .all(|node| node.display == Display::None);
        let hud_nodes_visible = !interface.hud_debug_nodes.is_empty()
            && !interface.hud_input_nodes.is_empty()
            && interface
                .hud_debug_nodes
                .iter()
                .all(|node| node.display != Display::None)
            && interface
                .hud_input_nodes
                .iter()
                .all(|node| node.display != Display::None);
        state.coverage.hud_layout |= !interface.hud_roots.is_empty()
            && !interface.hud_debug_nodes.is_empty()
            && !interface.hud_input_nodes.is_empty();
        state.interface_hud_hidden_seen |= !interface.hud_state.ui_visible && hud_nodes_hidden;
        state.interface_hud_restored_seen |=
            state.interface_hud_hidden_seen && interface.hud_state.ui_visible && hud_nodes_visible;
        state.coverage.hud_toggle =
            state.interface_hud_hidden_seen && state.interface_hud_restored_seen;

        state.interface_slomo_seen |= interface.hud_state.slomo_active
            && (interface.virtual_time.relative_speed() - 0.15).abs() < 1e-4;
        state.interface_slomo_restored_seen |= state.interface_slomo_seen
            && !interface.hud_state.slomo_active
            && (interface.virtual_time.relative_speed() - 1.0).abs() < 1e-4;
        state.coverage.hud_slomo =
            state.interface_slomo_seen && state.interface_slomo_restored_seen;

        state.interface_menu_open_seen |= interface.menu_state.open
            && interface.menu_state.root.is_some()
            && (interface.virtual_time.relative_speed() - 0.35).abs() < 1e-4;
        state.interface_menu_closed_seen |= state.interface_menu_open_seen
            && !interface.menu_state.open
            && interface.menu_state.root.is_none()
            && (interface.virtual_time.relative_speed() - 1.0).abs() < 1e-4;
        state.coverage.overlay_menu =
            state.interface_menu_open_seen && state.interface_menu_closed_seen;

        let selector = graph_global_f32(player, "als_root_overlay_index");
        let rifle_bound = *overlay == OverlayState::Rifle
            && selector.is_some_and(|value| (value - 5.0).abs() <= 1e-4)
            && overlay_object.is_some_and(|value| {
                value.visible && value.applied_overlay == Some(OverlayState::Rifle)
            });
        let default_bound = *overlay == OverlayState::Default
            && selector.is_some_and(|value| value.abs() <= 1e-4)
            && overlay_object.is_none_or(|value| !value.visible);
        if rifle_bound && !state.interface_overlay_rifle_seen {
            info!(
                "[MANUAL-INTEGRATION][ALI] selector=5 state=Rifle object_visible=true binding=PASS"
            );
        }
        if state.interface_overlay_rifle_seen
            && default_bound
            && !state.interface_overlay_default_after_rifle_seen
        {
            info!(
                "[MANUAL-INTEGRATION][ALI] selector=0 state=Default object_visible=false fallback=PASS"
            );
        }
        if state.interface_overlay_default_after_rifle_seen
            && rifle_bound
            && !state.interface_overlay_reentry_seen
        {
            info!("[MANUAL-INTEGRATION][ALI] selector=5 state=Rifle reentry=PASS");
        }
        state.interface_overlay_rifle_seen |= rifle_bound;
        state.interface_overlay_default_after_rifle_seen |=
            state.interface_overlay_rifle_seen && default_bound;
        state.interface_overlay_reentry_seen |=
            state.interface_overlay_default_after_rifle_seen && rifle_bound;
        state.coverage.overlay_interface_binding = state.interface_overlay_rifle_seen;
        state.coverage.overlay_interface_fallback =
            state.interface_overlay_default_after_rifle_seen;
        state.coverage.overlay_interface_reentry = state.interface_overlay_reentry_seen;

        let all_debug_categories_active = DebugCategory::ALL
            .into_iter()
            .all(|category| interface.show_debug.is_active(category));
        state.coverage.show_debug |= state.interface_debug_armed
            && all_debug_categories_active
            && !interface.show_debug_roots.is_empty();
        state.coverage.debug_draw |= state.coverage.show_debug && interface.debug_stats.total() > 0;
    }

    if let Some(camera) = state.camera
        && let Ok((_camera_transform, debug)) = cameras.get(camera)
    {
        let camera_debug = debug;
        state.coverage.camera_rig |= camera_debug.valid;
        if state.phase == IntegrationPhase::Camera
            && state.camera_scenario_armed
            && state.camera_obstacles.iter().any(Option::is_some)
            && camera_debug.trace_blocking_hit
            && camera_debug.trace_distance_ratio < 0.98
            && camera_debug.trace_adjustment_count >= 2
            && camera_debug.trace_adjustment_resolved
        {
            if !state.coverage.camera_collision {
                info!(
                    "[MANUAL-INTEGRATION][CAMERA][MTD] blocking={} adjustment_count={} resolved={} ratio={:.3}",
                    camera_debug.trace_blocking_hit,
                    camera_debug.trace_adjustment_count,
                    camera_debug.trace_adjustment_resolved,
                    camera_debug.trace_distance_ratio
                );
            }
            state.coverage.camera_collision = true;
            state.camera_blocked_ratio = Some(camera_debug.trace_distance_ratio);
        }
        if state.phase == IntegrationPhase::Camera
            && state.camera_scenario_armed
            && state.camera_obstacles.iter().any(Option::is_some)
        {
            let elapsed = state.phase_elapsed();
            if elapsed - state.camera_last_probe_secs >= 0.5 {
                state.camera_last_probe_secs = elapsed;
                info!(
                    "[MANUAL-INTEGRATION][CAMERA] blocking={} adjusted={} adjustment_count={} resolved={} ratio={:.3} first_person={:.3}",
                    camera_debug.trace_blocking_hit,
                    camera_debug.trace_adjusted_start,
                    camera_debug.trace_adjustment_count,
                    camera_debug.trace_adjustment_resolved,
                    camera_debug.trace_distance_ratio,
                    camera_debug.first_person_override
                );
            }
        }
        if state.camera_obstacle_removed
            && state
                .camera_blocked_ratio
                .is_some_and(|blocked| camera_debug.trace_distance_ratio > blocked + 0.02)
            && !camera_debug.trace_blocking_hit
        {
            state.coverage.camera_recovery = true;
        }
        state.coverage.camera_first_person |=
            first_person && camera_debug.first_person_override > 0.5;
        state.coverage.camera_shoulder |= left_shoulder && !camera_debug.right_shoulder;
        state.coverage.camera_shake |= camera_debug.shake_location_offset.length() > 1e-5
            || camera_debug.shake_rotation_degrees.length() > 1e-5;
        if state.dilation_active && real_time.delta_secs() > 1e-5 {
            let ratio = camera_debug.unscaled_delta_seconds / real_time.delta_secs();
            state.coverage.camera_unscaled_time |= (ratio - 1.0).abs() < 0.25;
        }
    }

    if state.phase == IntegrationPhase::Finish && !state.final_logged {
        state.final_logged = true;
        let grounded_roll_exit_detail = state.grounded_roll_exit_diagnostic.clone();
        let generated_transition_notify_detail = motion_probe
            .generated_transition_notify
            .get(character)
            .ok()
            .flatten()
            .map_or_else(
                || "generated transition telemetry is missing".to_owned(),
                |telemetry| {
                    format!(
                        "stop_quick_count={} observed_named_fsm_state_ids={} active_named_source_state_count={} last_active_named_source_state_id={} active_named_transition_count={} last_active_named_transition_id={}",
                        telemetry.stop_quick_count,
                        telemetry.observed_named_fsm_state_ids.join(","),
                        telemetry.active_named_source_state_count,
                        telemetry.last_active_named_source_state_id,
                        telemetry.active_named_transition_count,
                        telemetry.last_active_named_transition_id
                    )
                },
            );
        let coverage = &state.coverage;
        let direction_regression = coverage.direction_cases.iter().all(|covered| *covered);
        if matches!(
            state.scenario,
            IntegrationScenario::Base | IntegrationScenario::StandingDirection
        ) {
            for index in 0..DIRECTION_CASE_COUNT {
                if !coverage.direction_cases[index]
                    && let Some(case) = direction_regression_case(index)
                {
                    error!(
                        "[MANUAL-INTEGRATION][DIRECTION][NOT_COVERED] case={}",
                        case.name
                    );
                }
            }
        }
        if matches!(
            state.scenario,
            IntegrationScenario::Base | IntegrationScenario::Crouching
        ) {
            for index in 0..CROUCH_DIRECTION_CASE_COUNT {
                if !coverage.crouch_direction_cases[index]
                    && let Some(case) = crouch_direction_regression_case(index)
                {
                    error!(
                        "[MANUAL-INTEGRATION][CROUCH-DIRECTION][NOT_COVERED] case={}",
                        case.name
                    );
                }
            }
        }
        let results = match state.scenario {
            IntegrationScenario::StandingDirection => vec![
                ("runtime_assembly", coverage.assembly),
                ("animation_graph", coverage.graph),
                ("standing_graph", coverage.standing_graph),
                ("direction_regression", direction_regression),
            ],
            IntegrationScenario::Base => vec![
                ("runtime_assembly", coverage.assembly),
                ("animation_graph", coverage.graph),
                ("locomotion", coverage.locomotion),
                ("direction_backward", coverage.backward),
                ("direction_sideways", coverage.sideways),
                ("standing_graph", coverage.standing_graph),
                ("direction_regression", direction_regression),
                ("crouch_transaction", coverage.crouch_transaction),
                ("crouch_graph", coverage.crouch_graph),
                (
                    "crouch_air_forced_standing",
                    coverage.crouch_air_forced_standing,
                ),
                ("crouch_air_landed", coverage.crouch_air_landed),
                ("crouch_graph_recovered", coverage.crouch_graph_recovered),
                ("crouch_cycles_100", coverage.crouch_cycles),
                ("crouch_canopy_visible", coverage.crouch_canopy_visible),
                ("crouch_canopy_blocked", coverage.crouch_canopy_blocked),
                ("crouch_canopy_recovered", coverage.crouch_canopy_recovered),
                ("crouch_canopy_cleaned", coverage.crouch_canopy_cleaned),
                ("crouch_moving", coverage.crouch_moving),
                ("stance_crouching", coverage.crouching),
                (
                    "crouch_direction_six_way",
                    coverage
                        .crouch_direction_cases
                        .iter()
                        .all(|covered| *covered)
                        && coverage.crouch_direction_state_mask == 0b11_1111,
                ),
                ("crouch_direction_lean", coverage.crouch_direction_lean),
                (
                    "crouch_rotate_both_sides",
                    coverage.crouch_rotate_mask == 0b11
                        && coverage.crouch_rotate_completion_mask == 0b11,
                ),
                (
                    "crouch_turn_slot_active_both_sides",
                    coverage.crouch_turn_active_mask == 0b11,
                ),
                (
                    "crouch_turn_completion_both_sides",
                    coverage.crouch_turn_completion_mask == 0b11,
                ),
                ("gait_sprinting", coverage.sprinting),
                ("jump_air", coverage.airborne),
                ("jump_land", coverage.landed),
                ("jump_variant", coverage.jump_variant),
                ("overlay_layer", coverage.overlay),
                ("gait_layer", coverage.gait_layer),
                ("stance_layer", coverage.stance_layer),
                ("turn_in_place", coverage.turn_in_place),
                ("layering", coverage.layering),
                ("overlay_object", coverage.overlay_object),
                ("start_stop", coverage.start_stop),
                ("view_head", coverage.view_head),
                ("control_rig", coverage.control_rig),
                ("control_rig_stages", coverage.control_rig_stages),
                ("control_rig_input", coverage.control_rig_input),
                (
                    "control_rig_foot_targets",
                    coverage.control_rig_foot_targets,
                ),
                ("mantle_start", coverage.mantle_started),
                ("mantle_complete", coverage.mantle_completed),
                ("movement_base", coverage.movement_base),
                ("foot_probe", coverage.foot_probe),
                ("foot_lock", coverage.foot_lock),
                ("roll_start", coverage.roll_started),
                ("roll_slot", coverage.roll_slot),
                (
                    "grounded_roll_exit_transition",
                    coverage.grounded_roll_exit_transition,
                ),
            ],
            IntegrationScenario::Grounded => vec![
                ("runtime_assembly", coverage.assembly),
                ("animation_graph", coverage.graph),
                ("roll_start", coverage.roll_started),
                ("roll_slot", coverage.roll_slot),
                (
                    "grounded_roll_exit_transition",
                    coverage.grounded_roll_exit_transition,
                ),
            ],
            IntegrationScenario::StateMachine => vec![
                ("runtime_assembly", coverage.assembly),
                ("animation_graph", coverage.graph),
                ("p1_ground_idle_start_cycle_stop_pivot", coverage.p1_ground),
                ("p1_stance_crouch_and_recover", coverage.p1_stance),
                ("p1_gait_walk_run_sprint_guard", coverage.p1_gait),
                ("p1_air_jump_fall_land", coverage.p1_air),
                ("p1_turn_and_rotate_both_sides", coverage.p1_turn),
                ("p1_turn_slot_pose", coverage.turn_slot),
                (
                    "p1_turn_slot_inertialization",
                    coverage.turn_inertialization,
                ),
                (
                    "generated_stop_quick_transition_notify",
                    coverage.generated_transition_stop_quick,
                ),
            ],
            IntegrationScenario::GeneratedTransitionNotify => vec![
                ("runtime_assembly", coverage.assembly),
                ("animation_graph", coverage.graph),
                (
                    "generated_stop_quick_transition_notify",
                    coverage.generated_transition_stop_quick,
                ),
            ],
            IntegrationScenario::Crouching => vec![
                ("runtime_assembly", coverage.assembly),
                ("animation_graph", coverage.graph),
                (
                    "crouch_direction_six_way",
                    coverage
                        .crouch_direction_cases
                        .iter()
                        .all(|covered| *covered)
                        && coverage.crouch_direction_state_mask == 0b11_1111,
                ),
                ("crouch_direction_lean", coverage.crouch_direction_lean),
                (
                    "crouch_rotate_both_sides",
                    coverage.crouch_rotate_mask == 0b11
                        && coverage.crouch_rotate_completion_mask == 0b11,
                ),
                (
                    "crouch_turn_slot_active_both_sides",
                    coverage.crouch_turn_active_mask == 0b11,
                ),
                (
                    "crouch_turn_completion_both_sides",
                    coverage.crouch_turn_completion_mask == 0b11,
                ),
            ],
            IntegrationScenario::Sprint => vec![
                ("runtime_assembly", coverage.assembly),
                ("animation_graph", coverage.graph),
                ("standing_graph", coverage.standing_graph),
                ("locomotion", coverage.locomotion),
                ("gait_sprinting", coverage.sprinting),
                ("sprint_curve_relay", coverage.sprint_curve_relay),
            ],
            IntegrationScenario::ControlRig => vec![
                ("runtime_assembly", coverage.assembly),
                ("animation_graph", coverage.graph),
                ("standing_graph", coverage.standing_graph),
                ("control_rig", coverage.control_rig),
                ("control_rig_stages", coverage.control_rig_stages),
                ("control_rig_input", coverage.control_rig_input),
                (
                    "control_rig_foot_targets",
                    coverage.control_rig_foot_targets,
                ),
                ("foot_probe", coverage.foot_probe),
                ("foot_lock", coverage.foot_lock),
            ],
            IntegrationScenario::DefaultOverlay => vec![
                ("runtime_assembly", coverage.assembly),
                ("animation_graph", coverage.graph),
                ("default_overlay_idle", coverage.default_overlay_idle),
                ("default_overlay_moving", coverage.default_overlay_moving),
                (
                    "default_overlay_crouching",
                    coverage.default_overlay_crouching,
                ),
                ("default_overlay_air", coverage.default_overlay_air),
                ("default_overlay_curves", coverage.default_overlay_curves),
            ],
            IntegrationScenario::FootIk => vec![
                ("runtime_assembly", coverage.assembly),
                ("animation_graph", coverage.graph),
                ("standing_graph", coverage.standing_graph),
                ("direction_regression", direction_regression),
                ("foot_probe", coverage.foot_probe),
                ("foot_lock", coverage.foot_lock),
                ("foot_ik_space_baseline", coverage.foot_ik_space_baseline),
                (
                    "foot_ik_no_hit_recovered",
                    coverage.foot_ik_no_hit_recovered,
                ),
                ("foot_ik_weight_ramp", coverage.foot_ik_weight_ramp),
                ("foot_ik_teleport", coverage.foot_ik_teleport),
                ("foot_ik_slope", coverage.foot_ik_slope),
                ("foot_ik_step", coverage.foot_ik_step),
                ("foot_ik_crouching", coverage.foot_ik_crouching),
                ("foot_ik_jump_air", coverage.foot_ik_jump_air),
                ("foot_ik_landed", coverage.foot_ik_landed),
                ("foot_ik_roll", coverage.foot_ik_roll),
                ("foot_ik_mantle", coverage.foot_ik_mantle),
                ("foot_ik_terrain_cleaned", coverage.foot_ik_terrain_cleaned),
            ],
            IntegrationScenario::Ragdoll => vec![
                ("runtime_assembly", coverage.assembly),
                ("animation_graph", coverage.graph),
                ("ragdoll_enter", coverage.ragdoll_entered),
                ("ragdoll_pa_profile", coverage.ragdoll_pa_profile),
                ("ragdoll_follow", coverage.ragdoll_follow),
                ("ragdoll_joint_drive", coverage.ragdoll_joint_drive),
                ("ragdoll_recover", coverage.ragdoll_recovered),
                ("ragdoll_camera_pivot", coverage.ragdoll_camera_pivot),
                (
                    "get_up_input_locked",
                    coverage.get_up_input_locked.into_iter().any(|value| value),
                ),
                (
                    "get_up_input_released",
                    coverage
                        .get_up_input_released
                        .into_iter()
                        .any(|value| value),
                ),
            ],
            IntegrationScenario::GetUp => vec![
                ("runtime_assembly", coverage.assembly),
                ("animation_graph", coverage.graph),
                ("ragdoll_pa_profile", coverage.ragdoll_pa_profile),
                ("ragdoll_recover", coverage.ragdoll_recovered),
                ("ragdoll_camera_pivot", coverage.ragdoll_camera_pivot),
                ("get_up_front", coverage.get_up_front),
                ("get_up_back", coverage.get_up_back),
                ("get_up_front_input_locked", coverage.get_up_input_locked[0]),
                (
                    "get_up_front_input_released",
                    coverage.get_up_input_released[0],
                ),
                ("get_up_back_input_locked", coverage.get_up_input_locked[1]),
                (
                    "get_up_back_input_released",
                    coverage.get_up_input_released[1],
                ),
            ],
            IntegrationScenario::Camera => vec![
                ("runtime_assembly", coverage.assembly),
                ("animation_graph", coverage.graph),
                ("camera_rig", coverage.camera_rig),
                ("camera_collision", coverage.camera_collision),
                ("camera_collision_recovery", coverage.camera_recovery),
                ("camera_first_person", coverage.camera_first_person),
                ("camera_shoulder", coverage.camera_shoulder),
                ("camera_shake", coverage.camera_shake),
                ("camera_unscaled_time", coverage.camera_unscaled_time),
            ],
            IntegrationScenario::Interface => vec![
                ("runtime_assembly", coverage.assembly),
                ("animation_graph", coverage.graph),
                ("hud_layout", coverage.hud_layout),
                ("hud_toggle", coverage.hud_toggle),
                ("hud_slomo", coverage.hud_slomo),
                ("overlay_menu", coverage.overlay_menu),
                (
                    "overlay_interface_binding",
                    coverage.overlay_interface_binding,
                ),
                (
                    "overlay_interface_fallback",
                    coverage.overlay_interface_fallback,
                ),
                (
                    "overlay_interface_reentry",
                    coverage.overlay_interface_reentry,
                ),
                ("show_debug", coverage.show_debug),
                ("debug_draw", coverage.debug_draw),
            ],
            IntegrationScenario::Notify => vec![
                ("runtime_assembly", coverage.assembly),
                ("animation_graph", coverage.graph),
                ("notify_sprint", coverage.notify_sprint),
                (
                    "notify_sprint_acceleration",
                    coverage.notify_sprint_acceleration,
                ),
                ("notify_roll", coverage.notify_roll),
                ("notify_marker", coverage.notify_marker),
                ("notify_grounded_entry", coverage.notify_grounded_entry),
                ("notify_root_scale", coverage.notify_root_scale),
                ("notify_rolling_action", coverage.notify_rolling_action),
                ("notify_mantling_action", coverage.notify_mantling_action),
                ("notify_early_blend_out", coverage.notify_early_blend_out),
                ("camera_shake", coverage.camera_shake),
            ],
        };
        for (name, covered) in &results {
            if *covered {
                info!("[MANUAL-INTEGRATION][EXECUTED] {name}");
            } else {
                error!("[MANUAL-INTEGRATION][NOT_COVERED] {name}");
            }
        }
        let passed = !state.timed_out && results.iter().all(|(_, covered)| *covered);
        let ragdoll_profile_detail = format!(
            "handoff_animated_body_position_error_m={:.6}; handoff_animated_body_rotation_error_radians={:.6}; handoff_locked_joint_anchor_error_m={:.6}; handoff_locked_joint_anchor_label={}; current_joint_anchor_error_m={:.6}; current_joint_anchor_label={}; max_joint_anchor_error_m={:.6}; max_joint_anchor_label={}",
            state.ragdoll_handoff_pose_error_m,
            state.ragdoll_handoff_rotation_error_radians,
            state.ragdoll_handoff_locked_joint_anchor_error_m,
            state
                .ragdoll_handoff_locked_joint_anchor_label
                .as_deref()
                .unwrap_or("none"),
            state.ragdoll_current_joint_anchor_error_m,
            state
                .ragdoll_current_joint_anchor_label
                .as_deref()
                .unwrap_or("none"),
            state.ragdoll_max_joint_anchor_error_m,
            state
                .ragdoll_max_joint_anchor_label
                .as_deref()
                .unwrap_or("none"),
        );
        gate_report.scenario = format!("{:?}", state.scenario);
        gate_report.timed_out = state.timed_out;
        gate_report.completed = true;
        gate_report.passed = Some(passed);
        gate_report.results = results
            .iter()
            .map(|(name, covered)| ManualIntegrationGateResult {
                name: (*name).to_owned(),
                covered: *covered,
                detail: if *name == "grounded_roll_exit_transition" {
                    grounded_roll_exit_detail.clone()
                } else if *name == "generated_stop_quick_transition_notify" {
                    generated_transition_notify_detail.clone()
                } else if *name == "ragdoll_pa_profile" {
                    ragdoll_profile_detail.clone()
                } else if *covered {
                    "covered".to_owned()
                } else {
                    "not covered".to_owned()
                },
            })
            .collect();
        if passed {
            info!(
                "[MANUAL-INTEGRATION][VERDICT] scenario={:?} real_entry_paths -> PASS awaiting_visual_evidence",
                state.scenario
            );
        } else {
            error!(
                "[MANUAL-INTEGRATION][VERDICT] scenario={:?} real_entry_paths -> FAIL timed_out={} awaiting_visual_evidence",
                state.scenario, state.timed_out
            );
        }
    }
    if state.phase == IntegrationPhase::Finish
        && state.final_logged
        && !state.exit_requested
        && manual_gate_exit_ready(interface.visual_progress.as_deref(), state.timed_out)
    {
        state.exit_requested = true;
        if gate_report.passed == Some(true) {
            exit.write(AppExit::Success);
        } else {
            exit.write(AppExit::Error(NonZeroU8::new(91).unwrap()));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_up_fixture_covers_both_source_facing_branches() {
        for yaw in [-0.7, 0.0, 1.2] {
            for facing_up in [false, true] {
                assert_eq!(
                    crate::ragdoll_link::ragdoll_recovery_rotation(get_up_fixture_rotation(
                        facing_up, yaw
                    ))
                    .1,
                    facing_up
                );
            }
        }
    }

    #[derive(Resource, Default)]
    struct BridgeOrderProbe {
        published: u8,
        observed: u8,
    }

    fn publish_bridge_order_probe(mut probe: ResMut<BridgeOrderProbe>) {
        probe.published = 1;
    }

    fn observe_bridge_order_probe(mut probe: ResMut<BridgeOrderProbe>) {
        probe.observed = probe.published;
    }

    #[test]
    fn grounded_scenario_starts_with_the_source_roll_chain() {
        assert_eq!(
            initial_phase(IntegrationScenario::Grounded),
            IntegrationPhase::Roll
        );
    }

    #[test]
    fn generated_transition_notify_starts_at_the_source_stop_window() {
        assert_eq!(
            initial_phase(IntegrationScenario::GeneratedTransitionNotify),
            IntegrationPhase::GeneratedTransitionNotify
        );
    }

    #[test]
    fn generated_transition_notify_releases_on_actual_idle_to_move_transition() {
        assert_eq!(
            generated_stop_quick_input_step(false, false, None),
            (false, false, false)
        );
        let idle = AlsStandingGraphTelemetry {
            actual_standing_state: Some(0),
            actual_standing_pose_ready: Some(true),
            ..default()
        };
        assert_eq!(
            generated_stop_quick_input_step(false, false, Some(&idle)),
            (true, false, true)
        );
        let transitioning = AlsStandingGraphTelemetry {
            actual_standing_transition_active: Some(true),
            ..default()
        };
        assert_eq!(
            generated_stop_quick_input_step(true, false, Some(&transitioning)),
            (true, true, false)
        );
    }

    #[test]
    fn manual_exit_waits_for_visual_capture_completion() {
        assert!(!manual_gate_exit_ready(
            Some(&AnimationVisualValidationProgress {
                evidence_ready: false,
            }),
            false,
        ));
        assert!(manual_gate_exit_ready(
            Some(&AnimationVisualValidationProgress {
                evidence_ready: true,
            }),
            false,
        ));
        assert!(manual_gate_exit_ready(None, false));
        assert!(manual_gate_exit_ready(
            Some(&AnimationVisualValidationProgress {
                evidence_ready: false,
            }),
            true,
        ));
    }

    #[test]
    fn grounded_roll_exit_probe_requires_source_transition_and_slot_pose() {
        let roll = AlsRollState::default();
        let playback = AlsTransitionPlayback::standing_transition_right();
        let slot = AlsRootTransitionSlotRuntime {
            active: true,
            weight: 0.5,
            pose_ready: true,
        };
        let grounded = AlsGroundedGraphTelemetry {
            actual_state: Some(1),
            transition_active: Some(false),
            pose_ready: Some(true),
        };
        assert!(grounded_roll_exit_transition_matches_source(
            IntegrationPhase::Roll,
            Some(&roll),
            Some(&playback),
            Some(&slot),
            Some(&grounded),
        ));
        assert!(!grounded_roll_exit_transition_matches_source(
            IntegrationPhase::Roll,
            Some(&roll),
            Some(&playback),
            None,
            Some(&grounded),
        ));
        assert!(!grounded_roll_exit_transition_matches_source(
            IntegrationPhase::Locomotion,
            Some(&roll),
            Some(&playback),
            Some(&slot),
            Some(&grounded),
        ));
    }

    #[test]
    fn fixture_rotation_reinitialize_clears_stale_response_state() {
        let mut rotation_response = AlsRotationResponseState {
            initialized: true,
            smooth_target_yaw: 1.25,
            previous_view_yaw: -0.75,
            ..Default::default()
        };
        let mut aiming_yaw_constraint = AlsAimingYawConstraint {
            active_limit_degrees: 70.0,
            applied_last_frame: true,
        };
        let mut rotation_animation = AlsRotationAnimationState {
            yaw_offset_degrees: 12.0,
            yaw_speed_degrees: 90.0,
        };
        reset_fixture_rotation_response_state(
            &mut rotation_response,
            &mut aiming_yaw_constraint,
            Some(&mut rotation_animation),
        );
        assert!(!rotation_response.initialized);
        assert_eq!(rotation_response.smooth_target_yaw, 0.0);
        assert_eq!(rotation_response.previous_view_yaw, 0.0);
        assert_eq!(aiming_yaw_constraint, AlsAimingYawConstraint::default());
        assert_eq!(rotation_animation, AlsRotationAnimationState::default());
    }

    #[test]
    fn slot_lifecycle_observer_runs_after_the_bridge_set() {
        let mut app = App::new();
        app.init_resource::<BridgeOrderProbe>()
            .configure_sets(Update, AlsTurnSlotBridgeSet)
            .add_systems(
                Update,
                (
                    publish_bridge_order_probe.in_set(AlsTurnSlotBridgeSet),
                    observe_bridge_order_probe.after(AlsTurnSlotBridgeSet),
                ),
            );
        app.update();
        assert_eq!(app.world().resource::<BridgeOrderProbe>().observed, 1);
    }

    #[test]
    fn direction_regression_covers_source_state_sequence_and_sprint_guards() {
        let cases: Vec<_> = (0..DIRECTION_CASE_COUNT)
            .map(direction_regression_case)
            .collect();
        assert!(cases.iter().all(Option::is_some));
        assert_eq!(
            cases[0].as_ref().unwrap().branch_mask,
            DIRECTION_BRANCH_FORWARD
        );
        assert_eq!(
            cases[1].as_ref().unwrap().branch_mask,
            DIRECTION_BRANCH_BACKWARD
        );
        assert_eq!(
            cases[2].as_ref().unwrap().branch_mask,
            DIRECTION_BRANCH_RIGHT
        );
        assert_eq!(
            cases[3].as_ref().unwrap().branch_mask,
            DIRECTION_BRANCH_LEFT
        );
        assert_eq!(
            cases[8].as_ref().unwrap().expected_actual_gait,
            Gait::Sprinting
        );
        assert_eq!(
            cases[9].as_ref().unwrap().expected_allowed_gait,
            Gait::Running
        );
        assert!(cases[9].as_ref().unwrap().first_person);
        assert!((cases[10].as_ref().unwrap().yaw_offset.to_degrees() - 90.0).abs() < 1e-6);
        assert!(cases[10].as_ref().unwrap().branch_from_actual_local);
    }

    #[test]
    fn generated_standing_pose_chain_uses_the_installed_closure() {
        let chain = generated_standing_source_pose_chain();
        assert_eq!(chain.len(), 10);
        assert_eq!(chain[0].label, "standing_movement_cache");
        assert_eq!(
            chain[0].graph,
            "generated/ALS/Character/AnimationInstances/Stances/AB_Als_Standing/AnimGraph.animgraph.ron"
        );
        assert_eq!(chain[0].node, "Save cached pose 'Movement'");
        assert_eq!(chain[1].node, "Movement Details States\nState Machine");
        assert_eq!(chain[2].node, "Movement States\nState Machine");
        assert_eq!(chain[3].node, "Standing States\nState Machine");
        assert_eq!(chain[4].node, "Stop States\nState Machine");
        assert_eq!(chain[5].node, "Apply Additive");
        assert_eq!(chain[8].node, "Save cached pose 'Standing'");
        assert_eq!(chain[9].node, "Grounded States\nState Machine");
    }

    #[test]
    fn generated_stop_quick_pose_chain_uses_the_idle_endpoint_only() {
        let chain = generated_stop_quick_source_pose_chain();
        assert_eq!(chain.len(), 4);
        assert_eq!(chain[0].label, "standing_states_fsm");
        assert_eq!(chain[1].label, "standing_graph_output");
        assert_eq!(chain[2].label, "grounded_standing_cache");
        assert_eq!(chain[3].label, "grounded_fsm");
        assert!(chain.iter().all(|boundary| {
            boundary.minimum_translation_span == 0.0
                && boundary.minimum_rotation_span_radians == 0.0
        }));
    }

    #[test]
    fn state_machine_grounded_window_starts_after_the_fixture_boundary() {
        let entering = state_machine_grounded_window(0.0);
        assert!(!entering.settled);
        assert!(!entering.capture_source_window);
        let settled = state_machine_grounded_window(0.5);
        assert!(!settled.settled);
        let before_forward = state_machine_grounded_window(0.75);
        assert!(!before_forward.settled);
        let forward = state_machine_grounded_window(1.0);
        assert!(forward.settled);
        let stop = state_machine_grounded_window(2.4);
        assert!(!stop.settled);
        let before_backward = state_machine_grounded_window(3.2);
        assert!(!before_backward.settled);
        let backward = state_machine_grounded_window(3.5);
        assert!(backward.settled);
        assert!(forward.capture_source_window);
        assert_eq!(settled.source_pose_chain.len(), 10);
        assert_eq!(
            forward.source_pose_chain[0].node,
            "Save cached pose 'Movement'"
        );
        assert_eq!(settled.source_sync_group, Some("Movement"));
        assert_eq!(
            settled.sync_phase_global,
            Some("sync_groups.ab_als_standing.movement.phase")
        );
    }

    #[test]
    fn source_visible_standing_direction_signatures_have_exact_cases() {
        let expected = [
            (0, AlsMovementDirection::Forward, "Move Forward"),
            (1, AlsMovementDirection::Backward, "Move Backward"),
            (
                2,
                AlsMovementDirection::RightBackward,
                "Move Right Backward",
            ),
            (3, AlsMovementDirection::LeftForward, "Move Left Forward"),
            (7, AlsMovementDirection::RightForward, "Move Right Forward"),
        ];
        for (index, direction, source_state) in expected {
            let case = direction_regression_case(index).unwrap();
            assert_eq!(case.expected_movement_state, Some(direction));
            assert_eq!(case.source_state, Some(source_state));
            assert_eq!(
                case.direction_signature_group,
                Some("AB_Als_Standing/Movement States")
            );
            assert_eq!(
                case.expected_movement_state
                    .map(movement_direction_fsm_index),
                Some(movement_direction_fsm_index(direction))
            );
            assert_eq!(
                movement_direction_from_fsm_index(movement_direction_fsm_index(direction)),
                Some(direction)
            );
        }
    }

    #[test]
    fn left_backward_auto_edge_is_not_a_steady_direction_signature() {
        let backward = direction_regression_case(5).unwrap();
        let left_forward = direction_regression_case(6).unwrap();
        assert_eq!(
            backward.expected_movement_state,
            Some(AlsMovementDirection::Backward)
        );
        assert_eq!(backward.move_dir, Vec2::NEG_Y);
        assert_eq!(
            left_forward.expected_movement_state,
            Some(AlsMovementDirection::LeftForward)
        );
        assert_eq!(
            left_forward.source_state,
            Some("Move Left Backward to Move Left Forward")
        );
        assert!(left_forward.direction_signature_group.is_none());
        assert_eq!(left_forward.move_dir, Vec2::NEG_X);
    }

    #[test]
    fn direction_branch_masks_accept_only_declared_axes() {
        assert!(direction_branch_matches(
            Some(0),
            DIRECTION_BRANCH_FORWARD | DIRECTION_BRANCH_RIGHT
        ));
        assert!(direction_branch_matches(
            Some(3),
            DIRECTION_BRANCH_FORWARD | DIRECTION_BRANCH_RIGHT
        ));
        assert!(!direction_branch_matches(
            Some(2),
            DIRECTION_BRANCH_FORWARD | DIRECTION_BRANCH_RIGHT
        ));
        assert!(!direction_branch_matches(None, DIRECTION_BRANCH_FORWARD));
    }

    #[test]
    fn crouch_direction_fixture_clears_motion_before_rotate_and_turn_coverage() {
        let duration = CROUCH_DIRECTION_CASE_COUNT as f32 * CROUCH_DIRECTION_CASE_SECONDS;
        assert!(!crouch_direction_regression_requires_stationary_turn_phase(
            duration - 0.01,
            duration
        ));
        assert!(crouch_direction_regression_requires_stationary_turn_phase(
            duration, duration
        ));
    }

    #[test]
    fn crouch_direction_reference_frame_does_not_override_stationary_rotation() {
        let duration = CROUCH_DIRECTION_CASE_COUNT as f32 * CROUCH_DIRECTION_CASE_SECONDS;
        assert!(crouch_direction_reference_frame_active(duration - 0.01));
        assert!(!crouch_direction_reference_frame_active(duration));
    }

    #[test]
    fn crouch_direction_fixture_maps_target_axes_to_source_states() {
        let expected = [
            (0, AlsMovementDirection::Forward),
            (1, AlsMovementDirection::RightForward),
            (2, AlsMovementDirection::RightBackward),
            (3, AlsMovementDirection::Backward),
            (4, AlsMovementDirection::LeftBackward),
            (5, AlsMovementDirection::LeftForward),
            (6, AlsMovementDirection::Forward),
            (7, AlsMovementDirection::RightForward),
        ];
        for (index, direction) in expected {
            let case = crouch_direction_regression_case(index).unwrap();
            assert_eq!(case.expected_direction, direction);
            let world = als_core::motion::view_relative_move_direction(case.move_dir, 0.0);
            let angle = crate::direction_layer::view_space_direction_angle_from_radians(
                crate::direction_layer::velocity_yaw_deg(world),
                0.0,
            );
            assert_eq!(
                als_core::movement_direction::update_direction(None, angle),
                direction
            );
        }
    }

    #[test]
    fn crouch_stationary_fixture_holds_aim_only_for_the_source_rotate_actions() {
        assert!(!crouch_stationary_validation_holds_aim(None));
        assert!(!crouch_stationary_validation_holds_aim(Some(
            CrouchStationaryValidation::TurnRight
        )));
        assert!(!crouch_stationary_validation_holds_aim(Some(
            CrouchStationaryValidation::TurnLeft
        )));
        assert!(crouch_stationary_validation_holds_aim(Some(
            CrouchStationaryValidation::RotateRight
        )));
        assert!(crouch_stationary_validation_holds_aim(Some(
            CrouchStationaryValidation::RotateLeft
        )));
    }

    #[test]
    fn source_rotate_window_keeps_the_sequence_tail_observable() {
        let mut drive = AlsTurnDrive {
            rotate_active: true,
            ..default()
        };
        assert!(source_rotate_window_active(Some(&drive)));
        drive.rotate_active = false;
        drive.rotate_finishing = true;
        assert!(source_rotate_window_active(Some(&drive)));
        drive.rotate_finishing = false;
        assert!(!source_rotate_window_active(Some(&drive)));
    }

    #[test]
    fn source_stationary_fixture_targets_yaw_relative_to_actual_actor_heading() {
        let base = (-65.0_f32).to_radians();
        assert!(
            (source_stationary_target_yaw(base, false) - (-155.0_f32).to_radians()).abs() < 1e-6
        );
        assert!((source_stationary_target_yaw(base, true) - 25.0_f32.to_radians()).abs() < 1e-6);
    }

    #[test]
    fn source_stationary_fixture_requires_consecutive_stable_frames() {
        let mut frames = 0;
        for _ in 0..CROUCH_STATIONARY_STABLE_FRAMES - 1 {
            frames = advance_crouch_stationary_stability(frames, true);
        }
        assert!(frames < CROUCH_STATIONARY_STABLE_FRAMES);
        frames = advance_crouch_stationary_stability(frames, false);
        assert_eq!(frames, 0);
        for _ in 0..CROUCH_STATIONARY_STABLE_FRAMES {
            frames = advance_crouch_stationary_stability(frames, true);
        }
        assert_eq!(frames, CROUCH_STATIONARY_STABLE_FRAMES);
    }

    #[test]
    fn crouch_stationary_fixture_waits_for_each_completed_source_action() {
        let mut coverage = IntegrationCoverage::default();
        assert_eq!(
            next_crouch_stationary_validation(&coverage),
            Some(CrouchStationaryValidation::TurnRight)
        );
        coverage.crouch_turn_completion_mask |= 1 << 0;
        assert_eq!(
            next_crouch_stationary_validation(&coverage),
            Some(CrouchStationaryValidation::TurnLeft)
        );
        coverage.crouch_turn_completion_mask |= 1 << 1;
        assert_eq!(
            next_crouch_stationary_validation(&coverage),
            Some(CrouchStationaryValidation::RotateRight)
        );
        coverage.crouch_rotate_mask |= 1 << 0;
        assert_eq!(
            next_crouch_stationary_validation(&coverage),
            Some(CrouchStationaryValidation::RotateRight)
        );
        coverage.crouch_rotate_completion_mask |= 1 << 0;
        assert_eq!(
            next_crouch_stationary_validation(&coverage),
            Some(CrouchStationaryValidation::RotateLeft)
        );
        coverage.crouch_rotate_completion_mask |= 1 << 1;
        assert_eq!(next_crouch_stationary_validation(&coverage), None);
    }

    #[test]
    fn crouch_rotate_fixture_requires_the_source_loop_boundary_and_return_to_stationary() {
        assert!(!source_rotate_window_completed(true, false, true));
        assert!(!source_rotate_window_completed(true, true, false));
        assert!(!source_rotate_window_completed(false, true, true));
        assert!(source_rotate_window_completed(true, true, true));
    }

    #[test]
    fn crouch_turn_window_requires_source_measured_yaw_convergence() {
        let window = crouch_turn_window(false, true, &["slot_active"]);
        assert_eq!(
            window.minimum_actor_yaw_delta_radians,
            CROUCH_TURN_SOURCE_MIN_YAW_CONVERGENCE_RADIANS
        );
        assert_eq!(
            window.minimum_actor_yaw_convergence_radians,
            CROUCH_TURN_SOURCE_MIN_YAW_CONVERGENCE_RADIANS
        );
        assert_eq!(window.expected_actor_yaw_delta_sign, Some(-1));
        assert_eq!(window.minimum_observation_seconds, 0.0);
        assert_eq!(window.expected_terminal_parent_runtime_state, Some(0));
        assert_eq!(
            window.expected_terminal_requested_parent_transition_active,
            Some(false)
        );
        assert_eq!(
            crouch_turn_window(true, true, &["slot_active"]).expected_actor_yaw_delta_sign,
            Some(1)
        );
        assert!(
            window
                .expected_lifecycle_stages
                .contains(&"natural_completion")
        );
    }

    #[test]
    fn crouch_rotate_window_requires_source_measured_yaw_convergence() {
        let window = crouch_rotate_window(false, true, &[]);
        assert_eq!(
            window.minimum_actor_yaw_delta_radians,
            CROUCH_ROTATE_SOURCE_MIN_YAW_CONVERGENCE_RADIANS
        );
        assert_eq!(
            window.minimum_actor_yaw_convergence_radians,
            CROUCH_ROTATE_SOURCE_MIN_YAW_CONVERGENCE_RADIANS
        );
        assert_eq!(window.expected_actor_yaw_delta_sign, Some(-1));
        assert_eq!(window.expected_terminal_parent_runtime_state, Some(0));
        assert_eq!(
            window.expected_terminal_requested_parent_transition_active,
            Some(true)
        );
        assert_eq!(
            crouch_rotate_window(true, true, &[]).expected_actor_yaw_delta_sign,
            Some(1)
        );
        assert!(window.requires_source_pose_chain);
        assert_eq!(
            window.expected_lifecycle_stages,
            &["sequence_completed", "settled"]
        );
    }

    #[test]
    fn crouch_rotate_window_restores_the_common_runtime_trace() {
        let mut window = crouch_rotate_window(true, true, &[]);
        let transform = Transform {
            rotation: Quat::from_rotation_y(0.25),
            ..default()
        };
        let rotation_animation = AlsRotationAnimationState {
            yaw_speed_degrees: 42.0,
            ..default()
        };
        let crouching_graph = AlsCrouchingGraphTelemetry {
            graph_turn_curve_direct_yaw_speed_target: Some(21.0),
            ..default()
        };
        let turn_drive = AlsTurnDrive {
            rotate_active: true,
            rotate_clip_time: 0.4,
            rotate_lr: 0.0,
            rotate_rate: 1.2,
            ..default()
        };
        let turn_stack = AlsTurnMontageStack::default();
        record_animation_window_trace(
            &mut window,
            &transform,
            0.75,
            Some(&rotation_animation),
            Some(&crouching_graph),
            Some(&turn_drive),
            Some(&turn_stack),
            true,
            Some(crouch_stationary_animation_variant(true, false)),
        );
        assert!(window.observed_actor_yaw.is_some());
        assert_eq!(window.observed_view_yaw, Some(0.75));
        assert_eq!(window.observed_animation_yaw_speed_degrees, Some(42.0));
        assert_eq!(
            window.observed_animation_curve_direct_yaw_speed_degrees,
            Some(21.0)
        );
        assert_eq!(window.observed_animation_clip_time, Some(0.4));
        assert_eq!(window.observed_animation_weight, Some(1.0));
        assert_eq!(window.observed_animation_play_rate, Some(1.2));
        assert_eq!(window.observed_animation_curve_stack_weight, Some(0.0));
        assert!(window.observed_animation_curve_stack_entries.is_empty());
        assert_eq!(
            window.observed_animation_variant,
            Some("A_Als_Crouch_Rotate_90_Left")
        );
    }

    #[test]
    fn crouch_stationary_waits_for_the_source_idle_slot_precondition() {
        let mut motion = AlsActualMotion::default();
        let telemetry = AlsCrouchingGraphTelemetry {
            actual_outer_state: Some(0),
            actual_outer_transition_active: Some(false),
            actual_outer_pose_ready: Some(true),
            ..default()
        };
        let mut drive = AlsTurnDrive::default();
        let mut slot = AlsStandingTurnSlotRuntime::default();
        assert!(crouch_stationary_source_precondition(
            Some(&motion),
            Some(&telemetry),
            Some(&drive),
            Some(&slot),
        ));
        motion.horizontal_speed = crate::turn_layer::TURN_GROUND_SPEED_EPS * 2.0;
        assert!(!crouch_stationary_source_precondition(
            Some(&motion),
            Some(&telemetry),
            Some(&drive),
            Some(&slot),
        ));
        motion.horizontal_speed = 0.0;
        drive.rotate_active = true;
        assert!(!crouch_stationary_source_precondition(
            Some(&motion),
            Some(&telemetry),
            Some(&drive),
            Some(&slot),
        ));
        drive.rotate_active = false;
        slot.active = true;
        assert!(!crouch_stationary_source_precondition(
            Some(&motion),
            Some(&telemetry),
            Some(&drive),
            Some(&slot),
        ));
    }

    #[test]
    fn actual_velocity_branch_mask_preserves_dominant_local_axes() {
        let forward = AlsActualMotion {
            velocity_blend: als_ext::HorizontalVelocityBlend {
                forward: 1.0,
                ..default()
            },
            ..default()
        };
        let diagonal = AlsActualMotion {
            velocity_blend: als_ext::HorizontalVelocityBlend {
                forward: 0.5,
                right: 0.5,
                ..default()
            },
            ..default()
        };
        assert_eq!(
            actual_velocity_branch_mask(&forward),
            DIRECTION_BRANCH_FORWARD
        );
        assert_eq!(
            actual_velocity_branch_mask(&diagonal),
            DIRECTION_BRANCH_FORWARD | DIRECTION_BRANCH_RIGHT
        );
    }

    #[test]
    fn state_machine_turn_fixture_allows_both_source_montages_to_finish() {
        assert!(STATE_MACHINE_TURN_FIRST_END - STATE_MACHINE_TURN_FIRST_START >= 1.0);
        assert!(STATE_MACHINE_TURN_SECOND_END - STATE_MACHINE_TURN_SECOND_START >= 1.0);
        assert!(state_machine_turn_target_yaw(0.5, 0.0).is_some());
        assert_eq!(state_machine_turn_target_yaw(2.0, 0.0), Some(0.0));
        assert!(state_machine_turn_target_yaw(4.0, 0.0).is_some());
        assert!(!state_machine_turn_phase_complete(
            STATE_MACHINE_TURN_END - 0.01
        ));
        assert!(state_machine_turn_phase_complete(STATE_MACHINE_TURN_END));
    }
}
