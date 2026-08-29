use als_core::air::AlsAirState;
use als_core::input::AlsInputCommand;
use als_core::motion::{AlsInputOverride, AlsViewYaw};
use als_core::roll::{
    AlsRollState, ROLL_MONTAGE_SEQUENCE_LENGTH_SECS, ROLL_PLAY_RATE, ROLL_ROTATION_HALF_LIFE_SECS,
    damper_exact_angle, roll_displacement_dir, roll_weight_at,
};
use als_core::state::{LocomotionState, Stance};
use als_core::transition::AlsStateRequest;
use als_ext::AlsAnimCurves;
use avian3d::prelude::{LinearVelocity, Rotation};
use bevy::ecs::hierarchy::ChildOf;
use bevy::prelude::*;
use bevy_animation_graph::core::animated_scene::AnimatedSceneInstance;
use bevy_animation_graph::core::animation_graph_player::AnimationGraphPlayer;
use bevy_animation_graph::core::edge_data::DataValue;
use bevy_animation_graph::core::plugin::AnimationGraphSet;
use bevy_animation_graph::core::systems::RootMotionOutput;

use crate::character_assembly::AlsCharacterAvatar;
use crate::root_motion_link::{clamp_drive_speed, ensure_root_motion_output, read_local_delta};

const ROLL_SLOT_GRAPH: &str = "graphs/ab_als/grounded/roll_slot.animgraph.ron";
const ROLL_SLOT_TIME_KEY: &str = "als_roll_slot_time";
const ROOT_POST_LOCOMOTION_SLOT_POSE_KEY: &str = "als_root_post_locomotion_slot_pose";
const ROOT_POST_LOCOMOTION_SLOT_WEIGHT_KEY: &str = "als_root_post_locomotion_slot_weight";
const ROOT_POST_LOCOMOTION_SLOT_LAYER_WEIGHT_KEY: &str =
    "als_root_post_locomotion_slot_layer_weight";
const ROOT_POST_LOCOMOTION_SLOT_CURVE_PREFIX: &str = "als_root_post_locomotion_slot_curve_";
const ROLL_SLOT_CURVES: &str = "animations/Actions/Roll/A_Als_Roll.gltf#AlsCurves";
const ROLL_SLOT_CURVE_NAMES: [&str; 2] = ["PoseStanding", "PoseGrounded"];

#[derive(Component, Debug, Clone, Copy)]
struct AlsRollSlotSampler {
    character: Entity,
    target_player: Entity,
    reported_unready: bool,
}

#[derive(Component, Debug, Clone, Copy, Default)]
pub struct AlsRollSlotRuntime {
    pub active: bool,
    pub weight: f32,
    pub pose_ready: bool,
    pub root_motion_delta: Vec3,
    pub pose_standing: f32,
    pub pose_grounded: f32,
}

#[derive(Resource)]
struct RollSlotCurveHandle(Handle<AlsAnimCurves>);

pub struct AlsRollDrivePlugin;

impl Plugin for AlsRollDrivePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, load_roll_slot_curve_asset)
            .add_systems(Update, ensure_roll_slot_samplers)
            .add_systems(
                FixedPostUpdate,
                (
                    prepare_roll_slot_samples.before(AnimationGraphSet::PrePhysics),
                    ensure_root_motion_output,
                    ensure_roll_state,
                    bridge_roll_slot_samples.after(AnimationGraphSet::PrePhysics),
                    roll_drive_system
                        .after(ensure_roll_state)
                        .after(bridge_roll_slot_samples)
                        .after(AnimationGraphSet::PrePhysics),
                ),
            );
    }
}

fn load_roll_slot_curve_asset(mut commands: Commands, assets: Res<AssetServer>) {
    commands.insert_resource(RollSlotCurveHandle(assets.load(ROLL_SLOT_CURVES)));
}

fn ensure_roll_state(
    mut commands: Commands,
    q: Query<
        Entity,
        (
            With<als_core::character::AlsCharacter>,
            Without<AlsRollState>,
        ),
    >,
) {
    for entity in &q {
        commands.entity(entity).insert(AlsRollState::default());
    }
}

fn ensure_roll_slot_samplers(
    asset_server: Res<AssetServer>,
    avatars: Query<(Entity, &ChildOf, &AnimatedSceneInstance), With<AlsCharacterAvatar>>,
    samplers: Query<&AlsRollSlotSampler>,
    players: Query<&AnimationGraphPlayer>,
    mut commands: Commands,
) {
    for (avatar, parent, instance) in &avatars {
        let target_player = instance.player_entity();
        if samplers
            .iter()
            .any(|sampler| sampler.target_player == target_player)
        {
            continue;
        }
        let Ok(player) = players.get(target_player) else {
            continue;
        };
        commands.spawn((
            Name::new("als_roll_slot_sampler"),
            Children::default(),
            ChildOf(avatar),
            AnimationGraphPlayer::new(player.skeleton().clone())
                .with_graph(asset_server.load(ROLL_SLOT_GRAPH)),
            RootMotionOutput::default(),
            AlsRollSlotSampler {
                character: parent.parent(),
                target_player,
                reported_unready: false,
            },
        ));
    }
}

fn prepare_roll_slot_samples(
    rolls: Query<&AlsRollState, With<als_core::character::AlsCharacter>>,
    mut samplers: Query<(&AlsRollSlotSampler, &mut AnimationGraphPlayer)>,
) {
    for (sampler, mut player) in &mut samplers {
        let clip_time = rolls
            .get(sampler.character)
            .ok()
            .filter(|roll| roll.active)
            .map(|roll| (roll.time * ROLL_PLAY_RATE).min(ROLL_MONTAGE_SEQUENCE_LENGTH_SECS))
            .unwrap_or(0.0);
        player.set_global_input_data(ROLL_SLOT_TIME_KEY, clip_time);
    }
}

fn write_roll_slot_curves(
    player: &mut AnimationGraphPlayer,
    curves: Option<&als_ext::AlsPoseCurves>,
    clip_time: f32,
) -> (f32, f32) {
    let mut values = [0.0_f32; 2];
    for (index, name) in ROLL_SLOT_CURVE_NAMES.iter().enumerate() {
        let value = curves
            .and_then(|curves| curves.sample(name, clip_time))
            .filter(|value| value.is_finite())
            .unwrap_or_default();
        player.set_global_input_data(
            format!("{ROOT_POST_LOCOMOTION_SLOT_CURVE_PREFIX}{name}"),
            value,
        );
        values[index] = value;
    }
    (values[0], values[1])
}

fn bridge_roll_slot_samples(
    rolls: Query<&AlsRollState, With<als_core::character::AlsCharacter>>,
    curve_handle: Res<RollSlotCurveHandle>,
    curves: Res<Assets<AlsAnimCurves>>,
    mut samplers: Query<
        (
            &mut AlsRollSlotSampler,
            &AnimationGraphPlayer,
            &RootMotionOutput,
        ),
        With<AlsRollSlotSampler>,
    >,
    mut target_players: Query<&mut AnimationGraphPlayer, Without<AlsRollSlotSampler>>,
    mut commands: Commands,
) {
    for (mut sampler, source_player, root_motion) in &mut samplers {
        let Ok(mut target_player) = target_players.get_mut(sampler.target_player) else {
            continue;
        };
        let Some(roll) = rolls.get(sampler.character).ok().filter(|roll| roll.active) else {
            target_player.set_global_input_data(ROOT_POST_LOCOMOTION_SLOT_WEIGHT_KEY, 0.0_f32);
            target_player
                .set_global_input_data(ROOT_POST_LOCOMOTION_SLOT_LAYER_WEIGHT_KEY, 0.0_f32);
            write_roll_slot_curves(&mut target_player, None, 0.0);
            commands
                .entity(sampler.character)
                .insert(AlsRollSlotRuntime::default());
            continue;
        };
        let weight = roll_weight_at(roll.time);
        let Some(pose) = source_player.get_default_output_pose().cloned() else {
            if !sampler.reported_unready {
                warn!(
                    "[ROLL-SLOT] sampler unavailable character={:?} error={:?}",
                    sampler.character,
                    source_player.get_error()
                );
                sampler.reported_unready = true;
            }
            target_player.set_global_input_data(ROOT_POST_LOCOMOTION_SLOT_WEIGHT_KEY, 0.0_f32);
            target_player
                .set_global_input_data(ROOT_POST_LOCOMOTION_SLOT_LAYER_WEIGHT_KEY, 0.0_f32);
            write_roll_slot_curves(&mut target_player, None, 0.0);
            commands
                .entity(sampler.character)
                .insert(AlsRollSlotRuntime {
                    active: true,
                    weight: 0.0,
                    pose_ready: false,
                    root_motion_delta: root_motion.translation_delta,
                    pose_standing: 0.0,
                    pose_grounded: 0.0,
                });
            continue;
        };
        target_player
            .set_global_input_data(ROOT_POST_LOCOMOTION_SLOT_POSE_KEY, DataValue::Pose(pose));
        target_player.set_global_input_data(ROOT_POST_LOCOMOTION_SLOT_WEIGHT_KEY, weight);
        target_player.set_global_input_data(ROOT_POST_LOCOMOTION_SLOT_LAYER_WEIGHT_KEY, weight);
        let clip_time = (roll.time * ROLL_PLAY_RATE).min(ROLL_MONTAGE_SEQUENCE_LENGTH_SECS);
        let (pose_standing, pose_grounded) = write_roll_slot_curves(
            &mut target_player,
            curves.get(&curve_handle.0).and_then(AlsAnimCurves::first),
            clip_time,
        );
        sampler.reported_unready = false;
        commands
            .entity(sampler.character)
            .insert(AlsRollSlotRuntime {
                active: true,
                weight,
                pose_ready: true,
                root_motion_delta: root_motion.translation_delta,
                pose_standing,
                pose_grounded,
            });
    }
}

#[allow(clippy::type_complexity)]
fn roll_drive_system(
    time: Res<Time>,
    mut q: Query<
        (
            Entity,
            &mut AlsRollState,
            &LocomotionState,
            &Stance,
            &AlsViewYaw,
            &mut Transform,
            Option<&mut Rotation>,
            &mut LinearVelocity,
            &mut AlsStateRequest,
            Option<&AlsAirState>,
            &AlsInputCommand,
            Option<&AlsInputOverride>,
        ),
        With<als_core::character::AlsCharacter>,
    >,
    sampler_root_motion: Query<(&AlsRollSlotSampler, &RootMotionOutput)>,
    instances: Query<&AnimatedSceneInstance>,
    mut players: Query<&mut AnimationGraphPlayer>,
) {
    let dt = time.delta_secs();
    let Ok((
        character,
        mut roll,
        loco,
        stance,
        view,
        mut transform,
        rotation,
        mut vel,
        mut request,
        air,
        cmd,
        ovr,
    )) = q.single_mut()
    else {
        return;
    };
    let effective = ovr.map(|o| &o.0).unwrap_or(cmd);

    let mut lifecycle = roll.update(effective, view, loco, stance, dt);
    if !lifecycle.started && !roll.active && air.is_some_and(|air| air.landing_time <= dt.max(0.05))
    {
        let (actor_yaw, _, _) = transform.rotation.to_euler(EulerRot::YXZ);
        lifecycle = roll.start_from_landing(air.unwrap().last_impact_vy, vel.0, actor_yaw, *stance);
    }
    if lifecycle.started {
        request.stance = Some(Stance::Crouching);
    } else if lifecycle.ended {
        request.stance = Some(roll.stance_before_roll);
    }

    if !roll.active || dt <= 1e-5 {
        return;
    }

    let (current_yaw, _, _) = transform.rotation.to_euler(EulerRot::YXZ);
    let next_rotation = Quat::from_rotation_y(damper_exact_angle(
        current_yaw,
        roll.direction_yaw,
        dt,
        ROLL_ROTATION_HALF_LIFE_SECS,
    ));
    transform.rotation = next_rotation;
    if let Some(mut rotation) = rotation {
        rotation.0 = next_rotation;
    }
    let weight = roll_weight_at(roll.time);
    let clip_time = (roll.time * ROLL_PLAY_RATE).min(ROLL_MONTAGE_SEQUENCE_LENGTH_SECS);
    let local = sampler_root_motion
        .iter()
        .find_map(|(sampler, output)| {
            (sampler.character == character).then_some(read_local_delta(Some(output)))
        })
        .unwrap_or(Vec3::ZERO);
    let forward_speed = -local.z / dt;
    let dir = roll_displacement_dir(roll.direction_yaw);
    let drive = clamp_drive_speed(dir * forward_speed, 14.0);
    vel.x = drive.x;
    vel.z = drive.z;

    for instance in instances.iter() {
        if let Ok(mut player) = players.get_mut(instance.player_entity()) {
            player.set_input_data("roll_clip_time", DataValue::from(clip_time));
            player.set_input_data("roll_weight", DataValue::from(weight));
        }
    }
}
