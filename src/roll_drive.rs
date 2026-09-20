use als_core::air::AlsAirState;
use als_core::input::AlsInputCommand;
use als_core::motion::{AlsInputOverride, AlsViewYaw};
use als_core::roll::{
    AlsRollState, ROLL_MONTAGE_SEQUENCE_LENGTH_SECS, ROLL_PLAY_RATE, ROLL_ROTATION_HALF_LIFE_SECS,
    damper_exact_angle, roll_weight_at,
};
use als_core::state::{LocomotionState, Stance};
use als_ext::AlsAnimCurves;
use avian3d::prelude::{LinearVelocity, Rotation};
use bevy::ecs::hierarchy::ChildOf;
use bevy::prelude::*;
use bevy_animation_graph::core::animated_scene::AnimatedSceneInstance;
use bevy_animation_graph::core::animation_graph_player::AnimationGraphPlayer;
use bevy_animation_graph::core::edge_data::DataValue;
use bevy_animation_graph::core::plugin::AnimationGraphSet;
use bevy_animation_graph::core::systems::RootMotionOutput;

use crate::animation_events::{AlsNotifyLocomotionAction, AlsNotifyRootMotionScale};
use crate::character_assembly::AlsCharacterAvatar;
use crate::root_motion_link::{
    ensure_root_motion_output, read_local_root_motion_delta, root_motion_world_transform,
};

const ROLL_SLOT_GRAPH: &str = "graphs/ab_als/grounded/roll_slot.animgraph.ron";
const ROLL_SLOT_TIME_KEY: &str = "als_roll_slot_time";
const ROOT_POST_LOCOMOTION_SLOT_POSE_KEY: &str = "als_root_post_locomotion_slot_pose";
const ROOT_POST_LOCOMOTION_SLOT_WEIGHT_KEY: &str = "als_root_post_locomotion_slot_weight";
const ROOT_POST_LOCOMOTION_SLOT_LAYER_WEIGHT_KEY: &str =
    "als_root_post_locomotion_slot_layer_weight";
const GENERATED_POST_LOCOMOTION_SLOT_POSE_KEY: &str = "post_locomotion.pose";
const GENERATED_POST_LOCOMOTION_SLOT_WEIGHT_KEY: &str = "post_locomotion.weight";
const GENERATED_POST_LOCOMOTION_SLOT_LAYER_WEIGHT_KEY: &str = "post_locomotion.layer_weight";
const ROOT_POST_LOCOMOTION_SLOT_CURVE_PREFIX: &str = "als_root_post_locomotion_slot_curve_";
const ROLL_SLOT_CURVES: &str = "animations/Actions/Roll/A_Als_Roll.gltf#AlsCurves";
const ROLL_SLOT_CURVE_NAMES: [&str; 2] = ["PoseStanding", "PoseGrounded"];

#[derive(Component, Debug, Clone, Copy)]
struct AlsRollSlotSampler {
    character: Entity,
    target_player: Entity,
    reported_unready: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;

    #[test]
    fn roll_sampling_and_velocity_share_the_current_frame_duration() {
        for initial_time in [Some(0.4_f32), None] {
        let mut app = App::new();
        app.add_plugins(AlsRollDrivePlugin)
            .insert_resource(Time::<()>::default())
            .insert_resource(RollSlotCurveHandle(default(), default()))
            .init_resource::<Assets<AlsAnimCurves>>();
        let character = app.world_mut().spawn((
            als_core::character::AlsCharacter,
            AlsRollState { active: initial_time.is_some(), time: initial_time.unwrap_or_default(), ..default() },
            LocomotionState::Grounded, Stance::Standing, AlsViewYaw::default(),
            Transform::IDENTITY, LinearVelocity::ZERO, AlsInputCommand { roll_pressed: initial_time.is_none(), ..default() },
            AnimationGraphPlayer::new(default()),
        )).id();
        app.world_mut().spawn((
            AlsRollSlotSampler { character, target_player: character, reported_unready: true },
            AnimationGraphPlayer::new(default()), RootMotionOutput::default(),
        ));
        let mut previous = initial_time.unwrap_or_default();
        app.add_systems(PostUpdate, (move |mut samplers: Query<(&AnimationGraphPlayer, &mut RootMotionOutput), With<AlsRollSlotSampler>>| {
            for (player, mut output) in &mut samplers {
                let current = player.get_global_input_data(ROLL_SLOT_TIME_KEY).unwrap().as_f32().unwrap() / ROLL_PLAY_RATE;
                output.translation_delta = Vec3::NEG_Z * (current - previous);
                previous = current;
            }
        }).in_set(AnimationGraphSet::PrePhysics));
        for delta in [0.013_f32, 0.025, 0.011, 0.033] {
            app.world_mut().resource_mut::<Time>().advance_by(std::time::Duration::from_secs_f32(delta));
            app.world_mut().run_schedule(PostUpdate);
            assert!(app.world().get::<LinearVelocity>(character).unwrap().distance(Vec3::Z) < 1e-4,
                "a constant-speed root track must stay constant across frame durations: delta={delta}");
        }
        }
    }

    #[test]
    fn roll_blend_out_releases_root_motion_while_the_pose_keeps_playing() {
        let mut world = World::new();
        let mut time = Time::<()>::default();
        time.advance_by(std::time::Duration::from_secs_f32(0.05));
        world.insert_resource(time);
        let character = world.spawn((
            als_core::character::AlsCharacter,
            AlsRollState { active: true, time: 0.83, ..default() },
            LocomotionState::Grounded, Stance::Standing, AlsViewYaw::default(),
            Transform::default(), LinearVelocity(Vec3::X), AlsInputCommand::default(),
        )).id();
        world.spawn((
            AlsRollSlotSampler { character, target_player: character, reported_unready: false },
            RootMotionOutput { translation_delta: Vec3::NEG_Z * 0.05, ..default() },
        ));
        world.run_system_once(advance_rolls).unwrap();
        world.run_system_once(roll_drive_system).unwrap();
        assert!(world.get::<LinearVelocity>(character).unwrap().distance(Vec3::Z) < 1e-5);
        world.entity_mut(character).insert(LinearVelocity(Vec3::X * 0.5));
        world.run_system_once(advance_rolls).unwrap();
        world.run_system_once(roll_drive_system).unwrap();
        assert_eq!(*world.get::<LinearVelocity>(character).unwrap(), LinearVelocity(Vec3::X * 0.5));
        let roll = world.get::<AlsRollState>(character).unwrap();
        assert!(roll.active && roll_weight_at(roll.time) > 0.0);
    }

    #[test]
    fn mantling_notify_blocks_roll_until_the_action_is_released() {
        let mut world = World::new();
        let mut time = Time::<()>::default();
        time.advance_by(std::time::Duration::from_secs_f32(0.05));
        world.insert_resource(time);
        let character = world.spawn((als_core::character::AlsCharacter,
            crate::animation_events::AlsNotifyLocomotionAction::Mantling,
            AlsRollState::default(), LocomotionState::Grounded, Stance::Standing,
            AlsViewYaw::default(), Transform::default(), LinearVelocity::ZERO,
            AlsInputCommand { roll_pressed: true, ..default() })).id();
        world.run_system_once(advance_rolls).unwrap();
        assert!(!world.get::<AlsRollState>(character).unwrap().active);
        world.entity_mut(character).insert(crate::animation_events::AlsNotifyLocomotionAction::None);
        world.entity_mut(character).insert((LocomotionState::Custom,
            AlsAirState { landing_time: 0.0, last_impact_vy: 10.0, ..default() }));
        world.run_system_once(advance_rolls).unwrap();
        assert!(!world.get::<AlsRollState>(character).unwrap().active);
        world.entity_mut(character).insert(LocomotionState::Grounded);
        world.run_system_once(advance_rolls).unwrap();
        assert!(world.get::<AlsRollState>(character).unwrap().active);
    }

    #[test]
    fn get_up_input_block_prevents_an_automatic_landing_roll() {
        let mut world = World::new();
        let mut time = Time::<()>::default();
        time.advance_by(std::time::Duration::from_secs_f32(0.05));
        world.insert_resource(time);
        let character = world
            .spawn((
                als_core::character::AlsCharacter,
                als_core::motion::AlsMovementInputBlocked,
                AlsRollState::default(),
                LocomotionState::Grounded,
                Stance::Standing,
                AlsViewYaw::default(),
                Transform::default(),
                LinearVelocity(Vec3::X),
                AlsAirState {
                    landing_time: 0.0,
                    last_impact_vy: 10.0,
                    ..default()
                },
                AlsInputCommand::default(),
            ))
            .id();
        world.run_system_once(advance_rolls).unwrap();
        world.run_system_once(roll_drive_system).unwrap();
        assert!(!world.get::<AlsRollState>(character).unwrap().active);
        world
            .entity_mut(character)
            .remove::<als_core::motion::AlsMovementInputBlocked>();
        world.run_system_once(advance_rolls).unwrap();
        world.run_system_once(roll_drive_system).unwrap();
        assert!(world.get::<AlsRollState>(character).unwrap().active);
        assert_eq!(*world.get::<LinearVelocity>(character).unwrap(), LinearVelocity::ZERO);
    }

    #[test]
    fn ragdoll_stops_roll_motion_while_preserving_the_montage_fade() {
        let mut world = World::new();
        let mut time = Time::<()>::default();
        time.advance_by(std::time::Duration::from_secs_f32(0.1));
        world.insert_resource(time);
        let roll = AlsRollState {
            active: true,
            time: 0.3,
            ..default()
        };
        let transform =
            Transform::from_xyz(3.0, 1.0, -2.0).with_rotation(Quat::from_rotation_y(0.7));
        let velocity = LinearVelocity(Vec3::new(1.0, -3.0, 2.0));
        let character = world
            .spawn((
                als_core::character::AlsCharacter,
                roll,
                AlsRollBlendOut::new(&roll, crate::get_up::RAGDOLL_SETTINGS.stop_montage_blend_seconds),
                LocomotionState::Grounded,
                AlsNotifyLocomotionAction::Ragdolling,
                Stance::Standing,
                AlsViewYaw::default(),
                transform,
                velocity,
                AlsInputCommand::default(),
            ))
            .id();
        world.run_system_once(advance_rolls).unwrap();
        world.run_system_once(roll_drive_system).unwrap();
        assert_eq!(*world.get::<Transform>(character).unwrap(), transform);
        assert_eq!(*world.get::<LinearVelocity>(character).unwrap(), velocity);
        assert!((world.get::<AlsRollBlendOut>(character).unwrap().weight() - 0.5).abs() < 1e-5);
        assert!(world.get::<AlsRollState>(character).unwrap().active);
        world.run_system_once(advance_rolls).unwrap();
        world.run_system_once(roll_drive_system).unwrap();
        assert!(!world.get::<AlsRollState>(character).unwrap().active);
        assert!(world.get::<AlsRollBlendOut>(character).is_none());
    }
}

#[derive(Component, Debug, Clone, Copy, Default)]
pub struct AlsRollSlotRuntime {
    pub active: bool,
    pub weight: f32,
    pub pose_ready: bool,
    pub root_motion_delta: Vec3,
    pub root_motion_rotation: Quat,
    pub pose_standing: f32,
    pub pose_grounded: f32,
}

#[derive(Resource)]
struct RollSlotCurveHandle(Handle<AlsAnimCurves>, Handle<bevy_animation_graph::core::animation_clip::GraphClip>);

#[derive(Component, Debug, Clone, Copy)]
pub(crate) struct AlsRollBlendOut {
    elapsed: f32,
    start_weight: f32,
    duration: f32,
}

impl AlsRollBlendOut {
    pub(crate) fn new(roll: &AlsRollState, duration: f32) -> Self {
        Self {
            elapsed: 0.0,
            start_weight: roll_weight_at(roll.time),
            duration,
        }
    }

    fn weight(self) -> f32 {
        let alpha = if self.duration > 0.0 { self.elapsed / self.duration } else { 1.0 };
        self.start_weight
            * (1.0 - als_ext::transition_alpha(alpha, als_ext::AlphaBlendMode::Cubic, &[]))
    }

    pub(crate) fn stop(&mut self, duration: f32) {
        if duration < self.duration {
            self.start_weight = self.weight();
            self.elapsed = 0.0;
            self.duration = duration;
        }
    }
}

pub struct AlsRollDrivePlugin;

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AlsRollDriveSet;

impl Plugin for AlsRollDrivePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, load_roll_slot_curve_asset)
            .add_systems(Update, ensure_roll_slot_samplers)
            .configure_sets(
                PostUpdate,
                AlsRollDriveSet.after(AnimationGraphSet::PrePhysics),
            )
            .add_systems(
                PostUpdate,
                (
                    advance_rolls.after(ensure_roll_state).before(prepare_roll_slot_samples),
                    prepare_roll_slot_samples.before(AnimationGraphSet::PrePhysics),
                    ensure_root_motion_output,
                    ensure_roll_state,
                    bridge_roll_slot_samples.after(AnimationGraphSet::PrePhysics),
                    roll_drive_system
                        .after(ensure_roll_state)
                        .after(bridge_roll_slot_samples)
                        .after(AnimationGraphSet::PrePhysics)
                        .in_set(AlsRollDriveSet),
                ),
            );
    }
}

fn load_roll_slot_curve_asset(mut commands: Commands, assets: Res<AssetServer>) {
    commands.insert_resource(RollSlotCurveHandle(assets.load(ROLL_SLOT_CURVES), assets.load("animations/Actions/Roll/A_Als_Roll.anim.ron")));
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
    avatars: Query<(Entity, &AlsCharacterAvatar, &AnimatedSceneInstance), With<AlsCharacterAvatar>>,
    samplers: Query<&AlsRollSlotSampler>,
    players: Query<&AnimationGraphPlayer>,
    mut commands: Commands,
) {
    for (avatar, character_avatar, instance) in &avatars {
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
                character: character_avatar.character,
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
    get_ups: Query<&crate::get_up::AlsGetUpState>,
    blend_outs: Query<&AlsRollBlendOut>,
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
            target_player.set_global_input_data("roll.weight", 0.0_f32);
            als_ext::source_gameplay_tag_input::set_slot_input_context(
                sampler.target_player,
                "roll.pose",
                None,
            );
            if get_ups.contains(sampler.character) {
                continue;
            }
            als_ext::source_gameplay_tag_input::set_slot_input_context(
                sampler.target_player,
                GENERATED_POST_LOCOMOTION_SLOT_POSE_KEY,
                None,
            );
            target_player.set_global_input_data(ROOT_POST_LOCOMOTION_SLOT_WEIGHT_KEY, 0.0_f32);
            target_player
                .set_global_input_data(ROOT_POST_LOCOMOTION_SLOT_LAYER_WEIGHT_KEY, 0.0_f32);
            target_player.set_global_input_data(GENERATED_POST_LOCOMOTION_SLOT_WEIGHT_KEY, 0.0_f32);
            target_player
                .set_global_input_data(GENERATED_POST_LOCOMOTION_SLOT_LAYER_WEIGHT_KEY, 0.0_f32);
            write_roll_slot_curves(&mut target_player, None, 0.0);
            commands
                .entity(sampler.character)
                .insert(AlsRollSlotRuntime::default());
            continue;
        };
        let weight = blend_outs
            .get(sampler.character)
            .map_or_else(|_| roll_weight_at(roll.time), |blend| blend.weight());
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
            target_player.set_global_input_data(GENERATED_POST_LOCOMOTION_SLOT_WEIGHT_KEY, 0.0_f32);
            target_player
                .set_global_input_data(GENERATED_POST_LOCOMOTION_SLOT_LAYER_WEIGHT_KEY, 0.0_f32);
            write_roll_slot_curves(&mut target_player, None, 0.0);
            commands
                .entity(sampler.character)
                .insert(AlsRollSlotRuntime {
                    active: true,
                    weight: 0.0,
                    pose_ready: false,
                    root_motion_delta: root_motion.translation_delta,
                    root_motion_rotation: root_motion.rotation_delta,
                    pose_standing: 0.0,
                    pose_grounded: 0.0,
                });
            continue;
        };
        target_player.set_global_input_data("roll.pose", DataValue::Pose(pose.clone()));
        target_player.set_global_input_data("roll.weight", weight);
        let clip_time = (roll.time * ROLL_PLAY_RATE).min(ROLL_MONTAGE_SEQUENCE_LENGTH_SECS);
        let context = curves
            .get(&curve_handle.0)
            .and_then(AlsAnimCurves::first)
            .map(|curves| {
                let mut context = crate::get_up::sampled_curve_context(curves, clip_time);
                context.asset_samples.push(als_ext::animation_context::SourceAssetSample {
                    identity: "roll".to_owned(), clip: curve_handle.1.clone(), montage: true,
                    clock: als_ext::SourceRelevantAsset { duration: ROLL_MONTAGE_SEQUENCE_LENGTH_SECS, position: clip_time, weight, ..default() },
                    ..default()
                });
                context
            });
        als_ext::source_gameplay_tag_input::set_slot_input_context(
            sampler.target_player,
            "roll.pose",
            context.clone(),
        );
        if get_ups.contains(sampler.character) {
            continue;
        }
        als_ext::source_gameplay_tag_input::set_slot_input_context(
            sampler.target_player,
            GENERATED_POST_LOCOMOTION_SLOT_POSE_KEY,
            context,
        );
        target_player.set_global_input_data(
            ROOT_POST_LOCOMOTION_SLOT_POSE_KEY,
            DataValue::Pose(pose.clone()),
        );
        target_player.set_global_input_data(ROOT_POST_LOCOMOTION_SLOT_WEIGHT_KEY, weight);
        target_player.set_global_input_data(ROOT_POST_LOCOMOTION_SLOT_LAYER_WEIGHT_KEY, weight);
        target_player.set_global_input_data(
            GENERATED_POST_LOCOMOTION_SLOT_POSE_KEY,
            DataValue::Pose(pose),
        );
        target_player.set_global_input_data(GENERATED_POST_LOCOMOTION_SLOT_WEIGHT_KEY, weight);
        target_player
            .set_global_input_data(GENERATED_POST_LOCOMOTION_SLOT_LAYER_WEIGHT_KEY, 1.0_f32);
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
                root_motion_rotation: root_motion.rotation_delta,
                pose_standing,
                pose_grounded,
            });
    }
}

#[allow(clippy::type_complexity)]
fn advance_rolls(
    time: Res<Time>,
    mut commands: Commands,
    blocked: Query<(), With<als_core::motion::AlsMovementInputBlocked>>,
    actions: Query<&crate::animation_events::AlsNotifyLocomotionAction>,
    mut blend_outs: Query<&mut AlsRollBlendOut>,
    mut q: Query<
        (
            Entity,
            &mut AlsRollState,
            &LocomotionState,
            &Stance,
            &AlsViewYaw,
            &Transform,
            &LinearVelocity,
            Option<&AlsAirState>,
            &AlsInputCommand,
            Option<&AlsInputOverride>,
        ),
        With<als_core::character::AlsCharacter>,
    >,
) {
    let dt = time.delta_secs();
    for (
        character,
        mut roll,
        loco,
        stance,
        view,
        transform,
        vel,
        air,
        cmd,
        ovr,
    ) in &mut q
    {
        let action_blocked = actions.get(character).is_ok_and(|action| !matches!(*action,
            crate::animation_events::AlsNotifyLocomotionAction::None | crate::animation_events::AlsNotifyLocomotionAction::Rolling));
        let effective = als_core::motion::movement_input_for_action(
            *ovr.map(|o| &o.0).unwrap_or(cmd),
            blocked.contains(character) || action_blocked,
        );

        if let Ok(mut blend) = blend_outs.get_mut(character) {
            roll.time += dt;
            blend.elapsed += dt;
            if blend.weight() <= 0.0 {
                roll.active = false;
                commands.entity(character).remove::<AlsRollBlendOut>();
            }
            continue;
        }

        let mut lifecycle = roll.update(&effective, view, loco, stance, dt);
        if !lifecycle.started
            && !roll.active
            && *loco == LocomotionState::Grounded
            && !blocked.contains(character)
            && !action_blocked
            && air.is_some_and(|air| air.landing_time <= dt.max(0.05))
        {
            let (actor_yaw, _, _) = transform.rotation.to_euler(EulerRot::YXZ);
            lifecycle = roll.start_from_landing(air.unwrap().last_impact_vy, vel.0, actor_yaw, *stance);
        }
        if lifecycle.started {
            roll.time = dt;
        }
    }
}

fn roll_drive_system(
    time: Res<Time>,
    blend_outs: Query<(), With<AlsRollBlendOut>>,
    mut q: Query<(Entity, &AlsRollState, Option<&AlsNotifyLocomotionAction>, &mut Transform, Option<&mut Rotation>,
        &mut LinearVelocity, Option<&AlsNotifyRootMotionScale>), With<als_core::character::AlsCharacter>>,
    sampler_root_motion: Query<(&AlsRollSlotSampler, &RootMotionOutput)>,
    avatars: Query<(&AlsCharacterAvatar, &AnimatedSceneInstance)>,
    mut players: Query<&mut AnimationGraphPlayer>,
) {
    let dt = time.delta_secs();
    for (character, roll, action, mut transform, rotation, mut vel, notify_root_scale) in &mut q {
        if !roll.active || action == Some(&AlsNotifyLocomotionAction::Ragdolling) || dt <= 1e-5 || blend_outs.contains(character)
            || roll.time - dt >= als_core::roll::ROLL_DURATION_SECS - als_core::roll::ROLL_WEIGHT_OUT_SECS {
            continue;
        }

        let (mut local_translation, local_rotation) = sampler_root_motion
            .iter()
            .find_map(|(sampler, output)| {
                (sampler.character == character)
                    .then_some(read_local_root_motion_delta(Some(output)))
            })
            .unwrap_or((Vec3::ZERO, Quat::IDENTITY));
        local_translation *= notify_root_scale.map_or(1.0, |scale| scale.0).max(0.0);
        let (world_translation, root_rotation) =
            root_motion_world_transform(transform.rotation, local_translation, local_rotation);
        let next_rotation = if local_rotation.dot(Quat::IDENTITY).abs() < 1.0 - 1e-6 {
            root_rotation
        } else {
            let (current_yaw, _, _) = transform.rotation.to_euler(EulerRot::YXZ);
            Quat::from_rotation_y(damper_exact_angle(
                current_yaw,
                roll.direction_yaw,
                dt,
                ROLL_ROTATION_HALF_LIFE_SECS,
            ))
        };
        transform.rotation = next_rotation;
        if let Some(mut rotation) = rotation {
            rotation.0 = next_rotation;
        }
        let weight = roll_weight_at(roll.time);
        let clip_time = (roll.time * ROLL_PLAY_RATE).min(ROLL_MONTAGE_SEQUENCE_LENGTH_SECS);
        vel.0 = world_translation / dt;

        let Some(instance) = avatars
            .iter()
            .find_map(|(avatar, instance)| (avatar.character == character).then_some(instance))
        else {
            continue;
        };
        if let Ok(mut player) = players.get_mut(instance.player_entity()) {
            player.set_input_data("roll_clip_time", DataValue::from(clip_time));
            player.set_input_data("roll_weight", DataValue::from(weight));
        }
    }
}
