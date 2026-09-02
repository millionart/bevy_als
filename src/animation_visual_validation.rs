use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::PathBuf,
    time::{Duration, Instant},
};

use als_camera::AlsCameraSet;
use als_core::character::AlsCharacter;
use als_ext::player_animation_context_state;
use avian3d::prelude::PhysicsSystems;
use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use bevy_animation_graph::core::{
    animated_scene::AnimatedSceneInstance,
    animation_graph::AnimationGraph,
    animation_graph_player::{AnimationGraphPlayer, AnimationSource},
    plugin::AnimationGraphSet,
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
    manual_playground_integration::{ManualDirectionEvidence, ManualRuntimePhaseEvidence},
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
    previous_relative: BTreeMap<String, Vec3>,
    previous_segments: BTreeMap<String, f32>,
    captured_direction_cases: BTreeSet<String>,
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
    faults: Vec<String>,
    graph_errors: Vec<String>,
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
                collect_animation_visual_evidence
                    .after(FootIkApplySet::Telemetry)
                    .after(AlsCameraSet),
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
) -> BTreeMap<String, (Vec3, Quat, Vec3, Vec3)> {
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
    FootIkSample {
        left: foot_ik_side_sample(&spaces.left, &apply.left),
        right: foot_ik_side_sample(&spaces.right, &apply.right),
        source_curves_available: pose_moving.is_some()
            && foot_left_ik_curve.is_some()
            && foot_right_ik_curve.is_some(),
        pose_moving: safe(pose_moving.unwrap_or_default()),
        foot_left_ik_curve: safe(foot_left_ik_curve.unwrap_or_default()),
        foot_right_ik_curve: safe(foot_right_ik_curve.unwrap_or_default()),
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
        && state.graph_errors.is_empty()
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
    ]);
    let report = AnimationVisualReport {
        format: "als_animation_visual_evidence_v2",
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
        faults: state.faults.clone(),
        graph_errors: state.graph_errors.clone(),
        thresholds,
    };
    let path = config.directory.join("report.json");
    if let Ok(serialized) = serde_json::to_vec_pretty(&report) {
        let _ = fs::write(path, serialized);
    }
}

fn collect_animation_visual_evidence(
    mut commands: Commands,
    config: Res<AnimationVisualValidationConfig>,
    mut state: ResMut<AnimationVisualValidationState>,
    mut stages: ResMut<AnimationVisualStageTiming>,
    direction_evidence: Option<Res<ManualDirectionEvidence>>,
    phase_evidence: Option<Res<ManualRuntimePhaseEvidence>>,
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
    let graph_asset = match player.get_animation_source() {
        AnimationSource::Graph(handle) => graphs.get(handle),
        _ => None,
    };
    let graph_ready = graph_asset.is_some();
    let graph_error = player.get_error().map(|error| format!("{error:?}"));
    let direction_case = direction_evidence
        .as_deref()
        .and_then(|evidence| evidence.case)
        .map(str::to_owned);
    let direction_settled = direction_evidence
        .as_deref()
        .is_some_and(|evidence| evidence.settled);
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
    if state.started.is_none() {
        if !graph_ready || graph_error.is_some() {
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
    if let Some(error) = graph_error {
        fault_detected |= append_unique(&mut state.graph_errors, error);
    }
    let bones = collect_bones(avatar, &children, &named_transforms);
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
    let mut segments = BTreeMap::new();
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
        if let Some(previous) = state.previous_relative.get(name) {
            if previous.distance(relative) / frame_seconds > 45.0 {
                fault_detected |=
                    append_unique(&mut state.faults, format!("bone_discontinuity:{name}"));
            }
        }
        state.previous_relative.insert(name.clone(), relative);
    }
    for (first, second) in SEGMENTS {
        let Some((first_position, _, _, _)) = bones.get(*first) else {
            continue;
        };
        let Some((second_position, _, _, _)) = bones.get(*second) else {
            continue;
        };
        let name = segment_name(first, second);
        let length = first_position.distance(*second_position);
        if !length.is_finite() || !(0.015..=3.0).contains(&length) {
            fault_detected |=
                append_unique(&mut state.faults, format!("segment_out_of_range:{name}"));
        }
        if let Some(previous) = state.previous_segments.get(&name) {
            if (length - previous).abs() > 0.8 {
                fault_detected |=
                    append_unique(&mut state.faults, format!("segment_discontinuity:{name}"));
            }
        }
        state.previous_segments.insert(name.clone(), length);
        segments.insert(name, length);
    }
    let mut sampled = false;
    if let Some(next_pose_sample) = state.next_pose_sample {
        if now >= next_pose_sample || fault_detected || teleport_detected {
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
            );
            sample.segments = segments.clone();
            state.pose_samples.push(sample);
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
        );
        sample.segments = segments.clone();
        state.pose_samples.push(sample);
    }
    if !direction_settled
        && state.capture_index < config.capture_count
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
    if direction_settled
        && let Some(direction_case) = direction_case.as_ref()
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
    if teleport_detected
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
    if state
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
    fn stats_uses_ordered_percentiles() {
        let value = stats(&[4.0, 1.0, 3.0, 2.0, 5.0]);
        assert_eq!(value.samples, 5);
        assert_eq!(value.p50_ms, 3.0);
        assert_eq!(value.p95_ms, 5.0);
        assert_eq!(value.max_ms, 5.0);
    }

    #[test]
    fn segment_name_is_stable() {
        assert_eq!(segment_name("pelvis", "spine_01"), "pelvis->spine_01");
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
}
