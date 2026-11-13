use crate::als::air::AlsAirState;
use crate::als::input::AlsInputCommand;
use crate::als::motion::{AlsInputOverride, AlsViewYaw};
use crate::als::roll::{
    AlsRollState, ROLL_MONTAGE_SEQUENCE_LENGTH_SECS, ROLL_ROTATION_HALF_LIFE_SECS, advance_roll_clip,
    damper_exact_angle, select_roll_direction,
};
#[cfg(test)]
use crate::als::roll::{ROLL_PLAY_RATE, ROLL_WEIGHT_IN_SECS};
use crate::extensions::profile_blend::{AlphaBlendMode, MontageBlend};
use crate::als::state::{LocomotionState, Stance};
use crate::extensions::AlsAnimCurves;
use avian3d::prelude::{LinearVelocity, Rotation};
use bevy::ecs::hierarchy::ChildOf;
use bevy::prelude::*;
use bevy_animation_graph::core::animated_scene::AnimatedSceneInstance;
use bevy_animation_graph::core::animation_clip::GraphClip;
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
    pub(crate) clip_time: f32,
    blend: AlsRollBlendOut,
}

impl AlsRollTail {
    fn from_roll(roll: &AlsRollState, blend: AlsRollBlendOut) -> Self {
        Self { clip_time: roll.clip_time(), blend }
    }

    fn advance(&mut self, delta: f32) {
        self.blend.advance(delta);
        advance_roll_clip(&mut self.clip_time, delta, |_| {});
    }

    pub(crate) fn weight(&self) -> f32 { self.blend.weight() }
}

#[cfg(test)]
pub(crate) mod tests {
    #[test]
    fn roll_input_commits_action_and_stance_only_when_started() {
        use bevy::ecs::system::RunSystemOnce;
        for (locomotion, pressed, blocked, expected) in [(LocomotionState::Grounded, true, false, true),
            (LocomotionState::Grounded, false, false, false), (LocomotionState::Grounded, true, true, false),
            (LocomotionState::Air, true, false, false)] {
            let mut world = World::new();
            let actor = world.spawn((crate::als::character::AlsCharacter, AlsRollState::default(),
                locomotion, Stance::Standing, AlsViewYaw::default(), Transform::IDENTITY,
                AlsInputCommand { roll_pressed: pressed, ..default() }, AlsNotifyLocomotionAction::None)).id();
            if blocked { world.entity_mut(actor).insert(crate::als::motion::AlsMovementInputBlocked); }
            world.run_system_once(start_roll_input).unwrap();
            assert_eq!(world.get::<AlsRollState>(actor).unwrap().active, expected);
            assert_eq!(world.get::<AlsNotifyLocomotionAction>(actor).unwrap(),
                &if expected { AlsNotifyLocomotionAction::Rolling } else { AlsNotifyLocomotionAction::None });
            assert_eq!(world.get::<crate::als::transition::AlsPhysicalStanceOverride>(actor).map(|value| value.0),
                expected.then_some(Stance::Crouching));
            assert_eq!(world.get::<AlsRollInputHandled>(actor).is_some(), pressed);
        }
    }

    #[test]
    fn roll_acceptance_uses_the_mode_when_input_arrives() {
        use bevy::ecs::system::RunSystemOnce;
        for (input_mode, movement_mode, expected) in [(LocomotionState::Air, LocomotionState::Grounded, false),
            (LocomotionState::Grounded, LocomotionState::Air, true),
            (LocomotionState::Grounded, LocomotionState::Grounded, true),
            (LocomotionState::Air, LocomotionState::Air, false)] {
            let mut world = World::new();
            world.insert_resource(Time::<()>::default());
            world.init_resource::<Messages<crate::als::transition::StateTransitionEvent>>();
            let actor = world.spawn((crate::als::character::AlsCharacter, AlsRollState::default(),
                input_mode, Stance::Standing, AlsViewYaw::default(), Transform::IDENTITY,
                AlsInputCommand { roll_pressed: true, ..default() }, AlsNotifyLocomotionAction::None)).id();
            world.run_system_once(start_roll_input).unwrap();
            *world.get_mut::<LocomotionState>(actor).unwrap() = movement_mode;
            world.run_system_once(advance_rolls).unwrap();
            assert_eq!(world.get::<AlsRollState>(actor).unwrap().active, expected,
                "input={input_mode:?} after_movement={movement_mode:?}");
        }
    }

    #[test]
    fn loaded_roll_start_commits_action_and_crouch_before_movement() {
        for (stance, dilation, pressed) in [(Stance::Standing, 1.0, true), (Stance::Standing, 0.0, true),
            (Stance::Crouching, 1.0, true), (Stance::Crouching, 0.0, true),
            (Stance::Standing, 1.0, false), (Stance::Crouching, 1.0, false)] {
            let (mut app, actor, _) = loaded_roll_character();
            if stance == Stance::Crouching {
                app.world_mut().get_mut::<crate::als::transition::AlsStateRequest>(actor).unwrap().stance = Some(stance);
                for _ in 0..8 { app.update(); }
                assert_eq!(*app.world().get::<Stance>(actor).unwrap(), stance);
            }
            app.world_mut().entity_mut(actor).insert(crate::als_camera::AlsCameraCustomTimeDilation(dilation));
            app.world_mut().get_mut::<AlsInputCommand>(actor).unwrap().roll_pressed = pressed;
            app.update();
            assert_eq!(app.world().get::<AlsRollState>(actor).unwrap().active, pressed);
            assert_eq!(app.world().get::<AlsNotifyLocomotionAction>(actor).copied(), Some(if pressed {
                AlsNotifyLocomotionAction::Rolling
            } else { AlsNotifyLocomotionAction::None }));
            let expected = if pressed { Stance::Crouching } else { stance };
            assert_eq!(*app.world().get::<Stance>(actor).unwrap(), expected);
            let half = app.world().get::<avian3d::prelude::Collider>(actor).unwrap().shape_scaled().compute_local_aabb().half_extents().y;
            let expected_half = if expected == Stance::Crouching { 0.56 } else { 0.9 };
            assert!((half - expected_half).abs() < 1e-5, "stance={stance:?} dilation={dilation} pressed={pressed} half={half}");
        }
    }

    #[test]
    fn loaded_roll_input_uses_pre_movement_grounding() {
        use bevy::time::TimeUpdateStrategy;
        for (landing, pressed, delta, expected_roll) in [(true, true, 0.05, false),
            (true, false, 0.05, false), (false, true, 0.1, true), (false, true, 0.05, true)] {
            let (mut app, actor, _) = loaded_roll_character();
            let floor = app.world_mut().spawn((avian3d::prelude::RigidBody::Static,
                avian3d::prelude::Collider::cuboid(if landing { 4.0 } else { 1.0 }, 0.2, 4.0),
                Transform::from_xyz(0.0, 1.9, 0.0))).id();
            app.update();
            app.update();
            let start = if landing { Vec3::new(0.0, 2.925, 0.0) } else { Vec3::new(0.55, 2.9215, 0.0) };
            let velocity = if landing { Vec3::NEG_Y * 2.0 } else { Vec3::X * 3.75 };
            app.world_mut().entity_mut(actor).insert((avian3d::prelude::Position(start), Transform::from_translation(start),
                LinearVelocity(velocity), if landing { LocomotionState::Air } else { LocomotionState::Grounded },
                AlsRollState::default(), AlsNotifyLocomotionAction::None, AlsAirState::default()));
            {
                let mut base = app.world_mut().get_mut::<crate::als::dynamics::AlsMovementBase>(actor).unwrap();
                *base = default();
                if !landing { base.entity = Some(floor); base.location = Vec3::new(0.0, 1.9, 0.0); }
            }
            {
                let mut base = app.world_mut().get_mut::<crate::als::dynamics::AlsMovementBaseDynamicsState>(actor).unwrap();
                *base = default();
                if !landing { base.entity = Some(floor); base.relative_velocity = velocity; }
            }
            let input = &mut *app.world_mut().get_mut::<AlsInputCommand>(actor).unwrap();
            input.roll_pressed = pressed;
            input.move_dir = if landing { Vec2::ZERO } else { Vec2::X };
            *app.world_mut().resource_mut::<TimeUpdateStrategy>() =
                TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(delta));
            app.update();
            let mode = *app.world().get::<LocomotionState>(actor).unwrap();
            if landing { assert_eq!(mode, LocomotionState::Grounded); }
            if !landing && delta > 0.05 { assert_eq!(mode, LocomotionState::Air); }
            assert_eq!(app.world().get::<AlsRollState>(actor).unwrap().active, expected_roll,
                "landing={landing} pressed={pressed} delta={delta} mode={mode:?}");
        }
    }

    #[test]
    fn roll_small_input_keeps_consumed_direction_through_the_full_frame() {
        for amount in [0.01_f32, 0.0002] {
            let (mut app, actor, _) = loaded_roll_character();
            app.world_mut().get_mut::<crate::als::input::AlsInputCommand>(actor).unwrap().move_dir = Vec2::X * amount;
            for _ in 0..3 { app.update(); }
            let motion = *app.world().get::<crate::actual_motion::AlsActualMotion>(actor).unwrap();
            assert!(motion.has_input, "The source input must have been consumed before rolling");
            app.world_mut().get_mut::<crate::als::input::AlsInputCommand>(actor).unwrap().roll_pressed = true;
            app.update();
            let roll = app.world().get::<crate::als::roll::AlsRollState>(actor).unwrap();
            assert!(roll.active);
            assert!(crate::als::roll::wrap_angle(roll.direction_yaw - motion.input_yaw_degrees.to_radians()).abs() < 1e-4,
                "Source consumed small-input yaw was lost during the real frame: amount={amount} yaw={} consumed={}", roll.direction_yaw, motion.input_yaw_degrees);
        }
    }

    #[test]
    fn roll_direction_uses_consumed_input_instead_of_raw_command_or_view() {
        use bevy::ecs::system::RunSystemOnce;
        let (mut app, actor, _) = loaded_roll_character();
        let mut failures = Vec::new();
        for (raw, has_input, yaw) in [(0.01_f32, true, 0.9 - std::f32::consts::FRAC_PI_2),
            (0.0002, true, 0.9 - std::f32::consts::FRAC_PI_2), (1.0, false, -1.3), (0.0, true, 0.7)] {
            let command = crate::als::input::AlsInputCommand { roll_pressed: true, move_dir: Vec2::X * raw, ..default() };
            app.world_mut().entity_mut(actor).insert((crate::als::roll::AlsRollState::default(),
                crate::als::state::LocomotionState::Grounded, AlsNotifyLocomotionAction::None,
                crate::als::motion::AlsViewYaw(0.3), crate::als::view::AlsPlayerViewYaw(0.9), command,
                crate::als::motion::AlsInputOverride(command),
                crate::actual_motion::AlsActualMotion { has_input, input_yaw_degrees: yaw.to_degrees(), ..default() }));
            app.world_mut().entity_mut(actor).remove::<crate::als::motion::AlsMovementInputBlocked>();
            app.world_mut().run_system_once(super::advance_rolls).unwrap();
            let state = app.world().get::<crate::als::roll::AlsRollState>(actor).unwrap();
            assert!(state.active, "The source roll request must be admitted");
            let expected = if has_input { yaw } else { 0.0 };
            if crate::als::roll::wrap_angle(state.direction_yaw - expected).abs() > 1e-5 {
                failures.push((raw, has_input, yaw, state.direction_yaw, expected));
            }
        }
        assert!(failures.is_empty(), "Source rolling consumes bHasInput and InputYawAngle: {failures:?}");
    }

    use super::*;
    use bevy::ecs::system::RunSystemOnce;

    #[test]
    fn landing_roll_direction_uses_consumed_velocity_validity_and_yaw() {
        for (speed, yaw, expected) in [(0.02_f32, 0.8_f32, 0.8_f32), (0.01, -0.9, -0.9),
            (0.0099, -0.9, 0.7), (0.0, 1.1, 0.7)] {
            let mut world = World::new();
            world.init_resource::<Messages<crate::als::transition::StateTransitionEvent>>();
            let mut time = Time::<()>::default();
            time.advance_by(std::time::Duration::from_secs_f32(0.02));
            world.insert_resource(time);
            let actor = world.spawn((crate::als::character::AlsCharacter, AlsRollState::default(),
                LocomotionState::Grounded, Stance::Standing, AlsViewYaw(0.3),
                Transform::from_rotation(Quat::from_rotation_y(0.7)), LinearVelocity(Vec3::X * 4.0),
                AlsInputCommand::default(), crate::als::air::AlsPreImpactVelocity { last_air_vertical_velocity: -8.0 },
                crate::actual_motion::AlsActualMotion { world_velocity: Vec3::NEG_Y * 8.0, horizontal_speed: speed,
                    velocity_yaw_degrees: yaw.to_degrees(), ..default() })).id();
            world.resource_mut::<Messages<crate::als::transition::StateTransitionEvent>>().write(
                crate::als::transition::StateTransitionEvent { entity: actor,
                    dimension: crate::als::transition::Dimension::LocomotionState,
                    from: crate::als::transition::StateValue::LocomotionState(LocomotionState::Air),
                    to: crate::als::transition::StateValue::LocomotionState(LocomotionState::Grounded) });
            world.run_system_once(advance_rolls).unwrap();
            let roll = world.get::<AlsRollState>(actor).unwrap();
            assert!(roll.active);
            assert!(crate::als::roll::wrap_angle(roll.direction_yaw - expected).abs() < 1e-5,
                "Landing Roll used current Walking velocity instead of the source consumed yaw: {roll:?}");
        }
    }

    #[test]
    fn landing_event_starts_roll_even_when_the_frame_ends_airborne() {
        use crate::als::transition::{Dimension, StateTransitionEvent, StateValue};
        for landed in [false, true] {
            for speed in [6.999, 7.0, 8.0] {
                let mut world = World::new();
                world.init_resource::<Messages<StateTransitionEvent>>();
                let mut time = Time::<()>::default();
                time.advance_by(std::time::Duration::from_secs_f32(0.02));
                world.insert_resource(time);
                let actor = world.spawn((crate::als::character::AlsCharacter, AlsRollState::default(),
                    LocomotionState::Air, Stance::Standing, AlsViewYaw::default(), Transform::IDENTITY,
                    LinearVelocity(Vec3::NEG_Z * 3.75), AlsAirState::default(), AlsInputCommand::default(),
                    crate::als::air::AlsPreImpactVelocity { last_air_vertical_velocity: -speed },
                    crate::actual_motion::AlsActualMotion { world_velocity: Vec3::NEG_Y * speed, ..default() })).id();
                if landed {
                    world.resource_mut::<Messages<StateTransitionEvent>>().write(StateTransitionEvent {
                        entity: actor, dimension: Dimension::LocomotionState,
                        from: StateValue::LocomotionState(LocomotionState::Air), to: StateValue::LocomotionState(LocomotionState::Grounded),
                    });
                }
                world.run_system_once(advance_rolls).unwrap();
                assert_eq!(world.get::<AlsRollState>(actor).unwrap().active, landed && speed >= 7.0,
                    "Source automatic Roll follows the landing callback and retained impact speed");
            }
        }
    }

    #[test]
    fn landing_roll_uses_the_retained_character_velocity() {
        use crate::als::transition::{Dimension, StateTransitionEvent, StateValue};
        for (retained, current, expected) in [(3.0_f32, 8.0_f32, false), (8.0, 3.0, true),
            (3.0, 3.0, false), (8.0, 8.0, true), (7.0, 3.0, true)] {
            let mut world = World::new();
            world.init_resource::<Messages<StateTransitionEvent>>();
            let mut time = Time::<()>::default();
            time.advance_by(std::time::Duration::from_secs_f32(0.02));
            world.insert_resource(time);
            let actor = world.spawn((crate::als::character::AlsCharacter, AlsRollState::default(),
                LocomotionState::Grounded, Stance::Standing, AlsViewYaw::default(), Transform::IDENTITY,
                AlsInputCommand::default(), LinearVelocity(Vec3::NEG_Y * current),
                crate::als::air::AlsPreImpactVelocity { last_air_vertical_velocity: -current },
                crate::actual_motion::AlsActualMotion { world_velocity: Vec3::NEG_Y * retained, ..default() })).id();
            world.resource_mut::<Messages<StateTransitionEvent>>().write(StateTransitionEvent { entity: actor,
                dimension: Dimension::LocomotionState, from: StateValue::LocomotionState(LocomotionState::Air),
                to: StateValue::LocomotionState(LocomotionState::Grounded) });
            world.run_system_once(advance_rolls).unwrap();
            assert_eq!(world.get::<AlsRollState>(actor).unwrap().active, expected,
                "retained={retained} current={current}");
        }
    }

    #[test]
    fn loaded_landing_roll_uses_the_pre_movement_snapshot() {
        for (retained, current, expected) in [(3.0_f32, 8.0_f32, false), (8.0, 3.0, true),
            (3.0, 3.0, false), (8.0, 8.0, true)] {
            let (mut app, actor, _) = loaded_roll_character();
            let start = Vec3::Y * 0.94;
            app.world_mut().entity_mut(actor).insert((avian3d::prelude::Position(start), Transform::from_translation(start),
                LocomotionState::Air, LinearVelocity(Vec3::NEG_Y * current), bevy_tnua::TnuaMotor::default(), AlsRollState::default(),
                crate::als::character::AlsEffectiveGravity { value: Vec3::NEG_Y * 9.81, ..default() },
                crate::als::character::AlsCmcAdapterRuntime::default(),
                crate::als::air::AlsPreImpactVelocity { last_air_vertical_velocity: -current },
                crate::actual_motion::AlsActualMotion { world_velocity: Vec3::NEG_Y * retained, ..default() }));
            app.world_mut().get_mut::<bevy_tnua::TnuaController<crate::als::control_scheme::AlsControlScheme>>(actor).unwrap().basis.desired_motion = Vec3::ZERO;
            app.world_mut().resource_mut::<Time>().advance_by(std::time::Duration::from_secs_f32(0.05));
            app.world_mut().run_system_once(crate::als::motion::cmc_movement_transaction_system).unwrap();
            app.world_mut().run_system_once(advance_rolls).unwrap();
            assert_eq!(app.world().get::<AlsRollState>(actor).unwrap().active, expected,
                "retained={retained} current={current}");
        }
    }

    #[test]
    fn denied_high_speed_ragdoll_does_not_fall_back_to_a_landing_roll() {
        use bevy::ecs::system::RunSystemOnce;
        use crate::als::transition::{Dimension, StateTransitionEvent, StateValue};
        for (speed, roll_expected) in [(8.0_f32, true), (11.0, false)] {
            let (mut app, actor, avatar) = loaded_roll_character();
            let (_, bodies) = crate::ragdoll_link::tests::loaded_ragdoll_bodies(&mut app, avatar);
            if !roll_expected { crate::ragdoll_link::tests::remove_ragdoll_body(&mut app, &bodies, "pelvis"); }
            app.world_mut().entity_mut(actor).insert((LocomotionState::Grounded, AlsRollState::default(),
                AlsNotifyLocomotionAction::None, crate::actual_motion::AlsActualMotion {
                    world_velocity: Vec3::NEG_Y * speed, ..default()
                }));
            app.world_mut().resource_mut::<Messages<StateTransitionEvent>>().write(StateTransitionEvent {
                entity: actor, dimension: Dimension::LocomotionState,
                from: StateValue::LocomotionState(LocomotionState::Air),
                to: StateValue::LocomotionState(LocomotionState::Grounded),
            });
            app.world_mut().run_system_once(advance_rolls).unwrap();
            assert_eq!(app.world().get::<AlsRollState>(actor).unwrap().active, roll_expected, "speed={speed}");
        }
    }

    #[test]
    fn actual_landing_roll_starts_after_a_same_frame_platform_crossing() {
        let (mut app, actor, _) = loaded_roll_character();
        app.world_mut().spawn((avian3d::prelude::RigidBody::Static, avian3d::prelude::Collider::cuboid(4.0, 0.6, 0.1),
            Transform::from_xyz(5.0, 0.3, 0.0)));
        app.update();
        let start = Vec3::new(5.0, 1.6, 0.0);
        app.world_mut().entity_mut(actor).insert((avian3d::prelude::Position(start), Transform::from_translation(start),
            GlobalTransform::from_translation(start), LinearVelocity(Vec3::new(0.0, -8.0, -3.75)), LocomotionState::Air,
            crate::als::transition::AlsStateRequest::default(), AlsInputCommand { move_dir: Vec2::Y, ..default() },
            crate::als_camera::AlsCameraCustomTimeDilation(0.0)));
        for _ in 0..2 { app.update(); }
        let handle = app.world().get::<crate::als::character::AlsMovementParamsHandle>(actor).unwrap().0.clone();
        assert!(!app.world().resource::<Assets<crate::als::params::AlsMovementParams>>().get(&handle).unwrap().rolling.interrupt_when_in_air,
            "The native default asset overrides the source constructor's interrupt switch");
        app.world_mut().entity_mut(actor).insert(crate::als_camera::AlsCameraCustomTimeDilation(1.0));
        app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(0.2)));
        app.update();
        assert_eq!(app.world().get::<LocomotionState>(actor), Some(&LocomotionState::Air));
        assert!(app.world().get::<AlsRollState>(actor).unwrap().active,
            "Source landing Roll must survive the same-frame return to Air when interruption is disabled");
        let delta = app.world().get::<avian3d::prelude::Position>(actor).unwrap().0 - start;
        assert!((delta.z + 0.75).abs() < 0.001, "The newly started montage must not consume the already resolved movement frame: {delta:?}");
    }

    #[test]
    fn automatic_scaled_landing_ragdoll_recovery_has_no_deferred_motor_impulse() {
        use avian3d::prelude::Position;
        use crate::als::transition::AlsStateRequest;
        let (mut app, actor, _) = loaded_roll_character();
        let start = Vec3::new(-15.0, 10.824, -15.0);
        let transform = Transform::from_translation(start).with_scale(Vec3::splat(2.0));
        app.world_mut().entity_mut(actor).insert((Position(start), transform, GlobalTransform::from(transform), LinearVelocity::ZERO));
        for frame in 0..240 {
            app.update();
            if frame >= 120 && app.world().get::<crate::ragdoll_link::AlsRagdollGroundFollow>(actor).is_some_and(|follow| follow.grounded) { break; }
        }
        assert_eq!(app.world().get::<crate::als::state::AlsNotifyLocomotionAction>(actor), Some(&crate::als::state::AlsNotifyLocomotionAction::Ragdolling));
        println!("before_exit mode={:?} velocity={:?} motor={:?}", app.world().get::<LocomotionState>(actor), app.world().get::<LinearVelocity>(actor), app.world().get::<bevy_tnua::TnuaMotor>(actor));
        app.world_mut().get_mut::<AlsStateRequest>(actor).unwrap().ragdoll_exit=true;
        let mut max_up=0.0_f32;
        let mut recovered=false;
        for frame in 0..180 {
            app.update();
            let velocity=app.world().get::<LinearVelocity>(actor).unwrap().0;
            let mode=*app.world().get::<LocomotionState>(actor).unwrap();
            max_up=max_up.max(velocity.y);
            if frame%15==0 { println!("recovery frame={frame} mode={mode:?} velocity={velocity:?} position={:?} action={:?}", app.world().get::<Position>(actor), app.world().get::<crate::als::state::AlsNotifyLocomotionAction>(actor)); }
            recovered |= mode==LocomotionState::Grounded && velocity.length()<0.5;
        }
        assert!(max_up<0.5,"Source Grounded ragdoll exit has zero velocity and no deferred CMC impulse: {max_up}");
        assert!(recovered,"Source scaled ragdoll must complete grounded recovery");
    }
    #[test]
    fn fresh_audit_roll_and_tail_use_the_current_actor_clock() {
        let mut world = World::new();
        world.init_resource::<Messages<crate::als::transition::StateTransitionEvent>>();
        let mut time = Time::<()>::default();
        time.advance_by(std::time::Duration::from_secs_f32(0.05));
        world.insert_resource(time);
        let mut failures = Vec::new();
        for scale in [0.0_f32, 0.5, 1.0, 2.0] {
            let roll = AlsRollState { active: true, time: 0.3, previous_time: 0.3, ..default() };
            let actor = world.spawn((crate::als::character::AlsCharacter, roll, LocomotionState::Grounded,
                Stance::Standing, AlsViewYaw::default(), Transform::IDENTITY, LinearVelocity::ZERO,
                AlsInputCommand::default(), crate::als_camera::AlsCameraCustomTimeDilation(scale))).id();
            let tail = world.spawn((AlsRollTail::from_roll(&roll, AlsRollBlendOut::new(&roll, 0.2)),
                AlsRollSlotSampler { character: actor, target_player: Entity::PLACEHOLDER, reported_unready: false })).id();
            world.run_system_once(advance_rolls).unwrap();
            let expected = 0.3 + 0.05 * scale;
            let current = world.get::<AlsRollState>(actor).unwrap().time;
            let tail_time = world.get::<AlsRollTail>(tail).unwrap().clip_time;
            if (current - expected).abs() > 1e-6 || (tail_time - expected * ROLL_PLAY_RATE).abs() > 1e-6 {
                failures.push((scale, expected, current, tail_time));
            }
        }
        assert!(failures.is_empty(), "Roll ignored actor time dilation: {failures:?}");
    }

    #[test]
    fn animation_root_velocity_restoration_preserves_falling_physics_axis() {
        let (mut app, actor, _) = loaded_roll_character();
        play_loaded_roll(&mut app, actor);
        app.world_mut().entity_mut(actor).insert((avian3d::prelude::Position(Vec3::Y * 10.0),
            Transform::from_xyz(0.0, 10.0, 0.0), LocomotionState::Air));
        app.world_mut().run_system_once(roll_drive_system).unwrap();
        let root = app.world().get::<crate::als::character::AlsCmcAdapterRuntime>(actor).unwrap().grounded_collision_velocity.unwrap();
        app.world_mut().get_mut::<LinearVelocity>(actor).unwrap().0 = root.with_y(-2.1635);
        app.world_mut().run_system_once(crate::als::motion::locomotion_state_sync_system).unwrap();
        let actual = app.world().get::<LinearVelocity>(actor).unwrap().0;
        assert!(actual.abs_diff_eq(root.with_y(-2.1635), 1e-5),
            "Source ConstrainAnimRootMotionVelocity preserves falling physics: actual={actual:?}");
        app.world_mut().get_mut::<crate::als::character::AlsCmcAdapterRuntime>(actor).unwrap().grounded_collision_velocity = Some(Vec3::new(3.0, -1.0, -4.0));
        app.world_mut().get_mut::<LinearVelocity>(actor).unwrap().0 = Vec3::new(1.5, 0.0, -2.0);
        app.world_mut().run_system_once(crate::als::motion::locomotion_state_sync_system).unwrap();
        let actual = app.world().get::<LinearVelocity>(actor).unwrap().0;
        assert!(actual.abs_diff_eq(Vec3::new(3.0, -1.0, -4.0), 1e-5),
            "Source regular CMC velocity remains authoritative when Roll starts after movement: actual={actual:?}");
    }

    #[test]
    fn roll_drive_preserves_existing_falling_velocity_before_physics() {
        use bevy::ecs::system::RunSystemOnce;
        for speed in [-2.0, 0.0, 2.0] {
            let mut world = World::new();
            world.init_resource::<Messages<crate::als::transition::StateTransitionEvent>>();
            let mut time = Time::<()>::default();
            time.advance_by(std::time::Duration::from_secs_f32(0.05));
            world.insert_resource(time);
            let actor = world.spawn((crate::als::character::AlsCharacter,
                AlsRollState { active: true, time: 0.1, previous_time: 0.05, ..default() },
                LocomotionState::Air, avian3d::prelude::RigidBody::Dynamic, Transform::IDENTITY,
                LinearVelocity(Vec3::new(0.0, speed, 0.0)))).id();
            let mesh = world.spawn((ChildOf(actor), Transform::IDENTITY)).id();
            world.spawn((AlsRollSlotSampler { character: actor, target_player: mesh, reported_unready: false },
                RootMotionOutput { translation_delta: Vec3::new(0.1, 0.2, 0.0), ..default() }));
            world.run_system_once(roll_drive_system).unwrap();
            assert_eq!(world.get::<LinearVelocity>(actor).unwrap().y, speed);
            let runtime = world.get::<crate::als::character::AlsCmcAdapterRuntime>(actor).unwrap();
            assert_eq!(runtime.grounded_collision_velocity.unwrap().y, speed);
            assert!(runtime.has_animation_root_velocity);
        }
    }

    #[test]
    fn roll_root_motion_clears_the_previous_grounded_movement_budget() {
        use bevy::ecs::system::RunSystemOnce;
        for (active, budget) in [(true, Vec3::ZERO), (true, Vec3::X * 0.2), (false, Vec3::ZERO)] {
            let mut world = World::new();
            world.init_resource::<Messages<crate::als::transition::StateTransitionEvent>>();
            let mut time = Time::<()>::default();
            time.advance_by(std::time::Duration::from_secs_f32(0.04));
            world.insert_resource(time);
            let actor = world.spawn((crate::als::character::AlsCharacter,
                AlsRollState { active, time: 0.1, previous_time: 0.05, ..default() },
                LocomotionState::Grounded, avian3d::prelude::RigidBody::Dynamic, Transform::IDENTITY,
                LinearVelocity::ZERO, crate::als::character::AlsCmcAdapterRuntime {
                    grounded_movement_delta: Some(budget), ..default()
                })).id();
            let mesh = world.spawn((ChildOf(actor), Transform::IDENTITY)).id();
            world.spawn((AlsRollSlotSampler { character: actor, target_player: mesh, reported_unready: false },
                RootMotionOutput { translation_delta: Vec3::X * 0.02, ..default() }));
            world.run_system_once(roll_drive_system).unwrap();
            let runtime = world.get::<crate::als::character::AlsCmcAdapterRuntime>(actor).unwrap();
            assert_eq!(runtime.grounded_movement_delta, if active { None } else { Some(budget) });
            if active { assert!(world.get::<LinearVelocity>(actor).unwrap().x.abs() > 0.1); }
        }
    }

    #[test]
    fn loaded_grounded_roll_respects_the_owner_minimum_movement_delta() {
        for dilation in [0.0_f32, 0.00001, 0.0001, 1.0] {
            let (mut app, actor, _) = loaded_roll_character();
            play_loaded_roll(&mut app, actor);
            app.world_mut().entity_mut(actor).insert((LinearVelocity::ZERO,
                crate::als_camera::AlsCameraCustomTimeDilation(dilation)));
            let start = app.world().get::<avian3d::prelude::Position>(actor).unwrap().0;
            app.update();
            let displacement = (app.world().get::<avian3d::prelude::Position>(actor).unwrap().0 - start).with_y(0.0).length();
            if dilation <= 0.00001 {
                assert!(displacement < 1e-7, "dilation={dilation} displacement={displacement}");
            } else {
                assert!(displacement > 1e-7, "dilation={dilation} displacement={displacement}");
            }
            assert!(app.world().get::<crate::als::motion::AlsCustomMovementPhysicsFrame>(actor).is_none());
        }
    }

    #[test]
    fn loaded_roll_first_frame_uses_root_motion_in_the_clamped_physics_window() {
        use bevy::time::TimeUpdateStrategy;
        for (delta, expected) in [(0.034, 0.01636579), (0.04, 0.02207679), (0.05, 0.03159513)] {
            let (mut app, actor, _) = loaded_roll_character();
            app.world_mut().get_mut::<crate::als::transition::AlsStateRequest>(actor).unwrap().stance = Some(Stance::Crouching);
            for _ in 0..8 { app.update(); }
            assert_eq!(*app.world().get::<Stance>(actor).unwrap(), Stance::Crouching);
            app.add_systems(PostUpdate, (move |mut actors: Query<(&AlsRollState, &mut crate::als::character::AlsCmcAdapterRuntime)>,
                mut injected: Local<bool>| {
                if !*injected && let Ok((roll, mut runtime)) = actors.get_mut(actor) && roll.active {
                    runtime.grounded_movement_delta = Some(Vec3::ZERO);
                    *injected = true;
                }
            }).after(AnimationGraphSet::PrePhysics).before(AlsRollDriveSet));
            let start = app.world().get::<avian3d::prelude::Position>(actor).unwrap().0;
            app.world_mut().get_mut::<AlsInputCommand>(actor).unwrap().roll_pressed = true;
            *app.world_mut().resource_mut::<TimeUpdateStrategy>() =
                TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(delta));
            app.update();
            let moved = (app.world().get::<avian3d::prelude::Position>(actor).unwrap().0 - start).with_y(0.0).length();
            assert!((moved - expected).abs() < 0.0005,
                "Roll root displacement differs: delta={delta} moved={moved} source={expected}");
            assert!(app.world().get::<crate::als::character::AlsCmcAdapterRuntime>(actor).unwrap().grounded_movement_delta.is_none());
        }
    }

    #[test]
    fn loaded_airborne_roll_preserves_vertical_velocity_through_gravity() {
        use bevy::time::TimeUpdateStrategy;
        for delta in [1.0 / 60.0, 0.1] {
            for speed in [-2.0, 0.0, 2.0] {
                for rolling in [false, true] {
                    let (mut app, actor, _) = loaded_roll_character();
                    if rolling { play_loaded_roll(&mut app, actor); }
                    let start = Vec3::Y * 10.0;
                    app.world_mut().entity_mut(actor).insert((avian3d::prelude::Position(start),
                        Transform::from_translation(start), LinearVelocity(Vec3::Y * speed), LocomotionState::Air));
                    *app.world_mut().resource_mut::<TimeUpdateStrategy>() =
                        TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(delta));
                    app.update();
                    let actual = app.world().get::<LinearVelocity>(actor).unwrap().y;
                    let expected = speed - 9.81 * delta;
                    assert!((actual - expected).abs() < 0.02,
                        "rolling={rolling} delta={delta} speed={speed} actual={actual} expected={expected}");
                }
            }
        }
    }

    #[test]
    fn loaded_airborne_roll_uses_source_midpoint_at_short_frames() {
        use bevy::time::TimeUpdateStrategy;
        for delta in [1.0 / 60.0, 0.025, 1.0 / 30.0, 0.034, 0.05, 0.1] {
            for rolling in [false, true] {
                let (mut app, actor, _) = loaded_roll_character();
                if rolling { play_loaded_roll(&mut app, actor); }
                let start = Vec3::Y * 10.0;
                app.world_mut().entity_mut(actor).insert((avian3d::prelude::Position(start),
                    Transform::from_translation(start), LinearVelocity::ZERO, LocomotionState::Air));
                *app.world_mut().resource_mut::<TimeUpdateStrategy>() =
                    TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(delta));
                app.update();
                let actual = app.world().get::<avian3d::prelude::Position>(actor).unwrap().y - start.y;
                let expected = -0.5 * 9.81 * delta * delta;
                assert!((actual - expected).abs() < 0.00005,
                    "rolling={rolling} delta={delta} displacement={actual} source={expected}");
            }
        }
    }

    #[test]
    fn loaded_roll_landing_adjusts_floor_before_remaining_walking() {
        use avian3d::prelude::{Collider, Position, RigidBody, SubstepCount};
        use bevy::time::TimeUpdateStrategy;
        for (rolling, delta, gap, landed) in [(true, 0.05, 0.005, true),
            (true, 0.1, 0.005, true), (true, 0.1, 0.02, true),
            (true, 0.05, 0.02, false), (true, 0.05, 0.1, false),
            (false, 0.1, 0.005, true)] {
            for substeps in [1, 6] {
                let (mut app, actor, _) = loaded_roll_character();
                let floor = app.world_mut().spawn((RigidBody::Static, Collider::cuboid(4.0, 1.0, 4.0),
                    Transform::from_xyz(5.0, 1.5, 0.0))).id();
                app.update();
                if rolling { play_loaded_roll(&mut app, actor); }
                let half = app.world().get::<Collider>(actor).unwrap().shape_scaled().compute_local_aabb().half_extents().y;
                let start = Vec3::new(5.0, 2.0 + half + gap, 0.0);
                app.world_mut().entity_mut(actor).insert((Position(start), Transform::from_translation(start),
                    LocomotionState::Air, LinearVelocity::ZERO, bevy_tnua::TnuaMotor::default(),
                    crate::als::character::AlsCmcAdapterRuntime::default(),
                    crate::als::dynamics::AlsMovementBase::default(),
                    crate::als::dynamics::AlsMovementBaseDynamicsState::default()));
                app.insert_resource(SubstepCount(substeps));
                app.insert_resource(TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(delta)));
                app.update();
                let position = app.world().get::<Position>(actor).unwrap().0;
                let actual_half = app.world().get::<Collider>(actor).unwrap().shape_scaled().compute_local_aabb().half_extents().y;
                let distance = position.y - 2.0 - actual_half;
                let mode = *app.world().get::<LocomotionState>(actor).unwrap();
                if landed {
                    assert_eq!(mode, LocomotionState::Grounded, "rolling={rolling} delta={delta} gap={gap} substeps={substeps}");
                    assert!((0.019..=0.024).contains(&distance),
                        "rolling={rolling} delta={delta} gap={gap} substeps={substeps} distance={distance}");
                    assert_eq!(app.world().get::<crate::als::dynamics::AlsMovementBase>(actor).unwrap().entity, Some(floor));
                    assert!(app.world().get::<LinearVelocity>(actor).unwrap().y.abs() < 1e-4);
                } else {
                    assert_eq!(mode, LocomotionState::Air, "rolling={rolling} delta={delta} gap={gap} substeps={substeps}");
                    let expected = gap - 0.5 * 9.81 * delta * delta;
                    assert!((distance - expected).abs() < 0.0001,
                        "rolling={rolling} delta={delta} gap={gap} substeps={substeps} distance={distance} expected={expected}");
                }
                if rolling {
                    assert!((position - start).with_y(0.0).length() > 0.001);
                }
            }
        }
    }

    #[test]
    fn loaded_roll_rotation_is_applied_once_through_physics() {
        use avian3d::prelude::AngularVelocity;
        for (dilation, yaw) in [(0.5, 0.7), (1.0, 0.7), (2.0, 0.7),
            (0.5, 0.0), (1.0, 0.0), (2.0, 0.0)] {
            let (mut app, actor, _) = loaded_roll_character();
            play_loaded_roll(&mut app, actor);
            let rotation = Quat::from_rotation_y(yaw);
            let mut transform = *app.world().get::<Transform>(actor).unwrap();
            transform.rotation = rotation;
            app.world_mut().entity_mut(actor).insert((transform, Rotation(rotation), AngularVelocity::ZERO,
                crate::als_camera::AlsCameraCustomTimeDilation(dilation)));
            app.world_mut().get_mut::<AlsRollState>(actor).unwrap().direction_yaw = 0.0;
            app.update();
            let actual = app.world().get::<Rotation>(actor).unwrap().0.to_euler(EulerRot::YXZ).0;
            let expected = damper_exact_angle(yaw, 0.0, dilation / 60.0, ROLL_ROTATION_HALF_LIFE_SECS);
            assert!((actual - expected).abs() < 1e-4,
                "dilation={dilation} yaw={yaw} actual={actual} expected={expected}");
        }
    }

    #[test]
    fn roll_yaw_damping_stops_with_the_action_but_root_motion_remains() {
        use bevy::ecs::system::RunSystemOnce;
        for (action, root_yaw, expected) in [(AlsNotifyLocomotionAction::Rolling, 0.0,
            damper_exact_angle(0.7, 0.0, 1.0 / 60.0, ROLL_ROTATION_HALF_LIFE_SECS)),
            (AlsNotifyLocomotionAction::None, 0.0, 0.7),
            (AlsNotifyLocomotionAction::None, 0.2, 0.9)] {
            let mut world = World::new();
            world.init_resource::<Messages<crate::als::transition::StateTransitionEvent>>();
            let mut time = Time::<()>::default();
            time.advance_by(std::time::Duration::from_secs_f32(1.0 / 60.0));
            world.insert_resource(time);
            let actor = world.spawn((crate::als::character::AlsCharacter,
                AlsRollState { active: true, time: 0.75, previous_time: 0.74, direction_yaw: 0.0, ..default() },
                LocomotionState::Grounded, avian3d::prelude::RigidBody::Dynamic, action,
                Transform::from_rotation(Quat::from_rotation_y(0.7)), LinearVelocity::ZERO)).id();
            let mesh = world.spawn((ChildOf(actor), Transform::IDENTITY)).id();
            world.spawn((AlsRollSlotSampler { character: actor, target_player: mesh, reported_unready: false },
                RootMotionOutput { translation_delta: Vec3::X * 0.02, rotation_delta: Quat::from_rotation_y(root_yaw), ..default() }));
            world.run_system_once(roll_drive_system).unwrap();
            let actual = world.get::<Transform>(actor).unwrap().rotation.to_euler(EulerRot::YXZ).0;
            assert!((actual - expected).abs() < 1e-5, "action={action:?} root_yaw={root_yaw} actual={actual} expected={expected}");
            assert!(world.get::<LinearVelocity>(actor).unwrap().0.with_y(0.0).length() > 0.0,
                "The action gate must not discard root translation");
        }
    }

    #[test]
    fn loaded_roll_stops_turning_when_the_action_notify_ends() {
        #[derive(Resource, Default)]
        struct RotationSample(Option<(f32, AlsNotifyLocomotionAction)>);
        for (target_time, expected_action) in [(0.3, AlsNotifyLocomotionAction::Rolling),
            (0.716667, AlsNotifyLocomotionAction::None), (0.75, AlsNotifyLocomotionAction::None),
            (0.816667, AlsNotifyLocomotionAction::None)] {
            let (mut app, actor, _) = loaded_roll_character();
            play_loaded_roll(&mut app, actor);
            let current = app.world().get::<AlsRollState>(actor).unwrap().time;
            let steps = ((target_time - current).max(0.0_f32) * 60.0).round() as usize;
            for _ in 0..steps { app.update(); }
            let reached = app.world().get::<AlsRollState>(actor).unwrap().time;
            assert!((reached - target_time).abs() < 1e-4, "Roll clock did not reach the requested time: {target_time} actual={reached}");
            if target_time == 0.716667 {
                assert!(reached < 0.7180254 && reached + 1.0 / 60.0 > 0.7180254,
                    "The next frame must cross the source action end: {reached}");
            }
            app.init_resource::<RotationSample>().add_systems(PostUpdate,
                (move |mut sample: ResMut<RotationSample>, q: Query<(&Transform, &AlsNotifyLocomotionAction)>| {
                    let (transform, action) = q.get(actor).unwrap();
                    sample.0 = Some((transform.rotation.to_euler(EulerRot::YXZ).0, *action));
                }).after(AlsRollDriveSet).before(avian3d::prelude::PhysicsSystems::First));
            let mut transform = *app.world().get::<Transform>(actor).unwrap();
            transform.rotation = Quat::from_rotation_y(0.7);
            app.world_mut().entity_mut(actor).insert((transform, Rotation(transform.rotation), avian3d::prelude::AngularVelocity::ZERO));
            app.world_mut().get_mut::<AlsRollState>(actor).unwrap().direction_yaw = 0.0;
            app.update();
            let (actual, action) = app.world().resource::<RotationSample>().0.unwrap();
            assert_eq!(action, expected_action, "time={target_time}");
            let expected = if action == AlsNotifyLocomotionAction::Rolling {
                damper_exact_angle(0.7, 0.0, 1.0 / 60.0, ROLL_ROTATION_HALF_LIFE_SECS)
            } else { 0.7 };
            assert!((actual - expected).abs() < 1e-4,
                "time={target_time} action={action:?} actual={actual} expected={expected}");
            assert!(app.world().get::<AlsRollState>(actor).unwrap().root_motion_active());
        }
    }

    #[test]
    fn loaded_roll_source_velocity_preserves_owner_clock_displacement_base_and_handoff() {
        for (mode, base_speed) in [(LocomotionState::Grounded, 0.0_f32), (LocomotionState::Grounded, 0.75), (LocomotionState::Custom, 0.0)] {
            for dilation in [0.5_f32, 1.0, 2.0] {
                let (mut app, character, avatar) = loaded_roll_character();
                let base = app.world_mut().query_filtered::<(Entity, &avian3d::prelude::Collider), Without<crate::als::character::AlsCharacter>>()
                    .iter(app.world()).find(|(_, collider)| collider.shape_scaled().compute_local_aabb().half_extents().x > 40.0).unwrap().0;
                if base_speed > 0.0 {
                    app.world_mut().entity_mut(base).insert((avian3d::prelude::RigidBody::Kinematic, LinearVelocity(Vec3::X * base_speed)));
                }
                for _ in 0..40 { app.update(); }
                play_loaded_roll(&mut app, character);
                let player = app.world().get::<AnimatedSceneInstance>(avatar).unwrap().player_entity();
                app.world_mut().entity_mut(character).insert((mode, crate::als_camera::AlsCameraCustomTimeDilation(dilation)));
                for frame in 0..8 {
                    let before = app.world().get::<avian3d::prelude::Position>(character).unwrap().0;
                    let base_before = app.world().get::<avian3d::prelude::Position>(base).unwrap().0;
                    app.update();
                    let runtime = *app.world().get::<AlsRollSlotRuntime>(character).unwrap();
                    assert!(runtime.pose_ready && app.world().get::<AlsRollState>(character).unwrap().root_motion_active());
                    let (actor_world, mesh_world) = app.world_mut().run_system_once(move |helper: bevy::transform::helper::TransformHelper| {
                        (helper.compute_global_transform(character).unwrap(), helper.compute_global_transform(player).unwrap())
                    }).unwrap();
                    let root = root_motion_world_transform(actor_world, mesh_world,
                        runtime.root_motion_delta * runtime.root_motion_scale, runtime.root_motion_rotation).0;
                    let delta = app.world().resource::<Time>().delta_secs();
                    let expected = root / (delta * dilation);
                    let actual = app.world().get::<crate::actual_motion::AlsActualMotion>(character).unwrap().world_velocity;
                    assert!(actual.with_y(0.0).abs_diff_eq(expected.with_y(0.0), 0.005),
                        "Source Roll velocity uses owner delta: base={base_speed} dilation={dilation} frame={frame} actual={actual:?} expected={expected:?}");
                    let moved = app.world().get::<avian3d::prelude::Position>(character).unwrap().0 - before;
                    let carried = app.world().get::<avian3d::prelude::Position>(base).unwrap().0 - base_before;
                    assert!((moved - carried - root).with_y(0.0).length() < 0.005,
                        "Source Roll root displacement retains based movement: base={base_speed} dilation={dilation} frame={frame} moved={moved:?} carried={carried:?} root={root:?}");
                }
                let before = app.world().get::<avian3d::prelude::Position>(character).unwrap().0;
                let base_before = app.world().get::<avian3d::prelude::Position>(base).unwrap().0;
                let previous = app.world().get::<crate::actual_motion::AlsActualMotion>(character).unwrap().world_velocity;
                app.world_mut().get_mut::<crate::als_camera::AlsCameraCustomTimeDilation>(character).unwrap().0 = 0.0;
                app.update();
                let actual = app.world().get::<crate::actual_motion::AlsActualMotion>(character).unwrap().world_velocity;
                let moved = app.world().get::<avian3d::prelude::Position>(character).unwrap().0 - before;
                let carried = app.world().get::<avian3d::prelude::Position>(base).unwrap().0 - base_before;
                assert!(actual.with_y(0.0).abs_diff_eq(previous.with_y(0.0), 0.005) && (moved - carried).with_y(0.0).length() < 0.005,
                    "Source zero-delta Roll preserves current velocity and consumes no root displacement: actual={actual:?} previous={previous:?} moved={moved:?} carried={carried:?}");
                app.world_mut().entity_mut(character).insert((AlsRollState::default(), AlsNotifyLocomotionAction::None, LocomotionState::Grounded,
                    crate::als_camera::AlsCameraCustomTimeDilation(dilation)));
                app.update();
                let actual = app.world().get::<crate::actual_motion::AlsActualMotion>(character).unwrap().world_velocity;
                let deceleration = app.world().get::<crate::als::dynamics::AlsGroundedDynamics>(character).unwrap().0.braking_deceleration;
                assert!((actual - previous).with_y(0.0).length() <= deceleration * app.world().resource::<Time>().delta_secs() * dilation + 0.005,
                    "Source ordinary movement inherits the undilated Roll velocity: dilation={dilation} actual={actual:?} previous={previous:?} deceleration={deceleration}");
            }
        }
    }

    #[test]
    fn starting_roll_inside_the_natural_window_records_its_first_stop() {
        let mut world = World::new();
        world.init_resource::<Messages<crate::als::transition::StateTransitionEvent>>();
        let mut time = Time::<()>::default();
        time.advance_by(std::time::Duration::from_secs_f32(0.1));
        world.insert_resource(time);
        let actor = world.spawn((crate::als::character::AlsCharacter, AlsRollState::default(),
            LocomotionState::Grounded, Stance::Standing, AlsViewYaw::default(), Transform::IDENTITY,
            AlsInputCommand { roll_pressed: true, ..default() },
            crate::als_camera::AlsCameraCustomTimeDilation(10.0))).id();
        world.run_system_once(advance_rolls).unwrap();
        let roll = *world.get::<AlsRollState>(actor).unwrap();
        assert_eq!(roll.natural_blend_start, Some(1.0));
        assert_eq!(roll.weight(), 1.0);
        world.entity_mut(actor).insert(AlsInputCommand::default());
        let delta = (crate::als::roll::ROLL_DURATION_SECS - 1.0) * 0.05;
        world.resource_mut::<Time>().advance_by(std::time::Duration::from_secs_f32(delta));
        world.run_system_once(advance_rolls).unwrap();
        assert!((world.get::<AlsRollState>(actor).unwrap().weight() - 0.5).abs() < 1e-5);
    }

    #[test]
    fn fresh_audit_roll_rotation_uses_the_current_actor_clock() {
        let (mut app, character, _) = loaded_roll_character();
        let sampler = play_loaded_roll(&mut app, character);
        let mut failures = Vec::new();
        for scale in [0.0_f32, 0.5, 1.0, 2.0] {
            app.world_mut().entity_mut(character).insert((Transform::IDENTITY,
                GlobalTransform::IDENTITY, Rotation(Quat::IDENTITY),
                crate::als_camera::AlsCameraCustomTimeDilation(scale),
                AlsRollState { active: true, time: 0.3, previous_time: 0.3, direction_yaw: 1.0, ..default() }));
            *app.world_mut().get_mut::<RootMotionOutput>(sampler).unwrap() = RootMotionOutput::default();
            app.world_mut().resource_mut::<Time>().advance_by(std::time::Duration::from_secs_f32(0.05));
            app.world_mut().run_system_once(roll_drive_system).unwrap();
            let expected = damper_exact_angle(0.0, 1.0, 0.05 * scale, ROLL_ROTATION_HALF_LIFE_SECS);
            let actual = app.world().get::<Transform>(character).unwrap().rotation.to_euler(EulerRot::YXZ).0;
            if crate::als::roll::wrap_angle(actual - expected).abs() > 1e-5 { failures.push((scale, expected, actual)); }
        }
        assert!(failures.is_empty(), "Roll rotation ignored actor time dilation: {failures:?}");
    }

    #[test]
    fn fresh_audit_roll_loaded_actor_clock_and_custom_displacement() {
        let (mut app, character, _) = loaded_roll_character();
        let sampler = play_loaded_roll(&mut app, character);
        let target = app.world().get::<AlsRollSlotSampler>(sampler).unwrap().target_player;
        app.world_mut().get_mut::<crate::als::transition::AlsStateRequest>(character).unwrap().locomotion_state = Some(LocomotionState::Custom);
        app.update();
        app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(0.05)));
        for scale in [0.0_f32, 0.5, 2.0] {
            app.world_mut().entity_mut(character).insert(crate::als_camera::AlsCameraCustomTimeDilation(scale));
            let before_time = app.world().get::<AlsRollState>(character).unwrap().time;
            let (actor, mesh) = app.world_mut().run_system_once(move |helper: bevy::transform::helper::TransformHelper| {
                (helper.compute_global_transform(character).unwrap(), helper.compute_global_transform(target).unwrap())
            }).unwrap();
            app.update();
            let expected_time = before_time + app.world().resource::<Time>().delta_secs() * scale;
            let actual_time = app.world().get::<AlsRollState>(character).unwrap().time;
            let clip_time = app.world().get::<AnimationGraphPlayer>(sampler).unwrap()
                .get_global_input_data(ROLL_SLOT_TIME_KEY).unwrap().as_f32().unwrap();
            let root = app.world().get::<RootMotionOutput>(sampler).unwrap();
            let runtime = app.world().get::<AlsRollSlotRuntime>(character).unwrap();
            let expected_position = actor.translation() + root_motion_world_transform(actor, mesh,
                root.translation_delta * runtime.root_motion_scale, root.rotation_delta).0;
            let actual_position = app.world().get::<avian3d::prelude::Position>(character).unwrap().0;
            assert!((actual_time - expected_time).abs() < 1e-6
                && (clip_time - expected_time * ROLL_PLAY_RATE).abs() < 1e-6
                && actual_position.distance(expected_position) < 1e-4,
                "Loaded roll clock or full-frame root displacement differs: scale={scale} expected_time={expected_time} actual_time={actual_time} clip_time={clip_time} expected_position={expected_position:?} actual_position={actual_position:?}");
        }
    }

    #[test]
    fn fresh_audit_roll_root_uses_actor_interval_and_minimum_delta() {
        for (world_delta, scale, crossing) in [(0.05_f32, 2.0_f32, true), (1e-6, 0.5, false)] {
            let (mut app, character, _) = loaded_roll_character();
            let sampler = play_loaded_roll(&mut app, character);
            let end = crate::als::roll::ROLL_DURATION_SECS - crate::als::roll::ROLL_WEIGHT_OUT_SECS;
            let current = if crossing { end + 0.06 } else { 0.3 + world_delta * scale };
            let previous = if crossing { end - 0.04 } else { 0.3 };
            app.world_mut().entity_mut(character).insert((Transform::IDENTITY, GlobalTransform::IDENTITY,
                avian3d::prelude::Position(Vec3::ZERO), Rotation(Quat::IDENTITY), LocomotionState::Custom,
                crate::als_camera::AlsCameraCustomTimeDilation(scale),
                AlsRollState { active: true, time: current, previous_time: previous, ..default() }));
            *app.world_mut().get_mut::<RootMotionOutput>(sampler).unwrap() = RootMotionOutput {
                translation_delta: Vec3::NEG_Z * if crossing { 0.01 } else { 1e-6 },
                rotation_delta: Quat::IDENTITY,
            };
            app.world_mut().resource_mut::<Time>().advance_by(std::time::Duration::from_secs_f32(world_delta));
            app.world_mut().run_system_once(roll_drive_system).unwrap();
            let distance = app.world().get::<avian3d::prelude::Position>(character).unwrap().0.length();
            assert!(if crossing { distance > 0.005 } else { distance < 1e-8 },
                "Roll used the world interval or world movement minimum: crossing={crossing} distance={distance}");
        }
    }

    #[test]
    fn fresh_audit_roll_parent_keeps_world_rotation_and_position() {
        for (scale, pitch, world_roll, changed_parent) in [
            (1.5_f32, 0.0_f32, 0.0_f32, false),
            (0.5, 0.12, -0.18, false),
            (2.0, -0.1, 0.2, true),
        ] {
        let (mut app, character, _) = loaded_roll_character();
        let sampler = play_loaded_roll(&mut app, character);
        let target = app.world().get::<AlsRollSlotSampler>(sampler).unwrap().target_player;
        let parent_transform = Transform::from_xyz(2.0, 1.0, -3.0)
            .with_rotation(Quat::from_euler(EulerRot::YXZ, 0.7, pitch, world_roll)).with_scale(Vec3::splat(scale));
        let parent = app.world_mut().spawn((parent_transform, GlobalTransform::from(parent_transform))).id();
        app.world_mut().entity_mut(character).insert((ChildOf(parent), Transform::IDENTITY,
            GlobalTransform::from(parent_transform), LocomotionState::Custom,
            AlsRollState { active: true, time: 0.3, previous_time: 0.3, direction_yaw: 1.0, ..default() }));
        if changed_parent {
            let mut current = app.world_mut().get_mut::<Transform>(parent).unwrap();
            current.translation += Vec3::new(0.4, 0.1, -0.2);
            current.rotation = Quat::from_euler(EulerRot::YXZ, 1.1, pitch, world_roll);
        }
        let (actor_world, mesh_world) = app.world_mut().run_system_once(move |helper: bevy::transform::helper::TransformHelper| {
            (helper.compute_global_transform(character).unwrap(), helper.compute_global_transform(target).unwrap())
        }).unwrap();
        let local_delta = Vec3::NEG_Z * 0.1;
        *app.world_mut().get_mut::<RootMotionOutput>(sampler).unwrap() = RootMotionOutput {
            translation_delta: local_delta, rotation_delta: Quat::IDENTITY,
        };
        app.world_mut().resource_mut::<Time>().advance_by(std::time::Duration::from_secs_f32(0.05));
        app.world_mut().run_system_once(roll_drive_system).unwrap();
        let actual_world = app.world_mut().run_system_once(move |helper: bevy::transform::helper::TransformHelper| {
            helper.compute_global_transform(character).unwrap()
        }).unwrap();
        let root_scale = app.world().get::<AlsRollSlotRuntime>(character).unwrap().root_motion_scale;
        let expected_position = actor_world.translation()
            + root_motion_world_transform(actor_world, mesh_world, local_delta * root_scale, Quat::IDENTITY).0;
        let (yaw, current_pitch, current_roll) = actor_world.rotation().to_euler(EulerRot::YXZ);
        let expected_rotation = Quat::from_euler(EulerRot::YXZ,
            damper_exact_angle(yaw, 1.0, 0.05, ROLL_ROTATION_HALF_LIFE_SECS), current_pitch, current_roll);
        assert!(actual_world.translation().distance(expected_position) < 1e-5
            && actual_world.rotation().dot(expected_rotation).abs() > 1.0 - 1e-5
            && app.world().get::<Rotation>(character).unwrap().0.dot(expected_rotation).abs() > 1.0 - 1e-5
            && app.world().get::<avian3d::prelude::Position>(character).unwrap().0.distance(expected_position) < 1e-5,
            "Roll wrote world movement through local transforms: expected_position={expected_position:?} actual_world={actual_world:?} physics={:?}",
            app.world().get::<avian3d::prelude::Position>(character));
        }
    }

    #[test]
    fn fresh_audit_roll_parent_start_uses_world_actor_yaw_without_input() {
        let (mut app, character, _) = loaded_roll_character();
        let parent_transform = Transform::from_rotation(Quat::from_rotation_y(0.7));
        let parent = app.world_mut().spawn((parent_transform, GlobalTransform::from(parent_transform))).id();
        app.world_mut().entity_mut(character).insert((ChildOf(parent), Transform::IDENTITY,
            GlobalTransform::from(parent_transform), AlsRollState::default(),
            AlsNotifyLocomotionAction::None, LocomotionState::Grounded,
            AlsInputCommand { roll_pressed: true, ..default() }));
        app.world_mut().resource_mut::<Time>().advance_by(std::time::Duration::from_secs_f32(0.05));
        app.world_mut().run_system_once(advance_rolls).unwrap();
        let roll = app.world().get::<AlsRollState>(character).unwrap();
        assert!(roll.active && crate::als::roll::wrap_angle(roll.direction_yaw - 0.7).abs() < 1e-5,
            "Roll used local actor yaw when source GetActorRotation is world: {roll:?}");
    }

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
                        .add_message::<crate::als::transition::StateTransitionEvent>()
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
        world.init_resource::<Messages<crate::als::transition::StateTransitionEvent>>();
            let mut time = Time::<()>::default();
            time.advance_by(std::time::Duration::from_secs_f32(1.0 / 60.0));
            world.insert_resource(time);
            let character = world.spawn((crate::als::character::AlsCharacter, AlsRollState::default(),
                LocomotionState::Grounded, Stance::Standing, AlsViewYaw(1.2), crate::als::view::AlsPlayerViewYaw(0.4),
                Transform::from_rotation(Quat::from_rotation_y(0.7)), LinearVelocity::ZERO,
                crate::actual_motion::AlsActualMotion { has_input: input != Vec2::ZERO,
                    input_yaw_degrees: expected.to_degrees(), ..default() },
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
        while app.world().get::<crate::als::character::AlsBirthAnimationPending>(character).is_some()
            || app.world().get::<crate::als::character::AlsInitialMovementModePending>(character).is_some()
            || *app.world().get::<LocomotionState>(character).unwrap() != LocomotionState::Grounded {
            app.update();
            assert!(std::time::Instant::now() < deadline, "Roll fixture did not reach the ground");
        }
        (app, character, avatar)
    }

    pub(crate) fn play_loaded_roll(app: &mut App, character: Entity) -> Entity {
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
        let support = app.world().get::<crate::als::dynamics::AlsMovementBaseDynamicsState>(character)
            .filter(|base| base.entity.is_some()).map_or(Vec3::ZERO, |base| base.support_velocity);
        assert!((app.world().get::<LinearVelocity>(character).unwrap().0 - support).abs_diff_eq(expected, 1e-4),
            "Root motion did not come from the current roll player: expected={expected:?}, actual={:?}", app.world().get::<LinearVelocity>(character));
        entity
    }

    #[test]
    fn browser_review_montage_roll_slot_samples_keep_the_pose_weight() {
        let (mut app, character, avatar) = loaded_roll_character();
        play_loaded_roll(&mut app, character);
        app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(0.01)));
        {
            let mut roll = app.world_mut().get_mut::<AlsRollState>(character).unwrap();
            roll.time = 0.02;
            roll.montage = None;
        }
        app.update();
        let player = app.world().get::<AnimatedSceneInstance>(avatar).unwrap().player_entity();
        let graph = app.world().get::<AnimationGraphPlayer>(player).unwrap();
        let expected = graph.get_global_input_data(GENERATED_POST_LOCOMOTION_SLOT_WEIGHT_KEY).unwrap().as_f32().unwrap();
        assert!(expected > 0.0 && expected < 0.99, "The real pose must use a partial montage weight");
        let context = app.world().get::<crate::extensions::SourceAnimationInputs>(player).unwrap()
            .slot_input_context(GENERATED_POST_LOCOMOTION_SLOT_POSE_KEY).unwrap();
        assert_eq!(context.asset_samples.len(), 1);
        assert!((context.asset_samples[0].clock.weight - expected).abs() < 1e-6,
            "The roll sample used full notify weight for a partial pose");
    }

    #[test]
    fn browser_review_montage_interrupted_roll_tail_does_not_emit_footsteps() {
        let (mut app, character, _) = loaded_roll_character();
        let old = play_loaded_roll(&mut app, character);
        app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(0.05)));
        {
            let mut roll = app.world_mut().get_mut::<AlsRollState>(character).unwrap();
            roll.time = 0.65;
            roll.montage = None;
        }
        app.update();
        app.world_mut().resource_mut::<Messages<crate::animation_events::AlsAnimationEvent>>().clear();
        app.world_mut().entity_mut(character).insert(AlsNotifyLocomotionAction::None);
        app.world_mut().get_mut::<AlsInputCommand>(character).unwrap().roll_pressed = true;
        app.update();
        app.world_mut().get_mut::<AlsInputCommand>(character).unwrap().roll_pressed = false;
        assert!(app.world().get::<AlsRollTail>(old).is_some(), "The replaced montage must retain its pose tail");
        let footsteps = app.world_mut().resource_mut::<Messages<crate::animation_events::AlsAnimationEvent>>()
            .drain().filter(|event| event.character == character
                && event.source_asset.ends_with("A_Als_Roll.A_Als_Roll")
                && event.class_name == "AlsAnimNotify_FootstepEffects").count();
        assert_eq!(footsteps, 0, "The interrupted old montage emitted a graph footstep");
    }

    #[test]
    fn mantle_target_destruction_detaches_before_the_physics_handoff() {
        let (mut app, character, avatar) = loaded_roll_character();
        let target = app.world_mut().spawn(GlobalTransform::IDENTITY).id();
        let origin = app.world().get::<Transform>(character).unwrap().translation;
        let mut mantle = crate::als::mantle::AlsMantleState::default();
        mantle.begin(crate::als::mantle::MantleKind::Low, origin, origin + Vec3::Y, 0.75, 0.0, 0.0);
        mantle.root_motion_ready = true;
        mantle.target_base = Some(crate::als::mantle::AlsMantleTargetBase { entity: target,
            translation: Vec3::ZERO, rotation: Quat::IDENTITY, scale: Vec3::ONE, relative_location: false });
        app.world_mut().entity_mut(character).insert((mantle, LocomotionState::Custom,
            AlsNotifyLocomotionAction::Mantling));
        app.world_mut().despawn(target);
        app.add_systems(PostUpdate, (move |actions: Query<&AlsNotifyLocomotionAction>,
            instances: Query<&AnimatedSceneInstance>, parents: Query<&ChildOf>| {
                assert_eq!(actions.get(character).unwrap(), &AlsNotifyLocomotionAction::Ragdolling);
                let player = instances.get(avatar).unwrap().player_entity();
                assert!(parents.get(player).is_err(),
                    "Source target destruction must detach before activating the ragdoll physics bodies");
            }).after(crate::mantle_drive::AlsMantleDriveSet)
                .after(crate::extensions::RagdollPhysicsHandoffSet).before(crate::extensions::RagdollAngularDriveSet));
        app.update();
    }

    #[test]
    fn source_landing_ragdoll_uses_saved_speed_and_interrupts_get_up() {
        use crate::actual_motion::AlsActualMotion;
        for (saved_downward_speed, getting_up, expected_ragdoll) in [
            (9.999, false, false), (10.0, false, true), (10.001, false, true), (10.0, true, true),
        ] {
            let (mut app, character, _) = loaded_roll_character();
            for _ in 0..2 { app.update(); }
            *app.world_mut().get_mut::<LocomotionState>(character).unwrap() = LocomotionState::Air;
            app.world_mut().get_mut::<AlsActualMotion>(character).unwrap().world_velocity = Vec3::NEG_Y * saved_downward_speed;
            if getting_up {
                app.world_mut().entity_mut(character).insert((crate::als::motion::AlsMovementInputBlocked,
                    AlsNotifyLocomotionAction::GettingUp));
            }
            app.world_mut().run_system_once(crate::als::motion::locomotion_state_sync_system).unwrap();
            assert_eq!(app.world().get::<AlsNotifyLocomotionAction>(character) == Some(&AlsNotifyLocomotionAction::Ragdolling),
                expected_ragdoll, "Source landing callback must use saved speed and its inclusive threshold");
            assert_eq!(*app.world().get::<LocomotionState>(character).unwrap(),
                if expected_ragdoll { LocomotionState::None } else { LocomotionState::Grounded });
        }
    }

    #[test]
    fn source_high_speed_landing_starts_ragdoll_before_roll() {
        for (horizontal_position, scale, delta) in [(0.0, 1.0, 1.0_f32 / 60.0), (-15.0, 2.0, 1.0 / 60.0), (-15.0, 2.0, 0.0119366), (-15.0, 2.0, 0.0)] {
        let (mut app, character, avatar) = loaded_roll_character();
        app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(delta.max(0.0119366))));
        let drop_position = Vec3::new(horizontal_position, if horizontal_position == -15.0 { 10.824 } else { 9.0 }, horizontal_position);
        let drop_transform = Transform::from_translation(drop_position).with_scale(Vec3::splat(scale))
            .with_rotation(Quat::from_rotation_y(if horizontal_position == -15.0 { -std::f32::consts::FRAC_PI_4 } else { 0.0 }));
        app.world_mut().entity_mut(character).insert((
            avian3d::prelude::Position(drop_position), drop_transform,
            GlobalTransform::from(drop_transform),
            LinearVelocity(Vec3::ZERO),
        ));
        let mut entered_air = false;
        let mut landed = false;
        let mut maximum_downward_speed = 0.0_f32;
        for frame in 0..150 {
            if delta == 0.0 {
                let cadence = [0.0119366, 0.0136784, 0.0118618, 0.0123884, 0.015421, 0.0274669];
                *app.world_mut().resource_mut::<bevy::time::TimeUpdateStrategy>() = bevy::time::TimeUpdateStrategy::ManualDuration(
                    std::time::Duration::from_secs_f32(cadence[frame % cadence.len()]));
            }
            app.update();
            maximum_downward_speed = maximum_downward_speed.max(-app.world().get::<LinearVelocity>(character).unwrap().y);
            let locomotion = *app.world().get::<LocomotionState>(character).unwrap();
            entered_air |= locomotion == LocomotionState::Air;
            if entered_air && (locomotion == LocomotionState::Grounded
                || app.world().get::<AlsNotifyLocomotionAction>(character) == Some(&AlsNotifyLocomotionAction::Ragdolling)) {
                landed = true; break;
            }
        }
        assert!(entered_air && landed, "Actual source landing fixture must fall and reach the blocking floor");
        let impact = -app.world().get::<crate::als::air::AlsPreImpactVelocity>(character).unwrap().last_air_vertical_velocity;
        assert!(impact >= 10.0, "Actual impact must exceed the source ragdoll threshold: impact={impact} maximum={maximum_downward_speed}");
        let player = app.world().get::<AnimatedSceneInstance>(avatar).unwrap().player_entity();
        assert!(app.world().get::<ChildOf>(player).is_none(),
            "Source ragdoll entry must detach the visible mesh before the capsule follows its pelvis");
        for body in app.world().get::<AnimationGraphPlayer>(player).unwrap().get_spawned_ragdoll().unwrap().bodies.values() {
            let expected = app.world().get::<crate::extensions::RagdollActiveMode>(*body).unwrap().0;
            let expected = match expected {
                bevy_animation_graph::core::ragdoll::definition::BodyMode::Dynamic => avian3d::prelude::RigidBody::Dynamic,
                bevy_animation_graph::core::ragdoll::definition::BodyMode::Kinematic => avian3d::prelude::RigidBody::Kinematic,
            };
            assert_eq!(*app.world().get::<avian3d::prelude::RigidBody>(*body).unwrap(), expected,
                "Source ragdoll entry must activate each configured body mode in the same callback");
        }
        let scene = app.world().get::<bevy_animation_graph::core::animated_scene::AnimatedSceneHandle>(avatar).unwrap();
        let scene = app.world().resource::<Assets<bevy_animation_graph::core::animated_scene::AnimatedScene>>().get(&scene.handle).unwrap();
        let mapping = app.world().resource::<Assets<bevy_animation_graph::core::ragdoll::bone_mapping::RagdollBoneMap>>()
            .get(scene.ragdoll_bone_map.as_ref().unwrap()).unwrap();
        let animation = app.world().get::<AnimationGraphPlayer>(player).unwrap();
        for (id, body) in &animation.get_spawned_ragdoll().unwrap().bodies {
            let bone = &mapping.bodies_from_bones[id].bone;
            let global = app.world().get::<GlobalTransform>(animation.bone_entity(bone.bone.id()).unwrap()).unwrap();
            let expected = global.to_isometry() * bone.offset;
            let actual = app.world().get::<avian3d::prelude::Position>(*body).unwrap().0;
            assert!(Vec3::from(expected.translation).distance(actual) <= 1e-4,
                "Source entry body must match the visible bone before any physics step: scale={scale} delta={delta} label={:?} expected={:?} actual={actual:?} error={}",
                app.world().get::<bevy_animation_graph::core::ragdoll::definition::BodyLabel>(*body).map(|label| &label.0),
                Vec3::from(expected.translation), Vec3::from(expected.translation).distance(actual));
        }
        app.update();
        assert_eq!(app.world().get::<AlsNotifyLocomotionAction>(character), Some(&AlsNotifyLocomotionAction::Ragdolling),
            "Source high speed landing must start ragdoll before evaluating rolling");
        assert!(!app.world().get::<AlsRollState>(character).unwrap().active);
        for _ in 0..3 { app.update(); }
        let player = app.world().get::<AnimatedSceneInstance>(avatar).unwrap().player_entity();
        let profile = app.world().get::<crate::extensions::RagdollPhysicsProfileApplied>(player).unwrap();
        assert_eq!(profile.handoff_animated_body_count, 21);
        assert!(profile.handoff_animated_body_position_error_m <= 1e-4,
            "Actual high landing handoff must match the current visible pose: scale={scale} delta={delta} error={}",
            profile.handoff_animated_body_position_error_m);
        }
    }

    #[test]
    fn actual_get_up_action_rejects_source_native_jump_until_empty() {
        let (mut app, character, _) = loaded_roll_character();
        crate::als::input::spawn_als_actions(&mut app.world_mut().commands(), character, &crate::als::input::AlsInputMap::default());
        app.update();
        app.world_mut().entity_mut(character).insert(crate::get_up::AlsGetUpRequest(false));
        for _ in 0..4 { app.update(); }
        assert!(app.world().get::<crate::get_up::AlsGetUpState>(character).unwrap().pose_ready);
        assert_eq!(app.world().get::<AlsNotifyLocomotionAction>(character), Some(&AlsNotifyLocomotionAction::GettingUp));
        assert_eq!(app.world().get::<LocomotionState>(character), Some(&LocomotionState::Grounded));
        app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::Space);
        app.update();
        let velocity = app.world().get::<LinearVelocity>(character).unwrap().y;
        assert!(velocity.abs() < 0.02,
            "Source native Jump rejects any nonempty action even when input reaches Input_OnJump: velocity={velocity}, action={:?}", app.world().get::<AlsNotifyLocomotionAction>(character));
        assert_eq!(app.world().get::<LocomotionState>(character), Some(&LocomotionState::Grounded));
        app.world_mut().resource_mut::<ButtonInput<KeyCode>>().release(KeyCode::Space);
        let mut ended = false;
        for _ in 0..120 {
            app.update();
            if app.world().get::<AlsNotifyLocomotionAction>(character) == Some(&AlsNotifyLocomotionAction::None) { ended = true; break; }
        }
        assert!(ended);
        app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::Space);
        app.update();
        assert!(app.world().get::<LinearVelocity>(character).unwrap().y > 3.0,
            "Source empty action accepts a fresh native Jump after the loaded end notify");
    }

    #[test]
    fn loaded_get_up_ragdoll_replacement_keeps_source_input_block_until_action_empty() {
        let (mut app, character, _) = loaded_roll_character();
        app.world_mut().entity_mut(character).insert(crate::get_up::AlsGetUpRequest(false));
        for _ in 0..4 { app.update(); }
        assert!(app.world().get::<crate::get_up::AlsGetUpState>(character).unwrap().pose_ready);
        assert_eq!(app.world().get::<AlsNotifyLocomotionAction>(character), Some(&AlsNotifyLocomotionAction::GettingUp));
        assert!(app.world().get::<crate::als::motion::AlsMovementInputBlocked>(character).is_some());
        app.world_mut().get_mut::<crate::als::transition::AlsStateRequest>(character).unwrap().ragdoll_start = true;
        app.world_mut().get_mut::<AlsInputCommand>(character).unwrap().move_dir = Vec2::Y;
        for _ in 0..20 {
            app.update();
            assert_eq!(app.world().get::<AlsNotifyLocomotionAction>(character), Some(&AlsNotifyLocomotionAction::Ragdolling));
            assert!(!app.world().get::<crate::actual_motion::AlsActualMotion>(character).unwrap().has_input,
                "Source blocked ConsumeInputVector must publish no animation input during GetUp-to-Ragdoll replacement");
        }
        assert!(app.world().get::<crate::als::motion::AlsMovementInputBlocked>(character).is_some(),
            "Source action replacement is still valid and cannot release input blocked by GettingUp");
        app.world_mut().get_mut::<crate::als::transition::AlsStateRequest>(character).unwrap().ragdoll_exit = true;
        let mut released = false;
        for _ in 0..240 {
            app.update();
            if app.world().get::<AlsNotifyLocomotionAction>(character) == Some(&AlsNotifyLocomotionAction::None) {
                assert!(app.world().get::<crate::als::motion::AlsMovementInputBlocked>(character).is_none(),
                    "Source empty action must release the previous input block");
                app.update();
                assert!(app.world().get::<crate::actual_motion::AlsActualMotion>(character).unwrap().has_input,
                    "Source input must resume after the action is empty");
                released = true;
                break;
            }
        }
        assert!(released, "Actual public Ragdoll exit must complete the action and release input");
    }

    #[test]
    fn loaded_get_up_landing_cannot_queue_a_roll_for_its_end_notify() {
        let (mut app, character, _) = loaded_roll_character();
        app.world_mut().entity_mut(character).insert(crate::get_up::AlsGetUpRequest(false));
        for _ in 0..4 { app.update(); }
        assert!(app.world().get::<crate::get_up::AlsGetUpState>(character).unwrap().pose_ready);
        let mut approaching_end = false;
        for _ in 0..150 {
            let state = app.world().get::<crate::get_up::AlsGetUpState>(character).unwrap();
            if state.clip_time >= crate::get_up::get_up_settings(false).notify_end_clip_seconds
                - 2.0 * app.world().resource::<Time>().delta_secs() * crate::get_up::get_up_settings(false).play_rate {
                approaching_end = true;
                break;
            }
            app.update();
        }
        assert!(approaching_end, "Actual GetUp must approach its source end notify");
        assert_eq!(app.world().get::<AlsNotifyLocomotionAction>(character), Some(&AlsNotifyLocomotionAction::GettingUp));
        assert!(app.world().get::<crate::als::motion::AlsMovementInputBlocked>(character).is_some());
        *app.world_mut().get_mut::<LocomotionState>(character).unwrap() = LocomotionState::Air;
        app.world_mut().get_mut::<crate::actual_motion::AlsActualMotion>(character).unwrap().world_velocity = Vec3::NEG_Y * 8.0;
        app.world_mut().entity_mut(character).insert((
            AlsAirState { phase: crate::als::air::AirPhase::Airborne, last_air_vertical_velocity: -8.0, ..default() },
            crate::als::air::AlsPreImpactVelocity { last_air_vertical_velocity: -8.0 }));
        app.world_mut().run_system_once(crate::als::motion::locomotion_state_sync_system).unwrap();
        assert_eq!(*app.world().get::<LocomotionState>(character).unwrap(), LocomotionState::Grounded);
        let mut notify_released = false;
        for _ in 0..4 {
            app.update();
            assert!(!app.world().get::<AlsRollState>(character).unwrap().active,
                "Actual source GetUp end notify cannot replay a previously rejected landing roll");
            if app.world().get::<AlsNotifyLocomotionAction>(character) == Some(&AlsNotifyLocomotionAction::None) {
                notify_released = true;
                break;
            }
        }
        assert!(notify_released, "Actual GetUp end notify must release the source action");
    }

    #[test]
    fn loaded_medium_landing_roll_uses_the_source_saved_velocity_direction() {
        for input in [Vec2::ZERO, Vec2::X] {
        let (mut app, character, _) = loaded_roll_character();
        let position = Vec3::new(0.0, 4.824, 0.0);
        let transform = Transform::from_translation(position).with_rotation(Quat::from_rotation_y(0.4));
        app.world_mut().entity_mut(character).insert((avian3d::prelude::Position(position), transform,
            GlobalTransform::from(transform), LinearVelocity(Vec3::X * 0.2)));
        app.world_mut().get_mut::<AlsInputCommand>(character).unwrap().move_dir = input;
        let mut entered_air = false;
        let mut landed = false;
        let mut saved = Vec3::ZERO;
        let mut landing_actor_yaw = 0.0;
        for _ in 0..100 {
            let previous_mode = *app.world().get::<LocomotionState>(character).unwrap();
            let previous_velocity = app.world().get::<crate::actual_motion::AlsActualMotion>(character).unwrap().world_velocity;
            app.update();
            let mode = *app.world().get::<LocomotionState>(character).unwrap();
            entered_air |= mode == LocomotionState::Air;
            if entered_air && previous_mode == LocomotionState::Air && mode == LocomotionState::Grounded {
                saved = previous_velocity;
                landing_actor_yaw = app.world().get::<Transform>(character).unwrap().rotation.to_euler(EulerRot::YXZ).0;
                landed = true;
                break;
            }
        }
        assert!(landed && -saved.y >= 7.0 && -saved.y < 10.0,
            "Actual source medium landing fixture must reach the roll branch: {saved:?}");
        app.update();
        let roll = app.world().get::<AlsRollState>(character).unwrap();
        assert!(roll.active, "Source medium landing starts its loaded Roll montage");
        let horizontal = Vec3::new(saved.x, 0.0, saved.z);
        let expected = if horizontal.length() >= 0.01 {
            crate::als::orientation::world_forward_to_yaw(horizontal)
        } else { landing_actor_yaw };
        assert!(crate::als::roll::wrap_angle(roll.direction_yaw - expected).abs() < 1e-4,
            "Source landing Roll uses saved velocity yaw: expected={expected} actual={} saved={saved:?}", roll.direction_yaw);
        }
    }


    #[test]
    fn paused_floor_owner_does_not_adjust_the_actual_roll_actor() {
        let (mut app, character, _) = loaded_roll_character();
        play_loaded_roll(&mut app, character);
        let before = *app.world().get::<Transform>(character).unwrap();
        app.world_mut().resource_mut::<Time<Virtual>>().pause();
        app.world_mut().run_system_once(crate::als::motion::locomotion_state_sync_system).unwrap();
        assert_eq!(*app.world().get::<Transform>(character).unwrap(), before,
            "The source CMC floor owner does not tick while the world is paused");
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
    fn loaded_roll_late_rotation_refresh_matches_the_actual_actor_yaw() {
        let (mut app, character, _) = loaded_roll_character();
        app.world_mut().get_mut::<AlsInputCommand>(character).unwrap().move_dir = Vec2::X;
        for _ in 0..3 { app.update(); }
        assert!(app.world().get::<crate::actual_motion::AlsActualMotion>(character).unwrap().has_input);
        app.world_mut().get_mut::<AlsInputCommand>(character).unwrap().roll_pressed = true;
        app.world_mut().run_system_once(advance_rolls).unwrap();
        app.world_mut().get_mut::<AlsInputCommand>(character).unwrap().roll_pressed = false;
        for _ in 0..5 { app.update(); }
        assert!(app.world().get::<AlsRollSlotRuntime>(character).unwrap().pose_ready);
        let mut rotated = false;
        for _ in 0..8 {
            let before = app.world().get::<Transform>(character).unwrap().rotation;
            app.update();
            let actor = app.world().get::<Transform>(character).unwrap();
            rotated |= actor.rotation.dot(before).abs() < 1.0 - 1e-7;
            let yaw = crate::als::orientation::world_forward_to_yaw(*actor.forward());
            let state = app.world().get::<crate::als::motion::AlsRotationResponseState>(character).unwrap();
            assert!(crate::als::roll::wrap_angle(state.target_yaw - yaw).abs() < 1e-4
                && crate::als::roll::wrap_angle(state.smooth_target_yaw - yaw).abs() < 1e-4,
                "Source active Roll late refresh resets target and smooth yaw to its actual actor: actor={yaw} target={} smooth={}",
                state.target_yaw, state.smooth_target_yaw);
        }
        assert!(rotated, "Actual loaded Roll must change actor yaw to exercise late refresh");
    }

    #[test]
    fn loaded_roll_root_motion_keeps_moving_in_source_custom_mode() {
        let (mut app, character, _) = loaded_roll_character();
        let sampler = play_loaded_roll(&mut app, character);
        app.world_mut().get_mut::<crate::als::transition::AlsStateRequest>(character).unwrap().locomotion_state = Some(LocomotionState::Custom);
        app.update();
        assert_eq!(*app.world().get::<LocomotionState>(character).unwrap(), LocomotionState::Custom);
        let start = app.world().get::<avian3d::prelude::Position>(character).unwrap().0;
        for _ in 0..3 { app.update(); }
        assert!(app.world().get::<AlsRollState>(character).unwrap().root_motion_active());
        assert_eq!(app.world().get::<AlsNotifyLocomotionAction>(character), Some(&AlsNotifyLocomotionAction::Rolling));
        assert!(app.world().get::<RootMotionOutput>(sampler).unwrap().translation_delta.length() > 1e-5);
        let actual = app.world().get::<avian3d::prelude::Position>(character).unwrap().0 - start;
        assert!(Vec2::new(actual.x, actual.z).length() > 0.05,
            "Source PhysCustom keeps active animation root motion: displacement={actual:?}");
    }

    #[test]
    fn loaded_unlocked_custom_mode_accepts_public_air_requests() {
        for rolling in [false, true] {
            let (mut app, character, _) = loaded_roll_character();
            if rolling { play_loaded_roll(&mut app, character); }
            app.world_mut().get_mut::<crate::als::transition::AlsStateRequest>(character).unwrap().locomotion_state = Some(LocomotionState::Custom);
            app.update();
            assert_eq!(*app.world().get::<LocomotionState>(character).unwrap(), LocomotionState::Custom);
            let mut transform = *app.world().get::<Transform>(character).unwrap();
            transform.translation.y += 5.0;
            app.world_mut().entity_mut(character).insert((avian3d::prelude::Position(transform.translation),
                transform, GlobalTransform::from(transform)));
            app.world_mut().get_mut::<crate::als::transition::AlsStateRequest>(character).unwrap().locomotion_state = Some(LocomotionState::Air);
            app.update();
            assert_eq!(*app.world().get::<LocomotionState>(character).unwrap(), LocomotionState::Air,
                "Source Custom alone does not lock SetMovementMode: rolling={rolling}");
            assert!(app.world().get::<LinearVelocity>(character).unwrap().0.y < 0.0,
                "Accepted source Falling must resume actual gravity: rolling={rolling}");
        }
    }

    #[test]
    fn loaded_mantle_override_updates_velocity_below_minimum_without_moving() {
        use std::sync::{Arc, Mutex};
        for delta in [0.0_f32, 5.0e-9, 1.0e-8, 2.0e-8, 0.5e-6, 1.0e-6, 0.1] {
            let (mut app, character, _) = loaded_roll_character();
            let start = app.world().get::<Transform>(character).unwrap().translation;
            app.world_mut().get_mut::<crate::als::mantle::AlsMantleState>(character).unwrap().begin(
                crate::als::mantle::MantleKind::Low, start, start + Vec3::Y + Vec3::NEG_Z,
                0.75, 0.0, 0.0);
            for _ in 0..10 { app.update(); }
            let mantle = app.world().get::<crate::als::mantle::AlsMantleState>(character).unwrap();
            assert!(mantle.active && mantle.root_motion_ready);
            assert_eq!(*app.world().get::<LocomotionState>(character).unwrap(), LocomotionState::Custom);
            let before = app.world().get::<avian3d::prelude::Position>(character).unwrap().0;
            let previous_velocity = app.world().get::<LinearVelocity>(character).unwrap().0;
            assert!(previous_velocity.length() > 0.1);
            app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(delta)));
            let source_velocity = Arc::new(Mutex::new(Vec3::ZERO));
            let capture = source_velocity.clone();
            app.add_systems(PostUpdate, (move |actors: Query<&LinearVelocity>| {
                *capture.lock().unwrap() = actors.get(character).unwrap().0;
            }).after(crate::mantle_drive::AlsMantleDriveSet).before(crate::source_physics_timing::SourcePhysicsTimingSet::Clamp));
            app.update();
            let expected = if delta <= 1.0e-8 { previous_velocity } else { *source_velocity.lock().unwrap() };
            if delta >= 0.5e-6 {
                assert!(expected.length() > 0.1, "Actual Mantle must publish its current override velocity: delta={delta} velocity={expected:?}");
            }
            let velocity = app.world().get::<LinearVelocity>(character).unwrap().0;
            assert!(velocity.abs_diff_eq(expected, 1e-4),
                "Source Mantle override keeps its stored velocity at a cleared step: delta={delta} expected={expected:?} actual={velocity:?}");
            let movement = if delta < crate::als::movement_constants::MIN_MOVEMENT_DELTA_SECONDS { Vec3::ZERO } else { expected * delta };
            let actual = app.world().get::<avian3d::prelude::Position>(character).unwrap().0 - before;
            assert!(actual.abs_diff_eq(movement, 1e-7),
                "Source Mantle override velocity precedes StartNewPhysics minimum movement gate: delta={delta} expected={movement:?} actual={actual:?}");
        }
    }

    #[test]
    fn loaded_custom_roll_consumes_root_at_the_source_minimum_delta() {
        use std::sync::{Arc, Mutex};
        for delta in [0.5e-6_f32, 1.0e-6, 5.0e-6, 1.0e-5, 2.0e-5] {
            let (mut app, character, _) = loaded_roll_character();
            let sampler = play_loaded_roll(&mut app, character);
            app.world_mut().get_mut::<crate::als::transition::AlsStateRequest>(character).unwrap().locomotion_state = Some(LocomotionState::Custom);
            app.update();
            let mut transform = *app.world().get::<Transform>(character).unwrap();
            transform.translation.y += 5.0;
            app.world_mut().entity_mut(character).insert((avian3d::prelude::Position(transform.translation),
                transform, GlobalTransform::from(transform)));
            app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(delta)));
            let source_delta = Arc::new(Mutex::new(Vec3::ZERO));
            let capture = source_delta.clone();
            app.add_systems(PostUpdate, (move |samplers: Query<(&AlsRollSlotSampler, &RootMotionOutput)>,
                actors: Query<&AlsNotifyRootMotionScale>, transforms: bevy::transform::helper::TransformHelper| {
                let (sampler, root) = samplers.get(sampler).unwrap();
                let mesh = transforms.compute_global_transform(sampler.target_player).unwrap();
                let actor = transforms.compute_global_transform(character).unwrap();
                *capture.lock().unwrap() = root_motion_world_transform(actor, mesh,
                    root.translation_delta * actors.get(character).unwrap().0, root.rotation_delta).0;
            }).after(AnimationGraphSet::PrePhysics).before(AlsRollDriveSet));
            let before = transform.translation;
            app.update();
            let root = app.world().get::<RootMotionOutput>(sampler).unwrap().translation_delta;
            let expected = *source_delta.lock().unwrap();
            assert!(expected.length() > 1e-7, "Actual source animation must advance at its minimum Delta: delta={delta} root={root:?}");
            let velocity = app.world().get::<LinearVelocity>(character).unwrap().0;
            assert!(velocity.length() > 1e-3,
                "Source PhysCustom consumes active animation root at Delta >= 1us: delta={delta} root={root:?} velocity={velocity:?}");
            assert!(velocity.abs_diff_eq(expected / delta, 1e-3),
                "Source minimum Delta keeps the full root velocity: delta={delta} expected={:?} actual={velocity:?}", expected / delta);
            let actual = app.world().get::<avian3d::prelude::Position>(character).unwrap().0 - before;
            let movement = if delta < crate::als::movement_constants::MIN_MOVEMENT_DELTA_SECONDS { Vec3::ZERO } else { expected };
            assert!(actual.abs_diff_eq(movement, 1e-7),
                "Source StartNewPhysics skips movement below its minimum Delta: delta={delta} expected={movement:?} actual={actual:?}");
        }
    }

    #[test]
    fn loaded_custom_roll_keeps_crossing_frame_root_then_stops_in_the_tail() {
        use std::sync::{Arc, Mutex};
        for delta in [1.0_f32 / 60.0, 0.1] {
            let (mut app, character, _) = loaded_roll_character();
            let sampler = play_loaded_roll(&mut app, character);
            app.world_mut().get_mut::<crate::als::transition::AlsStateRequest>(character).unwrap().locomotion_state = Some(LocomotionState::Custom);
            app.update();
            let mut transform = *app.world().get::<Transform>(character).unwrap();
            transform.translation.y += 5.0;
            app.world_mut().entity_mut(character).insert((avian3d::prelude::Position(transform.translation),
                transform, GlobalTransform::from(transform)));
            app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(delta)));
            let root_velocity = Arc::new(Mutex::new(Vec3::ZERO));
            let capture = root_velocity.clone();
            app.add_systems(PostUpdate, (move |actors: Query<&LinearVelocity>| {
                *capture.lock().unwrap() = actors.get(character).unwrap().0;
            }).after(AlsRollDriveSet).before(crate::source_physics_timing::SourcePhysicsTimingSet::Clamp));
            let cutoff = crate::als::roll::ROLL_DURATION_SECS - crate::als::roll::ROLL_WEIGHT_OUT_SECS;
            let mut crossed = false;
            let mut tail = false;
            for _ in 0..80 {
                let before = *app.world().get::<AlsRollState>(character).unwrap();
                let position = app.world().get::<avian3d::prelude::Position>(character).unwrap().0;
                app.update();
                let after = *app.world().get::<AlsRollState>(character).unwrap();
                if before.time < cutoff && after.time >= cutoff {
                    crossed = true;
                    let expected = *root_velocity.lock().unwrap();
                    assert!(expected.length() > 1e-5);
                    assert!(app.world().get::<RootMotionOutput>(sampler).unwrap().translation_delta.length() > 1e-5);
                    assert!(app.world().get::<LinearVelocity>(character).unwrap().0.abs_diff_eq(expected, 1e-4),
                        "Source crossing BlendOut frame still consumes extracted root velocity: delta={delta} expected={expected:?} actual={:?}",
                        app.world().get::<LinearVelocity>(character).unwrap().0);
                } else if crossed && before.time >= cutoff && after.active {
                    tail = true;
                    assert!(app.world().get::<LinearVelocity>(character).unwrap().0.abs_diff_eq(Vec3::ZERO, 1e-5));
                    assert!(app.world().get::<avian3d::prelude::Position>(character).unwrap().0.abs_diff_eq(position, 1e-5));
                }
                if crossed && !after.active { break; }
            }
            assert!(crossed && tail, "Actual Roll must cover the crossing frame and relevant tail: delta={delta}");
        }
    }

    #[test]
    fn loaded_custom_roll_root_uses_the_full_source_frame_displacement() {
        use std::sync::{Arc, Mutex};
        for custom_position in [false, true] {
        for blocked in [false, true] {
        for delta in [1.0_f32 / 60.0, 0.1] {
            let (mut app, character, _) = loaded_roll_character();
            play_loaded_roll(&mut app, character);
            app.world_mut().get_mut::<crate::als::transition::AlsStateRequest>(character).unwrap().locomotion_state = Some(LocomotionState::Custom);
            app.update();
            let position = app.world().get::<avian3d::prelude::Position>(character).unwrap().0 + Vec3::Y * 5.0;
            let mut transform = *app.world().get::<Transform>(character).unwrap();
            transform.translation = position;
            app.world_mut().entity_mut(character).insert((avian3d::prelude::Position(position), transform, GlobalTransform::from(transform)));
            if custom_position {
                app.world_mut().entity_mut(character).insert(avian3d::prelude::CustomPositionIntegration);
            }
            if blocked {
                let wall = position + transform.forward().as_vec3() * 0.36;
                app.world_mut().spawn((avian3d::prelude::RigidBody::Static,
                    avian3d::prelude::Collider::cuboid(4.0, 4.0, 0.1),
                    Transform::from_translation(wall).with_rotation(transform.rotation),
                    avian3d::prelude::CollisionLayers::new(crate::als::character::AlsCollisionLayer::Default,
                        avian3d::prelude::LayerMask::ALL)));
            }
            app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(delta)));
            let sample = Arc::new(Mutex::new((None, None)));
            let capture_start = sample.clone();
            app.add_systems(PostUpdate, (move |actors: Query<&avian3d::prelude::Position>| {
                capture_start.lock().unwrap().0 = Some(actors.get(character).unwrap().0);
            }).before(AlsRollDriveSet).before(crate::source_physics_timing::SourcePhysicsTimingSet::Clamp));
            let capture = sample.clone();
            app.add_systems(PostUpdate, (move |actors: Query<(&avian3d::prelude::Position, &LinearVelocity)>, time: Res<Time>| {
                let (_, velocity) = actors.get(character).unwrap();
                capture.lock().unwrap().1 = Some(velocity.0 * time.delta_secs());
            }).after(AlsRollDriveSet).before(crate::source_physics_timing::SourcePhysicsTimingSet::Clamp));
            app.update();
            let sample = sample.lock().unwrap();
            let (before, expected) = (sample.0.unwrap(), sample.1.unwrap());
            assert!(expected.length() > 1e-4, "Actual loaded Roll must publish a nonzero root velocity");
            let actual = app.world().get::<avian3d::prelude::Position>(character).unwrap().0 - before;
            assert!(actual.abs_diff_eq(expected, 1e-4),
                "Source PhysCustom moves unswept by full root velocity times frame Delta: blocked={blocked} delta={delta} expected={expected:?} actual={actual:?}");
            let velocity = app.world().get::<LinearVelocity>(character).unwrap().0;
            assert!(velocity.abs_diff_eq(expected / delta, 1e-4),
                "Source PhysCustom preserves animation root velocity: blocked={blocked} delta={delta} expected={:?} actual={velocity:?}", expected / delta);
            assert_eq!(*app.world().get::<avian3d::prelude::RigidBody>(character).unwrap(), avian3d::prelude::RigidBody::Dynamic);
            assert!(app.world().get::<crate::als::motion::AlsCustomMovementPhysicsFrame>(character).is_none());
            assert_eq!(app.world().get::<avian3d::prelude::CustomPositionIntegration>(character).is_some(), custom_position);
            assert!(app.world().get::<avian3d::prelude::CustomVelocityIntegration>(character).is_none());
            drop(sample);
            if !blocked {
                let before = *app.world().get::<Transform>(character).unwrap();
                let roll_time = app.world().get::<AlsRollState>(character).unwrap().time;
                app.world_mut().resource_mut::<Time<Virtual>>().pause();
                for _ in 0..4 {
                    app.update();
                    assert_eq!(*app.world().get::<Transform>(character).unwrap(), before);
                    assert_eq!(app.world().get::<AlsRollState>(character).unwrap().time, roll_time);
                    assert!(app.world().get::<crate::als::motion::AlsCustomMovementPhysicsFrame>(character).is_none());
                }
                app.world_mut().resource_mut::<Time<Virtual>>().unpause();
                app.update();
                assert!(app.world().get::<AlsRollState>(character).unwrap().time > roll_time);
                assert_eq!(*app.world().get::<avian3d::prelude::RigidBody>(character).unwrap(), avian3d::prelude::RigidBody::Dynamic);
                assert_eq!(app.world().get::<avian3d::prelude::CustomPositionIntegration>(character).is_some(), custom_position);
                app.world_mut().entity_mut(character).remove::<avian3d::prelude::CustomPositionIntegration>();
                *app.world_mut().get_mut::<LocomotionState>(character).unwrap() = LocomotionState::Air;
                app.update();
                assert_eq!(*app.world().get::<LocomotionState>(character).unwrap(), LocomotionState::Air);
                assert!(app.world().get::<LinearVelocity>(character).unwrap().0.y < 0.0);
                assert!(app.world().get::<crate::als::motion::AlsCustomMovementPhysicsFrame>(character).is_none());
                assert!(app.world().get::<avian3d::prelude::CustomPositionIntegration>(character).is_none());
            }
        }
        }
        }
    }



    #[test]
    fn hidden_roll_reentry_uses_the_scale_without_slot_relevance() {
        use std::sync::{Arc, Mutex};
        let (mut app, character, avatar) = loaded_roll_character();
        app.world_mut().entity_mut(avatar).remove::<crate::animation_visibility::AlsAnimationTickOption>().insert(Visibility::Hidden);
        app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(0.1)));
        for _ in 0..16 { app.update(); }
        app.world_mut().get_mut::<AlsInputCommand>(character).unwrap().roll_pressed = true;
        let mut reached_component = false;
        for frame in 0..12 {
            app.update();
            if frame == 0 { app.world_mut().get_mut::<AlsInputCommand>(character).unwrap().roll_pressed = false; }
            let roll = app.world().get::<AlsRollState>(character).unwrap();
            if roll.time - app.world().resource::<Time>().delta_secs()
                >= crate::als::roll::ROLL_DURATION_SECS - crate::als::roll::ROLL_WEIGHT_OUT_SECS {
                reached_component = true;
                break;
            }
        }
        assert!(reached_component);
        let sampled = Arc::new(Mutex::new(None));
        let capture = sampled.clone();
        app.add_systems(PostUpdate, (move |time: Res<Time>, actors: Query<&LinearVelocity>,
            samplers: Query<(&AlsRollSlotSampler, &RootMotionOutput), Without<AlsRollTail>>,
            transforms: bevy::transform::helper::TransformHelper| {
            let (sampler, root) = samplers.iter().find(|(sampler, _)| sampler.character == character).unwrap();
            let actor = transforms.compute_global_transform(character).unwrap();
            let mesh = transforms.compute_global_transform(sampler.target_player).unwrap();
            let expected = root_motion_world_transform(actor, mesh, root.translation_delta, root.rotation_delta).0;
            *capture.lock().unwrap() = Some((actors.get(character).unwrap().0 * time.delta_secs(), expected));
        }).after(AlsRollDriveSet).before(crate::source_physics_timing::SourcePhysicsTimingSet::Clamp));
        app.world_mut().get_mut::<AlsInputCommand>(character).unwrap().roll_pressed = true;
        app.update();
        let (actual, expected) = sampled.lock().unwrap().unwrap();
        assert!(expected.length() > 1e-5);
        assert!(actual.distance(expected) < 1e-5,
            "Hidden reentry applied a queued scale without slot relevance: expected={expected:?}, actual={actual:?}");
    }

    #[test]
    fn first_roll_root_step_uses_the_scale_cached_before_notify_dispatch() {
        use std::sync::{Arc, Mutex};
        for hidden in [false, true] {
        let (mut app, character, avatar) = loaded_roll_character();
        if hidden {
            app.world_mut().entity_mut(avatar).remove::<crate::animation_visibility::AlsAnimationTickOption>()
                .insert(Visibility::Hidden);
            app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(0.1)));
            for _ in 0..16 { app.update(); }
            let player = app.world().get::<AnimatedSceneInstance>(avatar).unwrap().player_entity();
            assert!(!app.world().get::<crate::extensions::post_physics_animation::AnimationPoseUpdateGate>(player).unwrap().update_graph);
            app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(1.0 / 60.0)));
        } else {
            app.world_mut().entity_mut(avatar).insert(crate::animation_visibility::AlsAnimationTickOption::AlwaysTickPoseAndRefreshBones);
        }
        let cached = Arc::new(Mutex::new(1.0_f32));
        let before = cached.clone();
        app.add_systems(PostUpdate, (move |scales: Query<&AlsNotifyRootMotionScale>| {
            *before.lock().unwrap() = scales.get(character).unwrap().0;
        }).before(advance_rolls));
        let sampled = Arc::new(Mutex::new(None));
        let after = sampled.clone();
        app.add_systems(PostUpdate, (move |time: Res<Time>,
            actors: Query<&LinearVelocity>, samplers: Query<(&AlsRollSlotSampler, &RootMotionOutput)>,
            transforms: bevy::transform::helper::TransformHelper| {
            let (sampler, root) = samplers.iter().find(|(sampler, _)| sampler.character == character).unwrap();
            let actor = transforms.compute_global_transform(character).unwrap();
            let mesh = transforms.compute_global_transform(sampler.target_player).unwrap();
            let scale = *cached.lock().unwrap();
            let expected = root_motion_world_transform(actor, mesh, root.translation_delta * scale, root.rotation_delta).0;
            *after.lock().unwrap() = Some((actors.get(character).unwrap().0 * time.delta_secs(), expected));
        }).after(AlsRollDriveSet).before(crate::source_physics_timing::SourcePhysicsTimingSet::Clamp));
        app.world_mut().get_mut::<AlsInputCommand>(character).unwrap().roll_pressed = true;
        app.update();
        let (actual, expected) = sampled.lock().unwrap().unwrap();
        assert!(expected.length() > 1e-5, "Actual first Roll did not extract root motion");
        let expected_scale = if hidden { 1.0 } else { 1.3 };
        assert_eq!(app.world().get::<AlsNotifyRootMotionScale>(character).unwrap().0, expected_scale);
        assert!(actual.distance(expected) < 1e-5,
            "First root step consumed the post-notify scale: hidden={hidden}, expected={expected:?}, actual={actual:?}");
        }
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
            let consumed_scale = app.world().get::<AlsNotifyRootMotionScale>(character).unwrap().0;
            app.world_mut().resource_mut::<Time>().advance_by(std::time::Duration::from_secs_f32(0.01));
            app.world_mut().run_schedule(PostUpdate);
            let clip_time = app.world().get::<AlsRollState>(character).unwrap().clip_time();
            let expected_scale = if (0.2..0.5).contains(&clip_time) { 2.0 } else if (0.1..0.2).contains(&clip_time) { 0.5 } else { 1.0 };
            assert_eq!(app.world().get::<AlsNotifyRootMotionScale>(character).unwrap().0, expected_scale,
                "Active notify reclaimed scale at actual roll time {clip_time}");
            let raw = app.world().get::<RootMotionOutput>(sampler).unwrap().translation_delta;
            let mesh = app.world_mut().run_system_once(move |transforms: bevy::transform::helper::TransformHelper|
                transforms.compute_global_transform(target).unwrap()).unwrap();
            let expected = mesh.affine().transform_vector3(raw) * consumed_scale;
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
    fn installed_roll_sampler_matches_the_fresh_source_root_track() {
        use bevy::platform::collections::HashMap;
        use bevy_animation_graph::core::{
            animation_graph::AnimationGraph,
            context::{deferred_gizmos::DeferredGizmos, graph_context_arena::GraphContextArena, system_resources::SystemResources},
        };
        fn query(In((graph, actor, samples)): In<(Handle<AnimationGraph>, Entity, Vec<[f32; 4]>)>, resources: SystemResources) {
            let graph_asset = resources.animation_graph_assets.get(&graph).unwrap();
            let mut arena = GraphContextArena::new(graph.id());
            let mut previous = Vec3::ZERO;
            for [time, x, y, z] in samples {
                let globals = HashMap::from([(ROLL_SLOT_TIME_KEY.to_owned(), DataValue::F32(time))]);
                let output = graph_asset.query(bevy_animation_graph::core::animation_graph::TimeUpdate::Delta(0.0),
                    &mut arena, &resources, actor, &HashMap::new(), &mut DeferredGizmos::default(), &globals).unwrap();
                let pose = output["pose"].as_pose().unwrap();
                let current = Vec3::new(x, z, y) * 0.01;
                assert!(pose.root_motion.as_ref().unwrap().translation.distance(current - previous) < 1e-5,
                    "Installed Roll root interval differs at time {time}");
                let root = resources.skeleton_assets.get(&pose.skeleton).unwrap().iter_paths()
                    .find(|path| path.parts.last().is_some_and(|part| part.as_str() == "root")).unwrap().id();
                assert!(pose.get_bone(root).unwrap().translation.unwrap_or_default().length() < 1e-5,
                    "Installed Roll pose retained consumed root translation at time {time}");
                previous = current;
            }
        }
        let source: serde_json::Value = serde_json::from_str(include_str!("../../../../tools/fixtures/roll_root_source_samples.json")).unwrap();
        let samples: Vec<[f32; 4]> = serde_json::from_value(source["samples"].clone()).unwrap();
        let (mut app, character, _) = loaded_roll_character();
        let sampler = play_loaded_roll(&mut app, character);
        let graph = match app.world().get::<AnimationGraphPlayer>(sampler).unwrap().get_animation_source() {
            bevy_animation_graph::core::animation_graph_player::AnimationSource::Graph(graph) => graph.clone(),
            _ => panic!("Installed Roll sampler did not use its graph"),
        };
        app.world_mut().run_system_once_with(query, (graph, character, samples)).unwrap();
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
                        app.world_mut().get_mut::<AlsNotifyRootMotionScale>(character).unwrap().0 = notify_scale;
                        app.update();
                        let step = app.world().get::<RequestedRootStep>(character).unwrap();
                        let moved = app.world().get::<avian3d::prelude::Position>(character).unwrap().0 - step.start;
                        assert!(moved.with_y(0.0).distance(step.delta.with_y(0.0)) < 0.0001,
                            "Free actual roll changed displacement after collision: scale={mesh_scale} frame={frame} requested={:?} moved={moved:?}", step.delta);
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
    fn natural_roll_overlap_keeps_native_slot_source_quaternion_weight() {
        let (mut app, character, avatar) = loaded_roll_character();
        let player_entity = app.world().get::<AnimatedSceneInstance>(avatar).unwrap().player_entity();
        app.world_mut().get_mut::<AlsInputCommand>(character).unwrap().roll_pressed = true;
        app.update();
        app.world_mut().get_mut::<AlsInputCommand>(character).unwrap().roll_pressed = false;
        for _ in 0..90 {
            let roll = app.world().get::<AlsRollState>(character).unwrap();
            if roll.active && roll.time > 1.05
                && *app.world().get::<AlsNotifyLocomotionAction>(character).unwrap() == AlsNotifyLocomotionAction::None { break; }
            app.update();
        }
        let old = *app.world().get::<AlsRollState>(character).unwrap();
        assert!(old.active && old.time > 1.05);
        assert!(app.world().get::<AlsRollBlendOut>(character).is_none());
        app.world_mut().get_mut::<AlsInputCommand>(character).unwrap().roll_pressed = true;
        app.update();
        app.world_mut().get_mut::<AlsInputCommand>(character).unwrap().roll_pressed = false;
        let base = crate::mantle_drive::tests::current_post_locomotion_base(&app, player_entity);
        let current = *app.world().get::<AlsRollState>(character).unwrap();
        assert!(current.time < old.time);
        let mut samples = Vec::new();
        for (sampler, player, tail) in app.world_mut().query::<(&AlsRollSlotSampler, &AnimationGraphPlayer, Option<&AlsRollTail>)>().iter(app.world()) {
            if sampler.character != character || sampler.target_player != player_entity { continue; }
            let weight = tail.map_or_else(|| current.weight(), |tail| tail.weight());
            samples.push((player.get_default_output_pose().unwrap().clone(), weight));
        }
        assert_eq!(samples.len(), 2);
        let total: f32 = samples.iter().map(|(_,weight)|weight).sum();
        assert!(total > 0.0 && total < 1.0, "Actual Roll overlap must include Source: total={total}");
        let player = app.world().get::<AnimationGraphPlayer>(player_entity).unwrap();
        let overlay = player.get_global_input_data(GENERATED_POST_LOCOMOTION_SLOT_POSE_KEY).unwrap().as_pose().unwrap();
        let weight = player.get_global_input_data(GENERATED_POST_LOCOMOTION_SLOT_WEIGHT_KEY).unwrap().as_f32().unwrap();
        assert!((weight-total).abs() < 1e-6);
        let actual = crate::extensions::slot_node::blend_slot_pose(base.clone(), Some(overlay), Some(weight));
        samples.push((base,1.0-total));
        let expected = crate::extensions::source_pose_nodes::blend_source_poses(&samples);
        let mut maximum = 0.0_f32;
        for bone in actual.paths.keys() {
            if let (Some(a),Some(b))=(actual.get_bone(*bone).unwrap().rotation,expected.get_bone(*bone).unwrap().rotation) {
                maximum=maximum.max((a-b).length().min((a+b).length()));
            }
        }
        assert!(maximum < 1e-6, "Actual Roll overlap normalized before adding Source: error={maximum}");
    }

    #[test]
    fn stopped_roll_tail_accepts_a_new_loaded_montage() {
        use crate::{
            animation_events::AlsAnimationEvent,
            extensions::{EventPhase, SourceAnimationInputs},
        };

        for explicit_stop in [false, true] {
            for delta in [1.0 / 60.0_f32, 0.1] {
                let (mut app, character, _) = loaded_roll_character();
                let old = play_loaded_roll(&mut app, character);
                let old_roll = AlsRollState { active: true, time: if explicit_stop { 0.4 } else { 0.95 }, ..default() };
                app.world_mut().entity_mut(character).insert((old_roll, AlsNotifyLocomotionAction::Rolling));
                if explicit_stop {
                    app.world_mut().entity_mut(character).insert(AlsRollBlendOut::new(&old_roll, 0.2));
                } else {
                    app.world_mut().resource_mut::<Time>().advance_by(std::time::Duration::ZERO);
                    app.world_mut().run_schedule(PostUpdate);
                    assert_eq!(app.world().get::<AlsNotifyLocomotionAction>(character), Some(&AlsNotifyLocomotionAction::None));
                }
                app.world_mut().resource_mut::<Messages<AlsAnimationEvent>>().clear();
                app.world_mut().get_mut::<AlsInputCommand>(character).unwrap().roll_pressed = true;
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
                let expected_tail = ((old_roll.time + delta) * ROLL_PLAY_RATE).min(ROLL_MONTAGE_SEQUENCE_LENGTH_SECS);
                assert!((tail.clip_time - expected_tail).abs() < 1e-6);
                if delta < 0.1 { assert!(tail.weight() > 0.0, "Restart cleared the old pose prematurely"); }
                app.world_mut().entity_mut(character).insert((avian3d::prelude::RigidBody::Kinematic, bevy_tnua::TnuaToggle::Disabled));
                app.world_mut().run_system_once(prepare_roll_slot_samples).unwrap();
                assert!((app.world().get::<AnimationGraphPlayer>(old).unwrap().get_global_input_data(ROLL_SLOT_TIME_KEY).unwrap().as_f32().unwrap()
                    - expected_tail).abs() < 1e-6);
                let target = app.world().get::<AlsRollSlotSampler>(current).unwrap().target_player;
                let mut tail_visible = false;
                let mut action = AlsNotifyLocomotionAction::Rolling;
                let mut new_began = false;
                let mut old_end_count = 0;
                for frame in 0..10 {
                    app.world_mut().resource_mut::<Time>().advance_by(std::time::Duration::from_secs_f32(1.0 / 60.0));
                    app.world_mut().run_schedule(PostUpdate);
                    let runtime = app.world().get::<AlsRollSlotRuntime>(character).unwrap();
                    assert!(runtime.pose_ready && runtime.active);
                    assert!((app.world().get::<AlsNotifyRootMotionScale>(character).unwrap().0 - 1.3).abs() < 1e-6,
                        "Roll reentry lost the active source root-motion scale at frame {frame}");
                    let root = app.world().get::<RootMotionOutput>(current).unwrap();
                    assert!(runtime.root_motion_delta.distance(root.translation_delta) < 1e-6, "Old root motion won frame {frame}");
                    if let Some(tail) = app.world().get::<AlsRollTail>(old) {
                        tail_visible |= tail.weight() > 0.0;
                        if tail.weight() > 0.0 {
                            let inputs = app.world().get::<SourceAnimationInputs>(target).unwrap();
                            let context = inputs.slot_input_context("roll.pose").unwrap();
                            assert!(context.asset_samples.iter().any(|sample| sample.identity == format!("roll:{}", old.to_bits())), "Old pose was omitted from the blend");
                            assert!(context.asset_samples.iter().any(|sample| sample.identity == format!("roll:{}", current.to_bits())));
                            let current_pose = app.world().get::<AnimationGraphPlayer>(current).unwrap().get_default_output_pose().unwrap();
                            let old_pose = app.world().get::<AnimationGraphPlayer>(old).unwrap().get_default_output_pose().unwrap();
                            let mixed = app.world().get::<AnimationGraphPlayer>(target).unwrap().get_global_input_data("roll.pose").unwrap().as_pose().unwrap();
                            let current_weight = app.world().get::<AlsRollState>(character).unwrap().weight();
                            let total = current_weight + tail.weight();
                            for (bone, index) in &current_pose.paths {
                                let Some(old_index) = old_pose.paths.get(bone) else { continue; };
                                let Some(mixed_index) = mixed.paths.get(bone) else { panic!("Blended bone is missing"); };
                                let new_bone = &current_pose.bones[*index];
                                let old_bone = &old_pose.bones[*old_index];
                                let actual = &mixed.bones[*mixed_index];
                                if let (Some(new), Some(old), Some(actual)) = (new_bone.rotation, old_bone.rotation, actual.rotation) {
                                    let old = if new.dot(old) < 0.0 { -old } else { old };
                                    let expected = new * (current_weight / total) + old * (tail.weight() / total);
                                    let actual = Vec4::from_array(actual.to_array());
                                    let expected = Vec4::from_array(expected.to_array());
                                    assert!((actual - expected).length().min((actual + expected).length()) < 1e-6, "Montage rotation did not preserve source raw weighted accumulation");
                                }
                                if let (Some(new), Some(old), Some(actual)) = (new_bone.translation, old_bone.translation, actual.translation) {
                                    let expected = (new * current_weight + old * tail.weight()) / total;
                                    assert!(actual.distance(expected) < 1e-5, "Old montage translation was not blended");
                                }
                            }
                        }
                    }
                    for event in app.world_mut().resource_mut::<Messages<AlsAnimationEvent>>().drain()
                        .filter(|event| event.character == character && event.class_name == "AlsAnimNotifyState_SetLocomotionAction") {
                        if event.instance_id == old.to_bits() && event.phase == EventPhase::End {
                            old_end_count += 1;
                            action = AlsNotifyLocomotionAction::None;
                        }
                        if event.instance_id == current.to_bits() && event.phase == EventPhase::Begin {
                            new_began = true;
                            action = AlsNotifyLocomotionAction::Rolling;
                        }
                    }
                    assert_eq!(app.world().get::<AlsNotifyLocomotionAction>(character), Some(&action));
                    assert_eq!(app.world().get::<SourceAnimationInputs>(target).unwrap().gameplay_tag_input("locomotion_action"), Some(action.tag()));
                }
                assert!(new_began);
                assert_eq!(old_end_count, usize::from(explicit_stop));
                if explicit_stop && delta < ROLL_WEIGHT_IN_SECS {
                    assert_eq!(action, AlsNotifyLocomotionAction::None);
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
        world.init_resource::<Messages<crate::als::transition::StateTransitionEvent>>();
        let mut time = Time::<()>::default();
        time.advance_by(std::time::Duration::from_secs_f32(0.05));
        world.insert_resource(time);
        let roll = AlsRollState { active: true, time: 0.1, ..default() };
        let character = world.spawn((crate::als::character::AlsCharacter, roll, Transform::default(), LinearVelocity::ZERO)).id();
        let mesh = world.spawn((ChildOf(character), Transform::from_rotation(Quat::from_rotation_y(std::f32::consts::PI)))).id();
        for old in [old_first, !old_first] {
            let entity = world.spawn((AlsRollSlotSampler { character, target_player: mesh, reported_unready: false },
                RootMotionOutput { translation_delta: Vec3::NEG_Z * if old { 10.0 } else { 0.2 }, ..default() })).id();
            if old { world.entity_mut(entity).insert(AlsRollTail { clip_time: ROLL_PLAY_RATE, blend: AlsRollBlendOut::new(&roll, 0.1) }); }
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
        world.init_resource::<Messages<crate::als::transition::StateTransitionEvent>>();
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
            let expected = Vec3::Z * app.world().get::<AlsNotifyRootMotionScale>(character).unwrap().0;
            app.world_mut().resource_mut::<Time>().advance_by(std::time::Duration::from_secs_f32(delta));
            app.world_mut().run_schedule(PostUpdate);
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
        world.init_resource::<Messages<crate::als::transition::StateTransitionEvent>>();
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
        assert!(roll.active && roll.weight() > 0.0);
    }

    #[test]
    fn mantling_notify_blocks_roll_until_the_action_is_released() {
        let mut world = World::new();
        world.init_resource::<Messages<crate::als::transition::StateTransitionEvent>>();
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
        world.init_resource::<Messages<crate::als::transition::StateTransitionEvent>>();
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
        world.init_resource::<Messages<crate::als::transition::StateTransitionEvent>>();
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
    pub root_motion_scale: f32,
    pub(crate) animation_locomotion: Option<RollAnimationLocomotion>,
    pub pose_standing: f32,
    pub pose_grounded: f32,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct RollAnimationLocomotion {
    pub speed: f32,
    pub has_input: bool,
}

#[derive(Resource)]
struct RollSlotCurveHandle(Handle<AlsAnimCurves>, Handle<GraphClip>);

#[derive(Component, Debug, Clone, Copy)]
pub(crate) struct AlsRollBlendOut {
    blend: MontageBlend,
    previous_weight: f32,
}

impl AlsRollBlendOut {
    pub(crate) fn new(roll: &AlsRollState, duration: f32) -> Self {
        let weight = roll.weight();
        Self {
            blend: MontageBlend::new(weight, 0.0, duration, AlphaBlendMode::HermiteCubic),
            previous_weight: weight,
        }
    }

    pub(crate) fn weight(&self) -> f32 {
        self.blend.weight()
    }

    pub(crate) fn previous_weight(&self) -> f32 {
        self.previous_weight
    }

    fn advance(&mut self, delta: f32) {
        self.previous_weight = self.weight();
        self.blend.advance(delta);
    }

    pub(crate) fn stop(&mut self, duration: f32) {
        if duration < self.blend.duration {
            self.blend = MontageBlend::new(self.weight(), 0.0, duration, AlphaBlendMode::HermiteCubic);
        }
    }
}

pub struct AlsRollDrivePlugin;

#[derive(Component)]
struct AlsRollInputHandled;

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AlsRollDriveSet;

impl Plugin for AlsRollDrivePlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<crate::als::transition::StateTransitionEvent>().add_systems(Startup, load_roll_slot_curve_asset)
            .add_observer(discard_roll_target)
            .add_systems(Update, (ensure_roll_state, start_roll_input).chain()
                .after(crate::als::motion::locomotion_drive_system)
                .before(crate::als::transition::state_transition_system))
            .add_systems(PostUpdate, ensure_roll_slot_samplers.after(ensure_roll_state).before(advance_rolls))
            .configure_sets(
                PostUpdate,
                AlsRollDriveSet.after(AnimationGraphSet::PrePhysics),
            )
            .add_systems(
                PostUpdate,
                (
                    advance_rolls.after(ensure_roll_state).after(crate::animation_visibility::AlsAnimationVisibilitySet)
                        .before(prepare_roll_slot_samples),
                    prepare_roll_slot_samples.before(AnimationGraphSet::PrePhysics),
                    ensure_root_motion_output,
                    ensure_roll_state,
                    bridge_roll_slot_samples
                        .after(AnimationGraphSet::PrePhysics)
                        .before(crate::animation_events::AlsAnimationEventSet::Dispatch)
                        .before(crate::extensions::post_physics_animation::PostPhysicsAnimationSet::Evaluate),
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

fn start_roll_input(
    mut commands: Commands,
    transforms: bevy::transform::helper::TransformHelper,
    blocked: Query<(), With<crate::als::motion::AlsMovementInputBlocked>>,
    actions: Query<&AlsNotifyLocomotionAction>,
    blend_outs: Query<&AlsRollBlendOut>,
    samplers: Query<(Entity, &AlsRollSlotSampler, &AnimationGraphPlayer), Without<AlsRollTail>>,
    mut characters: Query<(Entity, &mut AlsRollState, &LocomotionState, &Stance, &AlsViewYaw,
        Option<&crate::als::view::AlsPlayerViewYaw>, &AlsInputCommand, Option<&AlsInputOverride>,
        Option<&crate::actual_motion::AlsActualMotion>), With<crate::als::character::AlsCharacter>>,
) {
    for (character, mut roll, locomotion, stance, view, player_view, cmd, override_input, motion) in &mut characters {
        let input = *override_input.map(|value| &value.0).unwrap_or(cmd);
        if !input.roll_pressed { continue; }
        commands.entity(character).insert(AlsRollInputHandled);
        let action = actions.get(character).ok().copied().unwrap_or(AlsNotifyLocomotionAction::None);
        let action_blocked = !matches!(action, AlsNotifyLocomotionAction::None | AlsNotifyLocomotionAction::Rolling);
        let effective = crate::als::motion::movement_input_for_action(input, blocked.contains(character) || action_blocked);
        if !effective.roll_pressed { continue; }
        let Ok(world) = transforms.compute_global_transform(character) else { continue };
        let actor_yaw = world.rotation().to_euler(EulerRot::YXZ).0;
        let input_yaw = motion.and_then(|value| value.has_input.then_some(value.input_yaw_degrees.to_radians()));
        let direction_basis = AlsViewYaw(if input_yaw.is_some() { player_view.map_or(view.0, |value| value.0) } else { actor_yaw });
        let previous = *roll;
        if action == AlsNotifyLocomotionAction::None && *locomotion == LocomotionState::Grounded { roll.active = false; }
        let stopped_blend = blend_outs.get(character).ok().copied();
        if stopped_blend.is_some() && *locomotion == LocomotionState::Grounded { roll.active = false; }
        let started = roll.update(&effective, select_roll_direction(input_yaw, &direction_basis), locomotion, stance, 0.0).started;
        if !started { continue; }
        if previous.active {
            let mut blend = stopped_blend.unwrap_or_else(|| AlsRollBlendOut::new(&previous, previous.remaining_play_time()));
            blend.stop(crate::als::roll::ROLL_WEIGHT_IN_SECS);
            for (entity, sampler, player) in &samplers {
                if sampler.character != character { continue; }
                commands.entity(entity).insert(AlsRollTail::from_roll(&previous, blend));
                let bevy_animation_graph::core::animation_graph_player::AnimationSource::Graph(graph) = player.get_animation_source() else { continue };
                commands.spawn((Name::new("als_roll_slot_sampler"), Children::default(), ChildOf(character),
                    AnimationGraphPlayer::new(player.skeleton().clone()).with_graph(graph.clone()), RootMotionOutput::default(), *sampler));
            }
            commands.entity(character).remove::<AlsRollBlendOut>();
        }
        commands.entity(character).insert(AlsNotifyLocomotionAction::Rolling);
        if let Some(stance) = crate::animation_events::physical_stance_override(AlsNotifyLocomotionAction::Rolling) {
            commands.entity(character).insert(stance);
        }
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
    event: On<Discard<AnimationGraphPlayer>>,
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

pub(crate) fn prepare_roll_slot_samples(
    rolls: Query<&AlsRollState, With<crate::als::character::AlsCharacter>>,
    mut samplers: Query<(&AlsRollSlotSampler, &mut AnimationGraphPlayer, Option<&AlsRollTail>)>,
) {
    for (sampler, mut player, tail) in &mut samplers {
        let clip_time = tail.map(|tail| tail.clip_time).unwrap_or_else(|| rolls
            .get(sampler.character)
            .ok()
            .filter(|roll| roll.active)
            .map(AlsRollState::clip_time)
            .unwrap_or(0.0));
        player.set_global_input_data(ROLL_SLOT_TIME_KEY, clip_time);
    }
}

fn bridge_roll_slot_samples(
    time: Res<Time>,
    mut source_inputs: Query<&mut crate::extensions::SourceAnimationInputs>,
    rolls: Query<&AlsRollState, With<crate::als::character::AlsCharacter>>,
    get_ups: Query<&crate::get_up::AlsGetUpState>,
    blend_outs: Query<&AlsRollBlendOut>,
    root_scales: Query<&AlsNotifyRootMotionScale>,
    motion: Query<(&crate::actual_motion::AlsActualMotion, Option<&AlsNotifyLocomotionAction>,
        Option<&crate::als_camera::AlsCameraCustomTimeDilation>)>,
    curve_handle: Res<RollSlotCurveHandle>,
    curves: Res<Assets<AlsAnimCurves>>,
    clips: Res<Assets<GraphClip>>,
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
            let (clip_time, weight) = if let Some(tail) = tail { (tail.clip_time, tail.weight()) }
                else if let Some(roll) = roll {
                    root_motion = RootMotionOutput { translation_delta: output.translation_delta, rotation_delta: output.rotation_delta };
                    (roll.clip_time(), blend_outs.get(character).map_or_else(|_| roll.weight(), |blend| blend.weight()))
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
            let mut context = curves.get(&curve_handle.0).and_then(AlsAnimCurves::first)
                .map(|curves| crate::get_up::sampled_curve_context(clips.get(&curve_handle.1), curves, clip_time))
                .unwrap_or_default();
            let previous_position = roll.filter(|_| tail.is_none()).map_or(clip_time, AlsRollState::previous_clip_time);
            context.asset_samples.push(crate::extensions::animation_context::SourceAssetSample {
                identity: format!("roll:{}", entity.to_bits()), clip: curve_handle.1.clone(), montage: true,
                interrupted: tail.is_some() || blend_outs.contains(character),
                clock: crate::extensions::SourceRelevantAsset { duration: ROLL_MONTAGE_SEQUENCE_LENGTH_SECS,
                    position: clip_time, previous_position, has_previous_position: true,
                    playback_delta_seconds: clip_time - previous_position, weight, ..default() }, ..default()
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
        let pose = pose.map(|mut pose| {
            pose.root_motion = primary_root;
            pose
        });
        let context = pose.as_ref().map(|_| crate::extensions::source_pose_nodes::blend_animation_contexts(
            samples.iter().map(|(_, context, weight, _)| (context.clone(), weight / total_weight.max(f32::EPSILON)))));
        let weight = total_weight.clamp(0.0, 1.0);
        let pose_ready = pose.is_some();
        if let Some(pose) = &pose { target_player.set_global_input_data("roll.pose", DataValue::Pose(pose.clone())); }
        target_player.set_global_input_data("roll.weight", weight);
        let mut target_inputs = source_inputs.get_mut(target).unwrap();
        target_inputs.set_slot_input_context("roll.pose", context.clone());
        let curve_values = ROLL_SLOT_CURVE_NAMES.map(|name| context.as_ref().and_then(|context| context.curves.get(&crate::extensions::SourceCurveName::from(name))).copied().unwrap_or_default());
        for (name, value) in ROLL_SLOT_CURVE_NAMES.into_iter().zip(curve_values) {
            target_player.set_global_input_data(format!("{ROOT_POST_LOCOMOTION_SLOT_CURVE_PREFIX}{name}"), value);
        }
        if let Some(get_up) = get_ups.get(character).ok().filter(|get_up| get_up.pose_ready) {
            if let Some(roll_pose) = pose.as_ref()
                && let Some(get_up_pose) = target_player.get_global_input_data(GENERATED_POST_LOCOMOTION_SLOT_POSE_KEY)
                    .and_then(|value| value.as_pose().ok()).cloned()
            {
                let total = get_up.weight + weight;
                let alpha = weight / total;
                let mut mixed = get_up_pose.scalar_mult(1.0 - alpha).linear_add(&roll_pose.scalar_mult(alpha));
                mixed.root_motion = crate::extensions::normalized_blendspace::blend_normalized_root_motion(&[
                    (roll_pose.root_motion.clone(), alpha),
                    (get_up_pose.root_motion.clone(), 1.0 - alpha),
                ]);
                target_player.set_global_input_data(GENERATED_POST_LOCOMOTION_SLOT_POSE_KEY, DataValue::Pose(mixed));
                target_player.set_global_input_data(GENERATED_POST_LOCOMOTION_SLOT_WEIGHT_KEY, total.min(1.0));
                if let (Some(get_up_context), Some(roll_context)) =
                    (target_inputs.slot_input_context(GENERATED_POST_LOCOMOTION_SLOT_POSE_KEY), context)
                {
                    let mixed_context = crate::extensions::source_pose_nodes::blend_animation_contexts([
                        (get_up_context, 1.0 - alpha),
                        (roll_context, alpha),
                    ]);
                    target_inputs.set_slot_input_context(GENERATED_POST_LOCOMOTION_SLOT_POSE_KEY, Some(mixed_context));
                }
            }
        } else {
            target_inputs.set_slot_input_context(GENERATED_POST_LOCOMOTION_SLOT_POSE_KEY, context);
            if let Some(pose) = pose {
                target_player.set_global_input_data(ROOT_POST_LOCOMOTION_SLOT_POSE_KEY, DataValue::Pose(pose.clone()));
                target_player.set_global_input_data(GENERATED_POST_LOCOMOTION_SLOT_POSE_KEY, DataValue::Pose(pose));
            }
            for key in [ROOT_POST_LOCOMOTION_SLOT_WEIGHT_KEY, ROOT_POST_LOCOMOTION_SLOT_LAYER_WEIGHT_KEY, GENERATED_POST_LOCOMOTION_SLOT_WEIGHT_KEY] {
                target_player.set_global_input_data(key, weight);
            }
            target_player.set_global_input_data(GENERATED_POST_LOCOMOTION_SLOT_LAYER_WEIGHT_KEY, if pose_ready { 1.0_f32 } else { 0.0 });
        }
        let animation_locomotion = motion.get(character).ok().and_then(|(motion, action, dilation)| {
            let delta = time.delta_secs() * dilation.map_or(1.0, |value| value.0);
            (delta >= crate::als::movement_constants::MIN_MOVEMENT_DELTA_SECONDS
                && roll.is_some_and(AlsRollState::root_motion_active_at_frame_start)
                && !blend_outs.contains(character)
                && action != Some(&AlsNotifyLocomotionAction::Ragdolling))
                .then_some(RollAnimationLocomotion { speed: motion.horizontal_speed, has_input: motion.has_input })
        });
        commands.entity(character).insert(AlsRollSlotRuntime {
            active: roll.is_some() || samples.iter().any(|(_, _, _, current)| !current), weight, pose_ready,
            root_motion_delta: root_motion.translation_delta, root_motion_rotation: root_motion.rotation_delta,
            root_motion_scale: root_scales.get(character).map_or(1.0, |scale| scale.0),
            animation_locomotion,
            pose_standing: curve_values[0], pose_grounded: curve_values[1],
        });
    }
}

#[allow(clippy::type_complexity)]
fn advance_rolls(
    time: Res<Time>,
    mut commands: Commands,
    transform_helper: bevy::transform::helper::TransformHelper,
    custom_dilations: Query<&crate::als_camera::AlsCameraCustomTimeDilation>,
    mut mode_changes: MessageReader<crate::als::transition::StateTransitionEvent>,
    blocked: Query<(), With<crate::als::motion::AlsMovementInputBlocked>>,
    actions: Query<&crate::animation_events::AlsNotifyLocomotionAction>,
    mut blend_outs: Query<&mut AlsRollBlendOut>,
    mut tails: Query<(Entity, &mut AlsRollTail, &AlsRollSlotSampler)>,
    samplers: Query<(Entity, &AlsRollSlotSampler, &AnimationGraphPlayer), Without<AlsRollTail>>,
    mut q: Query<
        (
            Entity,
            &mut AlsRollState,
            &LocomotionState,
            &Stance,
            (&AlsViewYaw, Option<&crate::als::view::AlsPlayerViewYaw>),
            &Transform,
            Option<&AlsAirState>,
            &AlsInputCommand,
            Option<&AlsInputOverride>,
            Option<&crate::actual_motion::AlsActualMotion>,
            Has<AlsRollInputHandled>,
        ),
        With<crate::als::character::AlsCharacter>,
    >,
) {
    let landings = mode_changes.read().filter(|event|
        event.dimension == crate::als::transition::Dimension::LocomotionState
            && event.from == crate::als::transition::StateValue::LocomotionState(LocomotionState::Air)
            && event.to == crate::als::transition::StateValue::LocomotionState(LocomotionState::Grounded))
        .map(|event| event.entity).collect::<Vec<_>>();
    for (entity, mut tail, sampler) in &mut tails {
        let dt = time.delta_secs() * custom_dilations.get(sampler.character).map_or(1.0, |value| value.0);
        tail.advance(dt);
        if tail.weight() <= f32::EPSILON { commands.entity(entity).try_despawn(); }
    }
    for (
        character,
        mut roll,
        loco,
        stance,
        (view, player_view),
        _transform,
        air,
        cmd,
        ovr,
        motion,
        input_handled,
    ) in &mut q
    {
        if input_handled { commands.entity(character).remove::<AlsRollInputHandled>(); }
        let Ok(actor_world) = transform_helper.compute_global_transform(character) else { continue; };
        let actor_yaw = actor_world.rotation().to_euler(EulerRot::YXZ).0;
        let dt = time.delta_secs() * custom_dilations.get(character).map_or(1.0, |value| value.0);
        let action_blocked = actions.get(character).is_ok_and(|action| !matches!(*action,
            crate::animation_events::AlsNotifyLocomotionAction::None | crate::animation_events::AlsNotifyLocomotionAction::Rolling));
        let mut effective = crate::als::motion::movement_input_for_action(
            *ovr.map(|o| &o.0).unwrap_or(cmd),
            blocked.contains(character) || action_blocked,
        );
        if input_handled { effective.roll_pressed = false; }

        let previous_roll = *roll;
        let landed = landings.contains(&character);
        let landing_speed = if landed {
            motion.map_or(0.0, |state| (-state.world_velocity.y).max(0.0))
        } else { air.map_or(0.0, |air| air.last_impact_vy) };
        let landing_ready = (landed || (*loco == LocomotionState::Grounded
            && air.is_some_and(|air| air.landing_time <= dt.max(0.05))))
            && !blocked.contains(character) && !action_blocked
            && !(crate::get_up::RAGDOLL_SETTINGS.start_on_land
                && landing_speed >= crate::get_up::RAGDOLL_SETTINGS.on_land_speed_threshold_mps)
            && landing_speed >= crate::als::roll::ROLL_ON_LAND_SPEED_THRESHOLD;
        let empty_action = actions.get(character).ok().is_none_or(|action| *action == AlsNotifyLocomotionAction::None);
        if empty_action && *loco == LocomotionState::Grounded && effective.roll_pressed { roll.active = false; }
        let mut stopped_blend = None;
        if let Ok(mut blend) = blend_outs.get_mut(character) {
            stopped_blend = Some(*blend);
            roll.advance_playback(dt, false);
            blend.advance(dt);
            if blend.weight() <= 0.0 {
                roll.active = false;
                commands.entity(character).remove::<AlsRollBlendOut>();
            }
            if !(*loco == LocomotionState::Grounded && effective.roll_pressed) && !landing_ready { continue; }
            roll.active = false;
        }

        let input_yaw = motion.and_then(|motion| motion.has_input.then_some(motion.input_yaw_degrees.to_radians()));
        let direction_basis = AlsViewYaw(if input_yaw.is_some() {
            player_view.map_or(view.0, |view| view.0)
        } else { actor_yaw });
        let mut lifecycle = roll.update(&effective, select_roll_direction(input_yaw, &direction_basis), loco, stance, dt);
        if !lifecycle.started
            && (empty_action || stopped_blend.is_some() || !previous_roll.root_motion_active())
            && landing_ready
        {
            let velocity_yaw = motion.and_then(|motion|
                crate::als::motion::locomotion_movement_flags(motion.has_input, motion.horizontal_speed).0
                    .then_some(motion.velocity_yaw_degrees.to_radians()));
            lifecycle = roll.start_from_landing(landing_speed, velocity_yaw, actor_yaw, *stance);
        }
        if lifecycle.started {
            commands.entity(character).insert(AlsNotifyLocomotionAction::Rolling);
            if let Some(stance) = crate::animation_events::physical_stance_override(AlsNotifyLocomotionAction::Rolling) {
                commands.entity(character).insert(stance);
            }
            if previous_roll.active {
                let mut blend = stopped_blend.unwrap_or_else(|| AlsRollBlendOut::new(&previous_roll, previous_roll.remaining_play_time()));
                blend.stop(crate::als::roll::ROLL_WEIGHT_IN_SECS);
                let mut tail = AlsRollTail::from_roll(&previous_roll, blend);
                tail.advance(dt);
                for (entity, sampler, player) in &samplers {
                    if sampler.character != character { continue; }
                    commands.entity(entity).insert(tail);
                    let bevy_animation_graph::core::animation_graph_player::AnimationSource::Graph(graph) = player.get_animation_source() else { continue; };
                    commands.spawn((Name::new("als_roll_slot_sampler"), Children::default(), ChildOf(character),
                        AnimationGraphPlayer::new(player.skeleton().clone()).with_graph(graph.clone()),
                        RootMotionOutput::default(), *sampler));
                }
                commands.entity(character).remove::<AlsRollBlendOut>();
            }
            roll.advance_playback(dt, true);
        }
    }
}

fn roll_drive_system(
    time: Res<Time>,
    mut commands: Commands,
    custom_dilations: Query<&crate::als_camera::AlsCameraCustomTimeDilation>,
    mut mode_changes: MessageReader<crate::als::transition::StateTransitionEvent>,
    parents: Query<&ChildOf>,
    modes: Query<(&LocomotionState, &avian3d::prelude::RigidBody,
        Has<avian3d::prelude::CustomPositionIntegration>)>,
    blend_outs: Query<(), With<AlsRollBlendOut>>,
    mut space: ParamSet<(bevy::transform::helper::TransformHelper,
        Query<(Entity, &AlsRollState, Option<&AlsNotifyLocomotionAction>, &mut Transform, Option<&mut Rotation>,
            &mut LinearVelocity, Option<&AlsNotifyRootMotionScale>, Option<&AlsRollSlotRuntime>,
            Option<&mut crate::als::character::AlsCmcAdapterRuntime>,
            Option<&crate::als::dynamics::AlsMovementBaseDynamicsState>), With<crate::als::character::AlsCharacter>>)> ,
    sampler_root_motion: Query<(&AlsRollSlotSampler, &RootMotionOutput), Without<AlsRollTail>>,
    mut players: Query<&mut AnimationGraphPlayer>,
) {
    let landings = mode_changes.read().filter(|event|
        event.dimension == crate::als::transition::Dimension::LocomotionState
            && event.from == crate::als::transition::StateValue::LocomotionState(LocomotionState::Air)
            && event.to == crate::als::transition::StateValue::LocomotionState(LocomotionState::Grounded))
        .map(|event| event.entity).collect::<Vec<_>>();
    let dt = time.delta_secs();
    let characters: Vec<_> = space.p1().iter().map(|(entity, ..)| entity).collect();
    for character in characters {
        let actor_delta = dt * custom_dilations.get(character).map_or(1.0, |value| value.0);
        let sample = sampler_root_motion.iter().find(|(sampler, _)| sampler.character == character);
        let Ok(actor_world) = space.p0().compute_global_transform(character) else { continue; };
        let parent_world = if let Ok(parent) = parents.get(character) {
            let Ok(world) = space.p0().compute_global_transform(parent.parent()) else { continue; };
            Some(world)
        } else { None };
        let mesh_world = if let Some((sampler, _)) = sample {
            let Ok(mesh) = space.p0().compute_global_transform(sampler.target_player) else { continue; };
            mesh
        } else { actor_world };
        let mut query = space.p1();
        let (_, roll, action, mut transform, rotation, mut vel, notify_root_scale, runtime, mut cmc, base) = query.get_mut(character).unwrap();
        let custom = modes.get(character).is_ok_and(|(mode, ..)| *mode == LocomotionState::Custom);
        let delta_too_small = if custom { dt <= 0.0 } else { dt <= 1e-5 };
        let grounded_subminimum = modes.get(character).is_ok_and(|(mode, ..)| *mode == LocomotionState::Grounded)
            && actor_delta < crate::als::movement_constants::MIN_MOVEMENT_DELTA_SECONDS;
        if !roll.root_motion_active_at_frame_start() || action == Some(&AlsNotifyLocomotionAction::Ragdolling)
            || delta_too_small || grounded_subminimum || blend_outs.contains(character) {
            continue;
        }

        let movement_resolved = landings.contains(&character)
            && cmc.as_deref().is_some_and(|runtime| runtime.grounded_collision_velocity.is_some());
        let (mut local_translation, mut local_rotation) = sample.map_or((Vec3::ZERO, Quat::IDENTITY),
            |(_, output)| (output.translation_delta, output.rotation_delta));
        if movement_resolved { local_translation = Vec3::ZERO; local_rotation = Quat::IDENTITY; }
        local_translation *= runtime.map_or_else(|| notify_root_scale.map_or(1.0, |scale| scale.0), |runtime| runtime.root_motion_scale).max(0.0);
        let (world_translation, root_rotation) =
            root_motion_world_transform(actor_world, mesh_world, local_translation, local_rotation);
        let world_rotation = if local_rotation.dot(Quat::IDENTITY).abs() < 1.0 - 1e-6 {
            root_rotation
        } else if action == Some(&AlsNotifyLocomotionAction::Rolling) {
            let (current_yaw, pitch, world_roll) = actor_world.rotation().to_euler(EulerRot::YXZ);
            Quat::from_euler(EulerRot::YXZ, damper_exact_angle(
                current_yaw,
                roll.direction_yaw,
                actor_delta,
                ROLL_ROTATION_HALF_LIFE_SECS,
            ), pitch, world_roll)
        } else {
            actor_world.rotation()
        };
        transform.rotation = parent_world.map_or(world_rotation, |parent| parent.rotation().inverse() * world_rotation);
        if let Some(mut rotation) = rotation {
            rotation.0 = world_rotation;
        }
        let weight = roll.weight();
        let clip_time = roll.clip_time();
        if !movement_resolved {
            let falling = modes.get(character).is_ok_and(|(mode, ..)| *mode == LocomotionState::Air);
            let prior_vertical = vel.y;
            let support = base.filter(|base| base.entity.is_some()
                && modes.get(character).is_ok_and(|(mode, ..)| *mode == LocomotionState::Grounded))
                .map_or(Vec3::ZERO, |base| base.support_velocity);
            if let Some(cmc) = cmc.as_deref_mut() {
                let mut root_velocity = if actor_delta > 0.0 { world_translation / actor_delta + support.with_y(0.0) }
                    else { cmc.grounded_collision_velocity.unwrap_or(vel.0) };
                if falling { root_velocity.y = prior_vertical; }
                cmc.grounded_collision_velocity = Some(root_velocity);
                cmc.has_animation_root_velocity = true;
                cmc.grounded_movement_delta = None;
                cmc.grounded_move_start = None;
            }
            let mut root_velocity = world_translation / dt + support;
            if falling { root_velocity.y = prior_vertical; }
            vel.0 = root_velocity;
        }
        if let Ok((mode, body, custom_position)) = modes.get(character)
            && *mode == LocomotionState::Custom {
            let mut world_position = actor_world.translation();
            if actor_delta >= crate::als::movement_constants::MIN_MOVEMENT_DELTA_SECONDS {
                world_position += world_translation;
                transform.translation = parent_world.map_or(world_position,
                    |parent| parent.affine().inverse().transform_point3(world_position));
            }
            commands.entity(character).insert((avian3d::prelude::Position(world_position),
                avian3d::prelude::RigidBody::Kinematic,
                avian3d::prelude::CustomPositionIntegration,
                crate::als::motion::AlsCustomMovementPhysicsFrame { rigid_body: *body, custom_position }));
        }

        if let Some((sampler, _)) = sample && let Ok(mut player) = players.get_mut(sampler.target_player) {
            player.set_input_data("roll_clip_time", DataValue::from(clip_time));
            player.set_input_data("roll_weight", DataValue::from(weight));
        }
    }
}
