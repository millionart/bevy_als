use crate::als::air::AlsAirState;
use crate::als::input::AlsInputCommand;
use crate::als::motion::{AlsInputOverride, AlsViewYaw};
use crate::als::roll::{
    AlsRollState, ROLL_MONTAGE_SEQUENCE_LENGTH_SECS, ROLL_PLAY_RATE, ROLL_ROTATION_HALF_LIFE_SECS,
    damper_exact_angle, roll_weight_at,
};
use crate::als::state::{LocomotionState, Stance};
use crate::extensions::AlsAnimCurves;
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
    ensure_root_motion_output, root_motion_world_transform,
};

const ROLL_SLOT_GRAPH: &str = "ALS/Character/AnimationInstances/AB_Als_Grounded/RollSlot.animgraph.ron";
const ROLL_SLOT_TIME_KEY: &str = "als_roll_slot_time";
const ROOT_POST_LOCOMOTION_SLOT_POSE_KEY: &str = "als_root_post_locomotion_slot_pose";
const ROOT_POST_LOCOMOTION_SLOT_WEIGHT_KEY: &str = "als_root_post_locomotion_slot_weight";
const ROOT_POST_LOCOMOTION_SLOT_LAYER_WEIGHT_KEY: &str =
    "als_root_post_locomotion_slot_layer_weight";
const GENERATED_POST_LOCOMOTION_SLOT_POSE_KEY: &str = "post_locomotion.pose";
const GENERATED_POST_LOCOMOTION_SLOT_WEIGHT_KEY: &str = "post_locomotion.weight";
const GENERATED_POST_LOCOMOTION_SLOT_LAYER_WEIGHT_KEY: &str = "post_locomotion.layer_weight";
const ROOT_POST_LOCOMOTION_SLOT_CURVE_PREFIX: &str = "als_root_post_locomotion_slot_curve_";
const ROLL_SLOT_CURVES: &str = "ALS/Animations/Actions/Roll/A_Als_Roll.gltf#AlsCurves";
const ROLL_SLOT_CURVE_NAMES: [&str; 2] = ["PoseStanding", "PoseGrounded"];

#[derive(Component, Debug, Clone, Copy)]
pub(crate) struct AlsRollSlotSampler {
    pub(crate) character: Entity,
    target_player: Entity,
    reported_unready: bool,
}

#[derive(Component, Debug, Clone, Copy)]
pub(crate) struct AlsRollTail {
    pub(crate) time: f32,
    blend: AlsRollBlendOut,
}

impl AlsRollTail {
    pub(crate) fn weight(&self) -> f32 { self.blend.weight() }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;

    #[test]
    fn roll_paused_tap_release_does_not_start_the_actual_roll_driver() {
        use crate::als::input::{AlsInputMap, AlsInputPlugin, spawn_als_actions};
        use bevy::input::gamepad::{RawGamepadButtonChangedEvent, RawGamepadEvent};
        use bevy::time::TimeUpdateStrategy;
        use bevy_enhanced_input::prelude::EnhancedInputPlugin;
        let cases = [
            vec![(false,true,2,false),(true,true,1,false),(true,false,2,false),
                (false,false,1,false),(false,true,2,false),(false,false,1,true)],
            vec![(true,true,2,false),(true,false,2,false),(false,false,1,false)],
            vec![(true,true,2,false),(false,true,1,false),(false,false,1,true)],
            vec![(false,true,2,false),(true,true,20,false),(false,true,1,false),(false,false,1,false)],
            vec![(false,true,2,false),(true,true,2,false),(false,false,1,true)],
        ];
        for gamepad_input in [false, true] {
            for speed in [1.0, 0.25] {
                for (case_index, steps) in cases.iter().enumerate() {
                    let mut app = App::new();
                    app.add_plugins((MinimalPlugins, bevy::input::InputPlugin, EnhancedInputPlugin, AlsInputPlugin))
                        .insert_resource(TimeUpdateStrategy::ManualDuration(std::time::Duration::from_millis(20)))
                        .add_systems(PostUpdate, advance_rolls);
                    app.finish();
                    let character = app.world_mut().spawn((crate::als::character::AlsCharacter,
                        AlsRollState::default(), LocomotionState::Grounded, Stance::Standing,
                        AlsViewYaw(0.0), Transform::IDENTITY, LinearVelocity::ZERO)).id();
                    let gamepad = app.world_mut().spawn(Gamepad::default()).id();
                    spawn_als_actions(&mut app.world_mut().commands(), character, &AlsInputMap::default());
                    app.update();
                    app.world_mut().resource_mut::<Time<Virtual>>().set_relative_speed(speed);
                    for (step_index, &(paused, held, frames, expected)) in steps.iter().enumerate() {
                        let mut time = app.world_mut().resource_mut::<Time<Virtual>>();
                        if paused { time.pause(); } else { time.unpause(); }
                        if gamepad_input {
                            app.world_mut().write_message(RawGamepadEvent::Button(
                                RawGamepadButtonChangedEvent::new(gamepad, GamepadButton::East, if held { 1.0 } else { 0.0 })));
                        } else {
                            let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
                            if held { keys.press(KeyCode::ShiftLeft); } else { keys.release(KeyCode::ShiftLeft); }
                        }
                        for _ in 0..frames { app.update(); }
                        assert_eq!(app.world().get::<AlsRollState>(character).unwrap().active, expected,
                            "Source actual Roll activation for device={gamepad_input}, speed={speed}, case={case_index}, step={step_index}");
                        if paused {
                            assert!(!app.world().get::<AlsInputCommand>(character).unwrap().roll_pressed,
                                "Source paused Tap release has no Triggered callback");
                        }
                    }
                    app.update();
                    assert!(!app.world().get::<AlsInputCommand>(character).unwrap().roll_pressed,
                        "Source Tap callback is a single-frame pulse");
                    assert_eq!(app.world().get::<AlsRollState>(character).unwrap().active, steps.last().unwrap().3,
                        "Source actual Roll activation must survive its pulse without replaying a paused release");
                }
            }
        }
    }
    #[test]
    fn roll_direction_uses_player_view_for_input_and_actor_yaw_without_input() {
        let mut failures = Vec::new();
        for (input, expected) in [(Vec2::Y, 0.4_f32), (Vec2::ZERO, 0.7)] {
            let mut world = World::new();
            let mut time = Time::<()>::default();
            time.advance_by(std::time::Duration::from_secs_f32(1.0 / 60.0));
            world.insert_resource(time);
            let character = world.spawn((crate::als::character::AlsCharacter, AlsRollState::default(),
                LocomotionState::Grounded, Stance::Standing, AlsViewYaw(1.2), crate::als::view::AlsPlayerViewYaw(0.4),
                Transform::from_rotation(Quat::from_rotation_y(0.7)), LinearVelocity::ZERO,
                AlsInputCommand { roll_pressed: true, move_dir: input, ..default() })).id();
            world.run_system_once(advance_rolls).unwrap();
            let actual = world.get::<AlsRollState>(character).unwrap().direction_yaw;
            if crate::als::motion::wrap_yaw_radians(actual - expected).abs() > 1e-5 { failures.push((input, expected, actual)); }
        }
        assert!(failures.is_empty(), "Roll used the wrong input or actor basis: {failures:?}");
    }

    pub(crate) fn ready_roll_player(app: &mut App, avatar: Entity) -> Entity {
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

    pub(crate) fn loaded_roll_character() -> (App, Entity, Entity) {
        use crate::character_assembly::{AlsCharacterAssembly, AlsCharacterBlueprint, spawn_als_character};
        let (mut app, _, _, _) = crate::camera_component::tests::loaded_camera_scene();
        app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(1.0 / 60.0)));
        app.world_mut().spawn((avian3d::prelude::RigidBody::Static, avian3d::prelude::Collider::cuboid(100.0, 1.0, 100.0),
            Transform::from_xyz(0.0, -0.5, 0.0), avian3d::prelude::CollisionLayers::new(
                [crate::als::character::AlsCollisionLayer::Default, crate::als::character::AlsCollisionLayer::Visibility], avian3d::prelude::LayerMask::ALL)));
        let character = app.world_mut().run_system_once(|mut commands: Commands, assets: Res<AssetServer>,
            mut params: ResMut<Assets<crate::als::params::AlsMovementParams>>, mut configs: ResMut<Assets<crate::als::control_scheme::AlsControlSchemeConfig>>| {
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
        let raw_translation = output.translation_delta;
        let mesh = app.world_mut().run_system_once(move |transforms: bevy::transform::helper::TransformHelper| {
            transforms.compute_global_transform(target).unwrap()
        }).unwrap();
        let expected = mesh.affine().transform_vector3(raw_translation)
            * app.world().get::<AlsNotifyRootMotionScale>(character).unwrap().0 / app.world().resource::<Time>().delta_secs();
        assert!(app.world().get::<LinearVelocity>(character).unwrap().abs_diff_eq(expected, 1e-4),
            "Root motion did not come from the current roll player: expected={expected:?}, actual={:?}", app.world().get::<LinearVelocity>(character));
        entity
    }

    #[test]
    fn loaded_roll_pause_preserves_actor_and_slot_then_resumes() {
        let (mut app, character, _) = loaded_roll_character();
        let sampler = play_loaded_roll(&mut app, character);
        let before_transform = *app.world().get::<Transform>(character).unwrap();
        let before_roll_time = app.world().get::<AlsRollState>(character).unwrap().time;
        let before_weight = app.world().get::<AlsRollSlotRuntime>(character).unwrap().weight;
        app.world_mut().resource_mut::<Time<Virtual>>().pause();
        for _ in 0..4 {
            app.update();
            assert_eq!(*app.world().get::<Transform>(character).unwrap(), before_transform,
                "Source paused Roll must preserve its actual actor transform");
            let roll = app.world().get::<AlsRollState>(character).unwrap();
            assert!(roll.active);
            assert_eq!(roll.time, before_roll_time, "Source paused Roll must retain montage time");
            let runtime = app.world().get::<AlsRollSlotRuntime>(character).unwrap();
            assert!(runtime.active && runtime.pose_ready);
            assert_eq!(runtime.weight, before_weight);
            let player = app.world().get::<AnimationGraphPlayer>(sampler).unwrap();
            assert!(player.get_error().is_none() && player.get_default_output_pose().is_some());
        }
        app.world_mut().resource_mut::<Time<Virtual>>().unpause();
        app.update();
        assert!(app.world().get::<AlsRollState>(character).unwrap().time > before_roll_time,
            "Source resumed Roll must continue montage time");
        assert!(app.world().get::<Transform>(character).unwrap().translation.distance(before_transform.translation) > 1e-5,
            "Source resumed Roll must continue actual root motion");
    }

    #[test]
    fn root_motion_scale_lifecycle_drives_actual_roll_velocity_and_displacement() {
        let (mut app, character, avatar) = loaded_roll_character();
        let sampler = play_loaded_roll(&mut app, character);
        crate::animation_events::tests::configure_root_scale_notifications(&mut app, character,
            &[(0.1, 0.8, 0.5), (0.2, 0.5, 2.0)]);
        app.world_mut().entity_mut(character).insert((avian3d::prelude::RigidBody::Kinematic,
            bevy_tnua::TnuaToggle::Disabled, AlsRollState { active: true, ..default() }, AlsInputCommand::default()));
        app.world_mut().get_mut::<Transform>(avatar).unwrap().scale = Vec3::splat(1.7);
        let target = app.world().get::<AlsRollSlotSampler>(sampler).unwrap().target_player;
        let mut after_inner_end = false;
        let mut expected_total = Vec3::ZERO;
        let mut actual_total = Vec3::ZERO;
        for _ in 0..65 {
            let before = app.world().get::<avian3d::prelude::Position>(character).unwrap().0;
            app.world_mut().resource_mut::<Time>().advance_by(std::time::Duration::from_secs_f32(0.01));
            app.world_mut().run_schedule(PostUpdate);
            let clip_time = app.world().get::<AlsRollState>(character).unwrap().time * ROLL_PLAY_RATE;
            let expected_scale = if (0.2..0.5).contains(&clip_time) { 2.0 } else if (0.1..0.2).contains(&clip_time) { 0.5 } else { 1.0 };
            assert_eq!(app.world().get::<AlsNotifyRootMotionScale>(character).unwrap().0, expected_scale,
                "Active notify reclaimed scale at actual roll time {clip_time}");
            let raw = app.world().get::<RootMotionOutput>(sampler).unwrap().translation_delta;
            let mesh = app.world_mut().run_system_once(move |transforms: bevy::transform::helper::TransformHelper|
                transforms.compute_global_transform(target).unwrap()).unwrap();
            let expected = mesh.affine().transform_vector3(raw) * expected_scale;
            app.world_mut().run_system_once(roll_drive_system).unwrap();
            let actual = app.world().get::<LinearVelocity>(character).unwrap().0 * app.world().resource::<Time>().delta_secs();
            assert!(actual.distance(expected) < 1e-5, "Actual root motion did not consume the source scale");
            let after = app.world().get::<avian3d::prelude::Position>(character).unwrap().0;
            assert!(Vec2::new(after.x - before.x, after.z - before.z).distance(Vec2::new(expected.x, expected.z)) < 1e-4,
                "Actual roll physics did not apply the scaled root displacement");
            actual_total += after - before;
            expected_total += expected;
            after_inner_end |= clip_time > 0.5 && clip_time < 0.7 && raw.length() > 1e-6;
        }
        assert!(after_inner_end, "No real root sample was observed while the outer notify remained active");
        assert!(Vec2::new(actual_total.x, actual_total.z).distance(Vec2::new(expected_total.x, expected_total.z)) < 1e-4);
    }

    #[test]
    fn actual_roll_slot_uses_current_mesh_scale_and_basis() {
        #[derive(Component, Default)]
        struct RequestedRootStep { start: Vec3, delta: Vec3 }
        for (actor_scale, mesh_scale) in [(1.0, 0.5), (1.0, 1.0), (1.0, 2.0), (0.5, 1.0), (2.0, 1.0)] {
            for relative_yaw in [std::f32::consts::PI, 0.4, -0.7] {
                for notify_scale in [0.5, 1.0, 2.0] {
                    let (mut app, character, avatar) = loaded_roll_character();
                    if actor_scale != 1.0 {
                        let center = Vec3::Y * (0.9 * actor_scale + 0.0215);
                        app.world_mut().entity_mut(character).insert((Transform::from_translation(center).with_scale(Vec3::splat(actor_scale)),
                            avian3d::prelude::Position(center), LinearVelocity::ZERO));
                    }
                    let mut mesh = app.world_mut().get_mut::<Transform>(avatar).unwrap();
                    mesh.scale = Vec3::splat(mesh_scale); mesh.rotation = Quat::from_rotation_y(relative_yaw);
                    app.update();
                    let sampler = play_loaded_roll(&mut app, character);
                    let target = app.world().get::<AnimatedSceneInstance>(avatar).unwrap().player_entity();
                    app.world_mut().entity_mut(character).insert(RequestedRootStep::default());
                    app.add_systems(PostUpdate, (|time: Res<Time>, mut characters: Query<(&avian3d::prelude::Position, &LinearVelocity, &mut RequestedRootStep)>| {
                        for (position, velocity, mut step) in &mut characters { step.start = position.0; step.delta = velocity.0 * time.delta_secs(); }
                    }).after(roll_drive_system).before(avian3d::prelude::PhysicsSystems::First));
                    let mut expected_sum = Vec3::ZERO; let mut actual_sum = Vec3::ZERO;
                    for frame in 0..8 {
                        app.update();
                        let step = app.world().get::<RequestedRootStep>(character).unwrap();
                        let moved = app.world().get::<avian3d::prelude::Position>(character).unwrap().0 - step.start;
                        assert!(moved.with_y(0.0).distance(step.delta.with_y(0.0)) < 0.0001,
                            "Free actual roll changed displacement after collision: scale={mesh_scale} frame={frame} requested={:?} moved={moved:?}", step.delta);
                        app.world_mut().get_mut::<AlsNotifyRootMotionScale>(character).unwrap().0 = notify_scale;
                        app.world_mut().run_system_once(roll_drive_system).unwrap();
                        let raw = app.world().get::<RootMotionOutput>(sampler).unwrap().translation_delta;
                        let mesh = app.world_mut().run_system_once(move |transforms: bevy::transform::helper::TransformHelper| {
                            transforms.compute_global_transform(target).unwrap()
                        }).unwrap();
                        let expected_delta = mesh.affine().transform_vector3(raw) * notify_scale;
                        let delta = app.world().resource::<Time>().delta_secs();
                        let actual_delta = app.world().get::<LinearVelocity>(character).unwrap().0 * delta;
                        assert!(actual_delta.distance(expected_delta) < 0.0001,
                            "Actual RollSlot delta mismatch: mesh_scale={mesh_scale} yaw={relative_yaw} notify={notify_scale} frame={frame} raw={raw:?} expected={expected_delta:?} actual={actual_delta:?}");
                        expected_sum += expected_delta; actual_sum += actual_delta;
                    }
                    assert!(actual_sum.distance(expected_sum) < 0.0001);
                    assert!(expected_sum.length() > 0.001, "The real RollSlot did not advance its root track");
                }
            }
        }
    }

    #[test]
    fn stopped_roll_tail_accepts_a_new_loaded_montage() {
        for explicit_stop in [false, true] {
            for delta in [1.0 / 60.0_f32, 0.1] {
                let (mut app, character, _) = loaded_roll_character();
                let old = play_loaded_roll(&mut app, character);
                let old_roll = AlsRollState { active: true, time: if explicit_stop { 0.4 } else { 0.95 }, ..default() };
                app.world_mut().entity_mut(character).insert((old_roll, AlsNotifyLocomotionAction::Rolling,
                    AlsInputCommand { roll_pressed: true, ..default() }));
                if explicit_stop { app.world_mut().entity_mut(character).insert(AlsRollBlendOut::new(&old_roll, 0.2)); }
                app.world_mut().resource_mut::<Time>().advance_by(std::time::Duration::from_secs_f32(delta));
                app.world_mut().run_system_once(advance_rolls).unwrap();
                app.world_mut().get_mut::<AlsInputCommand>(character).unwrap().roll_pressed = false;
                let roll = app.world().get::<AlsRollState>(character).unwrap();
                assert!(roll.active && (roll.time - delta).abs() < 1e-6, "Stopped montage blocked reentry: {roll:?}");
                assert!(app.world().get::<AlsRollBlendOut>(character).is_none());
                let current = app.world_mut().query_filtered::<(Entity, &AlsRollSlotSampler), Without<AlsRollTail>>()
                    .iter(app.world()).find(|(_, sampler)| sampler.character == character).unwrap().0;
                assert_ne!(old, current, "Restart reused the old montage instance");
                let tail = app.world().get::<AlsRollTail>(old).unwrap();
                assert!((tail.time - old_roll.time - delta).abs() < 1e-6);
                if delta < 0.1 { assert!(tail.weight() > 0.0, "Restart cleared the old pose prematurely"); }
                app.world_mut().entity_mut(character).insert((avian3d::prelude::RigidBody::Kinematic, bevy_tnua::TnuaToggle::Disabled));
                app.world_mut().run_system_once(prepare_roll_slot_samples).unwrap();
                assert!((app.world().get::<AnimationGraphPlayer>(old).unwrap().get_global_input_data(ROLL_SLOT_TIME_KEY).unwrap().as_f32().unwrap()
                    - ((old_roll.time + delta) * ROLL_PLAY_RATE).min(ROLL_MONTAGE_SEQUENCE_LENGTH_SECS)).abs() < 1e-6);
                let mut tail_visible = false;
                for frame in 0..10 {
                    app.world_mut().resource_mut::<Time>().advance_by(std::time::Duration::from_secs_f32(1.0 / 60.0));
                    app.world_mut().run_schedule(PostUpdate);
                    let runtime = app.world().get::<AlsRollSlotRuntime>(character).unwrap();
                    assert!(runtime.pose_ready && runtime.active);
                    let root = app.world().get::<RootMotionOutput>(current).unwrap();
                    assert!(runtime.root_motion_delta.distance(root.translation_delta) < 1e-6, "Old root motion won frame {frame}");
                    if let Some(tail) = app.world().get::<AlsRollTail>(old) {
                        tail_visible |= tail.weight() > 0.0;
                        if tail.weight() > 0.0 {
                            let target = app.world().get::<AlsRollSlotSampler>(current).unwrap().target_player;
                            let inputs = app.world().get::<crate::extensions::SourceAnimationInputs>(target).unwrap();
                            let context = inputs.slot_input_context("roll.pose").unwrap();
                            assert!(context.asset_samples.iter().any(|sample| sample.identity == format!("roll:{}", old.to_bits())), "Old pose was omitted from the blend");
                            assert!(context.asset_samples.iter().any(|sample| sample.identity == format!("roll:{}", current.to_bits())));
                            let current_pose = app.world().get::<AnimationGraphPlayer>(current).unwrap().get_default_output_pose().unwrap();
                            let old_pose = app.world().get::<AnimationGraphPlayer>(old).unwrap().get_default_output_pose().unwrap();
                            let mixed = app.world().get::<AnimationGraphPlayer>(target).unwrap().get_global_input_data("roll.pose").unwrap().as_pose().unwrap();
                            let current_weight = roll_weight_at(app.world().get::<AlsRollState>(character).unwrap().time);
                            let total = current_weight + tail.weight();
                            for (bone, index) in &current_pose.paths {
                                let Some(old_index) = old_pose.paths.get(bone) else { continue; };
                                let Some(mixed_index) = mixed.paths.get(bone) else { panic!("Blended bone is missing"); };
                                let new_bone = &current_pose.bones[*index];
                                let old_bone = &old_pose.bones[*old_index];
                                let actual = &mixed.bones[*mixed_index];
                                if let (Some(new), Some(old), Some(actual)) = (new_bone.rotation, old_bone.rotation, actual.rotation) {
                                    let old = if new.dot(old) < 0.0 { -old } else { old };
                                    let expected = (new * (current_weight / total) + old * (tail.weight() / total)).normalize();
                                    assert!(actual.dot(expected).abs() > 1.0 - 1e-6, "Montage rotation did not use the source weighted accumulation");
                                }
                                if let (Some(new), Some(old), Some(actual)) = (new_bone.translation, old_bone.translation, actual.translation) {
                                    let expected = (new * current_weight + old * tail.weight()) / total;
                                    assert!(actual.distance(expected) < 1e-5, "Old montage translation was not blended");
                                }
                            }
                        }
                    }
                    if frame >= 1 {
                        assert_eq!(*app.world().get::<AlsNotifyLocomotionAction>(character).unwrap(), AlsNotifyLocomotionAction::Rolling,
                            "Old notify cleanup cleared the new roll");
                    }
                }
                if delta < 0.1 { assert!(tail_visible, "Old tail never contributed an evaluated pose"); }
                assert!(app.world().get_entity(old).is_err(), "Finished tail was not released");
                assert!(app.world().get_entity(current).is_ok());
                assert!(app.world().get::<AlsRollState>(character).unwrap().root_motion_active());
                assert!(app.world().get::<AlsRollState>(character).unwrap().time < 0.3);
            }
        }
    }

    #[test]
    fn an_old_tail_cannot_supply_root_motion_in_any_query_order() {
        for old_first in [true, false] {
        let mut world = World::new();
        let mut time = Time::<()>::default();
        time.advance_by(std::time::Duration::from_secs_f32(0.05));
        world.insert_resource(time);
        let roll = AlsRollState { active: true, time: 0.1, ..default() };
        let character = world.spawn((crate::als::character::AlsCharacter, roll, Transform::default(), LinearVelocity::ZERO)).id();
        let mesh = world.spawn((ChildOf(character), Transform::from_rotation(Quat::from_rotation_y(std::f32::consts::PI)))).id();
        for old in [old_first, !old_first] {
            let entity = world.spawn((AlsRollSlotSampler { character, target_player: mesh, reported_unready: false },
                RootMotionOutput { translation_delta: Vec3::NEG_Z * if old { 10.0 } else { 0.2 }, ..default() })).id();
            if old { world.entity_mut(entity).insert(AlsRollTail { time: 1.0, blend: AlsRollBlendOut::new(&roll, 0.1) }); }
        }
        world.run_system_once(roll_drive_system).unwrap();
        assert!(world.get::<LinearVelocity>(character).unwrap().distance(Vec3::Z * 4.0) < 1e-5,
            "Old tail root motion displaced the new montage");
        }
    }

    #[test]
    fn roll_reentry_obeys_playing_ground_and_action_gates() {
        for delta in [1.0 / 60.0_f32, 0.1] {
            for elapsed in [0.4, 0.83, 0.95, crate::als::roll::ROLL_DURATION_SECS - 0.001] {
                for explicit_stop in [false, true] {
                    for (loco, action, allowed) in [(LocomotionState::Grounded, AlsNotifyLocomotionAction::Rolling, true),
                        (LocomotionState::Grounded, AlsNotifyLocomotionAction::None, true),
                        (LocomotionState::Air, AlsNotifyLocomotionAction::Rolling, false),
                        (LocomotionState::Grounded, AlsNotifyLocomotionAction::Mantling, false),
                        (LocomotionState::Grounded, AlsNotifyLocomotionAction::Ragdolling, false)] {
                        let mut world = World::new();
                        let mut time = Time::<()>::default();
                        time.advance_by(std::time::Duration::from_secs_f32(delta));
                        world.insert_resource(time);
                        let roll = AlsRollState { active: true, time: elapsed, ..default() };
                        let character = world.spawn((crate::als::character::AlsCharacter, roll, loco, action, Stance::Standing,
                            AlsViewYaw::default(), Transform::default(), LinearVelocity::ZERO,
                            AlsInputCommand { roll_pressed: true, ..default() })).id();
                        if explicit_stop { world.entity_mut(character).insert(AlsRollBlendOut::new(&roll, 0.2)); }
                        world.run_system_once(advance_rolls).unwrap();
                        let started = world.get::<AlsRollState>(character).unwrap().time < delta + 1e-5;
                        assert_eq!(started, allowed && (action == AlsNotifyLocomotionAction::None || explicit_stop || !roll.root_motion_active()),
                            "Wrong start qualification: elapsed={elapsed}, delta={delta}, stop={explicit_stop}, loco={loco:?}, action={action:?}");
                    }
                }
            }
        }
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
            crate::als::character::AlsCharacter,
            AlsRollState { active: true, time: 0.83, ..default() },
            LocomotionState::Grounded, Stance::Standing, AlsViewYaw::default(),
            Transform::default(), LinearVelocity(Vec3::X), AlsInputCommand::default(),
        )).id();
        let mesh = world.spawn((ChildOf(character), Transform::from_rotation(Quat::from_rotation_y(std::f32::consts::PI)))).id();
        world.spawn((
            AlsRollSlotSampler { character, target_player: mesh, reported_unready: false },
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
        let character = world.spawn((crate::als::character::AlsCharacter,
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
                crate::als::character::AlsCharacter,
                crate::als::motion::AlsMovementInputBlocked,
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
            .remove::<crate::als::motion::AlsMovementInputBlocked>();
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
                crate::als::character::AlsCharacter,
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
            * (1.0 - crate::extensions::transition_alpha(alpha, crate::extensions::AlphaBlendMode::Cubic, &[]))
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
    commands.insert_resource(RollSlotCurveHandle(assets.load(ROLL_SLOT_CURVES), assets.load("ALS/Animations/Actions/Roll/A_Als_Roll.anim.ron")));
}

fn ensure_roll_state(
    mut commands: Commands,
    q: Query<
        Entity,
        (
            With<crate::als::character::AlsCharacter>,
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
    characters: Query<(Entity, &AlsCharacterAssembly), With<crate::als::character::AlsCharacter>>,
    avatars: Query<(&AlsCharacterAvatar, &AnimatedSceneInstance)>,
    samplers: Query<(Entity, &AlsRollSlotSampler, Option<&AlsRollTail>)>,
    players: Query<&AnimationGraphPlayer>,
    mut states: Query<(&mut AlsRollState, Option<&mut AlsRollSlotRuntime>, Option<&mut AlsNotifyLocomotionAction>)>,
    mut commands: Commands,
) {
    let current_target = |character| {
        let (_, assembly) = characters.get(character).ok()?;
        let (avatar, instance) = avatars.get(assembly.avatar).ok()?;
        (avatar.character == character && players.contains(instance.player_entity())).then_some(instance.player_entity())
    };
    for (entity, sampler, tail) in &samplers {
        if current_target(sampler.character) == Some(sampler.target_player) { continue; }
        if tail.is_none() && let Ok((mut roll, runtime, action)) = states.get_mut(sampler.character) {
            end_roll_instance(&mut roll, runtime, action);
            commands.entity(sampler.character).try_remove::<AlsRollBlendOut>();
        }
        commands.entity(entity).try_despawn();
    }
    for (character, _) in &characters {
        let Some(target_player) = current_target(character) else { continue; };
        if samplers
            .iter()
            .any(|(_, sampler, tail)| tail.is_none() && sampler.character == character && sampler.target_player == target_player)
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
    samplers: Query<(Entity, &AlsRollSlotSampler, Option<&AlsRollTail>)>,
    mut states: Query<(&mut AlsRollState, Option<&mut AlsRollSlotRuntime>, Option<&mut AlsNotifyLocomotionAction>)>,
    mut commands: Commands,
) {
    for (entity, sampler, tail) in &samplers {
        if sampler.target_player != event.entity { continue; }
        if tail.is_none() && let Ok((mut roll, runtime, action)) = states.get_mut(sampler.character) {
            end_roll_instance(&mut roll, runtime, action);
            commands.entity(sampler.character).try_remove::<AlsRollBlendOut>();
        }
        commands.entity(entity).try_despawn();
    }
}

fn prepare_roll_slot_samples(
    rolls: Query<&AlsRollState, With<crate::als::character::AlsCharacter>>,
    mut samplers: Query<(&AlsRollSlotSampler, &mut AnimationGraphPlayer, Option<&AlsRollTail>)>,
) {
    for (sampler, mut player, tail) in &mut samplers {
        let clip_time = tail.map(|tail| (tail.time * ROLL_PLAY_RATE).min(ROLL_MONTAGE_SEQUENCE_LENGTH_SECS)).unwrap_or_else(|| rolls
            .get(sampler.character)
            .ok()
            .filter(|roll| roll.active)
            .map(|roll| (roll.time * ROLL_PLAY_RATE).min(ROLL_MONTAGE_SEQUENCE_LENGTH_SECS))
            .unwrap_or(0.0));
        player.set_global_input_data(ROLL_SLOT_TIME_KEY, clip_time);
    }
}

fn bridge_roll_slot_samples(
    mut source_inputs: Query<&mut crate::extensions::SourceAnimationInputs>,
    rolls: Query<&AlsRollState, With<crate::als::character::AlsCharacter>>,
    get_ups: Query<&crate::get_up::AlsGetUpState>,
    blend_outs: Query<&AlsRollBlendOut>,
    curve_handle: Res<RollSlotCurveHandle>,
    curves: Res<Assets<AlsAnimCurves>>,
    mut samplers: Query<(Entity, &mut AlsRollSlotSampler, &AnimationGraphPlayer, &RootMotionOutput, Option<&AlsRollTail>)>,
    mut target_players: Query<&mut AnimationGraphPlayer, Without<AlsRollSlotSampler>>,
    mut commands: Commands,
) {
    for (character, target) in samplers.iter().filter(|(_, _, _, _, tail)| tail.is_none())
        .map(|(_, sampler, _, _, _)| (sampler.character, sampler.target_player)).collect::<Vec<_>>() {
        let Ok(mut target_player) = target_players.get_mut(target) else { continue; };
        let roll = rolls.get(character).ok().filter(|roll| roll.active);
        let mut samples = Vec::new();
        let mut root_motion = RootMotionOutput::default();
        for (entity, mut sampler, player, output, tail) in &mut samplers {
            if sampler.character != character || sampler.target_player != target { continue; }
            let (elapsed, weight) = if let Some(tail) = tail { (tail.time, tail.weight()) }
                else if let Some(roll) = roll {
                    root_motion = RootMotionOutput { translation_delta: output.translation_delta, rotation_delta: output.rotation_delta };
                    (roll.time, blend_outs.get(character).map_or_else(|_| roll_weight_at(roll.time), |blend| blend.weight()))
                } else { continue; };
            let Some(mut pose) = player.get_default_output_pose().cloned() else {
                if !sampler.reported_unready {
                    warn!("[ROLL-SLOT] sampler unavailable character={:?} error={:?}", character, player.get_error());
                    sampler.reported_unready = true;
                }
                continue;
            };
            if tail.is_some() { pose.root_motion = None; }
            sampler.reported_unready = false;
            let clip_time = (elapsed * ROLL_PLAY_RATE).min(ROLL_MONTAGE_SEQUENCE_LENGTH_SECS);
            let mut context = curves.get(&curve_handle.0).and_then(AlsAnimCurves::first)
                .map(|curves| crate::get_up::sampled_curve_context(curves, clip_time)).unwrap_or_default();
            context.asset_samples.push(crate::extensions::animation_context::SourceAssetSample {
                identity: format!("roll:{}", entity.to_bits()), clip: curve_handle.1.clone(), montage: true,
                clock: crate::extensions::SourceRelevantAsset { duration: ROLL_MONTAGE_SEQUENCE_LENGTH_SECS,
                    position: clip_time, weight: 1.0, ..default() }, ..default()
            });
            samples.push((pose, context, weight, tail.is_none()));
        }
        samples.sort_by_key(|(_, _, _, current)| !*current);
        let total_weight = samples.iter().map(|(_, _, weight, _)| weight).sum::<f32>();
        let primary_root = samples.first().and_then(|(pose, _, _, _)| pose.root_motion.clone());
        let mut pose: Option<bevy_animation_graph::core::pose::Pose> = None;
        for (sample, _, weight, _) in &samples {
            if *weight <= f32::EPSILON { continue; }
            let sample = sample.scalar_mult(weight / total_weight.max(f32::EPSILON));
            pose = Some(pose.map_or_else(|| sample.clone(), |previous| previous.linear_add(&sample)));
        }
        let pose = pose.map(|pose| {
            let mut pose = pose.normalize_quat();
            pose.root_motion = primary_root;
            pose
        });
        let context = pose.as_ref().map(|_| crate::extensions::source_pose_nodes::blend_animation_contexts(
            samples.iter().map(|(_, context, weight, _)| (context.clone(), weight / total_weight.max(f32::EPSILON)))));
        let weight = total_weight.clamp(0.0, 1.0);
        let pose_ready = pose.is_some();
        if let Some(pose) = &pose { target_player.set_global_input_data("roll.pose", DataValue::Pose(pose.clone())); }
        target_player.set_global_input_data("roll.weight", weight);
        source_inputs.get_mut(target).unwrap().set_slot_input_context("roll.pose", context.clone());
        let curve_values = ROLL_SLOT_CURVE_NAMES.map(|name| context.as_ref().and_then(|context| context.curves.get(&crate::extensions::SourceCurveName::from(name))).copied().unwrap_or_default());
        for (name, value) in ROLL_SLOT_CURVE_NAMES.into_iter().zip(curve_values) {
            target_player.set_global_input_data(format!("{ROOT_POST_LOCOMOTION_SLOT_CURVE_PREFIX}{name}"), value);
        }
        if !get_ups.contains(character) {
            source_inputs.get_mut(target).unwrap().set_slot_input_context(GENERATED_POST_LOCOMOTION_SLOT_POSE_KEY, context);
            if let Some(pose) = pose {
                target_player.set_global_input_data(ROOT_POST_LOCOMOTION_SLOT_POSE_KEY, DataValue::Pose(pose.clone()));
                target_player.set_global_input_data(GENERATED_POST_LOCOMOTION_SLOT_POSE_KEY, DataValue::Pose(pose));
            }
            for key in [ROOT_POST_LOCOMOTION_SLOT_WEIGHT_KEY, ROOT_POST_LOCOMOTION_SLOT_LAYER_WEIGHT_KEY, GENERATED_POST_LOCOMOTION_SLOT_WEIGHT_KEY] {
                target_player.set_global_input_data(key, weight);
            }
            target_player.set_global_input_data(GENERATED_POST_LOCOMOTION_SLOT_LAYER_WEIGHT_KEY, if pose_ready { 1.0_f32 } else { 0.0 });
        }
        commands.entity(character).insert(AlsRollSlotRuntime {
            active: roll.is_some() || samples.iter().any(|(_, _, _, current)| !current), weight, pose_ready,
            root_motion_delta: root_motion.translation_delta, root_motion_rotation: root_motion.rotation_delta,
            pose_standing: curve_values[0], pose_grounded: curve_values[1],
        });
    }
}

#[allow(clippy::type_complexity)]
fn advance_rolls(
    time: Res<Time>,
    mut commands: Commands,
    blocked: Query<(), With<crate::als::motion::AlsMovementInputBlocked>>,
    actions: Query<&crate::animation_events::AlsNotifyLocomotionAction>,
    mut blend_outs: Query<&mut AlsRollBlendOut>,
    mut tails: Query<(Entity, &mut AlsRollTail)>,
    samplers: Query<(Entity, &AlsRollSlotSampler, &AnimationGraphPlayer), Without<AlsRollTail>>,
    mut q: Query<
        (
            Entity,
            &mut AlsRollState,
            &LocomotionState,
            &Stance,
            (&AlsViewYaw, Option<&crate::als::view::AlsPlayerViewYaw>),
            &Transform,
            &LinearVelocity,
            Option<&AlsAirState>,
            &AlsInputCommand,
            Option<&AlsInputOverride>,
        ),
        With<crate::als::character::AlsCharacter>,
    >,
) {
    let dt = time.delta_secs();
    for (entity, mut tail) in &mut tails {
        tail.time += dt;
        tail.blend.elapsed += dt;
        if tail.weight() <= f32::EPSILON { commands.entity(entity).try_despawn(); }
    }
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
        let effective = crate::als::motion::movement_input_for_action(
            *ovr.map(|o| &o.0).unwrap_or(cmd),
            blocked.contains(character) || action_blocked,
        );

        let previous_roll = *roll;
        let landing_ready = *loco == LocomotionState::Grounded && !blocked.contains(character) && !action_blocked
            && air.is_some_and(|air| air.landing_time <= dt.max(0.05) && air.last_impact_vy >= crate::als::roll::ROLL_ON_LAND_SPEED_THRESHOLD);
        let empty_action = actions.get(character).ok().is_none_or(|action| *action == AlsNotifyLocomotionAction::None);
        if empty_action && *loco == LocomotionState::Grounded && effective.roll_pressed { roll.active = false; }
        let mut stopped_blend = None;
        if let Ok(mut blend) = blend_outs.get_mut(character) {
            stopped_blend = Some(*blend);
            roll.time += dt;
            blend.elapsed += dt;
            if blend.weight() <= 0.0 {
                roll.active = false;
                commands.entity(character).remove::<AlsRollBlendOut>();
            }
            if !(*loco == LocomotionState::Grounded && effective.roll_pressed) && !landing_ready { continue; }
            roll.active = false;
        }

        let direction_basis = AlsViewYaw(if effective.move_dir.length_squared() > 1e-6 {
            player_view.map_or(view.0, |view| view.0)
        } else { transform.rotation.to_euler(EulerRot::YXZ).0 });
        let mut lifecycle = roll.update(&effective, &direction_basis, loco, stance, dt);
        if !lifecycle.started
            && (empty_action || stopped_blend.is_some() || !previous_roll.root_motion_active())
            && landing_ready
        {
            let (actor_yaw, _, _) = transform.rotation.to_euler(EulerRot::YXZ);
            lifecycle = roll.start_from_landing(air.unwrap().last_impact_vy, vel.0, actor_yaw, *stance);
        }
        if lifecycle.started {
            if previous_roll.active {
                let mut blend = stopped_blend.unwrap_or_else(|| AlsRollBlendOut::new(&previous_roll,
                    (crate::als::roll::ROLL_DURATION_SECS - previous_roll.time).max(0.0)));
                blend.stop(crate::als::roll::ROLL_WEIGHT_IN_SECS);
                blend.elapsed += dt;
                for (entity, sampler, player) in &samplers {
                    if sampler.character != character { continue; }
                    commands.entity(entity).insert(AlsRollTail { time: previous_roll.time + dt, blend });
                    let bevy_animation_graph::core::animation_graph_player::AnimationSource::Graph(graph) = player.get_animation_source() else { continue; };
                    commands.spawn((Name::new("als_roll_slot_sampler"), Children::default(), ChildOf(character),
                        AnimationGraphPlayer::new(player.skeleton().clone()).with_graph(graph.clone()),
                        RootMotionOutput::default(), *sampler));
                }
                commands.entity(character).remove::<AlsRollBlendOut>();
            }
            roll.time = dt;
        }
    }
}

fn roll_drive_system(
    time: Res<Time>,
    blend_outs: Query<(), With<AlsRollBlendOut>>,
    mut space: ParamSet<(bevy::transform::helper::TransformHelper,
        Query<(Entity, &AlsRollState, Option<&AlsNotifyLocomotionAction>, &mut Transform, Option<&mut Rotation>,
            &mut LinearVelocity, Option<&AlsNotifyRootMotionScale>), With<crate::als::character::AlsCharacter>>)> ,
    sampler_root_motion: Query<(&AlsRollSlotSampler, &RootMotionOutput), Without<AlsRollTail>>,
    mut players: Query<&mut AnimationGraphPlayer>,
) {
    let dt = time.delta_secs();
    let characters: Vec<_> = space.p1().iter().map(|(entity, ..)| entity).collect();
    for character in characters {
        let sample = sampler_root_motion.iter().find(|(sampler, _)| sampler.character == character);
        let Ok(actor_world) = space.p0().compute_global_transform(character) else { continue; };
        let mesh_world = if let Some((sampler, _)) = sample {
            let Ok(mesh) = space.p0().compute_global_transform(sampler.target_player) else { continue; };
            mesh
        } else { actor_world };
        let mut query = space.p1();
        let (_, roll, action, mut transform, rotation, mut vel, notify_root_scale) = query.get_mut(character).unwrap();
        if !roll.active || action == Some(&AlsNotifyLocomotionAction::Ragdolling) || dt <= 1e-5 || blend_outs.contains(character)
            || roll.time - dt >= crate::als::roll::ROLL_DURATION_SECS - crate::als::roll::ROLL_WEIGHT_OUT_SECS {
            continue;
        }

        let (mut local_translation, local_rotation) = sample.map_or((Vec3::ZERO, Quat::IDENTITY),
            |(_, output)| (output.translation_delta, output.rotation_delta));
        local_translation *= notify_root_scale.map_or(1.0, |scale| scale.0).max(0.0);
        let (world_translation, root_rotation) =
            root_motion_world_transform(actor_world, mesh_world, local_translation, local_rotation);
        let next_rotation = if local_rotation.dot(Quat::IDENTITY).abs() < 1.0 - 1e-6 {
            (actor_world.rotation() * transform.rotation.conjugate()).conjugate() * root_rotation
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
