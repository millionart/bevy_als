use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    num::NonZeroU8,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use als_camera::AlsCameraSet;
use als_core::character::AlsCharacter;
use als_ext::{AnimationContextState, InterruptibleFsmState, player_animation_context_state};
use avian3d::prelude::PhysicsSystems;
use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use bevy_animation_graph::core::{
    animated_scene::AnimatedSceneInstance,
    animation_graph::{AnimationGraph, NodeId, SourcePin, TargetPin},
    animation_graph_player::{AnimationGraphPlayer, AnimationSource},
    context::node_states::StateKey,
    edge_data::DataValue,
    errors::GraphError,
    plugin::AnimationGraphSet,
    pose::{BonePose, Pose},
    systems::apply_animation_to_targets,
};
use serde::Serialize;

use crate::{
    animation_context_projection::AlsRootAnimationContext,
    character_assembly::AlsCharacterAvatar,
    control_rig_apply::{
        AlsControlRigInput, AlsControlRigPrePropagationSet, AlsControlRigPropagationSet,
        AlsControlRigSet,
    },
    foot_ik_apply::{
        AlsFootIkApplyTelemetry, AlsFootIkApplyTelemetrySide, AlsFootIkSpaceSide,
        AlsFootIkSpaceState, AlsFootLockTeleportTelemetry, FootIkApplySet,
    },
    manual_playground_integration::{
        ManualAnimationPoseBoundary, ManualAnimationWindowEvidence,
        ManualIntegrationObservationSet, ManualRuntimePhaseEvidence, ManualTurnStackEntryEvidence,
    },
};

const REQUIRED_BONES: &[&str] = &[
    "pelvis",
    "spine_01",
    "spine_02",
    "spine_03",
    "neck_01",
    "head",
    "upperarm_l",
    "lowerarm_l",
    "hand_l",
    "upperarm_r",
    "lowerarm_r",
    "hand_r",
    "thigh_l",
    "calf_l",
    "foot_l",
    "thigh_r",
    "calf_r",
    "foot_r",
];

const SEGMENTS: &[(&str, &str)] = &[
    ("pelvis", "spine_01"),
    ("spine_01", "spine_02"),
    ("spine_02", "spine_03"),
    ("spine_03", "neck_01"),
    ("neck_01", "head"),
    ("upperarm_l", "lowerarm_l"),
    ("lowerarm_l", "hand_l"),
    ("upperarm_r", "lowerarm_r"),
    ("lowerarm_r", "hand_r"),
    ("pelvis", "thigh_l"),
    ("thigh_l", "calf_l"),
    ("calf_l", "foot_l"),
    ("pelvis", "thigh_r"),
    ("thigh_r", "calf_r"),
    ("calf_r", "foot_r"),
];

const MOTION_BONES: &[&str] = &["thigh_l", "calf_l", "foot_l", "thigh_r", "calf_r", "foot_r"];

const LIMB_CHAINS: &[(&str, &str, &str, &str)] = &[
    ("left_leg", "thigh_l", "calf_l", "foot_l"),
    ("right_leg", "thigh_r", "calf_r", "foot_r"),
    ("left_arm", "upperarm_l", "lowerarm_l", "hand_l"),
    ("right_arm", "upperarm_r", "lowerarm_r", "hand_r"),
];

const TEMPORAL_GLOBAL_POSITION_SPEED_METERS_PER_SECOND: f32 = 24.0;
const TEMPORAL_LOCAL_TRANSLATION_JUMP_METERS: f32 = 0.35;
const TEMPORAL_LOCAL_TRANSLATION_SPEED_METERS_PER_SECOND: f32 = 14.0;
const TEMPORAL_LOCAL_ROTATION_JUMP_RADIANS: f32 = 2.2;
const TEMPORAL_LOCAL_ROTATION_SPEED_RADIANS_PER_SECOND: f32 = 40.0;
const TEMPORAL_LOCAL_ROTATION_SPEED_MIN_DELTA_RADIANS: f32 = 0.75;
const TEMPORAL_SCALE_RATIO: f32 = 1.5;
const TEMPORAL_SEGMENT_RATIO: f32 = 0.35;
const TEMPORAL_SEGMENT_MIN_DELTA_METERS: f32 = 0.05;
const TEMPORAL_LIMB_PLANE_FLIP_DOT: f32 = -0.5;
const ANIMATION_VISUAL_EVIDENCE_FORMAT_SOURCE: &str =
    include_str!("../tools/animation_visual_evidence_format.txt");

fn animation_visual_evidence_format() -> &'static str {
    ANIMATION_VISUAL_EVIDENCE_FORMAT_SOURCE.trim()
}
const TEMPORAL_LIMB_BEND_SINE: f32 = 0.08;
const MAX_TEMPORAL_ANOMALIES: usize = 64;

type FinalBone = (Vec3, Quat, Vec3, Vec3);

#[derive(Resource)]
struct AnimationVisualValidationConfig {
    directory: PathBuf,
    case: String,
    capture_count: usize,
    capture_interval: Duration,
    min_pose_samples: usize,
    warmup: Duration,
    max_frame_p95_ms: f64,
    max_frame_ms: f64,
    max_eval_p95_ms: f64,
    max_physics_steps_per_render: f64,
}

impl AnimationVisualValidationConfig {
    fn from_environment() -> Option<Self> {
        let directory = PathBuf::from(std::env::var("ALS_ANIMATION_VISUAL_EVIDENCE_DIR").ok()?);
        fs::create_dir_all(&directory).ok()?;
        Some(Self {
            directory,
            case: std::env::var("ALS_MANUAL_INTEGRATION_CASE")
                .unwrap_or_else(|_| "base".to_owned()),
            capture_count: env_usize("ALS_ANIMATION_VISUAL_CAPTURE_COUNT", 6),
            capture_interval: Duration::from_secs_f32(env_f32(
                "ALS_ANIMATION_VISUAL_CAPTURE_INTERVAL_SECS",
                1.5,
            )),
            min_pose_samples: env_usize("ALS_ANIMATION_VISUAL_MIN_POSE_SAMPLES", 24),
            warmup: Duration::from_secs_f32(env_f32("ALS_ANIMATION_VISUAL_WARMUP_SECS", 1.0)),
            max_frame_p95_ms: env_f64("ALS_ANIMATION_VISUAL_MAX_FRAME_P95_MS", 25.0),
            max_frame_ms: env_f64("ALS_ANIMATION_VISUAL_MAX_FRAME_MS", 75.0),
            max_eval_p95_ms: env_f64("ALS_ANIMATION_VISUAL_MAX_EVAL_P95_MS", 4.0),
            max_physics_steps_per_render: env_f64(
                "ALS_ANIMATION_VISUAL_MAX_PHYSICS_STEPS_PER_RENDER",
                3.0,
            ),
        })
    }
}

#[derive(Resource, Default)]
struct AnimationVisualValidationState {
    started: Option<Instant>,
    last_frame: Option<Instant>,
    next_pose_sample: Option<Instant>,
    next_capture: Option<Instant>,
    next_report: Option<Instant>,
    capture_index: usize,
    fault_captures: usize,
    eval_started: Option<Instant>,
    frame_started: Option<Instant>,
    skip_frame_samples: u8,
    excluded_frame_samples: usize,
    frame_ms: Vec<f64>,
    cpu_frame_ms: Vec<f64>,
    eval_ms: Vec<f64>,
    pose_samples: Vec<PoseSample>,
    screenshots: Vec<ScreenshotSample>,
    faults: Vec<String>,
    graph_errors: Vec<String>,
    primary_failure: Option<GraphEvaluationFailure>,
    fatal_exit_frames_remaining: Option<u8>,
    temporal_pre_rig_guard: TemporalSkeletonGuard,
    temporal_final_guard: TemporalSkeletonGuard,
    temporal_anomalies: Vec<TemporalSkeletonAnomaly>,
    temporal_context: Option<String>,
    temporal_window_settled: bool,
    temporal_rebase_epoch: Option<u64>,
    captured_direction_cases: BTreeSet<String>,
    motion_windows: BTreeMap<String, MotionWindowTracker>,
    required_motion_windows: BTreeSet<String>,
    last_teleport_count: u32,
    teleport_captures: usize,
    observed_bones: usize,
}

#[derive(Resource, Default)]
struct AnimationVisualStageTiming {
    physics_started: Option<Instant>,
    physics_step_started: Option<Instant>,
    control_rig_started: Option<Instant>,
    foot_ik_started: Option<Instant>,
    camera_started: Option<Instant>,
    samples: BTreeMap<String, Vec<f64>>,
    physics_steps_since_render: usize,
    physics_steps_per_render: Vec<f64>,
}

#[derive(Clone, Copy, Default)]
struct LocalBonePose {
    translation: Vec3,
    rotation: Quat,
    scale: Vec3,
}

#[derive(Clone, Copy)]
struct LimbPlane {
    normal: Vec3,
}

#[derive(Default)]
struct TemporalSkeletonGuard {
    previous_relative: BTreeMap<String, Vec3>,
    previous_local: BTreeMap<String, LocalBonePose>,
    previous_segments: BTreeMap<String, f32>,
    previous_limb_planes: BTreeMap<String, LimbPlane>,
}

#[derive(Clone, Debug, PartialEq)]
struct TemporalSkeletonMetric {
    kind: &'static str,
    subject: String,
    previous: f32,
    current: f32,
    threshold: f32,
    trigger: &'static str,
    rate: Option<f32>,
    rate_threshold: Option<f32>,
}

struct TemporalSkeletonObservation {
    metrics: Vec<TemporalSkeletonMetric>,
}

#[derive(Serialize, Clone)]
struct TemporalSkeletonAnomaly {
    stage: String,
    kind: String,
    subject: String,
    previous: f32,
    current: f32,
    threshold: f32,
    trigger: String,
    rate: Option<f32>,
    rate_threshold: Option<f32>,
    elapsed_secs: f64,
    frame_ms: f64,
    direction_case: Option<String>,
    phase: Option<String>,
}

#[derive(Clone, Copy, Default)]
struct RootPoseActivity {
    output_present: bool,
    bone_count: usize,
    active_channel_count: usize,
}

#[derive(Serialize, Clone)]
struct RootPoseSample {
    output_present: bool,
    bone_count: usize,
    active_channel_count: usize,
}

#[derive(Serialize, Clone)]
struct GraphFailurePin {
    kind: String,
    node_id: Option<String>,
    pin_id: Option<String>,
    graph_context: Option<String>,
}

#[derive(Serialize, Clone)]
struct GraphFailureNodeCandidate {
    graph_path: Option<String>,
    graph_asset_id: String,
    graph_context: Option<String>,
    node_id: String,
    node_name: String,
    node_kind: String,
    active_context: bool,
    context_curves: BTreeMap<String, f32>,
}

#[derive(Serialize, Clone)]
struct GraphFailureContextNode {
    graph_path: Option<String>,
    graph_context: String,
    node_id: String,
    node_name: String,
    node_kind: String,
    context_curves: BTreeMap<String, f32>,
}

#[derive(Serialize, Clone)]
struct GraphEvaluationFailure {
    category: String,
    raw_error: String,
    error_kind: String,
    pin: Option<GraphFailurePin>,
    node_candidates: Vec<GraphFailureNodeCandidate>,
    active_context_nodes: Vec<GraphFailureContextNode>,
    active_node_candidate_count: usize,
    candidate_resolution: String,
    root_graph_path: Option<String>,
    root_graph_recursive_dependencies: String,
    root_pose: RootPoseSample,
    pose_classification: String,
    case: String,
    direction_case: Option<String>,
    phase: Option<String>,
    phase_elapsed_secs: Option<f32>,
    locomotion: Option<String>,
    gait: Option<String>,
    stance: Option<String>,
    rotation_mode: Option<String>,
    screenshot_status: String,
    screenshot_path: Option<String>,
    reported_before_ready: bool,
}

#[derive(Serialize, Clone)]
struct LocalMotionBoneSample {
    translation: [f32; 3],
    rotation: [f32; 4],
}

#[derive(Resource, Default)]
struct AnimationVisualPreRigPose {
    bones: BTreeMap<String, LocalBonePose>,
}

#[derive(Serialize, Clone)]
struct ActorYawTraceSample {
    elapsed_secs: f64,
    actor_yaw_radians: f32,
    view_yaw_radians: Option<f32>,
    animation_yaw_speed_degrees: Option<f32>,
    animation_curve_direct_yaw_speed_degrees: Option<f32>,
    animation_clip_time: Option<f32>,
    animation_weight: Option<f32>,
    animation_play_rate: Option<f32>,
    animation_curve_stack_weight: Option<f32>,
    animation_curve_stack_entries: Vec<ManualTurnStackEntryEvidence>,
    animation_variant: Option<&'static str>,
    requested_parent_runtime_state: Option<u8>,
    requested_parent_transition_active: Option<bool>,
}

#[derive(Default)]
struct MotionWindowTracker {
    source_node: String,
    source_state: String,
    expected_direction: String,
    observed_directions: BTreeSet<String>,
    expected_runtime_state: Option<u8>,
    observed_runtime_states: BTreeSet<u8>,
    expected_parent_runtime_state: Option<u8>,
    expected_terminal_parent_runtime_state: Option<u8>,
    expected_terminal_parent_transition_active: Option<bool>,
    expected_terminal_requested_parent_transition_active: Option<bool>,
    observed_parent_runtime_states: BTreeSet<u8>,
    observed_parent_transition_active: BTreeSet<bool>,
    observed_requested_parent_runtime_states: BTreeSet<u8>,
    observed_requested_parent_transition_active: BTreeSet<bool>,
    observed_terminal_parent_runtime_states: BTreeSet<u8>,
    observed_terminal_parent_transition_active: BTreeSet<bool>,
    observed_terminal_requested_parent_runtime_states: BTreeSet<u8>,
    observed_terminal_requested_parent_transition_active: BTreeSet<bool>,
    sample_count: usize,
    requires_pre_rig_motion: bool,
    requires_final_pose_motion: bool,
    requires_non_reference_root_pose: bool,
    requires_source_pose_chain: bool,
    requires_actor_yaw_motion: bool,
    requires_actor_yaw_convergence: bool,
    expected_actor_yaw_delta_sign: Option<i8>,
    expected_actor_yaw_baseline: Option<f32>,
    expected_animation_variant: Option<&'static str>,
    minimum_pre_rig_translation_span: f32,
    minimum_pre_rig_rotation_span_radians: f32,
    minimum_final_translation_span: f32,
    minimum_final_rotation_span_radians: f32,
    minimum_actor_yaw_delta_radians: f32,
    minimum_actor_yaw_convergence_radians: f32,
    minimum_observation_seconds: f32,
    observation_seconds: f32,
    first_sample_elapsed_seconds: Option<f64>,
    max_pre_rig_translation_span: f32,
    max_pre_rig_rotation_span_radians: f32,
    max_final_translation_span: f32,
    max_final_rotation_span_radians: f32,
    pre_rig_reference: BTreeMap<String, LocalBonePose>,
    final_reference: BTreeMap<String, LocalBonePose>,
    root_pose_sample_count: usize,
    root_pose_active_samples: usize,
    root_pose_reference_samples: usize,
    root_pose_max_active_channels: usize,
    root_pose_max_translation_span: f32,
    root_pose_max_rotation_span_radians: f32,
    root_pose_reference: Option<Pose>,
    actor_yaw_sample_count: usize,
    actor_yaw_reference: Option<f32>,
    actor_yaw_error_reference: Option<f32>,
    actor_yaw_trace: Vec<ActorYawTraceSample>,
    max_actor_yaw_delta_radians: f32,
    max_actor_yaw_signed_delta_radians: f32,
    max_actor_yaw_opposite_delta_radians: f32,
    max_actor_yaw_convergence_radians: f32,
    source_sync_group: String,
    sync_phase_global: String,
    canonical_sync_phase: Option<f32>,
    maximum_sync_phase_distance: f32,
    canonical_sync_phase_distance: Option<f32>,
    direction_signature_group: String,
    required_direction_signature_members: usize,
    minimum_direction_signature_rotation_radians: f32,
    canonical_pre_rig_pose: BTreeMap<String, LocalBonePose>,
    source_pose_chain: Vec<SourcePoseBoundaryTracker>,
    expected_lifecycle_stages: BTreeSet<String>,
    observed_lifecycle_stages: BTreeSet<String>,
}

#[derive(Default)]
struct SourcePoseBoundaryTracker {
    label: String,
    graph: String,
    node: String,
    output: String,
    minimum_translation_span: f32,
    minimum_rotation_span_radians: f32,
    sample_count: usize,
    available_samples: usize,
    unavailable_samples: usize,
    max_translation_span: f32,
    max_rotation_span_radians: f32,
    reference: Option<Pose>,
}

#[derive(Serialize, Clone)]
struct MotionWindowSample {
    label: String,
    source_node: String,
    source_state: String,
    expected_direction: String,
    observed_directions: Vec<String>,
    expected_runtime_state: Option<u8>,
    observed_runtime_states: Vec<u8>,
    expected_parent_runtime_state: Option<u8>,
    expected_terminal_parent_runtime_state: Option<u8>,
    expected_terminal_parent_transition_active: Option<bool>,
    expected_terminal_requested_parent_transition_active: Option<bool>,
    observed_parent_runtime_states: Vec<u8>,
    observed_parent_transition_active: Vec<bool>,
    observed_requested_parent_runtime_states: Vec<u8>,
    observed_requested_parent_transition_active: Vec<bool>,
    observed_terminal_parent_runtime_states: Vec<u8>,
    observed_terminal_parent_transition_active: Vec<bool>,
    observed_terminal_requested_parent_runtime_states: Vec<u8>,
    observed_terminal_requested_parent_transition_active: Vec<bool>,
    sample_count: usize,
    requires_pre_rig_motion: bool,
    requires_final_pose_motion: bool,
    requires_non_reference_root_pose: bool,
    requires_source_pose_chain: bool,
    requires_actor_yaw_motion: bool,
    requires_actor_yaw_convergence: bool,
    expected_actor_yaw_delta_sign: Option<i8>,
    expected_actor_yaw_baseline: Option<f32>,
    expected_animation_variant: Option<&'static str>,
    minimum_pre_rig_translation_span: f32,
    minimum_pre_rig_rotation_span_radians: f32,
    minimum_final_translation_span: f32,
    minimum_final_rotation_span_radians: f32,
    minimum_actor_yaw_delta_radians: f32,
    minimum_actor_yaw_convergence_radians: f32,
    minimum_observation_seconds: f32,
    observation_seconds: f32,
    max_pre_rig_translation_span: f32,
    max_pre_rig_rotation_span_radians: f32,
    max_final_translation_span: f32,
    max_final_rotation_span_radians: f32,
    root_pose_sample_count: usize,
    root_pose_active_samples: usize,
    root_pose_reference_samples: usize,
    root_pose_max_active_channels: usize,
    root_pose_max_translation_span: f32,
    root_pose_max_rotation_span_radians: f32,
    actor_yaw_sample_count: usize,
    actor_yaw_trace: Vec<ActorYawTraceSample>,
    max_actor_yaw_delta_radians: f32,
    max_actor_yaw_signed_delta_radians: f32,
    max_actor_yaw_opposite_delta_radians: f32,
    max_actor_yaw_convergence_radians: f32,
    source_sync_group: String,
    sync_phase_global: String,
    canonical_sync_phase: Option<f32>,
    maximum_sync_phase_distance: f32,
    canonical_sync_phase_distance: Option<f32>,
    direction_signature_group: String,
    required_direction_signature_members: usize,
    minimum_direction_signature_rotation_radians: f32,
    canonical_pre_rig_pose: BTreeMap<String, LocalMotionBoneSample>,
    source_pose_chain: Vec<SourcePoseBoundarySample>,
    expected_lifecycle_stages: Vec<String>,
    observed_lifecycle_stages: Vec<String>,
    first_failed_source_pose_boundary: Option<String>,
}

#[derive(Serialize, Clone)]
struct SourcePoseBoundarySample {
    label: String,
    graph: String,
    node: String,
    output: String,
    minimum_translation_span: f32,
    minimum_rotation_span_radians: f32,
    sample_count: usize,
    available_samples: usize,
    unavailable_samples: usize,
    max_translation_span: f32,
    max_rotation_span_radians: f32,
}

#[derive(Serialize, Clone)]
struct BoneSample {
    translation: [f32; 3],
    rotation: [f32; 4],
    scale: [f32; 3],
    local_scale: [f32; 3],
}

#[derive(Serialize, Clone)]
struct ScreenshotSample {
    path: String,
    elapsed_secs: f64,
    kind: String,
    direction_case: Option<String>,
    phase: Option<String>,
    phase_elapsed_secs: Option<f32>,
    locomotion: Option<String>,
    gait: Option<String>,
    stance: Option<String>,
    rotation_mode: Option<String>,
    faults: Vec<String>,
}

#[derive(Serialize, Clone)]
struct PoseSample {
    elapsed_secs: f64,
    frame_ms: f64,
    direction_case: Option<String>,
    phase: Option<String>,
    phase_elapsed_secs: Option<f32>,
    locomotion: Option<String>,
    gait: Option<String>,
    stance: Option<String>,
    rotation_mode: Option<String>,
    bones: BTreeMap<String, BoneSample>,
    segments: BTreeMap<String, f32>,
    foot_ik: Option<FootIkSample>,
    root_pose: RootPoseSample,
}

#[derive(Serialize, Clone)]
struct FootIkSideSample {
    valid: bool,
    has_ground_hit: bool,
    trace_target_world: [f32; 3],
    solver_reference_world: [f32; 3],
    target_world: [f32; 3],
    resolved_target_world: [f32; 3],
    pole_location_world: [f32; 3],
    effector_world: [f32; 3],
    hip_world: [f32; 3],
    knee_world: [f32; 3],
    ankle_world: [f32; 3],
    raw_offset: f32,
    applied_weight: f32,
    pole_sign: f32,
    knee_error: f32,
    effector_error: f32,
}

#[derive(Serialize, Clone)]
struct FootLockTeleportSample {
    detected_count: u32,
    left_rebase_count: u32,
    right_rebase_count: u32,
    last_translation_delta_m: f32,
    remaining_seconds: f32,
}

#[derive(Serialize, Clone)]
struct FootIkSample {
    left: FootIkSideSample,
    right: FootIkSideSample,
    source_curves_available: bool,
    pose_moving: f32,
    foot_left_ik_curve: f32,
    foot_right_ik_curve: f32,
    foot_left_lock_curve: f32,
    foot_right_lock_curve: f32,
    standing_pose_moving_global: f32,
    pose_moving_lineage: Vec<ContextCurveSample>,
    velocity_blend_forward_amount: f32,
    velocity_blend_backward_amount: f32,
    teleport: FootLockTeleportSample,
}

#[derive(Serialize, Clone)]
struct ContextCurveSample {
    node: String,
    pose_moving: Option<f32>,
}

#[derive(Serialize)]
struct TimeStats {
    samples: usize,
    p50_ms: f64,
    p95_ms: f64,
    max_ms: f64,
}

#[derive(Serialize)]
struct CountStats {
    samples: usize,
    p50: f64,
    p95: f64,
    max: f64,
}

#[derive(Serialize)]
struct AnimationVisualReport {
    format: &'static str,
    case: String,
    ready: bool,
    pass: bool,
    observed_bones: usize,
    required_bones: Vec<String>,
    missing_bones: Vec<String>,
    frame_time: TimeStats,
    cpu_frame_time: TimeStats,
    animation_eval_time: TimeStats,
    physics_steps_per_render: CountStats,
    stage_time: BTreeMap<String, TimeStats>,
    excluded_frame_samples: usize,
    pose_samples: Vec<PoseSample>,
    screenshots: Vec<ScreenshotSample>,
    motion_windows: Vec<MotionWindowSample>,
    required_motion_windows: Vec<String>,
    temporal_anomalies: Vec<TemporalSkeletonAnomaly>,
    faults: Vec<String>,
    graph_errors: Vec<String>,
    primary_failure: Option<GraphEvaluationFailure>,
    thresholds: BTreeMap<String, f64>,
}

pub struct AnimationVisualValidationPlugin;

impl Plugin for AnimationVisualValidationPlugin {
    fn build(&self, app: &mut App) {
        let Some(config) = AnimationVisualValidationConfig::from_environment() else {
            return;
        };
        app.insert_resource(config)
            .init_resource::<AnimationVisualValidationState>()
            .init_resource::<AnimationVisualStageTiming>()
            .init_resource::<AnimationVisualPreRigPose>()
            .add_systems(First, animation_cpu_frame_start)
            .add_systems(
                PostUpdate,
                animation_eval_start
                    .after(AnimationGraphSet::PostPhysics)
                    .before(AnimationGraphSet::Final),
            )
            .add_systems(
                PostUpdate,
                animation_eval_end.after(AnimationGraphSet::Final),
            )
            .add_systems(
                PostUpdate,
                capture_animation_visual_pre_rig_pose
                    .after(AnimationGraphSet::Final)
                    .after(apply_animation_to_targets)
                    .before(AlsControlRigSet),
            )
            .add_systems(
                PostUpdate,
                collect_animation_visual_evidence
                    .after(FootIkApplySet::Telemetry)
                    .after(AlsControlRigPropagationSet)
                    .after(AlsCameraSet)
                    .after(ManualIntegrationObservationSet),
            )
            .add_systems(
                PostUpdate,
                finish_fatal_animation_visual_failure.after(collect_animation_visual_evidence),
            )
            .add_systems(
                PostUpdate,
                animation_visual_physics_start.before(PhysicsSystems::First),
            )
            .add_systems(
                PostUpdate,
                animation_visual_physics_step_start
                    .after(PhysicsSystems::Prepare)
                    .before(PhysicsSystems::StepSimulation),
            )
            .add_systems(
                PostUpdate,
                animation_visual_physics_step_end
                    .after(PhysicsSystems::StepSimulation)
                    .before(PhysicsSystems::Writeback),
            )
            .add_systems(
                PostUpdate,
                animation_visual_physics_end.after(PhysicsSystems::Last),
            )
            .add_systems(
                PostUpdate,
                animation_visual_control_rig_start
                    .after(AnimationGraphSet::Final)
                    .after(apply_animation_to_targets)
                    .before(AlsControlRigSet),
            )
            .add_systems(
                PostUpdate,
                animation_visual_control_rig_end.after(AlsControlRigPropagationSet),
            )
            .add_systems(
                PostUpdate,
                animation_visual_foot_ik_start
                    .after(AlsControlRigPrePropagationSet)
                    .before(FootIkApplySet::Solve),
            )
            .add_systems(
                PostUpdate,
                animation_visual_foot_ik_end.after(FootIkApplySet::Telemetry),
            )
            .add_systems(
                PostUpdate,
                animation_visual_camera_start
                    .after(FootIkApplySet::Telemetry)
                    .before(AlsCameraSet),
            )
            .add_systems(PostUpdate, animation_visual_camera_end.after(AlsCameraSet));
    }
}

fn env_f32(name: &str, fallback: f32) -> f32 {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse::<f32>().ok())
        .filter(|value| value.is_finite() && *value > 0.0)
        .unwrap_or(fallback)
}

fn env_f64(name: &str, fallback: f64) -> f64 {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse::<f64>().ok())
        .filter(|value| value.is_finite() && *value > 0.0)
        .unwrap_or(fallback)
}

fn env_usize(name: &str, fallback: usize) -> usize {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(fallback)
}

fn append_unique(values: &mut Vec<String>, value: String) -> bool {
    if values.len() < 128 && !values.iter().any(|existing| existing == &value) {
        values.push(value);
        return true;
    }
    false
}

fn temporal_scale_ratio(previous: Vec3, current: Vec3) -> f32 {
    let mut ratio = 1.0_f32;
    for (previous, current) in [
        (previous.x.abs(), current.x.abs()),
        (previous.y.abs(), current.y.abs()),
        (previous.z.abs(), current.z.abs()),
    ] {
        if previous <= f32::EPSILON || current <= f32::EPSILON {
            return f32::INFINITY;
        }
        ratio = ratio.max((previous / current).max(current / previous));
    }
    ratio
}

fn limb_plane(
    bones: &BTreeMap<String, FinalBone>,
    first: &str,
    second: &str,
    third: &str,
) -> Option<LimbPlane> {
    let first = bones.get(first)?.0;
    let second = bones.get(second)?.0;
    let third = bones.get(third)?.0;
    let upper = second - first;
    let lower = third - second;
    let lengths = upper.length() * lower.length();
    if !lengths.is_finite() || lengths <= f32::EPSILON {
        return None;
    }
    let normal = upper.cross(lower);
    let bend_sine = normal.length() / lengths;
    if !bend_sine.is_finite() || bend_sine < TEMPORAL_LIMB_BEND_SINE {
        return None;
    }
    Some(LimbPlane {
        normal: normal.normalize(),
    })
}

fn segment_lengths(bones: &BTreeMap<String, FinalBone>) -> BTreeMap<String, f32> {
    SEGMENTS
        .iter()
        .filter_map(|(first, second)| {
            let first_position = bones.get(*first)?.0;
            let second_position = bones.get(*second)?.0;
            Some((
                segment_name(first, second),
                first_position.distance(second_position),
            ))
        })
        .collect()
}

impl TemporalSkeletonGuard {
    fn reset(&mut self) {
        self.previous_relative.clear();
        self.previous_local.clear();
        self.previous_segments.clear();
        self.previous_limb_planes.clear();
    }

    fn observe(
        &mut self,
        local: &BTreeMap<String, LocalBonePose>,
        bones: &BTreeMap<String, FinalBone>,
        avatar_position: Vec3,
        frame_seconds: f32,
    ) -> TemporalSkeletonObservation {
        let mut metrics = Vec::new();
        for (name, (translation, _, _, _)) in bones {
            let relative = *translation - avatar_position;
            if let Some(previous) = self.previous_relative.get(name) {
                let speed = previous.distance(relative) / frame_seconds;
                if speed > TEMPORAL_GLOBAL_POSITION_SPEED_METERS_PER_SECOND {
                    metrics.push(TemporalSkeletonMetric {
                        kind: "global_position_speed",
                        subject: name.clone(),
                        previous: 0.0,
                        current: speed,
                        threshold: TEMPORAL_GLOBAL_POSITION_SPEED_METERS_PER_SECOND,
                        trigger: "rate",
                        rate: Some(speed),
                        rate_threshold: Some(TEMPORAL_GLOBAL_POSITION_SPEED_METERS_PER_SECOND),
                    });
                }
            }
            self.previous_relative.insert(name.clone(), relative);
        }
        for (name, current) in local {
            if let Some(previous) = self.previous_local.get(name).copied() {
                let translation_delta = previous.translation.distance(current.translation);
                let translation_speed = translation_delta / frame_seconds;
                let translation_trigger =
                    if translation_delta > TEMPORAL_LOCAL_TRANSLATION_JUMP_METERS {
                        Some(("jump", TEMPORAL_LOCAL_TRANSLATION_JUMP_METERS))
                    } else if translation_delta > TEMPORAL_LOCAL_TRANSLATION_JUMP_METERS * 0.35
                        && translation_speed > TEMPORAL_LOCAL_TRANSLATION_SPEED_METERS_PER_SECOND
                    {
                        Some(("rate", TEMPORAL_LOCAL_TRANSLATION_SPEED_METERS_PER_SECOND))
                    } else {
                        None
                    };
                if let Some((trigger, threshold)) = translation_trigger {
                    metrics.push(TemporalSkeletonMetric {
                        kind: "local_translation_spike",
                        subject: name.clone(),
                        previous: 0.0,
                        current: translation_delta,
                        threshold,
                        trigger,
                        rate: Some(translation_speed),
                        rate_threshold: Some(TEMPORAL_LOCAL_TRANSLATION_SPEED_METERS_PER_SECOND),
                    });
                }
                let rotation_delta = rotation_delta_radians(previous.rotation, current.rotation);
                let rotation_speed = rotation_delta / frame_seconds;
                let rotation_trigger = if rotation_delta > TEMPORAL_LOCAL_ROTATION_JUMP_RADIANS {
                    Some(("jump", TEMPORAL_LOCAL_ROTATION_JUMP_RADIANS))
                } else if rotation_delta > TEMPORAL_LOCAL_ROTATION_SPEED_MIN_DELTA_RADIANS
                    && rotation_speed > TEMPORAL_LOCAL_ROTATION_SPEED_RADIANS_PER_SECOND
                {
                    Some(("rate", TEMPORAL_LOCAL_ROTATION_SPEED_RADIANS_PER_SECOND))
                } else {
                    None
                };
                if let Some((trigger, threshold)) = rotation_trigger {
                    metrics.push(TemporalSkeletonMetric {
                        kind: "local_rotation_spike",
                        subject: name.clone(),
                        previous: 0.0,
                        current: rotation_delta,
                        threshold,
                        trigger,
                        rate: Some(rotation_speed),
                        rate_threshold: Some(TEMPORAL_LOCAL_ROTATION_SPEED_RADIANS_PER_SECOND),
                    });
                }
                let scale_ratio = temporal_scale_ratio(previous.scale, current.scale);
                if scale_ratio > TEMPORAL_SCALE_RATIO {
                    metrics.push(TemporalSkeletonMetric {
                        kind: "local_scale_spike",
                        subject: name.clone(),
                        previous: 1.0,
                        current: scale_ratio,
                        threshold: TEMPORAL_SCALE_RATIO,
                        trigger: "ratio",
                        rate: None,
                        rate_threshold: None,
                    });
                }
            }
            self.previous_local.insert(name.clone(), *current);
        }
        let segments = segment_lengths(bones);
        for (name, length) in &segments {
            if let Some(previous) = self.previous_segments.get(name) {
                let delta = (*length - previous).abs();
                let ratio = delta / previous.abs().max(0.01);
                if delta > TEMPORAL_SEGMENT_MIN_DELTA_METERS && ratio > TEMPORAL_SEGMENT_RATIO {
                    metrics.push(TemporalSkeletonMetric {
                        kind: "segment_length_spike",
                        subject: name.clone(),
                        previous: *previous,
                        current: *length,
                        threshold: TEMPORAL_SEGMENT_RATIO,
                        trigger: "ratio",
                        rate: None,
                        rate_threshold: None,
                    });
                }
            }
            self.previous_segments.insert(name.clone(), *length);
        }
        let mut limb_planes = BTreeMap::new();
        for (name, first, second, third) in LIMB_CHAINS {
            let Some(current) = limb_plane(bones, first, second, third) else {
                continue;
            };
            if let Some(previous) = self.previous_limb_planes.get(*name) {
                let dot = previous.normal.dot(current.normal);
                if dot < TEMPORAL_LIMB_PLANE_FLIP_DOT {
                    metrics.push(TemporalSkeletonMetric {
                        kind: "limb_plane_flip",
                        subject: (*name).to_owned(),
                        previous: 1.0,
                        current: dot,
                        threshold: TEMPORAL_LIMB_PLANE_FLIP_DOT,
                        trigger: "plane_flip",
                        rate: None,
                        rate_threshold: None,
                    });
                }
            }
            limb_planes.insert((*name).to_owned(), current);
        }
        self.previous_limb_planes = limb_planes;
        TemporalSkeletonObservation { metrics }
    }
}

fn record_temporal_metrics(
    state: &mut AnimationVisualValidationState,
    metrics: Vec<TemporalSkeletonMetric>,
    stage: &str,
    elapsed_secs: f64,
    frame_ms: f64,
    direction_case: Option<&str>,
    phase: Option<&str>,
) -> bool {
    let mut detected = false;
    for metric in metrics {
        let duplicate = state
            .temporal_anomalies
            .iter()
            .any(|value| value.kind == metric.kind && value.subject == metric.subject);
        if !duplicate && state.temporal_anomalies.len() < MAX_TEMPORAL_ANOMALIES {
            warn!(
                "[ANIMATION-VISUAL][TEMPORAL] stage={} kind={} trigger={} subject={} previous={:.6} current={:.6} threshold={:.6} rate={:?} rate_threshold={:?} elapsed={:.6} frame_ms={:.3}",
                stage,
                metric.kind,
                metric.trigger,
                metric.subject,
                metric.previous,
                metric.current,
                metric.threshold,
                metric.rate,
                metric.rate_threshold,
                elapsed_secs,
                frame_ms,
            );
            state.temporal_anomalies.push(TemporalSkeletonAnomaly {
                stage: stage.to_owned(),
                kind: metric.kind.to_owned(),
                subject: metric.subject.clone(),
                previous: metric.previous,
                current: metric.current,
                threshold: metric.threshold,
                trigger: metric.trigger.to_owned(),
                rate: metric.rate,
                rate_threshold: metric.rate_threshold,
                elapsed_secs,
                frame_ms,
                direction_case: direction_case.map(str::to_owned),
                phase: phase.map(str::to_owned),
            });
        }
        detected |= append_unique(
            &mut state.faults,
            format!("temporal_{stage}_{}:{}", metric.kind, metric.subject),
        );
    }
    detected
}

fn animation_eval_start(mut state: ResMut<AnimationVisualValidationState>) {
    state.eval_started = Some(Instant::now());
}

fn animation_cpu_frame_start(mut state: ResMut<AnimationVisualValidationState>) {
    state.frame_started = Some(Instant::now());
}

fn record_stage(
    config: &AnimationVisualValidationConfig,
    visual: &AnimationVisualValidationState,
    stages: &mut AnimationVisualStageTiming,
    name: &str,
    started: Option<Instant>,
) {
    let Some(run_started) = visual.started else {
        return;
    };
    let Some(started) = started else {
        return;
    };
    if run_started.elapsed() >= config.warmup {
        stages
            .samples
            .entry(name.to_owned())
            .or_default()
            .push(started.elapsed().as_secs_f64() * 1000.0);
    }
}

fn animation_visual_physics_start(mut stages: ResMut<AnimationVisualStageTiming>) {
    stages.physics_started = Some(Instant::now());
}

fn animation_visual_physics_end(
    config: Res<AnimationVisualValidationConfig>,
    visual: Res<AnimationVisualValidationState>,
    mut stages: ResMut<AnimationVisualStageTiming>,
) {
    if visual.started.is_some() {
        stages.physics_steps_since_render += 1;
    }
    let started = stages.physics_started.take();
    record_stage(&config, &visual, &mut stages, "physics", started);
}

fn animation_visual_physics_step_start(mut stages: ResMut<AnimationVisualStageTiming>) {
    stages.physics_step_started = Some(Instant::now());
}

fn animation_visual_physics_step_end(
    config: Res<AnimationVisualValidationConfig>,
    visual: Res<AnimationVisualValidationState>,
    mut stages: ResMut<AnimationVisualStageTiming>,
) {
    let started = stages.physics_step_started.take();
    record_stage(&config, &visual, &mut stages, "physics_step", started);
}

fn animation_visual_control_rig_start(mut stages: ResMut<AnimationVisualStageTiming>) {
    stages.control_rig_started = Some(Instant::now());
}

fn animation_visual_control_rig_end(
    config: Res<AnimationVisualValidationConfig>,
    visual: Res<AnimationVisualValidationState>,
    mut stages: ResMut<AnimationVisualStageTiming>,
) {
    let started = stages.control_rig_started.take();
    record_stage(&config, &visual, &mut stages, "control_rig", started);
}

fn animation_visual_foot_ik_start(mut stages: ResMut<AnimationVisualStageTiming>) {
    stages.foot_ik_started = Some(Instant::now());
}

fn animation_visual_foot_ik_end(
    config: Res<AnimationVisualValidationConfig>,
    visual: Res<AnimationVisualValidationState>,
    mut stages: ResMut<AnimationVisualStageTiming>,
) {
    let started = stages.foot_ik_started.take();
    record_stage(&config, &visual, &mut stages, "foot_ik", started);
}

fn animation_visual_camera_start(mut stages: ResMut<AnimationVisualStageTiming>) {
    stages.camera_started = Some(Instant::now());
}

fn animation_visual_camera_end(
    config: Res<AnimationVisualValidationConfig>,
    visual: Res<AnimationVisualValidationState>,
    mut stages: ResMut<AnimationVisualStageTiming>,
) {
    let started = stages.camera_started.take();
    record_stage(&config, &visual, &mut stages, "camera", started);
}

fn animation_eval_end(
    config: Res<AnimationVisualValidationConfig>,
    mut state: ResMut<AnimationVisualValidationState>,
) {
    let Some(started) = state.started else {
        return;
    };
    let Some(begin) = state.eval_started.take() else {
        return;
    };
    if started.elapsed() >= config.warmup {
        state.eval_ms.push(begin.elapsed().as_secs_f64() * 1000.0);
    }
}

fn collect_bones(
    root: Entity,
    children: &Query<&Children>,
    named_transforms: &Query<(&Name, &GlobalTransform, &Transform)>,
) -> BTreeMap<String, FinalBone> {
    let mut result = BTreeMap::new();
    let mut stack = vec![root];
    while let Some(entity) = stack.pop() {
        if let Ok((name, transform, local)) = named_transforms.get(entity) {
            let label = name.as_str();
            if REQUIRED_BONES.iter().any(|required| *required == label) {
                let (scale, rotation, translation) = transform.to_scale_rotation_translation();
                result.entry(label.to_owned()).or_insert((
                    translation,
                    rotation,
                    scale,
                    local.scale,
                ));
            }
        }
        if let Ok(child_entities) = children.get(entity) {
            stack.extend(child_entities.iter());
        }
    }
    result
}

fn collect_local_motion_bones(
    root: Entity,
    children: &Query<&Children>,
    named_transforms: &Query<(&Name, &GlobalTransform, &Transform)>,
) -> BTreeMap<String, LocalBonePose> {
    let mut result = BTreeMap::new();
    let mut stack = vec![root];
    while let Some(entity) = stack.pop() {
        if let Ok((name, _, local)) = named_transforms.get(entity) {
            let label = name.as_str();
            if REQUIRED_BONES.iter().any(|required| *required == label) {
                result.entry(label.to_owned()).or_insert(LocalBonePose {
                    translation: local.translation,
                    rotation: local.rotation,
                    scale: local.scale,
                });
            }
        }
        if let Ok(child_entities) = children.get(entity) {
            stack.extend(child_entities.iter());
        }
    }
    result
}

fn capture_animation_visual_pre_rig_pose(
    mut pre_rig: ResMut<AnimationVisualPreRigPose>,
    avatars: Query<Entity, With<AlsCharacterAvatar>>,
    children: Query<&Children>,
    named_transforms: Query<(&Name, &GlobalTransform, &Transform)>,
) {
    let Ok(avatar) = avatars.single() else {
        return;
    };
    pre_rig.bones = collect_local_motion_bones(avatar, &children, &named_transforms);
}

fn rotation_delta_radians(previous: Quat, current: Quat) -> f32 {
    if !previous.is_finite()
        || !current.is_finite()
        || previous.length_squared() <= f32::EPSILON
        || current.length_squared() <= f32::EPSILON
    {
        return f32::INFINITY;
    }
    (2.0 * previous
        .normalize()
        .dot(current.normalize())
        .abs()
        .clamp(0.0, 1.0)
        .acos())
    .max(0.0)
}

fn root_pose_activity_from_pose(pose: &Pose) -> RootPoseActivity {
    let mut activity = RootPoseActivity {
        output_present: true,
        bone_count: pose.bones.len(),
        ..default()
    };
    for BonePose {
        translation,
        rotation,
        scale,
        weights,
    } in &pose.bones
    {
        activity.active_channel_count += translation.is_some() as usize;
        activity.active_channel_count += rotation.is_some() as usize;
        activity.active_channel_count += scale.is_some() as usize;
        activity.active_channel_count += weights.is_some() as usize;
    }
    activity
}

fn root_pose_activity(player: &AnimationGraphPlayer) -> RootPoseActivity {
    match player.get_outputs().get("pose") {
        Some(DataValue::Pose(pose)) => root_pose_activity_from_pose(pose),
        _ => RootPoseActivity::default(),
    }
}

fn root_pose_has_animation_channels(activity: RootPoseActivity) -> bool {
    activity.output_present && activity.active_channel_count > 0
}

fn graph_dependency_state(asset_server: &AssetServer, player: &AnimationGraphPlayer) -> String {
    match player.get_animation_source() {
        AnimationSource::Graph(handle) => asset_server
            .get_recursive_dependency_load_state(handle.id())
            .map(|state| format!("{state:?}"))
            .unwrap_or_else(|| "Unknown".to_owned()),
        _ => "NotGraph".to_owned(),
    }
}

fn root_pose_sample(activity: RootPoseActivity) -> RootPoseSample {
    RootPoseSample {
        output_present: activity.output_present,
        bone_count: activity.bone_count,
        active_channel_count: activity.active_channel_count,
    }
}

fn source_pin_failure_pin(pin: &SourcePin) -> GraphFailurePin {
    match pin {
        SourcePin::NodeData(node, pin) => GraphFailurePin {
            kind: "node_data".to_owned(),
            node_id: Some(format!("{node:?}")),
            pin_id: Some(pin.clone()),
            graph_context: None,
        },
        SourcePin::InputData(pin) => GraphFailurePin {
            kind: "input_data".to_owned(),
            node_id: None,
            pin_id: Some(format!("{pin:?}")),
            graph_context: None,
        },
        SourcePin::NodeTime(node) => GraphFailurePin {
            kind: "node_time".to_owned(),
            node_id: Some(format!("{node:?}")),
            pin_id: None,
            graph_context: None,
        },
        SourcePin::InputTime(pin) => GraphFailurePin {
            kind: "input_time".to_owned(),
            node_id: None,
            pin_id: Some(format!("{pin:?}")),
            graph_context: None,
        },
    }
}

fn target_pin_failure_pin(pin: &TargetPin) -> GraphFailurePin {
    match pin {
        TargetPin::NodeData(node, pin) => GraphFailurePin {
            kind: "node_data".to_owned(),
            node_id: Some(format!("{node:?}")),
            pin_id: Some(pin.clone()),
            graph_context: None,
        },
        TargetPin::OutputData(pin) => GraphFailurePin {
            kind: "output_data".to_owned(),
            node_id: None,
            pin_id: Some(pin.clone()),
            graph_context: None,
        },
        TargetPin::NodeTime(node, pin) => GraphFailurePin {
            kind: "node_time".to_owned(),
            node_id: Some(format!("{node:?}")),
            pin_id: Some(pin.clone()),
            graph_context: None,
        },
        TargetPin::OutputTime => GraphFailurePin {
            kind: "output_time".to_owned(),
            node_id: None,
            pin_id: None,
            graph_context: None,
        },
    }
}

fn graph_error_kind(error: &GraphError) -> String {
    format!("{error:?}")
        .split(['(', '{', ' '])
        .next()
        .unwrap_or("GraphError")
        .to_owned()
}

fn graph_error_pin(error: &GraphError) -> Option<GraphFailurePin> {
    match error {
        GraphError::MissingEdgeToSource(pin)
        | GraphError::DurationMissing(pin)
        | GraphError::TimeUpdateMissingFwd(pin) => Some(source_pin_failure_pin(pin)),
        GraphError::MissingEdgeToTarget(pin) | GraphError::TimeUpdateMissingBack(pin) => {
            Some(target_pin_failure_pin(pin))
        }
        GraphError::OutputMissing { graph, node, pin }
        | GraphError::ExtraTimeUpdateNotAvailable { graph, node, pin } => Some(GraphFailurePin {
            kind: "node_output".to_owned(),
            node_id: Some(format!("{node:?}")),
            pin_id: Some(pin.clone()),
            graph_context: Some(format!("{graph:?}")),
        }),
        GraphError::UnknownErrorNode { graph, node, .. } => Some(GraphFailurePin {
            kind: "node".to_owned(),
            node_id: Some(format!("{node:?}")),
            pin_id: None,
            graph_context: Some(format!("{graph:?}")),
        }),
        _ => None,
    }
}

fn graph_error_node_id(error: &GraphError) -> Option<NodeId> {
    match error {
        GraphError::MissingEdgeToSource(SourcePin::NodeData(node, _))
        | GraphError::MissingEdgeToSource(SourcePin::NodeTime(node))
        | GraphError::DurationMissing(SourcePin::NodeData(node, _))
        | GraphError::DurationMissing(SourcePin::NodeTime(node))
        | GraphError::TimeUpdateMissingFwd(SourcePin::NodeData(node, _))
        | GraphError::TimeUpdateMissingFwd(SourcePin::NodeTime(node))
        | GraphError::MissingEdgeToTarget(TargetPin::NodeData(node, _))
        | GraphError::MissingEdgeToTarget(TargetPin::NodeTime(node, _))
        | GraphError::TimeUpdateMissingBack(TargetPin::NodeData(node, _))
        | GraphError::TimeUpdateMissingBack(TargetPin::NodeTime(node, _))
        | GraphError::OutputMissing { node, .. }
        | GraphError::ExtraTimeUpdateNotAvailable { node, .. }
        | GraphError::UnknownErrorNode { node, .. } => Some(*node),
        _ => None,
    }
}

fn add_graph_failure_node_candidate(
    candidates: &mut Vec<GraphFailureNodeCandidate>,
    asset_server: &AssetServer,
    graph_id: bevy::asset::AssetId<AnimationGraph>,
    graph: &AnimationGraph,
    node_id: NodeId,
    graph_context: Option<String>,
    active_context: bool,
    context_curves: BTreeMap<String, f32>,
) {
    let Some(node) = graph.nodes.get(&node_id) else {
        return;
    };
    let graph_asset_id = format!("{graph_id:?}");
    if let Some(existing) = candidates.iter_mut().find(|candidate| {
        candidate.graph_asset_id == graph_asset_id && candidate.node_id == format!("{node_id:?}")
    }) {
        existing.active_context |= active_context;
        if existing.graph_context.is_none() {
            existing.graph_context = graph_context;
        }
        if existing.context_curves.is_empty() {
            existing.context_curves = context_curves;
        }
        return;
    }
    candidates.push(GraphFailureNodeCandidate {
        graph_path: asset_server
            .get_path(graph_id)
            .map(|path| path.path().to_string_lossy().into_owned()),
        graph_asset_id,
        graph_context,
        node_id: format!("{node_id:?}"),
        node_name: node.name.clone(),
        node_kind: node.inner_ref().display_name(),
        active_context,
        context_curves,
    });
}

fn graph_failure_node_candidates(
    player: &AnimationGraphPlayer,
    asset_server: &AssetServer,
    graphs: &Assets<AnimationGraph>,
    node_id: NodeId,
) -> Vec<GraphFailureNodeCandidate> {
    let mut candidates = Vec::new();
    if let Some(arena) = player.get_context_arena() {
        for context_id in arena.iter_context_ids() {
            let Some(context) = arena.get_context(context_id) else {
                continue;
            };
            let graph_id = context.get_graph_id();
            let Some(graph) = graphs.get(graph_id) else {
                continue;
            };
            let context_curves = context
                .node_states
                .get::<AnimationContextState>(node_id, StateKey::Default)
                .ok()
                .map(|state| state.curves.clone())
                .or_else(|| {
                    context
                        .node_states
                        .get::<InterruptibleFsmState>(node_id, StateKey::Default)
                        .ok()
                        .map(|state| state.context.curves.clone())
                })
                .unwrap_or_default()
                .into_iter()
                .filter(|(_, value)| value.is_finite())
                .collect();
            add_graph_failure_node_candidate(
                &mut candidates,
                asset_server,
                graph_id,
                graph,
                node_id,
                Some(format!("{context_id:?}")),
                true,
                context_curves,
            );
        }
    }
    for (graph_id, graph) in graphs.iter() {
        add_graph_failure_node_candidate(
            &mut candidates,
            asset_server,
            graph_id,
            graph,
            node_id,
            None,
            false,
            BTreeMap::new(),
        );
    }
    candidates.sort_by(|left, right| {
        right
            .active_context
            .cmp(&left.active_context)
            .then_with(|| left.graph_path.cmp(&right.graph_path))
            .then_with(|| left.graph_asset_id.cmp(&right.graph_asset_id))
    });
    candidates
}

fn graph_failure_context_nodes(
    player: &AnimationGraphPlayer,
    asset_server: &AssetServer,
    graphs: &Assets<AnimationGraph>,
) -> Vec<GraphFailureContextNode> {
    let Some(arena) = player.get_context_arena() else {
        return Vec::new();
    };
    let mut nodes = Vec::new();
    for context_id in arena.iter_context_ids() {
        let Some(context) = arena.get_context(context_id) else {
            continue;
        };
        let graph_id = context.get_graph_id();
        let Some(graph) = graphs.get(graph_id) else {
            continue;
        };
        for node in graph.nodes.values() {
            let context_curves = context
                .node_states
                .get::<AnimationContextState>(node.id, StateKey::Default)
                .ok()
                .map(|state| state.curves.clone())
                .or_else(|| {
                    context
                        .node_states
                        .get::<InterruptibleFsmState>(node.id, StateKey::Default)
                        .ok()
                        .map(|state| state.context.curves.clone())
                });
            let Some(context_curves) = context_curves else {
                continue;
            };
            nodes.push(GraphFailureContextNode {
                graph_path: asset_server
                    .get_path(graph_id)
                    .map(|path| path.path().to_string_lossy().into_owned()),
                graph_context: format!("{context_id:?}"),
                node_id: format!("{:?}", node.id),
                node_name: node.name.clone(),
                node_kind: node.inner_ref().display_name(),
                context_curves: context_curves
                    .into_iter()
                    .filter(|(_, value)| value.is_finite())
                    .collect(),
            });
        }
    }
    nodes.sort_by(|left, right| {
        left.graph_context
            .cmp(&right.graph_context)
            .then_with(|| left.node_name.cmp(&right.node_name))
    });
    nodes.truncate(128);
    nodes
}

fn graph_error_pose_classification(root_pose: RootPoseActivity) -> String {
    if !root_pose.output_present {
        "missing_root_pose_likely_reference_or_a_pose".to_owned()
    } else if !root_pose_has_animation_channels(root_pose) {
        "reference_root_pose_likely_a_pose".to_owned()
    } else {
        "root_pose_channels_present_graph_error".to_owned()
    }
}

fn graph_evaluation_failure(
    config: &AnimationVisualValidationConfig,
    error: &GraphError,
    player: &AnimationGraphPlayer,
    asset_server: &AssetServer,
    graphs: &Assets<AnimationGraph>,
    root_pose: RootPoseActivity,
    direction_case: Option<String>,
    phase: Option<String>,
    phase_elapsed_secs: Option<f32>,
    locomotion: Option<String>,
    gait: Option<String>,
    stance: Option<String>,
    rotation_mode: Option<String>,
    reported_before_ready: bool,
) -> GraphEvaluationFailure {
    let node_candidates = graph_error_node_id(error)
        .map(|node_id| graph_failure_node_candidates(player, asset_server, graphs, node_id))
        .unwrap_or_default();
    let active_context_nodes = graph_failure_context_nodes(player, asset_server, graphs);
    let active_node_candidate_count = node_candidates
        .iter()
        .filter(|candidate| candidate.active_context)
        .count();
    let candidate_resolution = if graph_error_node_id(error).is_none() {
        "not_applicable".to_owned()
    } else if active_node_candidate_count == 1 {
        "resolved_active_context".to_owned()
    } else if active_node_candidate_count > 1 {
        "ambiguous_active_context".to_owned()
    } else {
        "not_resolved_from_active_context".to_owned()
    };
    let root_graph_path = match player.get_animation_source() {
        AnimationSource::Graph(handle) => asset_server
            .get_path(handle.id())
            .map(|path| path.path().to_string_lossy().into_owned()),
        _ => None,
    };
    GraphEvaluationFailure {
        category: "graph_evaluation_error".to_owned(),
        raw_error: format!("{error:?}"),
        error_kind: graph_error_kind(error),
        pin: graph_error_pin(error),
        node_candidates,
        active_context_nodes,
        active_node_candidate_count,
        candidate_resolution,
        root_graph_path,
        root_graph_recursive_dependencies: graph_dependency_state(asset_server, player),
        root_pose: root_pose_sample(root_pose),
        pose_classification: graph_error_pose_classification(root_pose),
        case: config.case.clone(),
        direction_case,
        phase,
        phase_elapsed_secs,
        locomotion,
        gait,
        stance,
        rotation_mode,
        screenshot_status: "not_attempted".to_owned(),
        screenshot_path: None,
        reported_before_ready,
    }
}

fn refresh_graph_failure_screenshot_status(
    state: &mut AnimationVisualValidationState,
    exiting: bool,
) {
    let Some(failure) = state.primary_failure.as_mut() else {
        return;
    };
    let Some(path) = failure.screenshot_path.as_deref() else {
        return;
    };
    if Path::new(path).is_file() {
        failure.screenshot_status = "captured".to_owned();
    } else if exiting && failure.screenshot_status == "capture_scheduled" {
        failure.screenshot_status = "capture_not_materialized_before_exit".to_owned();
    }
}

fn source_pose_motion_span(reference: &Pose, current: &Pose) -> (f32, f32) {
    let mut translation_span = 0.0_f32;
    let mut rotation_span = 0.0_f32;
    for (reference_bone, current_bone) in reference.bones.iter().zip(&current.bones) {
        if let (Some(reference), Some(current)) =
            (reference_bone.translation, current_bone.translation)
        {
            translation_span = translation_span.max(reference.distance(current));
        }
        if let (Some(reference), Some(current)) = (reference_bone.rotation, current_bone.rotation) {
            rotation_span = rotation_span.max(rotation_delta_radians(reference, current));
        }
    }
    (translation_span, rotation_span)
}

fn source_pose_boundary_output(
    player: &AnimationGraphPlayer,
    asset_server: &AssetServer,
    graphs: &Assets<AnimationGraph>,
    boundary: &ManualAnimationPoseBoundary,
) -> Option<Pose> {
    let arena = player.get_context_arena()?;
    for context_id in arena.iter_context_ids() {
        let context = arena.get_context(context_id)?;
        let Some(path) = asset_server.get_path(context.get_graph_id()) else {
            continue;
        };
        if path.path() != Path::new(boundary.graph) {
            continue;
        }
        let Some(graph) = graphs.get(context.get_graph_id()) else {
            continue;
        };
        let Some(node) = graph.nodes.values().find(|node| node.name == boundary.node) else {
            continue;
        };
        if let Some(DataValue::Pose(pose)) = context.node_caches.get_output_data(
            node.id,
            StateKey::Default,
            boundary.output.to_owned(),
        ) {
            return Some(pose);
        }
    }
    None
}

fn source_pose_boundary_tracker(
    boundary: &ManualAnimationPoseBoundary,
) -> SourcePoseBoundaryTracker {
    SourcePoseBoundaryTracker {
        label: boundary.label.to_owned(),
        graph: boundary.graph.to_owned(),
        node: boundary.node.to_owned(),
        output: boundary.output.to_owned(),
        minimum_translation_span: boundary.minimum_translation_span.max(0.0),
        minimum_rotation_span_radians: boundary.minimum_rotation_span_radians.max(0.0),
        ..default()
    }
}

fn record_source_pose_chain(
    tracker: &mut MotionWindowTracker,
    evidence: &ManualAnimationWindowEvidence,
    player: &AnimationGraphPlayer,
    asset_server: &AssetServer,
    graphs: &Assets<AnimationGraph>,
) {
    if tracker.source_pose_chain.is_empty() {
        tracker.source_pose_chain = evidence
            .source_pose_chain
            .iter()
            .map(source_pose_boundary_tracker)
            .collect();
    }
    for (definition, boundary) in evidence
        .source_pose_chain
        .iter()
        .zip(&mut tracker.source_pose_chain)
    {
        boundary.sample_count += 1;
        let Some(pose) = source_pose_boundary_output(player, asset_server, graphs, definition)
        else {
            boundary.unavailable_samples += 1;
            continue;
        };
        boundary.available_samples += 1;
        if let Some(reference) = boundary.reference.as_ref() {
            let (translation_span, rotation_span) = source_pose_motion_span(reference, &pose);
            boundary.max_translation_span = boundary.max_translation_span.max(translation_span);
            boundary.max_rotation_span_radians =
                boundary.max_rotation_span_radians.max(rotation_span);
        } else {
            boundary.reference = Some(pose.clone());
        }
    }
}

fn first_source_pose_boundary_failure(boundaries: &[SourcePoseBoundaryTracker]) -> Option<String> {
    for boundary in boundaries {
        if boundary.sample_count < 2 || boundary.available_samples < 2 {
            return Some(format!("{}: unavailable", boundary.label));
        }
        if boundary.unavailable_samples > 0 {
            return Some(format!("{}: intermittently unavailable", boundary.label));
        }
        if boundary.max_translation_span <= f32::EPSILON
            && boundary.max_rotation_span_radians <= f32::EPSILON
        {
            return Some(format!("{}: static", boundary.label));
        }
        if (boundary.minimum_translation_span > f32::EPSILON
            || boundary.minimum_rotation_span_radians > f32::EPSILON)
            && (boundary.minimum_translation_span <= f32::EPSILON
                || boundary.max_translation_span < boundary.minimum_translation_span)
            && (boundary.minimum_rotation_span_radians <= f32::EPSILON
                || boundary.max_rotation_span_radians < boundary.minimum_rotation_span_radians)
        {
            return Some(format!("{}: below source floor", boundary.label));
        }
    }
    None
}

fn local_motion_pose_sample(
    bones: &BTreeMap<String, LocalBonePose>,
) -> BTreeMap<String, LocalMotionBoneSample> {
    MOTION_BONES
        .iter()
        .filter_map(|name| {
            let pose = bones.get(*name)?;
            Some((
                (*name).to_owned(),
                LocalMotionBoneSample {
                    translation: [
                        safe(pose.translation.x),
                        safe(pose.translation.y),
                        safe(pose.translation.z),
                    ],
                    rotation: [
                        safe(pose.rotation.x),
                        safe(pose.rotation.y),
                        safe(pose.rotation.z),
                        safe(pose.rotation.w),
                    ],
                },
            ))
        })
        .collect()
}

fn local_motion_span(
    reference: &BTreeMap<String, LocalBonePose>,
    current: &BTreeMap<String, LocalBonePose>,
) -> (f32, f32) {
    let mut translation_span = 0.0_f32;
    let mut rotation_span = 0.0_f32;
    for bone in MOTION_BONES {
        let Some(reference) = reference.get(*bone) else {
            continue;
        };
        let Some(value) = current.get(*bone) else {
            continue;
        };
        translation_span = translation_span.max(reference.translation.distance(value.translation));
        rotation_span =
            rotation_span.max(rotation_delta_radians(reference.rotation, value.rotation));
    }
    (translation_span, rotation_span)
}

fn wrapped_phase_distance(left: f32, right: f32) -> f32 {
    if !left.is_finite() || !right.is_finite() {
        return f32::INFINITY;
    }
    let difference = (left.rem_euclid(1.0) - right.rem_euclid(1.0)).abs();
    difference.min((1.0 - difference).abs())
}

fn wrapped_yaw_distance(left: f32, right: f32) -> f32 {
    if !left.is_finite() || !right.is_finite() {
        return f32::INFINITY;
    }
    wrapped_yaw_delta(left, right).abs()
}

fn wrapped_yaw_delta(reference: f32, current: f32) -> f32 {
    if !reference.is_finite() || !current.is_finite() {
        return f32::INFINITY;
    }
    (current - reference + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU)
        - std::f32::consts::PI
}

fn append_actor_yaw_trace(
    tracker: &mut MotionWindowTracker,
    evidence: &ManualAnimationWindowEvidence,
    actor_yaw: Option<f32>,
    view_yaw: Option<f32>,
    elapsed_seconds: f64,
) {
    let Some(actor_yaw) = actor_yaw.filter(|value| value.is_finite()) else {
        return;
    };
    if tracker.expected_actor_yaw_delta_sign.is_none() || tracker.actor_yaw_trace.len() >= 512 {
        return;
    }
    tracker.actor_yaw_trace.push(ActorYawTraceSample {
        elapsed_secs: elapsed_seconds,
        actor_yaw_radians: actor_yaw,
        view_yaw_radians: view_yaw.filter(|value| value.is_finite()),
        animation_yaw_speed_degrees: evidence
            .observed_animation_yaw_speed_degrees
            .filter(|value| value.is_finite()),
        animation_curve_direct_yaw_speed_degrees: evidence
            .observed_animation_curve_direct_yaw_speed_degrees
            .filter(|value| value.is_finite()),
        animation_clip_time: evidence
            .observed_animation_clip_time
            .filter(|value| value.is_finite()),
        animation_weight: evidence
            .observed_animation_weight
            .filter(|value| value.is_finite()),
        animation_play_rate: evidence
            .observed_animation_play_rate
            .filter(|value| value.is_finite()),
        animation_curve_stack_weight: evidence
            .observed_animation_curve_stack_weight
            .filter(|value| value.is_finite()),
        animation_curve_stack_entries: evidence.observed_animation_curve_stack_entries.clone(),
        animation_variant: evidence.observed_animation_variant,
        requested_parent_runtime_state: evidence.observed_requested_parent_runtime_state,
        requested_parent_transition_active: evidence.observed_requested_parent_transition_active,
    });
}

fn record_motion_window_metadata(
    state: &mut AnimationVisualValidationState,
    evidence: &ManualAnimationWindowEvidence,
) -> Option<String> {
    let label = evidence.label?;
    let tracker = state.motion_windows.entry(label.to_owned()).or_default();
    if tracker.source_node.is_empty() {
        tracker.source_node = evidence.source_node.unwrap_or_default().to_owned();
    }
    if tracker.source_state.is_empty() {
        tracker.source_state = evidence.source_state.unwrap_or_default().to_owned();
    }
    if tracker.expected_actor_yaw_delta_sign.is_none() {
        tracker.expected_actor_yaw_delta_sign = evidence.expected_actor_yaw_delta_sign;
    }
    if tracker.expected_animation_variant.is_none() {
        tracker.expected_animation_variant = evidence.expected_animation_variant;
    }
    if tracker.expected_terminal_parent_runtime_state.is_none() {
        tracker.expected_terminal_parent_runtime_state =
            evidence.expected_terminal_parent_runtime_state;
    }
    if tracker.expected_terminal_parent_transition_active.is_none() {
        tracker.expected_terminal_parent_transition_active =
            evidence.expected_terminal_parent_transition_active;
    }
    if tracker
        .expected_terminal_requested_parent_transition_active
        .is_none()
    {
        tracker.expected_terminal_requested_parent_transition_active =
            evidence.expected_terminal_requested_parent_transition_active;
    }
    tracker.expected_lifecycle_stages.extend(
        evidence
            .expected_lifecycle_stages
            .iter()
            .map(|value| (*value).to_owned()),
    );
    if !evidence.capture_source_window {
        return None;
    }
    tracker.observed_lifecycle_stages.extend(
        evidence
            .observed_lifecycle_stages
            .iter()
            .map(|value| (*value).to_owned()),
    );
    Some(label.to_owned())
}

fn record_motion_window(
    state: &mut AnimationVisualValidationState,
    evidence: &ManualAnimationWindowEvidence,
    pre_rig: &AnimationVisualPreRigPose,
    final_pose: &BTreeMap<String, LocalBonePose>,
    actor_yaw: Option<f32>,
    view_yaw: Option<f32>,
    root_pose: RootPoseActivity,
    root_pose_value: Option<&Pose>,
    player: &AnimationGraphPlayer,
    asset_server: &AssetServer,
    graphs: &Assets<AnimationGraph>,
    elapsed_seconds: f64,
) -> bool {
    let Some(label) = record_motion_window_metadata(state, evidence) else {
        return false;
    };
    {
        let tracker = state.motion_windows.entry(label.clone()).or_default();
        if tracker.expected_actor_yaw_baseline.is_none() {
            tracker.expected_actor_yaw_baseline = evidence
                .expected_actor_yaw_baseline
                .filter(|value| value.is_finite());
        }
        if evidence.expected_actor_yaw_delta_sign.is_some() {
            if let Some(baseline) = tracker.expected_actor_yaw_baseline {
                tracker.actor_yaw_reference.get_or_insert(baseline);
                if let Some(view_yaw) = view_yaw.filter(|value| value.is_finite()) {
                    tracker
                        .actor_yaw_error_reference
                        .get_or_insert(wrapped_yaw_distance(baseline, view_yaw));
                }
                tracker
                    .first_sample_elapsed_seconds
                    .get_or_insert(elapsed_seconds);
            }
            append_actor_yaw_trace(tracker, evidence, actor_yaw, view_yaw, elapsed_seconds);
        }
    }
    if !evidence.settled
        || !(evidence.requires_pre_rig_motion
            || evidence.requires_final_pose_motion
            || evidence.requires_non_reference_root_pose
            || evidence.requires_source_pose_chain
            || evidence.requires_actor_yaw_motion
            || evidence.requires_actor_yaw_convergence
            || !evidence.expected_lifecycle_stages.is_empty())
    {
        return false;
    }
    let tracker = state.motion_windows.entry(label).or_default();
    if tracker.expected_direction.is_empty() {
        tracker.expected_direction = evidence.expected_direction.unwrap_or_default().to_owned();
    }
    if tracker.expected_runtime_state.is_none() {
        tracker.expected_runtime_state = evidence.expected_runtime_state;
    }
    if let Some(direction) = evidence.observed_direction {
        tracker.observed_directions.insert(direction.to_owned());
    }
    if let Some(runtime_state) = evidence.observed_runtime_state {
        tracker.observed_runtime_states.insert(runtime_state);
    }
    if tracker.expected_parent_runtime_state.is_none() {
        tracker.expected_parent_runtime_state = evidence.expected_parent_runtime_state;
    }
    if terminal_parent_state_sample(evidence) {
        if let Some(runtime_state) = evidence.observed_parent_runtime_state {
            tracker
                .observed_terminal_parent_runtime_states
                .insert(runtime_state);
        }
        if let Some(active) = evidence.observed_parent_transition_active {
            tracker
                .observed_terminal_parent_transition_active
                .insert(active);
        }
        if let Some(runtime_state) = evidence.observed_requested_parent_runtime_state {
            tracker
                .observed_terminal_requested_parent_runtime_states
                .insert(runtime_state);
        }
        if let Some(active) = evidence.observed_requested_parent_transition_active {
            tracker
                .observed_terminal_requested_parent_transition_active
                .insert(active);
        }
    } else {
        if let Some(runtime_state) = evidence.observed_parent_runtime_state {
            tracker.observed_parent_runtime_states.insert(runtime_state);
        }
        if let Some(active) = evidence.observed_parent_transition_active {
            tracker.observed_parent_transition_active.insert(active);
        }
        if let Some(runtime_state) = evidence.observed_requested_parent_runtime_state {
            tracker
                .observed_requested_parent_runtime_states
                .insert(runtime_state);
        }
        if let Some(active) = evidence.observed_requested_parent_transition_active {
            tracker
                .observed_requested_parent_transition_active
                .insert(active);
        }
    }
    tracker.requires_pre_rig_motion |= evidence.requires_pre_rig_motion;
    tracker.requires_final_pose_motion |= evidence.requires_final_pose_motion;
    tracker.requires_non_reference_root_pose |= evidence.requires_non_reference_root_pose;
    tracker.requires_source_pose_chain |= evidence.requires_source_pose_chain;
    tracker.requires_actor_yaw_motion |= evidence.requires_actor_yaw_motion;
    tracker.requires_actor_yaw_convergence |= evidence.requires_actor_yaw_convergence;
    tracker.minimum_pre_rig_translation_span = tracker
        .minimum_pre_rig_translation_span
        .max(evidence.minimum_pre_rig_translation_span.max(0.0));
    tracker.minimum_pre_rig_rotation_span_radians = tracker
        .minimum_pre_rig_rotation_span_radians
        .max(evidence.minimum_pre_rig_rotation_span_radians.max(0.0));
    tracker.minimum_final_translation_span = tracker
        .minimum_final_translation_span
        .max(evidence.minimum_final_translation_span.max(0.0));
    tracker.minimum_final_rotation_span_radians = tracker
        .minimum_final_rotation_span_radians
        .max(evidence.minimum_final_rotation_span_radians.max(0.0));
    tracker.minimum_actor_yaw_delta_radians = tracker
        .minimum_actor_yaw_delta_radians
        .max(evidence.minimum_actor_yaw_delta_radians.max(0.0));
    tracker.minimum_actor_yaw_convergence_radians = tracker
        .minimum_actor_yaw_convergence_radians
        .max(evidence.minimum_actor_yaw_convergence_radians.max(0.0));
    tracker.minimum_observation_seconds = tracker
        .minimum_observation_seconds
        .max(evidence.minimum_observation_seconds.max(0.0));
    if tracker.source_sync_group.is_empty() {
        tracker.source_sync_group = evidence.source_sync_group.unwrap_or_default().to_owned();
    }
    if tracker.sync_phase_global.is_empty() {
        tracker.sync_phase_global = evidence.sync_phase_global.unwrap_or_default().to_owned();
    }
    tracker.canonical_sync_phase = evidence
        .canonical_sync_phase
        .filter(|value| value.is_finite());
    tracker.maximum_sync_phase_distance = evidence.maximum_sync_phase_distance.max(0.0);
    if tracker.direction_signature_group.is_empty() {
        tracker.direction_signature_group = evidence
            .direction_signature_group
            .unwrap_or_default()
            .to_owned();
    }
    tracker.required_direction_signature_members = evidence.required_direction_signature_members;
    tracker.minimum_direction_signature_rotation_radians = evidence
        .minimum_direction_signature_rotation_radians
        .max(0.0);
    let pre_rig_current = pre_rig.bones.clone();
    if tracker.pre_rig_reference.is_empty() {
        tracker.pre_rig_reference = pre_rig_current.clone();
    }
    if tracker.final_reference.is_empty() {
        tracker.final_reference = final_pose.clone();
    }
    let first_sample_elapsed_seconds = tracker
        .first_sample_elapsed_seconds
        .get_or_insert(elapsed_seconds);
    tracker.observation_seconds = (elapsed_seconds - *first_sample_elapsed_seconds).max(0.0) as f32;
    let (pre_rig_translation_span, pre_rig_rotation_span) =
        local_motion_span(&tracker.pre_rig_reference, &pre_rig_current);
    tracker.max_pre_rig_translation_span = tracker
        .max_pre_rig_translation_span
        .max(pre_rig_translation_span);
    tracker.max_pre_rig_rotation_span_radians = tracker
        .max_pre_rig_rotation_span_radians
        .max(pre_rig_rotation_span);
    let (final_translation_span, final_rotation_span) =
        local_motion_span(&tracker.final_reference, final_pose);
    tracker.max_final_translation_span = tracker
        .max_final_translation_span
        .max(final_translation_span);
    tracker.max_final_rotation_span_radians = tracker
        .max_final_rotation_span_radians
        .max(final_rotation_span);
    if let Some(actor_yaw) = actor_yaw.filter(|value| value.is_finite()) {
        tracker.actor_yaw_sample_count += 1;
        if let Some(reference) = tracker.actor_yaw_reference {
            let delta = wrapped_yaw_delta(reference, actor_yaw);
            tracker.max_actor_yaw_delta_radians =
                tracker.max_actor_yaw_delta_radians.max(delta.abs());
            if let Some(expected_sign) = tracker.expected_actor_yaw_delta_sign {
                let signed_delta = delta * f32::from(expected_sign);
                tracker.max_actor_yaw_signed_delta_radians = tracker
                    .max_actor_yaw_signed_delta_radians
                    .max(signed_delta.max(0.0));
                tracker.max_actor_yaw_opposite_delta_radians = tracker
                    .max_actor_yaw_opposite_delta_radians
                    .max((-signed_delta).max(0.0));
            }
        } else {
            tracker.actor_yaw_reference = Some(actor_yaw);
        }
        if let Some(view_yaw) = view_yaw.filter(|value| value.is_finite()) {
            let error = wrapped_yaw_distance(actor_yaw, view_yaw);
            if let Some(reference) = tracker.actor_yaw_error_reference {
                tracker.max_actor_yaw_convergence_radians = tracker
                    .max_actor_yaw_convergence_radians
                    .max((reference - error).max(0.0));
            } else {
                tracker.actor_yaw_error_reference = Some(error);
            }
        }
    }
    tracker.root_pose_sample_count += 1;
    tracker.root_pose_max_active_channels = tracker
        .root_pose_max_active_channels
        .max(root_pose.active_channel_count);
    if root_pose.output_present && root_pose.active_channel_count > 0 {
        tracker.root_pose_active_samples += 1;
    } else {
        tracker.root_pose_reference_samples += 1;
    }
    if let Some(root_pose_value) = root_pose_value {
        if let Some(reference) = tracker.root_pose_reference.as_ref() {
            let (translation_span, rotation_span) =
                source_pose_motion_span(reference, root_pose_value);
            tracker.root_pose_max_translation_span =
                tracker.root_pose_max_translation_span.max(translation_span);
            tracker.root_pose_max_rotation_span_radians = tracker
                .root_pose_max_rotation_span_radians
                .max(rotation_span);
        } else {
            tracker.root_pose_reference = Some(root_pose_value.clone());
        }
    }
    if !tracker.direction_signature_group.is_empty()
        && let (Some(target_phase), Some(sync_phase)) = (
            tracker.canonical_sync_phase,
            evidence.sync_phase_global.and_then(|key| {
                player
                    .get_global_input_data(key)
                    .and_then(|value| value.as_f32().ok())
                    .filter(|value| value.is_finite())
            }),
        )
    {
        let distance = wrapped_phase_distance(sync_phase, target_phase);
        if distance <= tracker.maximum_sync_phase_distance
            && tracker
                .canonical_sync_phase_distance
                .is_none_or(|current| distance < current)
        {
            tracker.canonical_sync_phase_distance = Some(distance);
            tracker.canonical_pre_rig_pose = pre_rig_current;
        }
    }
    if evidence.requires_source_pose_chain {
        record_source_pose_chain(tracker, evidence, player, asset_server, graphs);
    }
    tracker.sample_count += 1;
    tracker.requires_non_reference_root_pose
        && tracker.root_pose_reference_samples >= 2
        && tracker.root_pose_active_samples == 0
}

fn stats(values: &[f64]) -> TimeStats {
    if values.is_empty() {
        return TimeStats {
            samples: 0,
            p50_ms: 0.0,
            p95_ms: 0.0,
            max_ms: 0.0,
        };
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(|left, right| left.total_cmp(right));
    let p50_index = (sorted.len() - 1) / 2;
    let p95_index = ((sorted.len() as f64 * 0.95).ceil() as usize)
        .saturating_sub(1)
        .min(sorted.len() - 1);
    TimeStats {
        samples: sorted.len(),
        p50_ms: sorted[p50_index],
        p95_ms: sorted[p95_index],
        max_ms: *sorted.last().unwrap_or(&0.0),
    }
}

fn count_stats(values: &[f64]) -> CountStats {
    let stats = stats(values);
    CountStats {
        samples: stats.samples,
        p50: stats.p50_ms,
        p95: stats.p95_ms,
        max: stats.max_ms,
    }
}

fn safe(value: f32) -> f32 {
    value.is_finite().then_some(value).unwrap_or(0.0)
}

fn scheduled_capture_allowed(direction_settled: bool, teleport_detected: bool) -> bool {
    !direction_settled && !teleport_detected
}

fn direction_capture_allowed(direction_settled: bool, teleport_detected: bool) -> bool {
    direction_settled && !teleport_detected
}

fn nonfault_capture_allowed(fault_detected: bool, capture_allowed: bool) -> bool {
    !fault_detected && capture_allowed
}

fn terminal_lifecycle_sample_required(evidence: &ManualAnimationWindowEvidence) -> bool {
    evidence.capture_source_window
        && evidence.observed_lifecycle_stages.iter().any(|stage| {
            matches!(
                *stage,
                "sequence_completed" | "natural_completion" | "inertialization" | "settled"
            )
        })
}

fn terminal_parent_state_sample(evidence: &ManualAnimationWindowEvidence) -> bool {
    evidence.capture_source_window
        && evidence
            .observed_lifecycle_stages
            .iter()
            .any(|stage| *stage == "settled")
}

fn temporal_guard_should_rebase(
    previous_context: Option<&str>,
    context: &str,
    previous_window_settled: bool,
    window_settled: bool,
    previous_rebase_epoch: Option<u64>,
    rebase_epoch: u64,
) -> bool {
    previous_rebase_epoch != Some(rebase_epoch)
        || previous_context != Some(context)
        || (window_settled && !previous_window_settled)
}

fn sample_vec3(value: Vec3) -> [f32; 3] {
    [safe(value.x), safe(value.y), safe(value.z)]
}

fn foot_ik_side_sample(
    space: &AlsFootIkSpaceSide,
    apply: &AlsFootIkApplyTelemetrySide,
) -> FootIkSideSample {
    FootIkSideSample {
        valid: space.valid && apply.valid,
        has_ground_hit: space.has_ground_hit,
        trace_target_world: sample_vec3(space.trace_target_world),
        solver_reference_world: sample_vec3(space.solver_reference_world),
        target_world: sample_vec3(space.target_world),
        resolved_target_world: sample_vec3(space.resolved_target_world),
        pole_location_world: sample_vec3(space.pole_location_world),
        effector_world: sample_vec3(apply.effector_world),
        hip_world: sample_vec3(apply.hip_world),
        knee_world: sample_vec3(apply.knee_world),
        ankle_world: sample_vec3(apply.ankle_world),
        raw_offset: safe(space.raw_offset),
        applied_weight: safe(apply.applied_weight),
        pole_sign: safe(apply.pole_sign),
        knee_error: safe(apply.knee_error),
        effector_error: safe(apply.effector_error),
    }
}

fn foot_ik_sample(
    spaces: &AlsFootIkSpaceState,
    apply: &AlsFootIkApplyTelemetry,
    input: &AlsControlRigInput,
    teleport: Option<&AlsFootLockTeleportTelemetry>,
    context: Option<&AlsRootAnimationContext>,
    standing_pose_moving_global: f32,
    pose_moving_lineage: Vec<ContextCurveSample>,
) -> FootIkSample {
    let teleport = teleport.copied().unwrap_or_default();
    let pose_moving = context.and_then(|value| value.f32("PoseMoving"));
    let foot_left_ik_curve = context.and_then(|value| value.f32("FootLeftIk"));
    let foot_right_ik_curve = context.and_then(|value| value.f32("FootRightIk"));
    let foot_left_lock_curve = context.and_then(|value| value.f32("FootLeftLock"));
    let foot_right_lock_curve = context.and_then(|value| value.f32("FootRightLock"));
    FootIkSample {
        left: foot_ik_side_sample(&spaces.left, &apply.left),
        right: foot_ik_side_sample(&spaces.right, &apply.right),
        source_curves_available: pose_moving.is_some()
            && foot_left_ik_curve.is_some()
            && foot_right_ik_curve.is_some()
            && foot_left_lock_curve.is_some()
            && foot_right_lock_curve.is_some(),
        pose_moving: safe(pose_moving.unwrap_or_default()),
        foot_left_ik_curve: safe(foot_left_ik_curve.unwrap_or_default()),
        foot_right_ik_curve: safe(foot_right_ik_curve.unwrap_or_default()),
        foot_left_lock_curve: safe(foot_left_lock_curve.unwrap_or_default()),
        foot_right_lock_curve: safe(foot_right_lock_curve.unwrap_or_default()),
        standing_pose_moving_global: safe(standing_pose_moving_global),
        pose_moving_lineage,
        velocity_blend_forward_amount: safe(input.velocity_blend_forward_amount),
        velocity_blend_backward_amount: safe(input.velocity_blend_backward_amount),
        teleport: FootLockTeleportSample {
            detected_count: teleport.detected_count,
            left_rebase_count: teleport.left_rebase_count,
            right_rebase_count: teleport.right_rebase_count,
            last_translation_delta_m: safe(teleport.last_translation_delta_m),
            remaining_seconds: safe(teleport.remaining_seconds),
        },
    }
}

fn pose_moving_lineage(
    player: &AnimationGraphPlayer,
    graph: &AnimationGraph,
) -> Vec<ContextCurveSample> {
    [
        "AB_Als_Standing",
        "AB_Als_Grounded",
        "Slot 'Transition'",
        "AB_Als_Locomotion",
        "Slot 'PostLocomotion'",
        "ALI_Overlay - Overlay",
        "AB_Als_Layering",
        "AB_Als_Head",
        "ControlRig CR_Als",
        "AB_Als_Ragdolling",
    ]
    .into_iter()
    .map(|node_name| {
        let pose_moving = graph
            .nodes
            .values()
            .find(|node| node.name == node_name)
            .and_then(|node| player_animation_context_state(player, node.id))
            .and_then(|state| state.curves.get("PoseMoving"))
            .copied()
            .filter(|value| value.is_finite());
        ContextCurveSample {
            node: node_name.to_owned(),
            pose_moving,
        }
    })
    .collect()
}

fn segment_name(first: &str, second: &str) -> String {
    format!("{first}->{second}")
}

fn snapshot(
    elapsed_secs: f64,
    frame_ms: f64,
    bones: &BTreeMap<String, (Vec3, Quat, Vec3, Vec3)>,
    direction_case: Option<&str>,
    phase: Option<&str>,
    phase_elapsed_secs: Option<f32>,
    locomotion: Option<&str>,
    gait: Option<&str>,
    stance: Option<&str>,
    rotation_mode: Option<&str>,
    foot_ik: Option<&FootIkSample>,
    root_pose: RootPoseActivity,
) -> PoseSample {
    let bones = bones
        .iter()
        .map(|(name, (translation, rotation, scale, local_scale))| {
            (
                name.clone(),
                BoneSample {
                    translation: [
                        safe(translation.x),
                        safe(translation.y),
                        safe(translation.z),
                    ],
                    rotation: [
                        safe(rotation.x),
                        safe(rotation.y),
                        safe(rotation.z),
                        safe(rotation.w),
                    ],
                    scale: [safe(scale.x), safe(scale.y), safe(scale.z)],
                    local_scale: [
                        safe(local_scale.x),
                        safe(local_scale.y),
                        safe(local_scale.z),
                    ],
                },
            )
        })
        .collect::<BTreeMap<_, _>>();
    let segments = SEGMENTS
        .iter()
        .filter_map(|(first, second)| {
            let first_position = bones.get(*first)?;
            let second_position = bones.get(*second)?;
            let first_translation = Vec3::from_array(first_position.translation);
            let second_translation = Vec3::from_array(second_position.translation);
            Some((
                segment_name(first, second),
                first_translation.distance(second_translation),
            ))
        })
        .collect();
    PoseSample {
        elapsed_secs,
        frame_ms,
        direction_case: direction_case.map(str::to_owned),
        phase: phase.map(str::to_owned),
        phase_elapsed_secs,
        locomotion: locomotion.map(str::to_owned),
        gait: gait.map(str::to_owned),
        stance: stance.map(str::to_owned),
        rotation_mode: rotation_mode.map(str::to_owned),
        bones,
        segments,
        foot_ik: foot_ik.cloned(),
        root_pose: root_pose_sample(root_pose),
    }
}

fn write_report(
    config: &AnimationVisualValidationConfig,
    state: &AnimationVisualValidationState,
    stages: &AnimationVisualStageTiming,
) {
    let observed = state
        .pose_samples
        .last()
        .map(|sample| sample.bones.keys().cloned().collect::<Vec<_>>())
        .unwrap_or_default();
    let missing_bones = REQUIRED_BONES
        .iter()
        .filter(|name| !observed.iter().any(|observed_name| observed_name == **name))
        .map(|name| (*name).to_owned())
        .collect::<Vec<_>>();
    let frame_time = stats(&state.frame_ms);
    let cpu_frame_time = stats(&state.cpu_frame_ms);
    let animation_eval_time = stats(&state.eval_ms);
    let physics_steps_per_render = count_stats(&stages.physics_steps_per_render);
    let stage_time = stages
        .samples
        .iter()
        .map(|(name, values)| (name.clone(), stats(values)))
        .collect();
    let ready = state.pose_samples.len() >= config.min_pose_samples
        && state.screenshots.len() >= config.capture_count
        && frame_time.samples >= config.min_pose_samples
        && animation_eval_time.samples >= config.min_pose_samples
        && physics_steps_per_render.samples >= config.min_pose_samples;
    let pass = ready
        && missing_bones.is_empty()
        && state.faults.is_empty()
        && state.temporal_anomalies.is_empty()
        && state.graph_errors.is_empty()
        && state.primary_failure.is_none()
        && frame_time.p95_ms <= config.max_frame_p95_ms
        && frame_time.max_ms <= config.max_frame_ms
        && animation_eval_time.p95_ms <= config.max_eval_p95_ms
        && physics_steps_per_render.max <= config.max_physics_steps_per_render;
    let thresholds = BTreeMap::from([
        ("max_frame_p95_ms".to_owned(), config.max_frame_p95_ms),
        ("max_frame_ms".to_owned(), config.max_frame_ms),
        ("max_eval_p95_ms".to_owned(), config.max_eval_p95_ms),
        (
            "max_physics_steps_per_render".to_owned(),
            config.max_physics_steps_per_render,
        ),
        (
            "temporal_global_position_speed_mps".to_owned(),
            TEMPORAL_GLOBAL_POSITION_SPEED_METERS_PER_SECOND as f64,
        ),
        (
            "temporal_local_translation_jump_m".to_owned(),
            TEMPORAL_LOCAL_TRANSLATION_JUMP_METERS as f64,
        ),
        (
            "temporal_local_rotation_jump_radians".to_owned(),
            TEMPORAL_LOCAL_ROTATION_JUMP_RADIANS as f64,
        ),
        (
            "temporal_scale_ratio".to_owned(),
            TEMPORAL_SCALE_RATIO as f64,
        ),
        (
            "temporal_segment_ratio".to_owned(),
            TEMPORAL_SEGMENT_RATIO as f64,
        ),
        (
            "temporal_limb_plane_flip_dot".to_owned(),
            TEMPORAL_LIMB_PLANE_FLIP_DOT as f64,
        ),
    ]);
    let report = AnimationVisualReport {
        format: animation_visual_evidence_format(),
        case: config.case.clone(),
        ready,
        pass,
        observed_bones: state.observed_bones,
        required_bones: REQUIRED_BONES
            .iter()
            .map(|name| (*name).to_owned())
            .collect(),
        missing_bones,
        frame_time,
        cpu_frame_time,
        animation_eval_time,
        physics_steps_per_render,
        stage_time,
        excluded_frame_samples: state.excluded_frame_samples,
        pose_samples: state.pose_samples.clone(),
        screenshots: state.screenshots.clone(),
        motion_windows: state
            .motion_windows
            .iter()
            .map(|(label, tracker)| MotionWindowSample {
                label: label.clone(),
                source_node: tracker.source_node.clone(),
                source_state: tracker.source_state.clone(),
                expected_direction: tracker.expected_direction.clone(),
                observed_directions: tracker.observed_directions.iter().cloned().collect(),
                expected_runtime_state: tracker.expected_runtime_state,
                observed_runtime_states: tracker.observed_runtime_states.iter().copied().collect(),
                expected_parent_runtime_state: tracker.expected_parent_runtime_state,
                expected_terminal_parent_runtime_state: tracker
                    .expected_terminal_parent_runtime_state,
                expected_terminal_parent_transition_active: tracker
                    .expected_terminal_parent_transition_active,
                expected_terminal_requested_parent_transition_active: tracker
                    .expected_terminal_requested_parent_transition_active,
                observed_parent_runtime_states: tracker
                    .observed_parent_runtime_states
                    .iter()
                    .copied()
                    .collect(),
                observed_parent_transition_active: tracker
                    .observed_parent_transition_active
                    .iter()
                    .copied()
                    .collect(),
                observed_requested_parent_runtime_states: tracker
                    .observed_requested_parent_runtime_states
                    .iter()
                    .copied()
                    .collect(),
                observed_requested_parent_transition_active: tracker
                    .observed_requested_parent_transition_active
                    .iter()
                    .copied()
                    .collect(),
                observed_terminal_parent_runtime_states: tracker
                    .observed_terminal_parent_runtime_states
                    .iter()
                    .copied()
                    .collect(),
                observed_terminal_parent_transition_active: tracker
                    .observed_terminal_parent_transition_active
                    .iter()
                    .copied()
                    .collect(),
                observed_terminal_requested_parent_runtime_states: tracker
                    .observed_terminal_requested_parent_runtime_states
                    .iter()
                    .copied()
                    .collect(),
                observed_terminal_requested_parent_transition_active: tracker
                    .observed_terminal_requested_parent_transition_active
                    .iter()
                    .copied()
                    .collect(),
                sample_count: tracker.sample_count,
                requires_pre_rig_motion: tracker.requires_pre_rig_motion,
                requires_final_pose_motion: tracker.requires_final_pose_motion,
                requires_non_reference_root_pose: tracker.requires_non_reference_root_pose,
                requires_source_pose_chain: tracker.requires_source_pose_chain,
                requires_actor_yaw_motion: tracker.requires_actor_yaw_motion,
                requires_actor_yaw_convergence: tracker.requires_actor_yaw_convergence,
                expected_actor_yaw_delta_sign: tracker.expected_actor_yaw_delta_sign,
                expected_actor_yaw_baseline: tracker.expected_actor_yaw_baseline,
                expected_animation_variant: tracker.expected_animation_variant,
                minimum_pre_rig_translation_span: tracker.minimum_pre_rig_translation_span,
                minimum_pre_rig_rotation_span_radians: tracker
                    .minimum_pre_rig_rotation_span_radians,
                minimum_final_translation_span: tracker.minimum_final_translation_span,
                minimum_final_rotation_span_radians: tracker.minimum_final_rotation_span_radians,
                minimum_actor_yaw_delta_radians: tracker.minimum_actor_yaw_delta_radians,
                minimum_actor_yaw_convergence_radians: tracker
                    .minimum_actor_yaw_convergence_radians,
                minimum_observation_seconds: tracker.minimum_observation_seconds,
                observation_seconds: tracker.observation_seconds,
                max_pre_rig_translation_span: tracker.max_pre_rig_translation_span,
                max_pre_rig_rotation_span_radians: tracker.max_pre_rig_rotation_span_radians,
                max_final_translation_span: tracker.max_final_translation_span,
                max_final_rotation_span_radians: tracker.max_final_rotation_span_radians,
                root_pose_sample_count: tracker.root_pose_sample_count,
                root_pose_active_samples: tracker.root_pose_active_samples,
                root_pose_reference_samples: tracker.root_pose_reference_samples,
                root_pose_max_active_channels: tracker.root_pose_max_active_channels,
                root_pose_max_translation_span: tracker.root_pose_max_translation_span,
                root_pose_max_rotation_span_radians: tracker.root_pose_max_rotation_span_radians,
                actor_yaw_sample_count: tracker.actor_yaw_sample_count,
                actor_yaw_trace: tracker.actor_yaw_trace.clone(),
                max_actor_yaw_delta_radians: tracker.max_actor_yaw_delta_radians,
                max_actor_yaw_signed_delta_radians: tracker.max_actor_yaw_signed_delta_radians,
                max_actor_yaw_opposite_delta_radians: tracker.max_actor_yaw_opposite_delta_radians,
                max_actor_yaw_convergence_radians: tracker.max_actor_yaw_convergence_radians,
                source_sync_group: tracker.source_sync_group.clone(),
                sync_phase_global: tracker.sync_phase_global.clone(),
                canonical_sync_phase: tracker.canonical_sync_phase,
                maximum_sync_phase_distance: tracker.maximum_sync_phase_distance,
                canonical_sync_phase_distance: tracker.canonical_sync_phase_distance,
                direction_signature_group: tracker.direction_signature_group.clone(),
                required_direction_signature_members: tracker.required_direction_signature_members,
                minimum_direction_signature_rotation_radians: tracker
                    .minimum_direction_signature_rotation_radians,
                canonical_pre_rig_pose: local_motion_pose_sample(&tracker.canonical_pre_rig_pose),
                source_pose_chain: tracker
                    .source_pose_chain
                    .iter()
                    .map(|boundary| SourcePoseBoundarySample {
                        label: boundary.label.clone(),
                        graph: boundary.graph.clone(),
                        node: boundary.node.clone(),
                        output: boundary.output.clone(),
                        minimum_translation_span: boundary.minimum_translation_span,
                        minimum_rotation_span_radians: boundary.minimum_rotation_span_radians,
                        sample_count: boundary.sample_count,
                        available_samples: boundary.available_samples,
                        unavailable_samples: boundary.unavailable_samples,
                        max_translation_span: boundary.max_translation_span,
                        max_rotation_span_radians: boundary.max_rotation_span_radians,
                    })
                    .collect(),
                expected_lifecycle_stages: tracker
                    .expected_lifecycle_stages
                    .iter()
                    .cloned()
                    .collect(),
                observed_lifecycle_stages: tracker
                    .observed_lifecycle_stages
                    .iter()
                    .cloned()
                    .collect(),
                first_failed_source_pose_boundary: tracker
                    .requires_source_pose_chain
                    .then(|| first_source_pose_boundary_failure(&tracker.source_pose_chain))
                    .flatten(),
            })
            .collect(),
        required_motion_windows: state.required_motion_windows.iter().cloned().collect(),
        temporal_anomalies: state.temporal_anomalies.clone(),
        faults: state.faults.clone(),
        graph_errors: state.graph_errors.clone(),
        primary_failure: state.primary_failure.clone(),
        thresholds,
    };
    let path = config.directory.join("report.json");
    if let Ok(serialized) = serde_json::to_vec_pretty(&report) {
        let _ = fs::write(path, serialized);
    }
}

#[allow(clippy::too_many_arguments)]
fn record_graph_evaluation_failure(
    commands: &mut Commands,
    config: &AnimationVisualValidationConfig,
    state: &mut AnimationVisualValidationState,
    stages: &AnimationVisualStageTiming,
    error: &GraphError,
    player: &AnimationGraphPlayer,
    asset_server: &AssetServer,
    graphs: &Assets<AnimationGraph>,
    root_pose: RootPoseActivity,
    direction_case: Option<&str>,
    phase: Option<&str>,
    phase_elapsed_secs: Option<f32>,
    locomotion: Option<&str>,
    gait: Option<&str>,
    stance: Option<&str>,
    rotation_mode: Option<&str>,
    foot_ik: Option<&FootIkSample>,
    bones: &BTreeMap<String, FinalBone>,
    has_camera: bool,
) {
    let elapsed_secs = state
        .started
        .map(|started| started.elapsed().as_secs_f64())
        .unwrap_or_default();
    let mut failure = graph_evaluation_failure(
        config,
        error,
        player,
        asset_server,
        graphs,
        root_pose,
        direction_case.map(str::to_owned),
        phase.map(str::to_owned),
        phase_elapsed_secs,
        locomotion.map(str::to_owned),
        gait.map(str::to_owned),
        stance.map(str::to_owned),
        rotation_mode.map(str::to_owned),
        state.started.is_none(),
    );
    if has_camera {
        let path = config.directory.join("graph_error.png");
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(path.clone()));
        let path = path.to_string_lossy().into_owned();
        state.screenshots.push(ScreenshotSample {
            path: path.clone(),
            elapsed_secs,
            kind: "graph_error".to_owned(),
            direction_case: direction_case.map(str::to_owned),
            phase: phase.map(str::to_owned),
            phase_elapsed_secs,
            locomotion: locomotion.map(str::to_owned),
            gait: gait.map(str::to_owned),
            stance: stance.map(str::to_owned),
            rotation_mode: rotation_mode.map(str::to_owned),
            faults: vec![format!("graph_evaluation_error:{}", failure.error_kind)],
        });
        failure.screenshot_status = "capture_scheduled".to_owned();
        failure.screenshot_path = Some(path);
    } else {
        failure.screenshot_status = "unavailable_no_camera".to_owned();
    }
    append_unique(
        &mut state.graph_errors,
        format!(
            "{}; root_graph_recursive_dependencies={}",
            failure.raw_error, failure.root_graph_recursive_dependencies
        ),
    );
    state.observed_bones = state.observed_bones.max(bones.len());
    let mut sample = snapshot(
        elapsed_secs,
        0.0,
        bones,
        direction_case,
        phase,
        phase_elapsed_secs,
        locomotion,
        gait,
        stance,
        rotation_mode,
        foot_ik,
        root_pose,
    );
    sample.segments = segment_lengths(bones);
    state.pose_samples.push(sample);
    state.primary_failure = Some(failure);
    state.fatal_exit_frames_remaining = Some(2);
    write_report(config, state, stages);
}

fn finish_fatal_animation_visual_failure(
    config: Res<AnimationVisualValidationConfig>,
    mut state: ResMut<AnimationVisualValidationState>,
    stages: Res<AnimationVisualStageTiming>,
    mut exit: MessageWriter<AppExit>,
) {
    let Some(remaining) = state.fatal_exit_frames_remaining else {
        return;
    };
    let exiting = remaining <= 1;
    refresh_graph_failure_screenshot_status(&mut state, exiting);
    write_report(&config, &state, &stages);
    if exiting {
        state.fatal_exit_frames_remaining = None;
        exit.write(AppExit::Error(NonZeroU8::new(92).unwrap()));
    } else {
        state.fatal_exit_frames_remaining = Some(remaining - 1);
    }
}

fn collect_animation_visual_evidence(
    mut commands: Commands,
    config: Res<AnimationVisualValidationConfig>,
    mut state: ResMut<AnimationVisualValidationState>,
    mut stages: ResMut<AnimationVisualStageTiming>,
    animation_window: Option<Res<ManualAnimationWindowEvidence>>,
    phase_evidence: Option<Res<ManualRuntimePhaseEvidence>>,
    pre_rig: Res<AnimationVisualPreRigPose>,
    asset_server: Res<AssetServer>,
    graphs: Res<Assets<AnimationGraph>>,
    avatars: Query<(Entity, &AnimatedSceneInstance, &GlobalTransform), With<AlsCharacterAvatar>>,
    players: Query<(&AnimationGraphPlayer, Option<&AlsRootAnimationContext>)>,
    foot_ik: Query<
        (
            &AlsFootIkSpaceState,
            &AlsFootIkApplyTelemetry,
            &AlsControlRigInput,
            Option<&AlsFootLockTeleportTelemetry>,
        ),
        With<AlsCharacter>,
    >,
    children: Query<&Children>,
    named_transforms: Query<(&Name, &GlobalTransform, &Transform)>,
    cameras: Query<Entity, With<Camera3d>>,
) {
    let Ok((avatar, instance, avatar_transform)) = avatars.single() else {
        return;
    };
    let Ok((player, context)) = players.get(instance.player_entity()) else {
        return;
    };
    if state.fatal_exit_frames_remaining.is_some() {
        return;
    }
    let graph_asset = match player.get_animation_source() {
        AnimationSource::Graph(handle) => graphs.get(handle),
        _ => None,
    };
    let graph_ready = graph_asset.is_some();
    let graph_error = player.get_error();
    let root_pose = root_pose_activity(player);
    let root_pose_value = player.get_default_output_pose();
    let direction_case = animation_window
        .as_deref()
        .and_then(|evidence| evidence.label)
        .map(str::to_owned);
    let direction_settled = animation_window
        .as_deref()
        .is_some_and(|evidence| evidence.settled);
    if let Some(evidence) = animation_window.as_deref()
        && let Some(label) = evidence.label
        && (evidence.requires_pre_rig_motion
            || evidence.requires_final_pose_motion
            || evidence.requires_non_reference_root_pose
            || evidence.requires_source_pose_chain
            || evidence.requires_actor_yaw_motion
            || !evidence.expected_lifecycle_stages.is_empty())
    {
        state.required_motion_windows.insert(label.to_owned());
    }
    let phase = phase_evidence
        .as_deref()
        .and_then(|evidence| (!evidence.phase.is_empty()).then_some(evidence.phase.as_str()));
    let phase_elapsed_secs = phase_evidence
        .as_deref()
        .map(|evidence| evidence.elapsed_seconds)
        .filter(|value| value.is_finite() && *value >= 0.0);
    let locomotion = phase_evidence.as_deref().and_then(|evidence| {
        (!evidence.locomotion.is_empty()).then_some(evidence.locomotion.as_str())
    });
    let gait = phase_evidence
        .as_deref()
        .and_then(|evidence| (!evidence.gait.is_empty()).then_some(evidence.gait.as_str()));
    let stance = phase_evidence
        .as_deref()
        .and_then(|evidence| (!evidence.stance.is_empty()).then_some(evidence.stance.as_str()));
    let rotation_mode = phase_evidence.as_deref().and_then(|evidence| {
        (!evidence.rotation_mode.is_empty()).then_some(evidence.rotation_mode.as_str())
    });
    let temporal_rebase_epoch = phase_evidence
        .as_deref()
        .map(|evidence| evidence.temporal_rebase_epoch)
        .unwrap_or_default();
    let temporal_window_requires = animation_window.as_deref().is_some_and(|evidence| {
        evidence.requires_pre_rig_motion
            || evidence.requires_final_pose_motion
            || evidence.requires_non_reference_root_pose
            || evidence.requires_source_pose_chain
            || evidence.requires_actor_yaw_motion
            || !evidence.expected_lifecycle_stages.is_empty()
    });
    let temporal_observable = !temporal_window_requires || direction_settled;
    let temporal_context = format!(
        "{}|{}|{}",
        phase.unwrap_or_default(),
        direction_case.as_deref().unwrap_or_default(),
        stance.unwrap_or_default(),
    );
    let temporal_rebase = temporal_guard_should_rebase(
        state.temporal_context.as_deref(),
        &temporal_context,
        state.temporal_window_settled,
        direction_settled,
        state.temporal_rebase_epoch,
        temporal_rebase_epoch,
    );
    if temporal_rebase {
        state.temporal_pre_rig_guard.reset();
        state.temporal_final_guard.reset();
    }
    state.temporal_context = Some(temporal_context);
    state.temporal_window_settled = direction_settled;
    state.temporal_rebase_epoch = Some(temporal_rebase_epoch);
    let standing_pose_moving_global = player
        .get_global_input_data("als_standing_curve_pose_moving")
        .and_then(|value| value.as_f32().ok())
        .unwrap_or_default();
    let lineage = graph_asset
        .map(|graph| pose_moving_lineage(player, graph))
        .unwrap_or_default();
    let foot_ik_sample = foot_ik
        .single()
        .ok()
        .map(|(spaces, apply, input, teleport)| {
            foot_ik_sample(
                spaces,
                apply,
                input,
                teleport,
                context,
                standing_pose_moving_global,
                lineage,
            )
        });
    if let Some(error) = graph_error.as_ref() {
        let bones = collect_bones(avatar, &children, &named_transforms);
        record_graph_evaluation_failure(
            &mut commands,
            &config,
            &mut state,
            &stages,
            error,
            player,
            &asset_server,
            &graphs,
            root_pose,
            direction_case.as_deref(),
            phase,
            phase_elapsed_secs,
            locomotion,
            gait,
            stance,
            rotation_mode,
            foot_ik_sample.as_ref(),
            &bones,
            cameras.iter().next().is_some(),
        );
        return;
    }
    if state.started.is_none() {
        if !graph_ready {
            return;
        }
        let now = Instant::now();
        state.started = Some(now);
        state.last_frame = Some(now);
        state.next_pose_sample = Some(now);
        state.next_capture = Some(now + config.capture_interval);
        state.next_report = Some(now);
        stages.physics_steps_since_render = 0;
        stages.physics_steps_per_render.clear();
    }
    let now = Instant::now();
    let started = state.started.unwrap_or(now);
    let elapsed = started.elapsed();
    let terminal_lifecycle_sample = animation_window
        .as_deref()
        .is_some_and(terminal_lifecycle_sample_required);
    if let Some(evidence) = animation_window.as_deref() {
        let _ = record_motion_window_metadata(&mut state, evidence);
    }
    let frame_ms = state
        .last_frame
        .replace(now)
        .map(|previous| now.duration_since(previous).as_secs_f64() * 1000.0)
        .unwrap_or(0.0);
    let cpu_frame_ms = state
        .frame_started
        .map(|started| now.duration_since(started).as_secs_f64() * 1000.0)
        .unwrap_or(0.0);
    if elapsed >= config.warmup && frame_ms > 0.0 {
        if state.skip_frame_samples > 0 {
            state.skip_frame_samples -= 1;
            state.excluded_frame_samples += 1;
        } else {
            state.frame_ms.push(frame_ms);
            if cpu_frame_ms > 0.0 {
                state.cpu_frame_ms.push(cpu_frame_ms);
            }
        }
    }
    if elapsed >= config.warmup {
        let physics_steps_since_render = stages.physics_steps_since_render as f64;
        stages
            .physics_steps_per_render
            .push(physics_steps_since_render);
    }
    stages.physics_steps_since_render = 0;
    let mut fault_detected = false;
    if elapsed >= config.warmup && !root_pose_has_animation_channels(root_pose) {
        fault_detected |= append_unique(
            &mut state.faults,
            format!(
                "startup_root_pose:{}",
                graph_error_pose_classification(root_pose)
            ),
        );
    }
    let teleport_detected = foot_ik_sample.as_ref().is_some_and(|sample| {
        let detected = sample.teleport.detected_count > state.last_teleport_count;
        state.last_teleport_count = state
            .last_teleport_count
            .max(sample.teleport.detected_count);
        detected
    });
    if !graph_ready {
        fault_detected |= append_unique(&mut state.faults, "animation_source_not_graph".to_owned());
    }
    let bones = collect_bones(avatar, &children, &named_transforms);
    let final_pose = collect_local_motion_bones(avatar, &children, &named_transforms);
    state.observed_bones = state.observed_bones.max(bones.len());
    if elapsed >= Duration::from_secs(2) {
        for required in REQUIRED_BONES {
            if !bones.contains_key(*required) {
                fault_detected |=
                    append_unique(&mut state.faults, format!("missing_bone:{required}"));
            }
        }
    }
    let avatar_position = avatar_transform.translation();
    let frame_seconds = (frame_ms / 1000.0).max(1e-4) as f32;
    let segments = segment_lengths(&bones);
    let (pre_rig_metrics, final_metrics) = if temporal_observable {
        let pre_rig = state.temporal_pre_rig_guard.observe(
            &pre_rig.bones,
            &BTreeMap::<String, FinalBone>::new(),
            Vec3::ZERO,
            frame_seconds,
        );
        let final_pose =
            state
                .temporal_final_guard
                .observe(&final_pose, &bones, avatar_position, frame_seconds);
        (pre_rig.metrics, final_pose.metrics)
    } else {
        (Vec::new(), Vec::new())
    };
    if elapsed >= config.warmup && temporal_observable {
        fault_detected |= record_temporal_metrics(
            &mut state,
            pre_rig_metrics,
            "pre_rig",
            elapsed.as_secs_f64(),
            frame_ms,
            direction_case.as_deref(),
            phase,
        );
        fault_detected |= record_temporal_metrics(
            &mut state,
            final_metrics,
            "final",
            elapsed.as_secs_f64(),
            frame_ms,
            direction_case.as_deref(),
            phase,
        );
    }
    for (name, (translation, rotation, scale, local_scale)) in &bones {
        let scale_abs = scale.abs();
        let relative = *translation - avatar_position;
        if !translation.is_finite()
            || !rotation.is_finite()
            || !scale.is_finite()
            || rotation.length_squared() < 0.5
        {
            fault_detected |=
                append_unique(&mut state.faults, format!("non_finite_transform:{name}"));
        }
        if scale_abs.min_element() < 0.01 || scale_abs.max_element() > 10.0 {
            fault_detected |=
                append_unique(&mut state.faults, format!("scale_out_of_range:{name}"));
        }
        if !local_scale.is_finite()
            || local_scale.abs().min_element() < 0.01
            || local_scale.abs().max_element() > 10.0
        {
            fault_detected |= append_unique(
                &mut state.faults,
                format!("local_scale_out_of_range:{name}"),
            );
        }
        if relative.length() > 8.0 {
            fault_detected |= append_unique(
                &mut state.faults,
                format!("bone_radius_out_of_range:{name}"),
            );
        }
    }
    for (name, length) in &segments {
        if !length.is_finite() || !(0.015..=3.0).contains(length) {
            fault_detected |=
                append_unique(&mut state.faults, format!("segment_out_of_range:{name}"));
        }
    }
    let mut sampled = false;
    if let Some(next_pose_sample) = state.next_pose_sample {
        if now >= next_pose_sample
            || fault_detected
            || teleport_detected
            || terminal_lifecycle_sample
        {
            let mut sample = snapshot(
                elapsed.as_secs_f64(),
                frame_ms,
                &bones,
                direction_case.as_deref(),
                phase,
                phase_elapsed_secs,
                locomotion,
                gait,
                stance,
                rotation_mode,
                foot_ik_sample.as_ref(),
                root_pose,
            );
            sample.segments = segments.clone();
            state.pose_samples.push(sample);
            if let Some(evidence) = animation_window.as_deref() {
                if record_motion_window(
                    &mut state,
                    evidence,
                    &pre_rig,
                    &final_pose,
                    evidence.observed_actor_yaw,
                    evidence.observed_view_yaw,
                    root_pose,
                    root_pose_value,
                    player,
                    &asset_server,
                    &graphs,
                    elapsed.as_secs_f64(),
                ) {
                    fault_detected |= append_unique(
                        &mut state.faults,
                        format!("reference_root_pose:{}", evidence.label.unwrap_or_default()),
                    );
                }
            }
            state.next_pose_sample = Some(now + Duration::from_millis(100));
            sampled = true;
        }
    }
    if (fault_detected || teleport_detected) && !sampled {
        let mut sample = snapshot(
            elapsed.as_secs_f64(),
            frame_ms,
            &bones,
            direction_case.as_deref(),
            phase,
            phase_elapsed_secs,
            locomotion,
            gait,
            stance,
            rotation_mode,
            foot_ik_sample.as_ref(),
            root_pose,
        );
        sample.segments = segments.clone();
        state.pose_samples.push(sample);
        if let Some(evidence) = animation_window.as_deref() {
            if record_motion_window(
                &mut state,
                evidence,
                &pre_rig,
                &final_pose,
                evidence.observed_actor_yaw,
                evidence.observed_view_yaw,
                root_pose,
                root_pose_value,
                player,
                &asset_server,
                &graphs,
                elapsed.as_secs_f64(),
            ) {
                fault_detected |= append_unique(
                    &mut state.faults,
                    format!("reference_root_pose:{}", evidence.label.unwrap_or_default()),
                );
            }
        }
    }
    if nonfault_capture_allowed(
        fault_detected,
        scheduled_capture_allowed(direction_settled, teleport_detected),
    ) && state.capture_index < config.capture_count
        && state
            .next_capture
            .is_some_and(|next_capture| now >= next_capture)
        && let Some(camera) = cameras.iter().next()
    {
        let path = config
            .directory
            .join(format!("frame_{:02}.png", state.capture_index));
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(path.clone()));
        state.screenshots.push(ScreenshotSample {
            path: path.to_string_lossy().into_owned(),
            elapsed_secs: elapsed.as_secs_f64(),
            kind: "scheduled".to_owned(),
            direction_case: direction_case.clone(),
            phase: phase.map(str::to_owned),
            phase_elapsed_secs,
            locomotion: locomotion.map(str::to_owned),
            gait: gait.map(str::to_owned),
            stance: stance.map(str::to_owned),
            rotation_mode: rotation_mode.map(str::to_owned),
            faults: Vec::new(),
        });
        state.capture_index += 1;
        state.next_capture = Some(now + config.capture_interval);
        state.skip_frame_samples = state.skip_frame_samples.max(3);
        let _ = camera;
    }
    if nonfault_capture_allowed(
        fault_detected,
        direction_capture_allowed(direction_settled, teleport_detected),
    ) && let Some(direction_case) = direction_case.as_ref()
        && state
            .captured_direction_cases
            .insert(direction_case.clone())
        && let Some(camera) = cameras.iter().next()
    {
        let path = config
            .directory
            .join(format!("direction_{direction_case}.png"));
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(path.clone()));
        state.screenshots.push(ScreenshotSample {
            path: path.to_string_lossy().into_owned(),
            elapsed_secs: elapsed.as_secs_f64(),
            kind: "direction".to_owned(),
            direction_case: Some(direction_case.clone()),
            phase: phase.map(str::to_owned),
            phase_elapsed_secs,
            locomotion: locomotion.map(str::to_owned),
            gait: gait.map(str::to_owned),
            stance: stance.map(str::to_owned),
            rotation_mode: rotation_mode.map(str::to_owned),
            faults: Vec::new(),
        });
        state.skip_frame_samples = state.skip_frame_samples.max(3);
        let _ = camera;
    }
    if nonfault_capture_allowed(fault_detected, teleport_detected)
        && state.teleport_captures < 4
        && let Some(camera) = cameras.iter().next()
    {
        let path = config
            .directory
            .join(format!("teleport_{:02}.png", state.teleport_captures));
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(path.clone()));
        state.screenshots.push(ScreenshotSample {
            path: path.to_string_lossy().into_owned(),
            elapsed_secs: elapsed.as_secs_f64(),
            kind: "teleport".to_owned(),
            direction_case: direction_case.clone(),
            phase: phase.map(str::to_owned),
            phase_elapsed_secs,
            locomotion: locomotion.map(str::to_owned),
            gait: gait.map(str::to_owned),
            stance: stance.map(str::to_owned),
            rotation_mode: rotation_mode.map(str::to_owned),
            faults: Vec::new(),
        });
        state.teleport_captures += 1;
        state.skip_frame_samples = state.skip_frame_samples.max(3);
        let _ = camera;
    }
    if fault_detected && state.fault_captures < 4 {
        let path = config
            .directory
            .join(format!("fault_{:02}.png", state.fault_captures));
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(path.clone()));
        let faults = state.faults.clone();
        state.screenshots.push(ScreenshotSample {
            path: path.to_string_lossy().into_owned(),
            elapsed_secs: elapsed.as_secs_f64(),
            kind: "fault".to_owned(),
            direction_case,
            phase: phase.map(str::to_owned),
            phase_elapsed_secs,
            locomotion: locomotion.map(str::to_owned),
            gait: gait.map(str::to_owned),
            stance: stance.map(str::to_owned),
            rotation_mode: rotation_mode.map(str::to_owned),
            faults,
        });
        state.fault_captures += 1;
        state.skip_frame_samples = state.skip_frame_samples.max(3);
    }
    if terminal_lifecycle_sample
        || state
            .next_report
            .is_some_and(|next_report| now >= next_report)
    {
        write_report(&config, &state, &stages);
        state.next_report = Some(now + Duration::from_secs(1));
        state.skip_frame_samples = state.skip_frame_samples.max(2);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn visual_evidence_format_is_loaded_from_the_shared_contract() {
        assert_eq!(
            animation_visual_evidence_format(),
            "als_animation_visual_evidence_v21"
        );
    }

    #[test]
    fn graph_error_diagnostic_preserves_node_time_and_a_pose_risk() {
        let node = NodeId::default();
        let error = GraphError::MissingEdgeToSource(SourcePin::NodeTime(node));
        let pin = graph_error_pin(&error).unwrap();
        assert_eq!(graph_error_kind(&error), "MissingEdgeToSource");
        assert_eq!(pin.kind, "node_time");
        assert_eq!(pin.node_id, Some(format!("{node:?}")));
        assert_eq!(graph_error_node_id(&error), Some(node));
        assert_eq!(
            graph_error_pose_classification(RootPoseActivity::default()),
            "missing_root_pose_likely_reference_or_a_pose"
        );
    }

    #[test]
    fn stats_uses_ordered_percentiles() {
        let value = stats(&[4.0, 1.0, 3.0, 2.0, 5.0]);
        assert_eq!(value.samples, 5);
        assert_eq!(value.p50_ms, 3.0);
        assert_eq!(value.p95_ms, 5.0);
        assert_eq!(value.max_ms, 5.0);
    }

    #[test]
    fn wrapped_yaw_delta_preserves_turn_side() {
        assert!(
            (wrapped_yaw_delta(0.0, 90.0_f32.to_radians()) - 90.0_f32.to_radians()).abs() < 1e-6
        );
        assert!(
            (wrapped_yaw_delta(0.0, -90.0_f32.to_radians()) + 90.0_f32.to_radians()).abs() < 1e-6
        );
        assert!(
            (wrapped_yaw_delta(170.0_f32.to_radians(), -170.0_f32.to_radians())
                - 20.0_f32.to_radians())
            .abs()
                < 1e-6
        );
    }

    #[test]
    fn lifecycle_pulse_is_recorded_without_a_pose_sample() {
        let mut state = AnimationVisualValidationState::default();
        let evidence = ManualAnimationWindowEvidence {
            label: Some("turn"),
            capture_source_window: true,
            expected_actor_yaw_baseline: Some(0.5),
            expected_lifecycle_stages: &["slot_active", "natural_completion", "settled"],
            observed_lifecycle_stages: &["natural_completion", "settled"],
            ..default()
        };
        assert_eq!(
            record_motion_window_metadata(&mut state, &evidence).as_deref(),
            Some("turn")
        );
        let tracker = state.motion_windows.get("turn").unwrap();
        assert_eq!(tracker.sample_count, 0);
        assert!(tracker.expected_lifecycle_stages.contains("slot_active"));
        assert!(
            tracker
                .observed_lifecycle_stages
                .contains("natural_completion")
        );
        assert!(tracker.observed_lifecycle_stages.contains("settled"));
        assert!(tracker.expected_actor_yaw_baseline.is_none());
        assert!(tracker.actor_yaw_reference.is_none());
    }

    #[test]
    fn precondition_window_cannot_record_lifecycle_or_yaw_baseline() {
        let mut state = AnimationVisualValidationState::default();
        let evidence = ManualAnimationWindowEvidence {
            label: Some("turn"),
            capture_source_window: false,
            expected_actor_yaw_baseline: Some(0.5),
            expected_lifecycle_stages: &["slot_active"],
            observed_lifecycle_stages: &["slot_active"],
            ..default()
        };
        assert!(record_motion_window_metadata(&mut state, &evidence).is_none());
        let tracker = state.motion_windows.get("turn").unwrap();
        assert!(tracker.expected_lifecycle_stages.contains("slot_active"));
        assert!(tracker.observed_lifecycle_stages.is_empty());
        assert!(tracker.expected_actor_yaw_baseline.is_none());
        assert!(tracker.actor_yaw_reference.is_none());
    }

    #[test]
    fn terminal_lifecycle_forces_a_pose_sample_and_report_flush() {
        let active = ManualAnimationWindowEvidence {
            capture_source_window: true,
            observed_lifecycle_stages: &["slot_active"],
            ..default()
        };
        assert!(!terminal_lifecycle_sample_required(&active));
        let completion = ManualAnimationWindowEvidence {
            capture_source_window: true,
            observed_lifecycle_stages: &["natural_completion", "inertialization"],
            ..default()
        };
        assert!(terminal_lifecycle_sample_required(&completion));
        let settled = ManualAnimationWindowEvidence {
            capture_source_window: true,
            observed_lifecycle_stages: &["sequence_completed", "settled"],
            ..default()
        };
        assert!(terminal_lifecycle_sample_required(&settled));
        let precondition = ManualAnimationWindowEvidence {
            capture_source_window: false,
            observed_lifecycle_stages: &["settled"],
            ..default()
        };
        assert!(!terminal_lifecycle_sample_required(&precondition));
    }

    #[test]
    fn settled_lifecycle_routes_parent_state_to_the_terminal_contract() {
        let active = ManualAnimationWindowEvidence {
            capture_source_window: true,
            observed_lifecycle_stages: &["slot_active"],
            ..default()
        };
        assert!(!terminal_parent_state_sample(&active));
        let completion = ManualAnimationWindowEvidence {
            capture_source_window: true,
            observed_lifecycle_stages: &["natural_completion", "inertialization"],
            ..default()
        };
        assert!(!terminal_parent_state_sample(&completion));
        let settled = ManualAnimationWindowEvidence {
            capture_source_window: true,
            observed_lifecycle_stages: &["sequence_completed", "settled"],
            ..default()
        };
        assert!(terminal_parent_state_sample(&settled));
    }

    #[test]
    fn segment_name_is_stable() {
        assert_eq!(segment_name("pelvis", "spine_01"), "pelvis->spine_01");
    }

    fn temporal_local_pose(calf_rotation: Quat) -> BTreeMap<String, LocalBonePose> {
        BTreeMap::from([
            (
                "thigh_l".to_owned(),
                LocalBonePose {
                    scale: Vec3::ONE,
                    ..default()
                },
            ),
            (
                "calf_l".to_owned(),
                LocalBonePose {
                    rotation: calf_rotation,
                    scale: Vec3::ONE,
                    ..default()
                },
            ),
            (
                "foot_l".to_owned(),
                LocalBonePose {
                    scale: Vec3::ONE,
                    ..default()
                },
            ),
        ])
    }

    fn temporal_bones(knee_x: f32) -> BTreeMap<String, FinalBone> {
        BTreeMap::from([
            (
                "thigh_l".to_owned(),
                (
                    Vec3::new(0.0, 1.0, 0.0),
                    Quat::IDENTITY,
                    Vec3::ONE,
                    Vec3::ONE,
                ),
            ),
            (
                "calf_l".to_owned(),
                (
                    Vec3::new(knee_x, 0.5, 0.0),
                    Quat::IDENTITY,
                    Vec3::ONE,
                    Vec3::ONE,
                ),
            ),
            (
                "foot_l".to_owned(),
                (Vec3::ZERO, Quat::IDENTITY, Vec3::ONE, Vec3::ONE),
            ),
        ])
    }

    #[test]
    fn temporal_guard_rejects_an_instant_local_leg_twist() {
        let mut guard = TemporalSkeletonGuard::default();
        let bones = temporal_bones(0.25);
        assert!(
            guard
                .observe(
                    &temporal_local_pose(Quat::IDENTITY),
                    &bones,
                    Vec3::ZERO,
                    1.0 / 60.0,
                )
                .metrics
                .is_empty()
        );
        let observation = guard.observe(
            &temporal_local_pose(Quat::from_rotation_x(std::f32::consts::PI)),
            &bones,
            Vec3::ZERO,
            1.0 / 60.0,
        );
        assert!(
            observation
                .metrics
                .iter()
                .any(|value| value.kind == "local_rotation_spike" && value.subject == "calf_l")
        );
    }

    #[test]
    fn temporal_guard_rejects_an_instant_limb_plane_flip() {
        let mut guard = TemporalSkeletonGuard::default();
        let local = temporal_local_pose(Quat::IDENTITY);
        assert!(
            guard
                .observe(&local, &temporal_bones(0.25), Vec3::ZERO, 1.0 / 60.0)
                .metrics
                .is_empty()
        );
        let observation = guard.observe(&local, &temporal_bones(-0.25), Vec3::ZERO, 1.0 / 60.0);
        assert!(
            observation
                .metrics
                .iter()
                .any(|value| value.kind == "limb_plane_flip" && value.subject == "left_leg")
        );
    }

    #[test]
    fn temporal_guard_rejects_an_instant_local_scale_spike() {
        let mut guard = TemporalSkeletonGuard::default();
        let bones = temporal_bones(0.25);
        assert!(
            guard
                .observe(
                    &temporal_local_pose(Quat::IDENTITY),
                    &bones,
                    Vec3::ZERO,
                    1.0 / 60.0,
                )
                .metrics
                .is_empty()
        );
        let mut scaled = temporal_local_pose(Quat::IDENTITY);
        scaled.get_mut("calf_l").unwrap().scale = Vec3::splat(2.0);
        let observation = guard.observe(&scaled, &bones, Vec3::ZERO, 1.0 / 60.0);
        assert!(
            observation
                .metrics
                .iter()
                .any(|value| value.kind == "local_scale_spike" && value.subject == "calf_l")
        );
    }

    #[test]
    fn temporal_guard_rejects_an_instant_local_translation_spike() {
        let mut guard = TemporalSkeletonGuard::default();
        let bones = temporal_bones(0.25);
        let baseline = temporal_local_pose(Quat::IDENTITY);
        assert!(
            guard
                .observe(&baseline, &bones, Vec3::ZERO, 1.0 / 60.0)
                .metrics
                .is_empty()
        );
        let mut displaced = baseline;
        displaced.get_mut("calf_l").unwrap().translation = Vec3::new(1.0, 0.0, 0.0);
        let observation = guard.observe(&displaced, &bones, Vec3::ZERO, 1.0 / 60.0);
        assert!(
            observation
                .metrics
                .iter()
                .any(|value| value.kind == "local_translation_spike" && value.subject == "calf_l")
        );
    }

    #[test]
    fn temporal_guard_rejects_an_instant_segment_and_position_spike() {
        let mut guard = TemporalSkeletonGuard::default();
        let local = temporal_local_pose(Quat::IDENTITY);
        assert!(
            guard
                .observe(&local, &temporal_bones(0.25), Vec3::ZERO, 1.0 / 60.0)
                .metrics
                .is_empty()
        );
        let observation = guard.observe(&local, &temporal_bones(1.5), Vec3::ZERO, 1.0 / 60.0);
        assert!(observation.metrics.iter().any(
            |value| value.kind == "segment_length_spike" && value.subject == "thigh_l->calf_l"
        ));
        assert!(
            observation
                .metrics
                .iter()
                .any(|value| value.kind == "global_position_speed" && value.subject == "calf_l")
        );
    }

    #[test]
    fn temporal_guard_ablation_without_local_history_misses_a_twist() {
        let mut guard = TemporalSkeletonGuard::default();
        let bones = temporal_bones(0.25);
        let baseline = temporal_local_pose(Quat::IDENTITY);
        let _ = guard.observe(&baseline, &bones, Vec3::ZERO, 1.0 / 60.0);
        guard.previous_local.clear();
        let observation = guard.observe(
            &temporal_local_pose(Quat::from_rotation_x(std::f32::consts::PI)),
            &bones,
            Vec3::ZERO,
            1.0 / 60.0,
        );
        assert!(
            !observation
                .metrics
                .iter()
                .any(|value| value.kind == "local_rotation_spike" && value.subject == "calf_l")
        );
    }

    #[test]
    fn temporal_guard_reset_rebases_only_the_first_new_pose() {
        let mut guard = TemporalSkeletonGuard::default();
        let bones = temporal_bones(0.25);
        let baseline = temporal_local_pose(Quat::IDENTITY);
        let _ = guard.observe(&baseline, &bones, Vec3::ZERO, 1.0 / 60.0);
        guard.reset();
        let turned = temporal_local_pose(Quat::from_rotation_x(std::f32::consts::PI));
        assert!(
            !guard
                .observe(&turned, &bones, Vec3::ZERO, 1.0 / 60.0)
                .metrics
                .iter()
                .any(|value| value.kind == "local_rotation_spike")
        );
        assert!(
            guard
                .observe(&baseline, &bones, Vec3::ZERO, 1.0 / 60.0)
                .metrics
                .iter()
                .any(|value| value.kind == "local_rotation_spike")
        );
    }

    #[test]
    fn source_pose_chain_reports_the_first_static_boundary() {
        let boundaries = vec![
            SourcePoseBoundaryTracker {
                label: "sequence_player".to_owned(),
                sample_count: 2,
                available_samples: 2,
                max_rotation_span_radians: 0.1,
                ..default()
            },
            SourcePoseBoundaryTracker {
                label: "cache_producer".to_owned(),
                sample_count: 2,
                available_samples: 2,
                ..default()
            },
            SourcePoseBoundaryTracker {
                label: "movement_fsm".to_owned(),
                sample_count: 2,
                available_samples: 2,
                max_rotation_span_radians: 0.1,
                ..default()
            },
        ];
        assert_eq!(
            first_source_pose_boundary_failure(&boundaries),
            Some("cache_producer: static".to_owned())
        );
    }

    #[test]
    fn physics_step_stage_is_recorded_after_warmup() {
        let config = AnimationVisualValidationConfig {
            directory: PathBuf::new(),
            case: "fixture".to_owned(),
            capture_count: 1,
            capture_interval: Duration::from_secs(1),
            min_pose_samples: 1,
            warmup: Duration::ZERO,
            max_frame_p95_ms: 1.0,
            max_frame_ms: 1.0,
            max_eval_p95_ms: 1.0,
            max_physics_steps_per_render: 1.0,
        };
        let visual = AnimationVisualValidationState {
            started: Some(Instant::now()),
            ..default()
        };
        let mut stages = AnimationVisualStageTiming::default();
        record_stage(
            &config,
            &visual,
            &mut stages,
            "physics_step",
            Some(Instant::now()),
        );
        assert_eq!(stages.samples["physics_step"].len(), 1);
    }

    #[test]
    fn teleport_capture_excludes_competing_window_captures() {
        assert!(!scheduled_capture_allowed(false, true));
        assert!(!direction_capture_allowed(true, true));
        assert!(scheduled_capture_allowed(false, false));
        assert!(direction_capture_allowed(true, false));
    }

    #[test]
    fn fault_capture_preempts_competing_capture_requests() {
        assert!(!nonfault_capture_allowed(true, true));
        assert!(!nonfault_capture_allowed(true, false));
        assert!(nonfault_capture_allowed(false, true));
    }

    #[test]
    fn temporal_guard_rebases_at_declared_window_boundaries_only() {
        assert!(temporal_guard_should_rebase(
            None,
            "phase|case|stance",
            false,
            false,
            None,
            0,
        ));
        assert!(temporal_guard_should_rebase(
            Some("phase|case|stance"),
            "phase|next_case|stance",
            true,
            false,
            Some(0),
            0,
        ));
        assert!(temporal_guard_should_rebase(
            Some("phase|case|stance"),
            "phase|case|stance",
            false,
            true,
            Some(0),
            0,
        ));
        assert!(!temporal_guard_should_rebase(
            Some("phase|case|stance"),
            "phase|case|stance",
            true,
            true,
            Some(0),
            0,
        ));
        assert!(temporal_guard_should_rebase(
            Some("phase|case|stance"),
            "phase|case|stance",
            true,
            true,
            Some(0),
            1,
        ));
    }

    #[test]
    fn root_pose_activity_distinguishes_reference_and_animated_channels() {
        let reference = root_pose_activity_from_pose(&Pose::default());
        assert!(reference.output_present);
        assert_eq!(reference.bone_count, 0);
        assert_eq!(reference.active_channel_count, 0);
        assert!(!root_pose_has_animation_channels(reference));
        let animated = Pose {
            bones: vec![BonePose {
                rotation: Some(Quat::IDENTITY),
                ..default()
            }],
            ..default()
        };
        let activity = root_pose_activity_from_pose(&animated);
        assert!(activity.output_present);
        assert_eq!(activity.bone_count, 1);
        assert_eq!(activity.active_channel_count, 1);
        assert!(root_pose_has_animation_channels(activity));
    }

    #[test]
    fn sync_phase_distance_wraps_at_cycle_boundary() {
        assert!(wrapped_phase_distance(0.98, 0.02) < 0.05);
        assert!((wrapped_phase_distance(0.25, 0.5) - 0.25).abs() < 1e-6);
    }
}
