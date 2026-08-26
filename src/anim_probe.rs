use als_core::character::{AlsCharacter, AlsMovementParamsHandle};
use als_core::motion::AlsViewYaw;
use als_core::movement_direction::AlsMovementDirectionState;
use als_core::params::AlsMovementParams;
use als_core::state::{Gait, LocomotionState, RotationMode, Stance};
use avian3d::prelude::LinearVelocity;
use bevy::ecs::hierarchy::ChildOf;
use bevy::prelude::*;
use bevy_animation_graph::core::animated_scene::AnimatedSceneInstance;
use bevy_animation_graph::core::animation_graph_player::{AnimationGraphPlayer, AnimationSource};
use bevy_animation_graph::core::edge_data::DataValue;

use crate::character_assembly::AlsCharacterAvatar;

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

type ProbeCharacters = (
    Entity,
    &'static Transform,
    &'static LinearVelocity,
    &'static Stance,
    &'static Gait,
    &'static LocomotionState,
    &'static RotationMode,
    &'static AlsViewYaw,
    Option<&'static AlsMovementParamsHandle>,
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
    let Ok((entity, tf, vel, stance, gait, loco, rot, view_yaw, params_h)) = characters.single()
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
    let hspeed = Vec2::new(vel.x, vel.z).length();
    let face = yaw_deg(*tf.forward());
    let velyaw = if hspeed > 1e-3 {
        yaw_deg(vel.0)
    } else {
        f32::NAN
    };
    let dir_angle = dir_state.map(|s| s.angle).unwrap_or(f32::NAN);
    let dir_name = dir_state
        .map(|s| format!("{:?}", s.direction))
        .unwrap_or_else(|| "None".into());

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
            "[PROBE] t={t:.2} pos=({:.2},{:.2},{:.2}) cap_h={} stance={stance:?} gait={gait:?} loco={loco:?} rot={rot:?} face={face:.0} view={:.0} velyaw={velyaw:.0} hspd={hspeed:.2} dir={dir_name} dir_ang={dir_angle:.0} src={src_name} graph_err={err_text} pins(speed={},rate={},dir_ang={},stance_w={},air_w={},land_w={})",
            tf.translation.x,
            tf.translation.y,
            tf.translation.z,
            capsule_h
                .map(|h| format!("{h:.2}"))
                .unwrap_or_else(|| "?".into()),
            view_yaw.0,
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
