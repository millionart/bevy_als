use std::collections::VecDeque;

use als_core::air::AlsAirState;
use als_core::air::ground_prediction_sweep;
use als_core::character::{
    ALS_CHARACTER_CAPSULE_HALF_HEIGHT, ALS_CHARACTER_CAPSULE_RADIUS, AlsCharacter,
    AlsMovementParamsHandle,
};
use als_core::foot_ik::AlsFootLockState;
use als_core::input::AlsInputCommand;
use als_core::motion::AlsInputOverride;
use als_core::params::AlsMovementParams;
use als_core::state::{LocomotionState, RotationMode, Stance};
use als_ext::{AlsAnimCurves, CurveAsset, SyncPhaseInput, SyncPhaseState, advance_sync_phase};
use avian3d::prelude::{LinearVelocity, SpatialQuery};
use bevy::ecs::hierarchy::ChildOf;
use bevy::prelude::*;
use bevy_animation_graph::core::animated_scene::AnimatedSceneInstance;
use bevy_animation_graph::core::animation_graph_player::AnimationGraphPlayer;
use bevy_animation_graph::core::plugin::AnimationGraphSet;

use crate::character_assembly::AlsCharacterAvatar;
use crate::grounded_layer::AlsGroundedLayerSet;
use crate::turn_layer::AlsTurnDrive;

const JUMP_SPEED_REFERENCE: f32 = 6.0;
const JUMP_PLAY_RATE_MIN: f32 = 1.2;
const JUMP_PLAY_RATE_MAX: f32 = 1.5;
const JUMP_SUCCESS_MIN_UPWARD_VELOCITY: f32 = 0.1;
const JUMP_START_DURATION: f32 = 1.6666665;
const LAND_DURATION: f32 = 1.1666666;
const LAND_MOVEMENT_SPEED_THRESHOLD: f32 = 6.5;
const FALL_DURATION: f32 = 1.0;
const JUMP_DURATION: f32 = 1.666667;
const FLAIL_DURATION: f32 = 0.666667;
const AIR_LEAN_REFERENCE_SPEED: f32 = 3.5;
const LEAN_HALF_LIFE: f32 = 0.2;
const AIR_LEAN_CURVE: &str =
    "curves/ALS/Data/AnimationInstance/Air/CF_Als_LeanAmount_Air.curve.ron";
const GROUND_PREDICTION_CURVE: &str =
    "curves/ALS/Data/AnimationInstance/Air/CF_Als_GroundPredictionAmount.curve.ron";
const JUMP_WALK_LEFT_CURVES: &str = "animations/Air/Jump/A_Als_Jump_Walk_Left.gltf#AlsCurves";
const JUMP_RUN_LEFT_CURVES: &str = "animations/Air/Jump/A_Als_Jump_Run_Left.gltf#AlsCurves";
const JUMP_WALK_RIGHT_CURVES: &str = "animations/Air/Jump/A_Als_Jump_Walk_Right.gltf#AlsCurves";
const JUMP_RUN_RIGHT_CURVES: &str = "animations/Air/Jump/A_Als_Jump_Run_Right.gltf#AlsCurves";
const LAND_LIGHT_CURVES: &str = "animations/Air/Land/A_Als_Land_Light.gltf#AlsCurves";
const LAND_HEAVY_CURVES: &str = "animations/Air/Land/A_Als_Land_Heavy.gltf#AlsCurves";
const LAND_LIGHT_ADDITIVE_CURVES: &str =
    "animations/Air/Land/A_Als_Land_Light_Additive.gltf#AlsCurves";
const LAND_HEAVY_ADDITIVE_CURVES: &str =
    "animations/Air/Land/A_Als_Land_Heavy_Additive.gltf#AlsCurves";

#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AlsMovementBaseAnimationState {
    pub has_relative_location: bool,
}

#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct AlsGroundPredictionState {
    pub amount: f32,
}

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AlsLocomotionLayerSet;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum LocomotionLayerState {
    #[default]
    Grounded,
    Fall,
    Jump,
    Land,
    LandMovement,
    LandConduct,
}

impl LocomotionLayerState {
    fn index(self) -> f32 {
        match self {
            Self::Grounded => 0.0,
            Self::Fall => 1.0,
            Self::Jump => 2.0,
            Self::Land => 3.0,
            Self::LandMovement => 4.0,
            Self::LandConduct => 5.0,
        }
    }
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LocomotionTransition {
    GroundedToJump,
    GroundedToFall,
    FallToLandConduct,
    FallToGrounded,
    JumpToLandConduct,
    JumpToGrounded,
    LandToGroundedStance,
    LandToGroundedAutomatic,
    LandToLandMovement,
    LandToJump,
    LandToFall,
    LandMovementToGrounded,
    LandMovementToJump,
    LandMovementToFall,
    LandConductToLandMovement,
    LandConductToLand,
}

impl LocomotionTransition {
    fn index(self) -> f32 {
        match self {
            Self::GroundedToJump => 0.0,
            Self::GroundedToFall => 1.0,
            Self::FallToLandConduct => 2.0,
            Self::FallToGrounded => 3.0,
            Self::JumpToLandConduct => 4.0,
            Self::JumpToGrounded => 5.0,
            Self::LandToGroundedStance => 6.0,
            Self::LandToGroundedAutomatic => 7.0,
            Self::LandToLandMovement => 8.0,
            Self::LandToJump => 9.0,
            Self::LandToFall => 10.0,
            Self::LandMovementToGrounded => 11.0,
            Self::LandMovementToJump => 12.0,
            Self::LandMovementToFall => 13.0,
            Self::LandConductToLandMovement => 14.0,
            Self::LandConductToLand => 15.0,
        }
    }

    fn target(self) -> LocomotionLayerState {
        match self {
            Self::GroundedToJump | Self::LandToJump | Self::LandMovementToJump => {
                LocomotionLayerState::Jump
            }
            Self::GroundedToFall | Self::LandToFall | Self::LandMovementToFall => {
                LocomotionLayerState::Fall
            }
            Self::FallToLandConduct | Self::JumpToLandConduct => LocomotionLayerState::LandConduct,
            Self::FallToGrounded
            | Self::JumpToGrounded
            | Self::LandToGroundedStance
            | Self::LandToGroundedAutomatic
            | Self::LandMovementToGrounded => LocomotionLayerState::Grounded,
            Self::LandToLandMovement | Self::LandConductToLandMovement => {
                LocomotionLayerState::LandMovement
            }
            Self::LandConductToLand => LocomotionLayerState::Land,
        }
    }

    fn duration(self) -> f32 {
        match self {
            Self::GroundedToJump | Self::LandToJump | Self::LandMovementToJump => 0.1,
            Self::GroundedToFall | Self::LandToFall | Self::LandMovementToFall => 0.6,
            Self::FallToLandConduct
            | Self::FallToGrounded
            | Self::JumpToLandConduct
            | Self::JumpToGrounded
            | Self::LandToLandMovement => 0.2,
            Self::LandToGroundedStance => 0.2,
            Self::LandToGroundedAutomatic => 0.8,
            Self::LandMovementToGrounded => 0.3,
            Self::LandConductToLandMovement | Self::LandConductToLand => 0.1,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum JumpLayerState {
    #[default]
    EntryConduit,
    LeftFoot,
    RightFoot,
    Loop,
    Flail,
}

impl JumpLayerState {
    fn index(self) -> f32 {
        match self {
            Self::EntryConduit => 0.0,
            Self::LeftFoot => 1.0,
            Self::RightFoot => 2.0,
            Self::Loop => 3.0,
            Self::Flail => 4.0,
        }
    }
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum JumpTransition {
    EntryToRightFoot,
    EntryToLeftFoot,
    LeftFootToLoop,
    RightFootToLoop,
    LoopToFlail,
}

impl JumpTransition {
    fn index(self) -> f32 {
        match self {
            Self::EntryToRightFoot => 0.0,
            Self::EntryToLeftFoot => 1.0,
            Self::LeftFootToLoop => 2.0,
            Self::RightFootToLoop => 3.0,
            Self::LoopToFlail => 4.0,
        }
    }

    fn target(self) -> JumpLayerState {
        match self {
            Self::EntryToRightFoot => JumpLayerState::RightFoot,
            Self::EntryToLeftFoot => JumpLayerState::LeftFoot,
            Self::LeftFootToLoop | Self::RightFootToLoop => JumpLayerState::Loop,
            Self::LoopToFlail => JumpLayerState::Flail,
        }
    }
}

#[derive(Component, Clone, Debug, PartialEq)]
struct LocomotionLayerRuntime {
    state: LocomotionLayerState,
    state_elapsed: f32,
    transition_elapsed: f32,
    transition_duration: f32,
    jump_state: JumpLayerState,
    jump_state_elapsed: f32,
    jump_play_rate: f32,
    jump_latched: bool,
    previous_locomotion: LocomotionState,
    last_air_vertical_velocity: f32,
    landing_vertical_velocity: Option<f32>,
    air_lean: Vec2,
    land_movement_additive_alpha: f32,
    fall_phase: SyncPhaseState,
    jump_phase: SyncPhaseState,
    flail_phase: SyncPhaseState,
    land_phase: SyncPhaseState,
    outer_transition_requests: VecDeque<LocomotionTransition>,
    jump_transition_requests: VecDeque<JumpTransition>,
}

impl Default for LocomotionLayerRuntime {
    fn default() -> Self {
        Self {
            state: LocomotionLayerState::Grounded,
            state_elapsed: 0.0,
            transition_elapsed: 0.0,
            transition_duration: 0.0,
            jump_state: JumpLayerState::EntryConduit,
            jump_state_elapsed: 0.0,
            jump_play_rate: JUMP_PLAY_RATE_MIN,
            jump_latched: false,
            previous_locomotion: LocomotionState::Grounded,
            last_air_vertical_velocity: 0.0,
            landing_vertical_velocity: None,
            air_lean: Vec2::ZERO,
            land_movement_additive_alpha: 1.0,
            fall_phase: default(),
            jump_phase: default(),
            flail_phase: default(),
            land_phase: default(),
            outer_transition_requests: VecDeque::new(),
            jump_transition_requests: VecDeque::new(),
        }
    }
}

#[derive(Component, Debug, Clone, Default, PartialEq)]
pub struct AlsLocomotionTransitionReceipt {
    pub sequence: u64,
    pub outer_transition: Option<f32>,
    pub jump_transition: Option<f32>,
}

#[derive(Resource)]
struct LocomotionCurveHandles {
    air_lean: Handle<CurveAsset>,
    ground_prediction: Handle<CurveAsset>,
    jump_walk_left: Handle<AlsAnimCurves>,
    jump_run_left: Handle<AlsAnimCurves>,
    jump_walk_right: Handle<AlsAnimCurves>,
    jump_run_right: Handle<AlsAnimCurves>,
    land_light: Handle<AlsAnimCurves>,
    land_heavy: Handle<AlsAnimCurves>,
    land_light_additive: Handle<AlsAnimCurves>,
    land_heavy_additive: Handle<AlsAnimCurves>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct LocomotionCurves {
    pose_standing: f32,
    pose_crouching: f32,
    pose_moving: f32,
    rotation_yaw_offset: f32,
    foot_left_lock: f32,
    foot_right_lock: f32,
    allow_transitions: f32,
    rotation_yaw_speed: f32,
    pose_grounded: f32,
    foot_left_ik: f32,
    foot_right_ik: f32,
    pose_in_air: f32,
}

impl LocomotionCurves {
    fn air() -> Self {
        Self {
            pose_standing: 1.0,
            foot_left_ik: 1.0,
            foot_right_ik: 1.0,
            pose_in_air: 1.0,
            ..default()
        }
    }

    fn land() -> Self {
        Self {
            pose_standing: 1.0,
            foot_left_lock: 1.0,
            foot_right_lock: 1.0,
            pose_grounded: 1.0,
            foot_left_ik: 1.0,
            foot_right_ik: 1.0,
            ..default()
        }
    }
}

pub struct LocomotionLayerPlugin;

impl Plugin for LocomotionLayerPlugin {
    fn build(&self, app: &mut App) {
        app.configure_sets(
            PostUpdate,
            AlsLocomotionLayerSet
                .after(AlsGroundedLayerSet)
                .before(AnimationGraphSet::PrePhysics),
        )
        .add_systems(Startup, load_locomotion_curve_assets)
        .add_systems(
            PostUpdate,
            locomotion_layer_writer.in_set(AlsLocomotionLayerSet),
        )
        .add_systems(
            PostUpdate,
            acknowledge_locomotion_transition_requests.after(AnimationGraphSet::PrePhysics),
        );
    }
}

fn load_locomotion_curve_assets(mut commands: Commands, assets: Res<AssetServer>) {
    commands.insert_resource(LocomotionCurveHandles {
        air_lean: assets.load(AIR_LEAN_CURVE),
        ground_prediction: assets.load(GROUND_PREDICTION_CURVE),
        jump_walk_left: assets.load(JUMP_WALK_LEFT_CURVES),
        jump_run_left: assets.load(JUMP_RUN_LEFT_CURVES),
        jump_walk_right: assets.load(JUMP_WALK_RIGHT_CURVES),
        jump_run_right: assets.load(JUMP_RUN_RIGHT_CURVES),
        land_light: assets.load(LAND_LIGHT_CURVES),
        land_heavy: assets.load(LAND_HEAVY_CURVES),
        land_light_additive: assets.load(LAND_LIGHT_ADDITIVE_CURVES),
        land_heavy_additive: assets.load(LAND_HEAVY_ADDITIVE_CURVES),
    });
}

fn damp_alpha(delta_seconds: f32, half_life: f32) -> f32 {
    if half_life <= f32::EPSILON {
        1.0
    } else {
        1.0 - 2.0_f32.powf(-delta_seconds.max(0.0) / half_life)
    }
}

fn horizontal_speed(velocity: Vec3) -> f32 {
    Vec2::new(velocity.x, velocity.z).length()
}

fn map_range(value: f32, input_min: f32, input_max: f32, output_min: f32, output_max: f32) -> f32 {
    let extent = input_max - input_min;
    if extent.abs() <= f32::EPSILON {
        output_min
    } else {
        output_min + (value - input_min) / extent * (output_max - output_min)
    }
}

fn map_range_clamped(
    value: f32,
    input_min: f32,
    input_max: f32,
    output_min: f32,
    output_max: f32,
) -> f32 {
    map_range(value, input_min, input_max, output_min, output_max)
        .clamp(output_min.min(output_max), output_min.max(output_max))
}

fn jump_play_rate_for_speed(speed: f32) -> f32 {
    map_range_clamped(
        speed,
        0.0,
        JUMP_SPEED_REFERENCE,
        JUMP_PLAY_RATE_MIN,
        JUMP_PLAY_RATE_MAX,
    )
}

fn is_air(locomotion: LocomotionState) -> bool {
    locomotion == LocomotionState::Air
}

fn update_jump_success_latch(
    runtime: &mut LocomotionLayerRuntime,
    locomotion: LocomotionState,
    vertical_velocity: f32,
) {
    if runtime.previous_locomotion == LocomotionState::Grounded
        && locomotion == LocomotionState::Air
    {
        runtime.jump_latched = vertical_velocity > JUMP_SUCCESS_MIN_UPWARD_VELOCITY;
    } else if locomotion != LocomotionState::Air {
        runtime.jump_latched = false;
    }
}

fn update_landing_velocity_latch(
    runtime: &mut LocomotionLayerRuntime,
    locomotion: LocomotionState,
    vertical_velocity: f32,
    air_state: Option<&AlsAirState>,
) {
    if is_air(locomotion) {
        runtime.last_air_vertical_velocity = vertical_velocity;
        runtime.landing_vertical_velocity = None;
    } else if runtime.previous_locomotion == LocomotionState::Air {
        runtime.landing_vertical_velocity = Some(runtime.last_air_vertical_velocity);
    } else if runtime.landing_vertical_velocity.is_none() {
        runtime.landing_vertical_velocity =
            air_state.and_then(AlsAirState::landing_vertical_velocity_world_space);
    }
}

fn landing_animation_vertical_velocity(
    runtime: &LocomotionLayerRuntime,
    air_state: Option<&AlsAirState>,
    current_vertical_velocity: f32,
) -> f32 {
    if matches!(
        runtime.state,
        LocomotionLayerState::Land | LocomotionLayerState::LandMovement
    ) {
        return runtime
            .landing_vertical_velocity
            .or_else(|| air_state.and_then(AlsAirState::landing_vertical_velocity_world_space))
            .unwrap_or(current_vertical_velocity);
    }
    current_vertical_velocity
}

fn next_outer_transition(
    state: LocomotionLayerState,
    state_elapsed: f32,
    locomotion: LocomotionState,
    jumped: bool,
    standing: bool,
    has_input: bool,
    rotating: bool,
    speed: f32,
    has_relative_location: bool,
) -> Option<LocomotionTransition> {
    match state {
        LocomotionLayerState::Grounded if is_air(locomotion) && jumped => {
            Some(LocomotionTransition::GroundedToJump)
        }
        LocomotionLayerState::Grounded if is_air(locomotion) => {
            Some(LocomotionTransition::GroundedToFall)
        }
        LocomotionLayerState::Fall if !is_air(locomotion) => {
            Some(LocomotionTransition::FallToLandConduct)
        }
        LocomotionLayerState::Jump if !is_air(locomotion) => {
            Some(LocomotionTransition::JumpToLandConduct)
        }
        LocomotionLayerState::Land if is_air(locomotion) && jumped => {
            Some(LocomotionTransition::LandToJump)
        }
        LocomotionLayerState::Land if is_air(locomotion) => Some(LocomotionTransition::LandToFall),
        LocomotionLayerState::Land if !standing => Some(LocomotionTransition::LandToGroundedStance),
        LocomotionLayerState::Land
            if has_input || rotating || speed >= LAND_MOVEMENT_SPEED_THRESHOLD =>
        {
            Some(LocomotionTransition::LandToLandMovement)
        }
        LocomotionLayerState::Land if state_elapsed >= LAND_DURATION => {
            Some(LocomotionTransition::LandToGroundedAutomatic)
        }
        LocomotionLayerState::LandMovement if is_air(locomotion) && jumped => {
            Some(LocomotionTransition::LandMovementToJump)
        }
        LocomotionLayerState::LandMovement if is_air(locomotion) => {
            Some(LocomotionTransition::LandMovementToFall)
        }
        LocomotionLayerState::LandMovement if state_elapsed >= LAND_DURATION => {
            Some(LocomotionTransition::LandMovementToGrounded)
        }
        LocomotionLayerState::LandConduct if has_input || has_relative_location => {
            Some(LocomotionTransition::LandConductToLandMovement)
        }
        LocomotionLayerState::LandConduct => Some(LocomotionTransition::LandConductToLand),
        _ => None,
    }
}

fn next_jump_transition(
    state: JumpLayerState,
    elapsed: f32,
    foot_planted_amount: f32,
    play_rate: f32,
) -> Option<JumpTransition> {
    match state {
        JumpLayerState::EntryConduit if foot_planted_amount > 0.0 => {
            Some(JumpTransition::EntryToRightFoot)
        }
        JumpLayerState::EntryConduit => Some(JumpTransition::EntryToLeftFoot),
        JumpLayerState::LeftFoot if elapsed >= JUMP_START_DURATION / play_rate.max(1e-4) => {
            Some(JumpTransition::LeftFootToLoop)
        }
        JumpLayerState::RightFoot if elapsed >= JUMP_START_DURATION / play_rate.max(1e-4) => {
            Some(JumpTransition::RightFootToLoop)
        }
        JumpLayerState::LeftFoot
        | JumpLayerState::RightFoot
        | JumpLayerState::Loop
        | JumpLayerState::Flail => None,
    }
}

fn ground_prediction_amount(
    spatial: &SpatialQuery,
    entity: Entity,
    transform: &GlobalTransform,
    velocity: Vec3,
    params: Option<&AlsMovementParams>,
    stance: Stance,
    curve: Option<&CurveAsset>,
    allowance: f32,
) -> f32 {
    let (capsule_half_height, capsule_radius) =
        scaled_ground_prediction_capsule(params, stance, transform);
    let prediction = ground_prediction_sweep(
        spatial,
        entity,
        transform.translation(),
        velocity,
        capsule_half_height,
        capsule_radius,
    );
    let Some(time) = prediction.hit_fraction else {
        return 0.0;
    };
    curve.map(|curve| curve.sample(time)).unwrap_or(time) * allowance.clamp(0.0, 1.0)
}

fn scaled_ground_prediction_capsule(
    params: Option<&AlsMovementParams>,
    stance: Stance,
    transform: &GlobalTransform,
) -> (f32, f32) {
    let (half_height, radius) = params
        .map(|params| {
            let half_height = match stance {
                Stance::Standing => params.crouch.standing_capsule_half_height,
                Stance::Crouching => params.crouch.crouched_capsule_half_height,
            };
            (half_height, params.crouch.standing_capsule_radius)
        })
        .unwrap_or((
            ALS_CHARACTER_CAPSULE_HALF_HEIGHT,
            ALS_CHARACTER_CAPSULE_RADIUS,
        ));
    let (scale, _, _) = transform.to_scale_rotation_translation();
    let vertical_scale = scale.y.abs().max(1e-4);
    let horizontal_scale = scale.x.abs().max(scale.z.abs()).max(1e-4);
    let radius = radius * horizontal_scale;
    ((half_height * vertical_scale).max(radius), radius)
}

fn animation_curve_sample(
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

fn sprint_block_amount(
    runtime: &LocomotionLayerRuntime,
    assets: &Assets<AlsAnimCurves>,
    handles: &LocomotionCurveHandles,
    landing_heavy_alpha: f32,
    land_movement_heavy_alpha: f32,
) -> f32 {
    let (light, heavy, alpha) = match runtime.state {
        LocomotionLayerState::Land => (
            &handles.land_light,
            &handles.land_heavy,
            landing_heavy_alpha,
        ),
        LocomotionLayerState::LandMovement => (
            &handles.land_light_additive,
            &handles.land_heavy_additive,
            land_movement_heavy_alpha,
        ),
        _ => return 0.0,
    };
    let time = (runtime.land_phase.phase * LAND_DURATION).clamp(0.0, LAND_DURATION);
    blend_sprint_block_curve(
        animation_curve_sample(assets, light, "SprintBlock", time),
        animation_curve_sample(assets, heavy, "SprintBlock", time),
        alpha,
    )
}

fn blend_sprint_block_curve(light: f32, heavy: f32, alpha: f32) -> f32 {
    light.lerp(heavy, alpha.clamp(0.0, 1.0)).clamp(0.0, 1.0)
}

fn jump_curve_sample_time(runtime: &LocomotionLayerRuntime) -> f32 {
    (runtime.jump_phase.phase * JUMP_DURATION).clamp(0.0, JUMP_DURATION)
}

fn jump_ground_prediction_allowance(
    runtime: &LocomotionLayerRuntime,
    speed: f32,
    assets: &Assets<AlsAnimCurves>,
    handles: &LocomotionCurveHandles,
) -> f32 {
    if runtime.state != LocomotionLayerState::Jump {
        return 1.0;
    }
    let (walk, run) = match runtime.jump_state {
        JumpLayerState::LeftFoot => (&handles.jump_walk_left, &handles.jump_run_left),
        JumpLayerState::RightFoot => (&handles.jump_walk_right, &handles.jump_run_right),
        JumpLayerState::EntryConduit | JumpLayerState::Loop | JumpLayerState::Flail => {
            return 1.0;
        }
    };
    let alpha = map_range_clamped(speed, 2.0, 5.0, 0.0, 1.0);
    let time = jump_curve_sample_time(runtime);
    let block = animation_curve_sample(assets, walk, "GroundPredictionBlock", time).lerp(
        animation_curve_sample(assets, run, "GroundPredictionBlock", time),
        alpha,
    );
    1.0 - block.clamp(0.0, 1.0)
}

fn update_air_lean(
    current: Vec2,
    delta_seconds: f32,
    velocity: Vec3,
    transform: &Transform,
    curve: Option<&CurveAsset>,
) -> Vec2 {
    let forward = transform.rotation * Vec3::NEG_Z;
    let right = transform.rotation * Vec3::X;
    let local = Vec2::new(velocity.dot(forward), velocity.dot(right));
    let multiplier = curve
        .map(|curve| curve.sample(velocity.y * 100.0))
        .unwrap_or(0.0);
    let target = local / AIR_LEAN_REFERENCE_SPEED * multiplier;
    current.lerp(target, damp_alpha(delta_seconds, LEAN_HALF_LIFE))
}

fn air_curve_values() -> LocomotionCurves {
    LocomotionCurves::air()
}

fn state_curve_values(state: LocomotionLayerState) -> LocomotionCurves {
    match state {
        LocomotionLayerState::Fall | LocomotionLayerState::Jump => air_curve_values(),
        LocomotionLayerState::Land => LocomotionCurves::land(),
        LocomotionLayerState::Grounded
        | LocomotionLayerState::LandMovement
        | LocomotionLayerState::LandConduct => LocomotionCurves::default(),
    }
}

fn advance_locomotion_sync_phase(
    state: &mut SyncPhaseState,
    active: bool,
    rate: f32,
    duration: f32,
    looped: bool,
    delta_seconds: f32,
) {
    advance_sync_phase(
        state,
        SyncPhaseInput {
            active,
            reset: false,
            rate,
            duration,
            start_phase: 0.0,
            looped,
            delta_seconds,
        },
    );
}

fn update_locomotion_sync_phases(
    runtime: &mut LocomotionLayerRuntime,
    speed: f32,
    vertical_velocity: f32,
    delta_seconds: f32,
) {
    let jump_rate = if runtime.state == LocomotionLayerState::Jump {
        runtime.jump_play_rate
    } else {
        jump_play_rate_for_speed(speed)
    };
    let flail_alpha = map_range_clamped(vertical_velocity, -5.0, -30.0, 0.0, 1.0);
    let land_movement_heavy_alpha = map_range(vertical_velocity, -7.5, -15.0, 0.0, 0.75);
    advance_locomotion_sync_phase(
        &mut runtime.fall_phase,
        runtime.state == LocomotionLayerState::Fall,
        1.0,
        FALL_DURATION,
        true,
        delta_seconds,
    );
    advance_locomotion_sync_phase(
        &mut runtime.jump_phase,
        runtime.state == LocomotionLayerState::Jump
            && matches!(
                runtime.jump_state,
                JumpLayerState::LeftFoot | JumpLayerState::RightFoot
            ),
        jump_rate,
        JUMP_DURATION,
        false,
        delta_seconds,
    );
    let flail_rate = if runtime.jump_state == JumpLayerState::Loop && flail_alpha <= 0.5 {
        jump_rate
    } else {
        1.0
    };
    advance_locomotion_sync_phase(
        &mut runtime.flail_phase,
        runtime.state == LocomotionLayerState::Jump
            && matches!(
                runtime.jump_state,
                JumpLayerState::Loop | JumpLayerState::Flail
            ),
        flail_rate,
        FLAIL_DURATION,
        true,
        delta_seconds,
    );
    let land_active = matches!(
        runtime.state,
        LocomotionLayerState::Land | LocomotionLayerState::LandMovement
    );
    let land_rate = match runtime.state {
        LocomotionLayerState::Land => 1.0,
        LocomotionLayerState::LandMovement if land_movement_heavy_alpha > 0.5 => 1.5,
        LocomotionLayerState::LandMovement => 1.75,
        _ => 1.0,
    };
    advance_locomotion_sync_phase(
        &mut runtime.land_phase,
        land_active,
        land_rate,
        LAND_DURATION,
        false,
        delta_seconds,
    );
}

fn write_globals(
    player: &mut AnimationGraphPlayer,
    runtime: &LocomotionLayerRuntime,
    curves: LocomotionCurves,
    vertical_velocity: f32,
    ground_prediction: f32,
    speed: f32,
    sprint_block: f32,
) {
    let jump_play_rate = if runtime.state == LocomotionLayerState::Jump {
        runtime.jump_play_rate
    } else {
        jump_play_rate_for_speed(speed)
    };
    let jump_walk_run_alpha = map_range_clamped(speed, 2.0, 5.0, 0.0, 1.0);
    let fall_fast_alpha = map_range_clamped(vertical_velocity, 0.0, -10.0, 0.0, 1.0);
    let flail_alpha = map_range_clamped(vertical_velocity, -5.0, -30.0, 0.0, 1.0);
    let landing_heavy_alpha = map_range_clamped(vertical_velocity, -5.0, -10.0, 0.0, 1.0);
    let land_movement_heavy_alpha = map_range(vertical_velocity, -7.5, -15.0, 0.0, 0.75);
    let land_movement_weight = map_range_clamped(vertical_velocity, 0.0, -2.0, 0.25, 1.0);
    for (key, value) in [
        (
            "als_locomotion_transition_index",
            runtime
                .outer_transition_requests
                .front()
                .copied()
                .map_or(-1.0, LocomotionTransition::index),
        ),
        (
            "als_locomotion_jump_transition_index",
            runtime
                .jump_transition_requests
                .front()
                .copied()
                .map_or(-1.0, JumpTransition::index),
        ),
        ("als_locomotion_state_index", runtime.state.index()),
        (
            "als_locomotion_jump_state_index",
            runtime.jump_state.index(),
        ),
        ("als_locomotion_jump_play_rate", jump_play_rate),
        ("als_locomotion_fall_phase", runtime.fall_phase.phase),
        ("als_locomotion_jump_phase", runtime.jump_phase.phase),
        ("als_locomotion_flail_phase", runtime.flail_phase.phase),
        ("als_locomotion_land_phase", runtime.land_phase.phase),
        ("als_locomotion_jump_walk_run_alpha", jump_walk_run_alpha),
        ("als_locomotion_fall_fast_alpha", fall_fast_alpha),
        ("als_locomotion_flail_alpha", flail_alpha),
        ("als_locomotion_landing_heavy_alpha", landing_heavy_alpha),
        (
            "als_locomotion_land_movement_heavy_alpha",
            land_movement_heavy_alpha,
        ),
        ("als_locomotion_land_movement_weight", land_movement_weight),
        (
            "als_locomotion_land_movement_additive_alpha",
            runtime.land_movement_additive_alpha,
        ),
        ("als_locomotion_air_lean_right", runtime.air_lean.y),
        ("als_locomotion_air_lean_forward", runtime.air_lean.x),
        ("als_locomotion_ground_prediction", ground_prediction),
        ("als_locomotion_vertical_velocity", vertical_velocity),
        ("als_locomotion_curve_PoseStanding", curves.pose_standing),
        ("als_locomotion_curve_PoseCrouching", curves.pose_crouching),
        ("als_locomotion_curve_PoseMoving", curves.pose_moving),
        (
            "als_locomotion_curve_RotationYawOffset",
            curves.rotation_yaw_offset,
        ),
        ("als_locomotion_curve_FootLeftLock", curves.foot_left_lock),
        ("als_locomotion_curve_FootRightLock", curves.foot_right_lock),
        (
            "als_locomotion_curve_AllowTransitions",
            curves.allow_transitions,
        ),
        (
            "als_locomotion_curve_RotationYawSpeed",
            curves.rotation_yaw_speed,
        ),
        ("als_locomotion_curve_PoseGrounded", curves.pose_grounded),
        ("als_locomotion_curve_FootLeftIk", curves.foot_left_ik),
        ("als_locomotion_curve_FootRightIk", curves.foot_right_ik),
        ("als_locomotion_curve_PoseInAir", curves.pose_in_air),
        ("als_locomotion_curve_SprintBlock", sprint_block),
    ] {
        player.set_global_input_data(key, value);
    }
}

#[allow(clippy::type_complexity)]
fn locomotion_layer_writer(
    time: Res<Time>,
    spatial: SpatialQuery,
    mut commands: Commands,
    characters: Query<
        (
            Entity,
            &Transform,
            &GlobalTransform,
            &LinearVelocity,
            &AlsMovementParamsHandle,
            Option<&AlsAirState>,
            &LocomotionState,
            &Stance,
            &RotationMode,
            &AlsInputCommand,
            Option<&AlsInputOverride>,
            Option<&AlsFootLockState>,
            Option<&AlsMovementBaseAnimationState>,
        ),
        With<AlsCharacter>,
    >,
    mut avatars: Query<
        (
            Entity,
            &ChildOf,
            &AnimatedSceneInstance,
            Option<&mut LocomotionLayerRuntime>,
            Option<&AlsTurnDrive>,
        ),
        With<AlsCharacterAvatar>,
    >,
    curve_assets: Res<Assets<CurveAsset>>,
    animation_curves: Res<Assets<AlsAnimCurves>>,
    params_assets: Res<Assets<AlsMovementParams>>,
    handles: Res<LocomotionCurveHandles>,
    mut players: Query<&mut AnimationGraphPlayer>,
) {
    let delta_seconds = time.delta_secs();
    for (avatar, parent, instance, current_runtime, turn) in &mut avatars {
        let Ok((
            character,
            transform,
            global_transform,
            velocity,
            params_handle,
            air_state,
            locomotion,
            stance,
            rotation_mode,
            command,
            override_command,
            feet,
            movement_base,
        )) = characters.get(parent.parent())
        else {
            continue;
        };
        let Ok(mut player) = players.get_mut(instance.player_entity()) else {
            continue;
        };
        let mut runtime = current_runtime.as_deref().cloned().unwrap_or_default();
        update_landing_velocity_latch(&mut runtime, *locomotion, velocity.y, air_state);
        let effective = override_command.map_or(command, |command| &command.0);
        let speed = horizontal_speed(velocity.0);
        let has_input = effective.move_dir.length_squared() > 1e-6;
        let rotating = turn.is_some_and(|turn| turn.rotate_active);
        let has_relative_location = movement_base.is_some_and(|state| state.has_relative_location);
        update_jump_success_latch(&mut runtime, *locomotion, velocity.y);
        runtime.state_elapsed += delta_seconds.max(0.0);
        runtime.transition_elapsed = (runtime.transition_elapsed + delta_seconds.max(0.0))
            .min(runtime.transition_duration.max(0.0));
        runtime.jump_state_elapsed += delta_seconds.max(0.0);
        let outer_request = next_outer_transition(
            runtime.state,
            runtime.state_elapsed,
            *locomotion,
            runtime.jump_latched,
            *stance == Stance::Standing,
            has_input,
            rotating,
            speed,
            has_relative_location,
        );
        if let Some(transition) = outer_request {
            runtime.state = transition.target();
            runtime.state_elapsed = 0.0;
            runtime.transition_elapsed = 0.0;
            runtime.transition_duration = transition.duration();
            runtime.outer_transition_requests.push_back(transition);
            if runtime.state == LocomotionLayerState::Jump {
                runtime.jump_state = JumpLayerState::EntryConduit;
                runtime.jump_state_elapsed = 0.0;
                runtime.jump_play_rate = jump_play_rate_for_speed(speed);
                runtime.jump_latched = false;
            }
        }
        let jump_play_rate = if runtime.state == LocomotionLayerState::Jump {
            runtime.jump_play_rate
        } else {
            jump_play_rate_for_speed(speed)
        };
        let jump_request = (runtime.state == LocomotionLayerState::Jump)
            .then(|| {
                next_jump_transition(
                    runtime.jump_state,
                    runtime.jump_state_elapsed,
                    feet.map(|state| state.foot_planted_amount).unwrap_or(0.0),
                    jump_play_rate,
                )
            })
            .flatten();
        if let Some(transition) = jump_request {
            runtime.jump_state = transition.target();
            runtime.jump_state_elapsed = 0.0;
            runtime.jump_transition_requests.push_back(transition);
        }
        let prediction = ground_prediction_amount(
            &spatial,
            character,
            global_transform,
            velocity.0,
            params_assets.get(params_handle.0.id()),
            *stance,
            curve_assets.get(&handles.ground_prediction),
            jump_ground_prediction_allowance(&runtime, speed, &animation_curves, &handles),
        );
        runtime.air_lean = update_air_lean(
            runtime.air_lean,
            delta_seconds,
            velocity.0,
            transform,
            curve_assets.get(&handles.air_lean),
        );
        let additive_target = if *rotation_mode == RotationMode::Aiming {
            0.5
        } else {
            1.0
        };
        let additive_speed = if additive_target > runtime.land_movement_additive_alpha {
            2.5
        } else {
            20.0
        };
        runtime.land_movement_additive_alpha = runtime.land_movement_additive_alpha.lerp(
            additive_target,
            (additive_speed * delta_seconds).clamp(0.0, 1.0),
        );
        let animation_vertical_velocity =
            landing_animation_vertical_velocity(&runtime, air_state, velocity.y);
        update_locomotion_sync_phases(
            &mut runtime,
            speed,
            animation_vertical_velocity,
            delta_seconds,
        );
        let landing_heavy_alpha =
            map_range_clamped(animation_vertical_velocity, -5.0, -10.0, 0.0, 1.0);
        let land_movement_heavy_alpha =
            map_range(animation_vertical_velocity, -7.5, -15.0, 0.0, 0.75);
        let sprint_block = sprint_block_amount(
            &runtime,
            &animation_curves,
            &handles,
            landing_heavy_alpha,
            land_movement_heavy_alpha,
        );
        let curves = state_curve_values(runtime.state);
        write_globals(
            &mut player,
            &runtime,
            curves,
            animation_vertical_velocity,
            prediction,
            speed,
            sprint_block,
        );
        commands
            .entity(character)
            .insert(AlsGroundPredictionState { amount: prediction });
        if movement_base.is_none() {
            commands
                .entity(character)
                .insert(AlsMovementBaseAnimationState::default());
        }
        runtime.previous_locomotion = *locomotion;
        if let Some(mut current_runtime) = current_runtime {
            *current_runtime = runtime;
        } else {
            commands.entity(avatar).insert(runtime);
        }
    }
}

fn acknowledge_locomotion_transition_requests(
    mut commands: Commands,
    mut avatars: Query<
        (
            Entity,
            &AnimatedSceneInstance,
            &mut LocomotionLayerRuntime,
            Option<&mut AlsLocomotionTransitionReceipt>,
        ),
        With<AlsCharacterAvatar>,
    >,
    players: Query<&AnimationGraphPlayer>,
) {
    for (avatar, instance, mut runtime, receipt) in &mut avatars {
        let Ok(player) = players.get(instance.player_entity()) else {
            continue;
        };
        if player.get_error().is_some() || player.get_default_output_pose().is_none() {
            continue;
        }
        let outer_transition = runtime
            .outer_transition_requests
            .pop_front()
            .map(LocomotionTransition::index);
        let jump_transition = runtime
            .jump_transition_requests
            .pop_front()
            .map(JumpTransition::index);
        let consumed = outer_transition.is_some() || jump_transition.is_some();
        if let Some(mut receipt) = receipt {
            if consumed {
                receipt.sequence += 1;
                receipt.outer_transition = outer_transition;
                receipt.jump_transition = jump_transition;
            }
        } else {
            commands
                .entity(avatar)
                .insert(AlsLocomotionTransitionReceipt {
                    sequence: u64::from(consumed),
                    outer_transition,
                    jump_transition,
                });
        }
    }
}

#[cfg(test)]
mod tests {
    use als_core::air::AirPhase;

    use super::*;

    fn delivered_transition_count(render_hz: f32, fixed_hz: f32) -> usize {
        let render_dt = 1.0 / render_hz;
        let fixed_dt = 1.0 / fixed_hz;
        let request_time = fixed_dt * 0.25;
        let end_time = (fixed_dt * 4.0).max(render_dt * 3.0);
        let mut next_render = 0.0;
        let mut next_fixed = fixed_dt;
        let mut time = 0.0;
        let mut issued = false;
        let mut requests = VecDeque::new();
        let mut previous = None;
        let mut deliveries = 0;
        while time <= end_time {
            if next_render <= next_fixed {
                time = next_render;
                if !issued && time >= request_time {
                    requests.push_back(4.0_f32);
                    issued = true;
                }
                next_render += render_dt;
            } else {
                time = next_fixed;
                let value = requests.front().copied().unwrap_or(-1.0);
                if als_ext::direct_transition_event::take_direct_transition_edge(
                    &mut previous,
                    value,
                    16,
                )
                .is_some()
                {
                    deliveries += 1;
                    requests.pop_front();
                }
                next_fixed += fixed_dt;
            }
        }
        assert!(issued);
        assert!(requests.is_empty());
        deliveries
    }

    #[test]
    fn transition_requests_survive_render_frames_until_fixed_consumption() {
        for render_hz in [30.0, 60.0, 144.0, 240.0] {
            for fixed_hz in [30.0, 60.0, 120.0] {
                let deliveries = delivered_transition_count(render_hz, fixed_hz);
                println!(
                    "[BF102.05] render_hz={render_hz:.0} fixed_hz={fixed_hz:.0} deliveries={deliveries}"
                );
                assert_eq!(deliveries, 1);
            }
        }
    }

    #[test]
    fn outer_in_air_priority_matches_the_source_alias() {
        assert_eq!(
            next_outer_transition(
                LocomotionLayerState::Grounded,
                0.0,
                LocomotionState::Air,
                true,
                true,
                false,
                false,
                0.0,
                false,
            ),
            Some(LocomotionTransition::GroundedToJump)
        );
        assert_eq!(
            next_outer_transition(
                LocomotionLayerState::Grounded,
                0.0,
                LocomotionState::Air,
                false,
                true,
                false,
                false,
                0.0,
                false,
            ),
            Some(LocomotionTransition::GroundedToFall)
        );
    }

    #[test]
    fn jump_entry_uses_the_source_foot_planted_boundary() {
        assert_eq!(
            next_jump_transition(JumpLayerState::EntryConduit, 0.0, 0.0, 1.2),
            Some(JumpTransition::EntryToLeftFoot)
        );
        assert_eq!(
            next_jump_transition(JumpLayerState::EntryConduit, 0.0, 0.001, 1.2),
            Some(JumpTransition::EntryToRightFoot)
        );
    }

    #[test]
    fn jump_start_uses_non_loop_duration_after_source_rate() {
        assert_eq!(
            next_jump_transition(
                JumpLayerState::LeftFoot,
                JUMP_START_DURATION / 1.2 - 0.001,
                0.0,
                1.2,
            ),
            None
        );
        assert_eq!(
            next_jump_transition(
                JumpLayerState::LeftFoot,
                JUMP_START_DURATION / 1.2,
                0.0,
                1.2,
            ),
            Some(JumpTransition::LeftFootToLoop)
        );
    }

    #[test]
    fn loop_to_flail_is_not_synthesized_from_the_unconnected_source_rule() {
        assert_eq!(
            next_jump_transition(JumpLayerState::Loop, 10.0, 0.0, 1.0),
            None
        );
    }

    #[test]
    fn locomotion_sync_groups_preserve_source_loop_and_non_loop_domains() {
        let mut jump = LocomotionLayerRuntime {
            state: LocomotionLayerState::Jump,
            jump_state: JumpLayerState::LeftFoot,
            ..default()
        };
        update_locomotion_sync_phases(&mut jump, 6.0, 0.0, 0.0);
        update_locomotion_sync_phases(&mut jump, 6.0, 0.0, 10.0);
        assert_eq!(jump.jump_phase.phase, 1.0);

        let mut fall = LocomotionLayerRuntime {
            state: LocomotionLayerState::Fall,
            ..default()
        };
        update_locomotion_sync_phases(&mut fall, 0.0, 0.0, 0.0);
        update_locomotion_sync_phases(&mut fall, 0.0, 0.0, 1.2);
        assert!((fall.fall_phase.phase - 0.2).abs() < 1e-6);

        let mut land = LocomotionLayerRuntime {
            state: LocomotionLayerState::Land,
            ..default()
        };
        update_locomotion_sync_phases(&mut land, 0.0, -8.0, 0.0);
        update_locomotion_sync_phases(&mut land, 0.0, -8.0, 10.0);
        assert_eq!(land.land_phase.phase, 1.0);
    }

    #[test]
    fn jump_play_rate_latches_the_takeoff_speed() {
        let takeoff_rate = jump_play_rate_for_speed(0.0);
        let after_takeoff_rate = jump_play_rate_for_speed(JUMP_SPEED_REFERENCE);
        let mut runtime = LocomotionLayerRuntime {
            state: LocomotionLayerState::Jump,
            jump_state: JumpLayerState::LeftFoot,
            jump_play_rate: takeoff_rate,
            ..Default::default()
        };
        update_locomotion_sync_phases(&mut runtime, JUMP_SPEED_REFERENCE, 0.0, 0.0);
        update_locomotion_sync_phases(&mut runtime, JUMP_SPEED_REFERENCE, 0.0, 0.5);
        let expected_phase = takeoff_rate * 0.5 / JUMP_DURATION;
        let loop_before = next_jump_transition(
            JumpLayerState::LeftFoot,
            JUMP_START_DURATION / takeoff_rate - 1e-3,
            0.0,
            runtime.jump_play_rate,
        );
        let loop_after = next_jump_transition(
            JumpLayerState::LeftFoot,
            JUMP_START_DURATION / takeoff_rate + 1e-3,
            0.0,
            runtime.jump_play_rate,
        );
        let passed = (runtime.jump_play_rate - takeoff_rate).abs() < 1e-6
            && (runtime.jump_phase.phase - expected_phase).abs() < 1e-6
            && loop_before.is_none()
            && loop_after == Some(JumpTransition::LeftFootToLoop)
            && after_takeoff_rate > runtime.jump_play_rate;
        let verdict = if passed { "PASS" } else { "FAIL" };
        println!(
            "[BF102.33][VERDICT] takeoff_rate={takeoff_rate:.3} post_takeoff_speed_rate={after_takeoff_rate:.3} latched_rate={:.3} phase={:.3} -> {verdict}",
            runtime.jump_play_rate, runtime.jump_phase.phase,
        );
        assert!(passed);
    }

    #[test]
    fn jump_curve_time_uses_the_integrated_sync_phase() {
        let mut runtime = LocomotionLayerRuntime {
            state: LocomotionLayerState::Jump,
            jump_state: JumpLayerState::LeftFoot,
            jump_play_rate: 1.2,
            ..Default::default()
        };
        update_locomotion_sync_phases(&mut runtime, 0.0, 0.0, 0.0);
        update_locomotion_sync_phases(&mut runtime, 0.0, 0.0, 0.5);
        runtime.jump_play_rate = 1.5;
        update_locomotion_sync_phases(&mut runtime, 0.0, 0.0, 0.5);
        runtime.jump_state_elapsed = 1.0;
        let sample_time = jump_curve_sample_time(&runtime);
        let expected_integrated_time = 0.5 * 1.2 + 0.5 * 1.5;
        let legacy_time = runtime.jump_state_elapsed * runtime.jump_play_rate;
        let passed = (sample_time - expected_integrated_time).abs() < 1e-6
            && (legacy_time - 1.5).abs() < 1e-6
            && (sample_time - legacy_time).abs() > 0.1;
        let verdict = if passed { "PASS" } else { "FAIL" };
        println!(
            "[BF102.34][VERDICT] integrated_time={sample_time:.3} legacy_time={legacy_time:.3} expected={expected_integrated_time:.3} -> {verdict}"
        );
        assert!(passed);
    }

    #[test]
    fn jump_latch_requires_an_actual_upward_air_transition() {
        let mut rejected = LocomotionLayerRuntime {
            jump_latched: true,
            ..Default::default()
        };
        update_jump_success_latch(&mut rejected, LocomotionState::Air, 0.0);
        let rejected_transition = next_outer_transition(
            LocomotionLayerState::Grounded,
            0.0,
            LocomotionState::Air,
            rejected.jump_latched,
            true,
            false,
            false,
            0.0,
            false,
        );
        let mut succeeded = LocomotionLayerRuntime::default();
        update_jump_success_latch(&mut succeeded, LocomotionState::Air, 4.2);
        let succeeded_transition = next_outer_transition(
            LocomotionLayerState::Grounded,
            0.0,
            LocomotionState::Air,
            succeeded.jump_latched,
            true,
            false,
            false,
            0.0,
            false,
        );
        let passed = !rejected.jump_latched
            && rejected_transition == Some(LocomotionTransition::GroundedToFall)
            && succeeded.jump_latched
            && succeeded_transition == Some(LocomotionTransition::GroundedToJump);
        let verdict = if passed { "PASS" } else { "FAIL" };
        println!(
            "[BF102.35][VERDICT] rejected_latch={} rejected_transition={rejected_transition:?} successful_latch={} successful_transition={succeeded_transition:?} -> {verdict}",
            rejected.jump_latched, succeeded.jump_latched,
        );
        assert!(passed);
    }

    #[test]
    fn land_movement_source_threshold_includes_rotation_and_speed() {
        assert_eq!(
            next_outer_transition(
                LocomotionLayerState::Land,
                0.0,
                LocomotionState::Grounded,
                false,
                true,
                false,
                true,
                0.0,
                false,
            ),
            Some(LocomotionTransition::LandToLandMovement)
        );
        assert_eq!(
            next_outer_transition(
                LocomotionLayerState::Land,
                0.0,
                LocomotionState::Grounded,
                false,
                true,
                false,
                false,
                LAND_MOVEMENT_SPEED_THRESHOLD,
                false,
            ),
            Some(LocomotionTransition::LandToLandMovement)
        );
    }

    #[test]
    fn landing_animation_velocity_latches_the_pre_collision_value() {
        let mut runtime = LocomotionLayerRuntime::default();
        update_landing_velocity_latch(&mut runtime, LocomotionState::Air, -10.0, None);
        runtime.previous_locomotion = LocomotionState::Air;
        update_landing_velocity_latch(&mut runtime, LocomotionState::Grounded, 0.0, None);
        runtime.state = LocomotionLayerState::Land;
        let land_velocity = landing_animation_vertical_velocity(&runtime, None, 0.0);
        runtime.state = LocomotionLayerState::LandMovement;
        let land_movement_velocity = landing_animation_vertical_velocity(&runtime, None, 0.0);
        let air_state = AlsAirState {
            phase: AirPhase::Landing,
            last_impact_vy: 10.0,
            ..AlsAirState::default()
        };
        let mut recovered_runtime = LocomotionLayerRuntime::default();
        recovered_runtime.state = LocomotionLayerState::Land;
        update_landing_velocity_latch(
            &mut recovered_runtime,
            LocomotionState::Grounded,
            0.0,
            Some(&air_state),
        );
        let recovered_velocity = landing_animation_vertical_velocity(&recovered_runtime, None, 0.0);
        let heavy_alpha = map_range_clamped(land_velocity, -5.0, -10.0, 0.0, 1.0);
        let movement_weight = map_range_clamped(land_movement_velocity, 0.0, -2.0, 0.25, 1.0);
        let passed = (land_velocity + 10.0).abs() < 1e-6
            && (land_movement_velocity + 10.0).abs() < 1e-6
            && (recovered_velocity + 10.0).abs() < 1e-6
            && (heavy_alpha - 1.0).abs() < 1e-6
            && (movement_weight - 1.0).abs() < 1e-6;
        let verdict = if passed { "PASS" } else { "FAIL" };
        println!(
            "[BF102.31][VERDICT] pre_collision=-10.000 post_collision=0.000 land={land_velocity:.3} land_movement={land_movement_velocity:.3} recovered={recovered_velocity:.3} heavy={heavy_alpha:.3} movement_weight={movement_weight:.3} -> {verdict}"
        );
        assert!(passed);
    }

    #[test]
    fn sprint_block_curve_blends_and_clamps_the_landing_samples() {
        assert_eq!(blend_sprint_block_curve(0.0, 1.0, 0.25), 0.25);
        assert_eq!(blend_sprint_block_curve(-1.0, 2.0, 0.5), 0.5);
        assert_eq!(blend_sprint_block_curve(0.2, 0.8, 2.0), 0.8);
    }

    #[test]
    fn ground_prediction_uses_scaled_actual_capsule_dimensions() {
        let params = AlsMovementParams::default();
        let transform = GlobalTransform::from(Transform {
            scale: Vec3::new(1.5, 2.0, 0.5),
            ..Default::default()
        });
        let standing =
            scaled_ground_prediction_capsule(Some(&params), Stance::Standing, &transform);
        let crouching =
            scaled_ground_prediction_capsule(Some(&params), Stance::Crouching, &transform);
        let passed = (standing.0 - 1.8).abs() < 1e-6
            && (standing.1 - 0.45).abs() < 1e-6
            && (crouching.0 - 1.12).abs() < 1e-6
            && (crouching.1 - 0.45).abs() < 1e-6;
        let verdict = if passed { "PASS" } else { "FAIL" };
        println!(
            "[BF102.32][VERDICT] standing_half={:.3} standing_radius={:.3} crouching_half={:.3} crouching_radius={:.3} shape=capsule_sweep -> {verdict}",
            standing.0, standing.1, crouching.0, crouching.1,
        );
        assert!(passed);
    }

    #[test]
    fn source_vertical_velocity_maps_use_meter_converted_thresholds() {
        assert!((map_range_clamped(-5.0, -5.0, -10.0, 0.0, 1.0) - 0.0).abs() < 1e-6);
        assert!((map_range_clamped(-10.0, -5.0, -10.0, 0.0, 1.0) - 1.0).abs() < 1e-6);
        assert!((map_range_clamped(-2.0, 0.0, -2.0, 0.25, 1.0) - 1.0).abs() < 1e-6);
    }
}
