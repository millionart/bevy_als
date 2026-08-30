use als_core::character::{AlsCharacter, AlsMovementParamsHandle};
use als_core::gait::{AlsDesiredGait, AlsMaxAllowedGait};
use als_core::motion::{AlsCrouchTransaction, AlsViewYaw};
use als_core::movement_direction::AlsMovementDirectionState;
use als_core::params::AlsMovementParams;
use als_core::state::{Gait, LocomotionState, RotationMode, Stance};
use bevy::ecs::hierarchy::ChildOf;
use bevy::prelude::*;
use bevy_animation_graph::core::animated_scene::AnimatedSceneInstance;
use bevy_animation_graph::core::animation_graph_player::{AnimationGraphPlayer, AnimationSource};
use bevy_animation_graph::core::edge_data::DataValue;

use crate::actual_motion::AlsActualMotion;
use crate::character_assembly::AlsCharacterAvatar;
use crate::standing_layer::AlsStandingGraphTelemetry;

#[derive(Resource, Clone, Copy)]
pub struct AnimProbeConfig {
    pub interval_secs: f32,
    pub facing_mismatch_warn_deg: f32,
}

impl Default for AnimProbeConfig {
    fn default() -> Self {
        Self {
            interval_secs: 0.25,
            facing_mismatch_warn_deg: 90.0,
        }
    }
}

pub struct AnimProbePlugin;

impl Plugin for AnimProbePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AnimProbeConfig>()
            .init_resource::<AnimProbeState>()
            .add_systems(Update, anim_probe_logger);
    }
}

#[derive(Resource, Default)]
struct AnimProbeState {
    last_log: f32,
    last_error_text: Option<String>,
    aposa_warned: bool,
    mismatch_warned: bool,
}

pub fn yaw_deg(v: Vec3) -> f32 {
    f32::atan2(v.x, -v.z).to_degrees()
}

pub fn angle_err(a: f32, b: f32) -> f32 {
    let d = (a - b).abs() % 360.0;
    if d > 180.0 { 360.0 - d } else { d }
}

fn standing_movement_state_label(value: Option<u8>) -> &'static str {
    match value {
        Some(0) => "Forward",
        Some(1) => "Backward",
        Some(2) => "RightForward",
        Some(3) => "RightBackward",
        Some(4) => "LeftForward",
        Some(5) => "LeftBackward",
        _ => "Inactive",
    }
}

fn movement_detail_label(value: Option<u8>) -> &'static str {
    match value {
        Some(0) => "Forward",
        Some(1) => "Backward",
        Some(2) => "Left",
        Some(3) => "Right",
        _ => "Inactive",
    }
}

type ProbeCharacters = (
    Entity,
    &'static Transform,
    &'static AlsActualMotion,
    &'static Stance,
    &'static AlsDesiredGait,
    &'static AlsMaxAllowedGait,
    &'static Gait,
    &'static LocomotionState,
    &'static RotationMode,
    &'static AlsViewYaw,
    Option<&'static AlsMovementParamsHandle>,
    Option<&'static AlsStandingGraphTelemetry>,
    Option<&'static AlsCrouchTransaction>,
);

type ProbeAvatars = (
    Entity,
    &'static ChildOf,
    &'static AnimatedSceneInstance,
    Option<&'static AlsMovementDirectionState>,
);

fn read_f32(player: &AnimationGraphPlayer, pin: &str) -> Option<f32> {
    match player.get_input_data(pin.to_string()) {
        Some(DataValue::F32(v)) => Some(v),
        _ => None,
    }
}

fn read_global_f32(player: &AnimationGraphPlayer, key: &str) -> Option<f32> {
    match player.get_global_input_data(key) {
        Some(DataValue::F32(v)) => Some(*v),
        _ => None,
    }
}

fn anim_probe_logger(
    time: Res<Time>,
    config: Res<AnimProbeConfig>,
    mut state: ResMut<AnimProbeState>,
    characters: Query<ProbeCharacters, With<AlsCharacter>>,
    avatars: Query<ProbeAvatars, With<AlsCharacterAvatar>>,
    params_assets: Res<Assets<AlsMovementParams>>,
    players: Query<&AnimationGraphPlayer>,
) {
    let t = time.elapsed_secs();
    let Ok((
        entity,
        tf,
        motion,
        stance,
        desired_gait,
        allowed_gait,
        gait,
        loco,
        rot,
        view_yaw,
        params_h,
        standing_telemetry,
        crouch_transaction,
    )) = characters.single()
    else {
        return;
    };
    let Some((_, instance, dir_state)) = avatars
        .iter()
        .find(|(_, p, _, _)| p.parent() == entity)
        .map(|(_, _, i, d)| (entity, i, d))
    else {
        return;
    };
    let Ok(player) = players.get(instance.player_entity()) else {
        return;
    };

    let capsule_h = params_h
        .and_then(|h| params_assets.get(&h.0))
        .map(|p| match stance {
            Stance::Crouching => p.crouch.crouched_capsule_half_height,
            Stance::Standing => p.crouch.standing_capsule_half_height,
        });
    let hspeed = motion.horizontal_speed;
    let face = yaw_deg(*tf.forward());
    let velyaw = if hspeed > 1e-3 {
        yaw_deg(motion.world_velocity)
    } else {
        f32::NAN
    };
    let dir_angle = dir_state.map(|s| s.angle).unwrap_or(f32::NAN);
    let dir_name = dir_state
        .map(|s| format!("{:?}", s.direction))
        .unwrap_or_else(|| "None".into());
    let fsm_state = standing_movement_state_label(
        standing_telemetry.and_then(|telemetry| telemetry.actual_movement_state),
    );
    let fsm_detail = movement_detail_label(
        standing_telemetry.and_then(|telemetry| telemetry.dominant_detail_index),
    );
    let fsm_transition_active = standing_telemetry
        .and_then(|telemetry| telemetry.actual_movement_transition_active)
        .map(|value| value.to_string())
        .unwrap_or_else(|| "?".into());
    let fsm_pose_ready = standing_telemetry
        .and_then(|telemetry| telemetry.actual_movement_pose_ready)
        .map(|value| value.to_string())
        .unwrap_or_else(|| "?".into());
    let dominant_weight = standing_telemetry
        .and_then(|telemetry| telemetry.dominant_detail_weight)
        .map(|value| format!("{value:.2}"))
        .unwrap_or_else(|| "?".into());
    let crouch_transaction_text = crouch_transaction
        .map(|transaction| {
            format!(
                "desired={:?},actual={:?},phase={:?},retries={},controller={},half={:.2},radius={:.2},blocked={}",
                transaction.desired_stance,
                transaction.actual_stance,
                transaction.phase,
                transaction.blocked_retry_count,
                transaction.controller_crouch_active,
                transaction.collider_half_height,
                transaction.collider_radius,
                transaction.headroom_blocked,
            )
        })
        .unwrap_or_else(|| "?".into());

    let (src_name, is_graph) = match player.get_animation_source() {
        AnimationSource::Graph(_) => ("Graph", true),
        AnimationSource::Pose(_) => ("Pose(APOSE)", false),
        AnimationSource::None => ("None(APOSE)", false),
    };
    let err_text = player
        .get_error()
        .map(|e| format!("{e:?}"))
        .unwrap_or_else(|| "none".into());

    let fmt_pin = |name: &str| {
        read_f32(player, name)
            .map(|f| format!("{f:.2}"))
            .unwrap_or_else(|| "?".into())
    };

    if t - state.last_log >= config.interval_secs {
        state.last_log = t;
        info!(
            "[PROBE] t={t:.2} pos=({:.2},{:.2},{:.2}) cap_h={} stance={stance:?} crouch_tx=({crouch_transaction_text}) gait=(desired={:?},allowed={:?},actual={gait:?}) loco={loco:?} rot={rot:?} face={face:.0} view={:.0} velyaw={velyaw:.0} hspd={hspeed:.2} world_v=({:.2},{:.2},{:.2}) local_v=({:.2},{:.2}) local_ang={:.1} blend=({:.2},{:.2},{:.2},{:.2}) dir={dir_name} dir_ang={dir_angle:.0} movement_fsm={fsm_state} transition_active={fsm_transition_active} pose_ready={fsm_pose_ready} dominant_branch={fsm_detail} dominant_weight={dominant_weight} src={src_name} graph_err={err_text} secondary_phase={} pins(speed={},rate={},dir_ang={},stance_w={},air_w={},land_w={})",
            tf.translation.x,
            tf.translation.y,
            tf.translation.z,
            capsule_h
                .map(|h| format!("{h:.2}"))
                .unwrap_or_else(|| "?".into()),
            desired_gait.0,
            allowed_gait.0,
            view_yaw.0,
            motion.world_velocity.x,
            motion.world_velocity.y,
            motion.world_velocity.z,
            motion.local_velocity.x,
            motion.local_velocity.y,
            motion.signed_direction_radians.to_degrees(),
            motion.velocity_blend.forward,
            motion.velocity_blend.backward,
            motion.velocity_blend.left,
            motion.velocity_blend.right,
            read_global_f32(player, "als_secondary_motion_phase")
                .map(|value| format!("{value:.3}"))
                .unwrap_or_else(|| "?".into()),
            fmt_pin("speed"),
            fmt_pin("play_rate"),
            fmt_pin("direction_angle"),
            fmt_pin("stance_weight"),
            fmt_pin("air_weight"),
            fmt_pin("land_weight"),
        );
    }

    if !is_graph && !state.aposa_warned {
        state.aposa_warned = true;
        warn!("[PROBE][APOSE_RISK] animation source is {src_name}; avatar will show bind pose");
    }
    if err_text != "none" && state.last_error_text.as_deref() != Some(err_text.as_str()) {
        state.last_error_text = Some(err_text.clone());
        warn!("[PROBE][GRAPH_ERROR] {err_text}");
    }
    if hspeed > 1.0 && angle_err(face, velyaw) > config.facing_mismatch_warn_deg {
        if !state.mismatch_warned {
            state.mismatch_warned = true;
            warn!(
                "[PROBE][FACING_MISMATCH] face={face:.0} velyaw={velyaw:.0} err={:.0}deg while hspd={hspeed:.2}",
                angle_err(face, velyaw)
            );
        }
    } else if hspeed <= 1.0 {
        state.mismatch_warned = false;
    }
}
