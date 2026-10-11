use std::time::Duration;
use avian3d::prelude::*;
use bevy::prelude::*;
use bevy_als::als::{character::{AlsCharacterBundle, AlsCharacterPlugin}, control_scheme::{AlsControlScheme, AlsControlSchemeConfig},
    input::AlsInputCommand, motion::{AlsMotionPlugin, initial_als_control_scheme_config}, params::AlsMovementParams,
    state::LocomotionState, transition::{AlsStateRequest, AlsStateTransitionPlugin}};
use bevy_als::als_camera::AlsCameraCustomTimeDilation;

#[test]
fn ordinary_actor_clock_preserves_source_motion_and_independent_world_physics() {
    for capped in [false, true] {
        for mode in [LocomotionState::Grounded, LocomotionState::Air] {
            for (dilation, moving_base) in [0.0_f32, 0.5, 1.0, 2.0].into_iter().flat_map(|rate| [(rate, false), (rate, true)]) {
                if moving_base && mode == LocomotionState::Air { continue; }
                let mut app = App::new();
                app.add_plugins((MinimalPlugins, AssetPlugin::default(), TransformPlugin, PhysicsPlugins::new(PostUpdate),
                    bevy_tnua_avian3d::TnuaAvian3dPlugin::new(Update),
                    bevy_tnua::TnuaControllerPlugin::<AlsControlScheme>::new(Update), AlsStateTransitionPlugin, AlsCharacterPlugin,
                    AlsMotionPlugin, bevy_als::actual_motion::AlsActualMotionPlugin))
                    .configure_sets(PostUpdate, bevy_als::actual_motion::AlsActualMotionSet.after(bevy_als::als::motion::AlsActualVelocitySet))
                    .init_resource::<Assets<Mesh>>().add_message::<bevy::asset::AssetEvent<Mesh>>()
                    .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(1.0 / 60.0)));
                if capped { app.add_plugins(bevy_als::source_physics_timing::SourcePhysicsTimingPlugin); }
                app.finish();
                let settings = AlsMovementParams::default();
                let params = app.world_mut().resource_mut::<Assets<AlsMovementParams>>().add(settings.clone());
                let config = app.world_mut().resource_mut::<Assets<AlsControlSchemeConfig>>().add(initial_als_control_scheme_config(&settings));
                let mut bundle = AlsCharacterBundle::new(params, config);
                bundle.transform = Transform::from_xyz(0.0, 0.924, 0.0);
                let actor = app.world_mut().spawn(bundle).id();
                let floor = app.world_mut().spawn((if moving_base { RigidBody::Kinematic } else { RigidBody::Static },
                    Collider::cuboid(20.0, 0.2, 30.0), Transform::from_xyz(0.0, -0.1, 0.0))).id();
                for _ in 0..30 { app.update(); }
                let mut start = app.world().get::<Position>(actor).unwrap().0;
                if mode == LocomotionState::Air { start.y = 10.0; }
                app.world_mut().entity_mut(actor).insert((Position(start), Transform::from_translation(start), GlobalTransform::from_translation(start),
                    LinearVelocity(Vec3::new(0.0, 0.0, -3.75)), mode, AlsStateRequest::default().with_locomotion_state(mode),
                    AlsInputCommand { move_dir: Vec2::Y, ..default() }, AlsCameraCustomTimeDilation(dilation)));
                if moving_base { app.world_mut().entity_mut(floor).insert(LinearVelocity(Vec3::X * 2.0)); }
                let floor_start = app.world().get::<Position>(floor).unwrap().0;
                let other = app.world_mut().spawn((RigidBody::Dynamic, Collider::sphere(0.1), Transform::from_xyz(8.0, 20.0, 0.0),
                    LinearVelocity::ZERO, LinearDamping(0.0))).id();
                app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(0.05)));
                app.update();
                let delta = app.world().get::<Position>(actor).unwrap().0 - start;
                let velocity = app.world().get::<LinearVelocity>(actor).unwrap().0;
                let other_velocity = app.world().get::<LinearVelocity>(other).unwrap().y;
                let actor_seconds = 0.05 * dilation;
                let physics_seconds = if capped { bevy_als::source_physics_timing::SOURCE_MAX_PHYSICS_DELTA.as_secs_f32() } else { 0.05 };
                println!("capped={capped} mode={mode:?} dilation={dilation} moving_base={moving_base} delta={delta:?} velocity={velocity:?} other_y={other_velocity}");
                assert!((other_velocity + 9.81 * physics_seconds).abs() < 1e-4, "Actor dilation changed independent world physics");
                assert!((delta.z + 3.75 * actor_seconds).abs() < 0.001, "Ordinary displacement ignored the source actor clock");
                assert!((velocity.z + 3.75).abs() < 0.001, "Source movement velocity must remain independent of the actor clock scale");
                if moving_base {
                    let floor_delta = app.world().get::<Position>(floor).unwrap().0 - floor_start;
                    assert!((floor_delta.x - 2.0 * physics_seconds).abs() < 0.001);
                    if mode == LocomotionState::Grounded {
                        assert!((delta.x - floor_delta.x).abs() < 0.001, "Base carry must use the independent world clock");
                        let actual = app.world().get::<bevy_als::als::motion::AlsActualVelocity>(actor).unwrap().world_velocity;
                        assert!(actual.x.abs() < 0.001, "Base carry entered the character's relative velocity: {actual:?}");
                    }
                }
                if mode == LocomotionState::Air {
                    assert!((velocity.y + 9.81 * actor_seconds).abs() < 0.001);
                    assert!((delta.y + 0.5 * 9.81 * actor_seconds * actor_seconds).abs() < 0.001);
                }
                app.world_mut().entity_mut(actor).insert(AlsCameraCustomTimeDilation(1.0));
                let before_resume = app.world().get::<Position>(actor).unwrap().0;
                app.update();
                let resumed = app.world().get::<Position>(actor).unwrap().0 - before_resume;
                assert!((resumed.z + 3.75 * 0.05).abs() < 0.001, "Clock changes must preserve source movement velocity");
                if mode == LocomotionState::Grounded && !moving_base {
                    app.world_mut().entity_mut(actor).insert(AlsCameraCustomTimeDilation(dilation));
                    app.world_mut().get_mut::<AlsInputCommand>(actor).unwrap().jump_pressed = true;
                    app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(0.02)));
                    let before_jump = app.world().get::<Position>(actor).unwrap().0;
                    app.update();
                    let jump = app.world().get::<Position>(actor).unwrap().0 - before_jump;
                    let launch = app.world().get::<LinearVelocity>(actor).unwrap().y;
                    let seconds = 0.02 * dilation;
                    let expected_launch = settings.jump.jump_z_velocity - 9.81 * seconds;
                    println!("jump capped={capped} dilation={dilation} delta={jump:?} launch={launch}");
                    assert_eq!(app.world().get::<LocomotionState>(actor), Some(&LocomotionState::Air));
                    assert!((launch - expected_launch).abs() < 0.001);
                    assert!((jump.y - (settings.jump.jump_z_velocity * seconds - 0.5 * 9.81 * seconds * seconds)).abs() < 0.001);
                    app.world_mut().get_mut::<AlsInputCommand>(actor).unwrap().jump_pressed = false;
                    if dilation == 0.0 {
                        let frozen = app.world().get::<Position>(actor).unwrap().0;
                        for _ in 0..3 { app.update(); }
                        assert!(app.world().get::<Position>(actor).unwrap().0.abs_diff_eq(frozen, 0.0001));
                    } else {
                        let mut landing_seconds = seconds;
                        for _ in 0..160 {
                            app.update();
                            landing_seconds += seconds;
                            if app.world().get::<LocomotionState>(actor) == Some(&LocomotionState::Grounded) { break; }
                        }
                        assert_eq!(app.world().get::<LocomotionState>(actor), Some(&LocomotionState::Grounded));
                        assert!((landing_seconds - 2.0 * settings.jump.jump_z_velocity / 9.81).abs() < 2.0 * seconds + 0.01,
                            "Landing did not use the actor's flight time: {landing_seconds}");
                    }
                }
            }
        }
    }
}
