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
use crate::character_assembly::{AlsCharacterAssembly, AlsCharacterAvatar};
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
    fn roll_direction_uses_player_view_for_input_and_actor_yaw_without_input() {
        let mut failures = Vec::new();
        for (input, expected) in [(Vec2::Y, 0.4_f32), (Vec2::ZERO, 0.7)] {
            let mut world = World::new();
            let mut time = Time::<()>::default();
            time.advance_by(std::time::Duration::from_secs_f32(1.0 / 60.0));
            world.insert_resource(time);
            let character = world.spawn((als_core::character::AlsCharacter, AlsRollState::default(),
                LocomotionState::Grounded, Stance::Standing, AlsViewYaw(1.2), als_core::view::AlsPlayerViewYaw(0.4),
                Transform::from_rotation(Quat::from_rotation_y(0.7)), LinearVelocity::ZERO,
                AlsInputCommand { roll_pressed: true, move_dir: input, ..default() })).id();
            world.run_system_once(advance_rolls).unwrap();
            let actual = world.get::<AlsRollState>(character).unwrap().direction_yaw;
            if als_core::motion::wrap_yaw_radians(actual - expected).abs() > 1e-5 { failures.push((input, expected, actual)); }
        }
        assert!(failures.is_empty(), "Roll used the wrong input or actor basis: {failures:?}");
    }

    fn ready_roll_player(app: &mut App, avatar: Entity) -> Entity {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(40);
        loop {
            app.update();
            if let Some(instance) = app.world().get::<AnimatedSceneInstance>(avatar)
                && let Some(player) = app.world().get::<AnimationGraphPlayer>(instance.player_entity())
                && player.get_default_output_pose().is_some() {
                let target = instance.player_entity();
                if app.world_mut().query::<(&AlsRollSlotSampler, &AnimationGraphPlayer)>().iter(app.world())
                    .any(|(sampler, player)| sampler.target_player == target && player.get_default_output_pose().is_some()) { return target; }
            }
            assert!(std::time::Instant::now() < deadline, "Roll player did not become ready");
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }

    fn loaded_roll_character() -> (App, Entity, Entity) {
        use crate::character_assembly::{AlsCharacterAssembly, AlsCharacterBlueprint, spawn_als_character};
        let (mut app, _, _, _) = crate::camera_component::tests::loaded_camera_scene();
        app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(1.0 / 60.0)));
        app.world_mut().spawn((avian3d::prelude::RigidBody::Static, avian3d::prelude::Collider::cuboid(100.0, 1.0, 100.0),
            Transform::from_xyz(0.0, -0.5, 0.0), avian3d::prelude::CollisionLayers::new(
                [als_core::character::AlsCollisionLayer::Default, als_core::character::AlsCollisionLayer::Visibility], avian3d::prelude::LayerMask::ALL)));
        let character = app.world_mut().run_system_once(|mut commands: Commands, assets: Res<AssetServer>,
            mut params: ResMut<Assets<als_core::params::AlsMovementParams>>, mut configs: ResMut<Assets<als_core::control_scheme::AlsControlSchemeConfig>>| {
            spawn_als_character(&mut commands, &assets, &mut params, &mut configs,
                AlsCharacterBlueprint { spawn_input: false, ..AlsCharacterBlueprint::new(Transform::from_xyz(0.0, 1.5, 0.0)) })
        }).unwrap();
        let avatar = app.world().get::<AlsCharacterAssembly>(character).unwrap().avatar;
        ready_roll_player(&mut app, avatar);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while *app.world().get::<LocomotionState>(character).unwrap() != LocomotionState::Grounded {
            app.update();
            assert!(std::time::Instant::now() < deadline, "Roll fixture did not reach the ground");
        }
        (app, character, avatar)
    }

    fn play_loaded_roll(app: &mut App, character: Entity) -> Entity {
        *app.world_mut().get_mut::<LocomotionState>(character).unwrap() = LocomotionState::Grounded;
        app.world_mut().get_mut::<AlsInputCommand>(character).unwrap().roll_pressed = true;
        app.world_mut().run_system_once(advance_rolls).unwrap();
        app.world_mut().get_mut::<AlsInputCommand>(character).unwrap().roll_pressed = false;
        for _ in 0..5 { app.update(); }
        assert!(app.world().get::<AlsRollState>(character).unwrap().active, "Roll did not start through its input consumer");
        assert!(app.world().get::<AlsRollSlotRuntime>(character).unwrap().pose_ready);
        assert!(app.world().get::<AlsRollBlendOut>(character).is_none(), "Roll fixture entered an early blend out");
        app.world_mut().run_system_once(roll_drive_system).unwrap();
        let avatar = app.world().get::<crate::character_assembly::AlsCharacterAssembly>(character).unwrap().avatar;
        let target = app.world().get::<AnimatedSceneInstance>(avatar).unwrap().player_entity();
        let (entity, _, player, output) = app.world_mut().query::<(Entity, &AlsRollSlotSampler, &AnimationGraphPlayer, &RootMotionOutput)>()
            .iter(app.world()).find(|(_, sampler, _, _)| sampler.target_player == target).unwrap();
        assert!(player.get_error().is_none() && player.get_default_output_pose().is_some());
        assert!(output.translation_delta.length() > 1e-5, "Loaded roll did not extract root motion");
        let expected = app.world().get::<Transform>(character).unwrap().rotation
            * app.world().get::<Transform>(avatar).unwrap().rotation * output.translation_delta;
        let expected = expected * app.world().get::<AlsNotifyRootMotionScale>(character).unwrap().0 / app.world().resource::<Time>().delta_secs();
        assert!(app.world().get::<LinearVelocity>(character).unwrap().abs_diff_eq(expected, 1e-4),
            "Root motion did not come from the current roll player: expected={expected:?}, actual={:?}", app.world().get::<LinearVelocity>(character));
        entity
    }

    #[test]
    fn roll_sampler_ends_when_the_same_player_entity_is_replaced() {
        use bevy_animation_graph::core::animation_graph_player::AnimationSource;
        let (mut app, character, avatar) = loaded_roll_character();
        let target = app.world().get::<AnimatedSceneInstance>(avatar).unwrap().player_entity();
        for remove_first in [false, true] {
            let sampler = play_loaded_roll(&mut app, character);
            let player = app.world().get::<AnimationGraphPlayer>(target).unwrap();
            let AnimationSource::Graph(graph) = player.get_animation_source() else { panic!("Character must use its actual graph"); };
            let replacement = AnimationGraphPlayer::new(player.skeleton().clone()).with_graph(graph.clone());
            if remove_first { app.world_mut().entity_mut(target).remove::<AnimationGraphPlayer>(); }
            app.world_mut().entity_mut(target).insert(replacement);
            assert!(app.world().get_entity(sampler).is_err(), "Old sampler survived player replacement");
            assert!(!app.world().get::<AlsRollState>(character).unwrap().active, "Old montage survived instance termination");
            ready_roll_player(&mut app, avatar);
            assert_eq!(app.world_mut().query::<&AlsRollSlotSampler>().iter(app.world()).filter(|sampler| sampler.character == character).count(), 1);
        }
        let sampler = play_loaded_roll(&mut app, character);
        app.world_mut().despawn(target);
        app.update();
        assert!(app.world().get_entity(sampler).is_err());
        assert!(!app.world().get::<AlsRollState>(character).unwrap().active);
    }

    #[test]
    fn roll_sampler_releases_the_retained_old_avatar_and_uses_the_new_player() {
        use crate::character_assembly::{AlsCharacterAssembly, ALS_LINKED_ANIMATED_SCENE, attach_avatar};
        let (mut app, character, old_avatar) = loaded_roll_character();
        let old_sampler = play_loaded_roll(&mut app, character);
        let old_player = app.world().get::<AnimatedSceneInstance>(old_avatar).unwrap().player_entity();
        app.world_mut().entity_mut(old_avatar).remove::<AlsCharacterAvatar>();
        app.world_mut().get_mut::<AnimationGraphPlayer>(old_player).unwrap().pause();
        let avatar = app.world_mut().run_system_once_with(|In(character): In<Entity>, mut commands: Commands, assets: Res<AssetServer>| {
            attach_avatar(&mut commands, &assets, character, ALS_LINKED_ANIMATED_SCENE)
        }, character).unwrap();
        app.world_mut().get_mut::<AlsCharacterAssembly>(character).unwrap().avatar = avatar;
        let target = ready_roll_player(&mut app, avatar);
        assert!(app.world().get_entity(old_sampler).is_err(), "Old avatar kept an evaluated roll sampler");
        assert!(!app.world().get::<AlsRollState>(character).unwrap().active);
        let sampler = play_loaded_roll(&mut app, character);
        let current: Vec<_> = app.world_mut().query::<(Entity, &AlsRollSlotSampler)>().iter(app.world())
            .filter(|(_, sampler)| sampler.character == character).map(|(entity, sampler)| (entity, sampler.target_player)).collect();
        assert_eq!(current, [(sampler, target)]);
        let expected = app.world().get::<RootMotionOutput>(sampler).unwrap().translation_delta;
        assert!(app.world().get::<AlsRollSlotRuntime>(character).unwrap().root_motion_delta.abs_diff_eq(expected, 1e-6));
        app.world_mut().despawn(old_avatar);
        app.update();
        assert!(app.world().get::<AlsRollState>(character).unwrap().active, "Deleting an old avatar stopped the new roll");
        app.world_mut().despawn(character);
        assert!(app.world().get_entity(sampler).is_err());
    }

    #[test]
    fn roll_sampler_ends_with_avatar_destruction_in_both_child_orders() {
        for reverse in [false, true] {
            let (mut app, character, avatar) = loaded_roll_character();
            let sampler = play_loaded_roll(&mut app, character);
            if reverse {
                let children: Vec<_> = app.world().get::<Children>(avatar).unwrap().iter().rev().collect();
                app.world_mut().entity_mut(avatar).replace_children(&children);
            }
            app.world_mut().despawn(avatar);
            app.update();
            assert!(app.world().get_entity(sampler).is_err());
            assert!(!app.world().get::<AlsRollState>(character).unwrap().active, "Avatar destruction retained an old montage");
        }
    }

    #[test]
    fn roll_sampling_and_velocity_share_the_current_frame_duration() {
        for initial_time in [Some(0.4_f32), None] {
        let (mut app, character, _) = loaded_roll_character();
        app.world_mut().entity_mut(character).insert((
            avian3d::prelude::RigidBody::Kinematic, bevy_tnua::TnuaToggle::Disabled,
            AlsRollState { active: initial_time.is_some(), time: initial_time.unwrap_or_default(), ..default() },
            LocomotionState::Grounded, Stance::Standing, AlsViewYaw::default(),
            Transform::IDENTITY, LinearVelocity::ZERO, AlsInputCommand { roll_pressed: initial_time.is_none(), ..default() },
        ));
        let mut previous = initial_time.unwrap_or_default();
        app.add_systems(PostUpdate, (move |mut samplers: Query<(&AnimationGraphPlayer, &mut RootMotionOutput), With<AlsRollSlotSampler>>| {
            for (player, mut output) in &mut samplers {
                let current = player.get_global_input_data(ROLL_SLOT_TIME_KEY).unwrap().as_f32().unwrap() / ROLL_PLAY_RATE;
                output.translation_delta = Vec3::NEG_Z * (current - previous);
                previous = current;
            }
        }).after(AnimationGraphSet::PrePhysics).before(bridge_roll_slot_samples));
        for delta in [0.013_f32, 0.025, 0.011, 0.033] {
            app.world_mut().resource_mut::<Time>().advance_by(std::time::Duration::from_secs_f32(delta));
            app.world_mut().run_schedule(PostUpdate);
            let expected = Vec3::Z * app.world().get::<AlsNotifyRootMotionScale>(character).unwrap().0;
            assert!(app.world().get::<LinearVelocity>(character).unwrap().distance(expected) < 1e-4,
                "a constant-speed root track must stay constant across frame durations: delta={delta}, actual={:?}, roll={:?}, time={}, scale={:?}",
                app.world().get::<LinearVelocity>(character), app.world().get::<AlsRollState>(character),
                app.world().resource::<Time>().delta_secs(), app.world().get::<AlsNotifyRootMotionScale>(character));
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
            .add_observer(discard_roll_target)
            .add_systems(PostUpdate, ensure_roll_slot_samplers.after(ensure_roll_state).before(advance_rolls))
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
    characters: Query<(Entity, &AlsCharacterAssembly), With<als_core::character::AlsCharacter>>,
    avatars: Query<(&AlsCharacterAvatar, &AnimatedSceneInstance)>,
    samplers: Query<(Entity, &AlsRollSlotSampler)>,
    players: Query<&AnimationGraphPlayer>,
    mut states: Query<(&mut AlsRollState, Option<&mut AlsRollSlotRuntime>, Option<&mut AlsNotifyLocomotionAction>)>,
    mut commands: Commands,
) {
    let current_target = |character| {
        let (_, assembly) = characters.get(character).ok()?;
        let (avatar, instance) = avatars.get(assembly.avatar).ok()?;
        (avatar.character == character && players.contains(instance.player_entity())).then_some(instance.player_entity())
    };
    for (entity, sampler) in &samplers {
        if current_target(sampler.character) == Some(sampler.target_player) { continue; }
        if let Ok((mut roll, runtime, action)) = states.get_mut(sampler.character) {
            end_roll_instance(&mut roll, runtime, action);
            commands.entity(sampler.character).try_remove::<AlsRollBlendOut>();
        }
        commands.entity(entity).try_despawn();
    }
    for (character, _) in &characters {
        let Some(target_player) = current_target(character) else { continue; };
        if samplers
            .iter()
            .any(|(_, sampler)| sampler.character == character && sampler.target_player == target_player)
        {
            continue;
        }
        let Ok(player) = players.get(target_player) else {
            continue;
        };
        commands.spawn((
            Name::new("als_roll_slot_sampler"),
            Children::default(),
            ChildOf(character),
            AnimationGraphPlayer::new(player.skeleton().clone())
                .with_graph(asset_server.load(ROLL_SLOT_GRAPH)),
            RootMotionOutput::default(),
            AlsRollSlotSampler {
                character,
                target_player,
                reported_unready: false,
            },
        ));
    }
}

fn end_roll_instance(roll: &mut AlsRollState, runtime: Option<Mut<AlsRollSlotRuntime>>, action: Option<Mut<AlsNotifyLocomotionAction>>) {
    roll.active = false;
    if let Some(mut runtime) = runtime { *runtime = default(); }
    if let Some(mut action) = action && *action == AlsNotifyLocomotionAction::Rolling { *action = AlsNotifyLocomotionAction::None; }
}

fn discard_roll_target(
    event: On<Discard, AnimationGraphPlayer>,
    samplers: Query<(Entity, &AlsRollSlotSampler)>,
    mut states: Query<(&mut AlsRollState, Option<&mut AlsRollSlotRuntime>, Option<&mut AlsNotifyLocomotionAction>)>,
    mut commands: Commands,
) {
    for (entity, sampler) in &samplers {
        if sampler.target_player != event.entity { continue; }
        if let Ok((mut roll, runtime, action)) = states.get_mut(sampler.character) {
            end_roll_instance(&mut roll, runtime, action);
            commands.entity(sampler.character).try_remove::<AlsRollBlendOut>();
        }
        commands.entity(entity).try_despawn();
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
    mut source_inputs: Query<&mut als_ext::SourceAnimationInputs>,
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
            source_inputs.get_mut(sampler.target_player).unwrap().set_slot_input_context("roll.pose",
                None,);
            if get_ups.contains(sampler.character) {
                continue;
            }
            source_inputs.get_mut(sampler.target_player).unwrap().set_slot_input_context(GENERATED_POST_LOCOMOTION_SLOT_POSE_KEY,
                None,);
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
        source_inputs.get_mut(sampler.target_player).unwrap().set_slot_input_context("roll.pose",
            context.clone(),);
        if get_ups.contains(sampler.character) {
            continue;
        }
        source_inputs.get_mut(sampler.target_player).unwrap().set_slot_input_context(GENERATED_POST_LOCOMOTION_SLOT_POSE_KEY,
            context,);
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
            (&AlsViewYaw, Option<&als_core::view::AlsPlayerViewYaw>),
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
        (view, player_view),
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

        let direction_basis = AlsViewYaw(if effective.move_dir.length_squared() > 1e-6 {
            player_view.map_or(view.0, |view| view.0)
        } else { transform.rotation.to_euler(EulerRot::YXZ).0 });
        let mut lifecycle = roll.update(&effective, &direction_basis, loco, stance, dt);
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
    mut players: Query<&mut AnimationGraphPlayer>,
) {
    let dt = time.delta_secs();
    for (character, roll, action, mut transform, rotation, mut vel, notify_root_scale) in &mut q {
        if !roll.active || action == Some(&AlsNotifyLocomotionAction::Ragdolling) || dt <= 1e-5 || blend_outs.contains(character)
            || roll.time - dt >= als_core::roll::ROLL_DURATION_SECS - als_core::roll::ROLL_WEIGHT_OUT_SECS {
            continue;
        }

        let sample = sampler_root_motion
            .iter()
            .find(|(sampler, _)| sampler.character == character);
        let (mut local_translation, local_rotation) = read_local_root_motion_delta(sample.map(|(_, output)| output));
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

        if let Some((sampler, _)) = sample && let Ok(mut player) = players.get_mut(sampler.target_player) {
            player.set_input_data("roll_clip_time", DataValue::from(clip_time));
            player.set_input_data("roll_weight", DataValue::from(weight));
        }
    }
}
