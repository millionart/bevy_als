use avian3d::prelude::*;
use bevy::prelude::*;
use bevy_als::als::{
    character::{AlsCharacterBundle, AlsCharacterPlugin},
    control_scheme::{AlsControlScheme, AlsControlSchemeConfig},
    input::AlsInputCommand,
    motion::{AlsMotionPlugin, initial_als_control_scheme_config},
    params::AlsMovementParams,
    state::LocomotionState,
    transition::{AlsStateRequest, AlsStateTransitionPlugin},
};
use bevy_als::als_camera::AlsCameraCustomTimeDilation;
use std::time::Duration;

#[test]
fn jump_hold_observes_owner_clock_including_midair_changes() {
    for rates in [
        vec![0.0; 24],
        vec![0.5; 24],
        vec![1.0; 24],
        vec![2.0; 24],
        [vec![0.5; 8], vec![2.0; 16]].concat(),
        [vec![2.0; 4], vec![0.0; 8], vec![0.5; 12]].concat(),
    ] {
        let (mut app, actor, _) = movement_actor(false);
        app.world_mut().spawn((
            RigidBody::Static,
            Collider::cuboid(20.0, 0.2, 30.0),
            Transform::from_xyz(0.0, -0.1, 0.0),
        ));
        for _ in 0..30 {
            app.update();
        }
        let handle = app
            .world()
            .get::<bevy_als::als::character::AlsMovementParamsHandle>(actor)
            .unwrap()
            .0
            .clone();
        app.world_mut()
            .resource_mut::<Assets<AlsMovementParams>>()
            .get_mut(&handle)
            .unwrap()
            .jump
            .max_hold_time = 0.2;
        let mut elapsed = 0.0_f32;
        let mut expected = 0.0_f32;
        for (frame, dilation) in rates.into_iter().enumerate() {
            app.world_mut()
                .entity_mut(actor)
                .insert(AlsCameraCustomTimeDilation(dilation));
            let mut input = app.world_mut().get_mut::<AlsInputCommand>(actor).unwrap();
            input.jump_pressed = frame == 0;
            input.jump_held = true;
            let delta = (1.0 / 60.0) * dilation;
            if frame == 0 || elapsed < 0.2 {
                expected = expected.max(4.2);
            }
            expected -= 9.81 * delta;
            app.update();
            let actual = app.world().get::<LinearVelocity>(actor).unwrap().y;
            assert!(
                (actual - expected).abs() < 0.025,
                "Source Jump hold differs: frame={frame} dilation={dilation} elapsed={elapsed} actual={actual} expected={expected}"
            );
            elapsed += delta;
        }
    }
}

#[test]
fn stationary_velocity_rotation_retains_source_half_life_at_each_camera_speed() {
    use bevy_als::als::{
        motion::{AlsRotationResponseState, AlsViewYaw},
        state::{RotationMode, Stance},
    };
    for stance in [Stance::Standing, Stance::Crouching] {
        for view_delta in [0.0_f32, 0.01, -0.01, 1.0, -1.0] {
            let (mut app, actor, _) = movement_actor(false);
            app.world_mut().spawn((
                RigidBody::Static,
                Collider::cuboid(20.0, 0.2, 30.0),
                Transform::from_xyz(0.0, -0.1, 0.0),
            ));
            for _ in 0..10 {
                app.update();
            }
            let target = 1.0_f32;
            app.world_mut().entity_mut(actor).insert((
                stance,
                RotationMode::VelocityDirection,
                AlsViewYaw(view_delta),
                AlsRotationResponseState {
                    initialized: true,
                    target_yaw: target,
                    smooth_target_yaw: target,
                    previous_view_yaw: 0.0,
                    previous_rotation_mode: Some(RotationMode::VelocityDirection),
                    rotation_towards_last_input_direction_blocked: true,
                    ..default()
                },
            ));
            app.update();
            let yaw = bevy_als::als::orientation::world_forward_to_yaw(
                app.world().get::<Rotation>(actor).unwrap().0 * Vec3::NEG_Z,
            );
            let expected = target
                * (1.0
                    - bevy_als::als::animation::inv_exp_approx(
                        std::f32::consts::LN_2 / (0.1 + 1e-8) / 60.0,
                    ));
            let actual = app
                .world()
                .get::<bevy_als::actual_motion::AlsActualMotion>(actor)
                .unwrap();
            assert!(!actual.has_input && actual.horizontal_speed < 0.01);
            assert!(
                (yaw - expected).abs() < 1e-4,
                "Source stationary VelocityDirection keeps its half-life: stance={stance:?} view_delta={view_delta} yaw={yaw} expected={expected}"
            );
        }
    }
}

fn movement_actor(capped: bool) -> (App, Entity, AlsMovementParams) {
    movement_actor_with_world_bounds(capped, false)
}

fn movement_actor_with_world_bounds(
    capped: bool,
    world_bounds: bool,
) -> (App, Entity, AlsMovementParams) {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        AssetPlugin::default(),
        TransformPlugin,
        PhysicsPlugins::new(PostUpdate),
        bevy_tnua_avian3d::TnuaAvian3dPlugin::new(Update),
        bevy_tnua::TnuaControllerPlugin::<AlsControlScheme>::new(Update),
        AlsStateTransitionPlugin,
        AlsCharacterPlugin,
        AlsMotionPlugin,
        bevy_als::actual_motion::AlsActualMotionPlugin,
    ))
    .configure_sets(
        PostUpdate,
        bevy_als::actual_motion::AlsActualMotionSet
            .after(bevy_als::als::motion::AlsActualVelocitySet),
    )
    .init_resource::<Assets<Mesh>>()
    .add_message::<bevy::asset::AssetEvent<Mesh>>()
    .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
        Duration::from_secs_f32(1.0 / 60.0),
    ));
    if capped {
        app.add_plugins(bevy_als::source_physics_timing::SourcePhysicsTimingPlugin);
    }
    if world_bounds {
        app.add_plugins(
            bevy_als::extensions::source_world_time::SourceWorldTimePlugin {
                minimum: Duration::from_micros(500),
                maximum: Duration::from_millis(400),
            },
        );
    }
    app.add_systems(
        PostUpdate,
        (
            bevy::transform::systems::mark_dirty_trees,
            bevy::transform::systems::propagate_parent_transforms,
            bevy::transform::systems::sync_simple_transforms,
        )
            .chain()
            .after(bevy_als::als::motion::AlsRotationResponseSet),
    );
    app.finish();
    let settings = AlsMovementParams::default();
    let params = app
        .world_mut()
        .resource_mut::<Assets<AlsMovementParams>>()
        .add(settings.clone());
    let config = initial_als_control_scheme_config(&settings);
    let mut bundle = AlsCharacterBundle::new(
        params,
        &mut app
            .world_mut()
            .resource_mut::<Assets<AlsControlSchemeConfig>>(),
        config,
    );
    bundle.transform = Transform::from_xyz(0.0, 0.924, 0.0);
    let actor = app.world_mut().spawn(bundle).id();
    (app, actor, settings)
}

#[test]
fn airborne_character_consumes_bounded_world_time_with_independent_physics() {
    use bevy::time::{Real, TimeUpdateStrategy, Virtual};
    use bevy_als::als::character::AlsCmcAdapterRuntime;

    for (raw, expected) in [(0.2_f32, 0.2_f32), (0.3, 0.3), (0.5, 0.4)] {
        let (mut app, actor, _) = movement_actor_with_world_bounds(true, true);
        for _ in 0..3 {
            app.update();
        }
        let start = Vec3::new(0.0, 20.0, 0.0);
        app.world_mut().entity_mut(actor).insert((
            Position(start),
            Transform::from_translation(start),
            GlobalTransform::from_translation(start),
            LinearVelocity(Vec3::X),
            LocomotionState::Air,
            AlsStateRequest::default().with_locomotion_state(LocomotionState::Air),
            AlsInputCommand::default(),
        ));
        let other = app
            .world_mut()
            .spawn((
                RigidBody::Dynamic,
                Collider::sphere(0.1),
                Transform::from_xyz(8.0, 20.0, 0.0),
                LinearVelocity::ZERO,
                LinearDamping(0.0),
            ))
            .id();
        app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(
            raw,
        )));
        app.update();
        let delta = app.world().get::<Position>(actor).unwrap().0 - start;
        let cmc = app.world().get::<AlsCmcAdapterRuntime>(actor).unwrap();
        let other_velocity = app.world().get::<LinearVelocity>(other).unwrap().y;
        assert!((app.world().resource::<Time<Real>>().delta_secs() - raw).abs() < 1e-6);
        assert!((app.world().resource::<Time<Virtual>>().delta_secs() - expected).abs() < 1e-6);
        assert!((cmc.simulation_delta_seconds - expected).abs() < 1e-6);
        assert!(
            (delta.x - expected).abs() < 1e-4,
            "raw={raw} displacement={delta:?}"
        );
        let physics = bevy_als::source_physics_timing::SOURCE_MAX_PHYSICS_DELTA.as_secs_f32();
        assert!((other_velocity + 9.81 * physics).abs() < 1e-4);
    }
}

fn scaled_movement_actor(scale: f32, hierarchy: usize) -> (App, Entity, AlsMovementParams) {
    let (mut app, actor, settings) = movement_actor(false);
    app.world_mut().spawn((
        RigidBody::Static,
        Collider::cuboid(20.0, 0.2, 30.0),
        Transform::from_xyz(0.0, -0.1, 0.0),
    ));
    let world = Transform::from_xyz(
        0.0,
        settings.crouch.standing_capsule_half_height * scale + 0.024,
        0.0,
    )
    .with_scale(Vec3::splat(scale));
    if hierarchy == 0 {
        app.world_mut().entity_mut(actor).insert((
            world,
            Position(world.translation),
            GlobalTransform::from(world),
        ));
    } else {
        let parent = if hierarchy == 1 {
            app.world_mut()
                .spawn(Transform::from_scale(Vec3::splat(scale)))
                .id()
        } else {
            let outer = app
                .world_mut()
                .spawn(Transform::from_scale(Vec3::splat(scale * 2.0)))
                .id();
            app.world_mut()
                .spawn((ChildOf(outer), Transform::from_scale(Vec3::splat(0.5))))
                .id()
        };
        app.world_mut().entity_mut(actor).insert((
            ChildOf(parent),
            Position(world.translation),
            GlobalTransform::from(world),
            Transform::from_translation(world.translation / scale),
        ));
    }
    for _ in 0..10 {
        app.update();
    }
    (app, actor, settings)
}

#[test]
fn capsule_floor_uses_world_scale_for_each_parent_decomposition() {
    for scale in [0.5, 1.0, 2.0] {
        for hierarchy in 0..3 {
            let (app, actor, settings) = scaled_movement_actor(scale, hierarchy);
            let extent = app
                .world()
                .get::<Collider>(actor)
                .unwrap()
                .shape_scaled()
                .compute_local_aabb()
                .half_extents();
            let position = app.world().get::<Position>(actor).unwrap().0;
            let mode = *app.world().get::<LocomotionState>(actor).unwrap();
            println!(
                "world_capsule scale={scale} hierarchy={hierarchy} extent={extent:?} position={position:?} mode={mode:?}"
            );
            assert!((extent.y - settings.crouch.standing_capsule_half_height * scale).abs() < 1e-4);
            assert_eq!(
                mode,
                LocomotionState::Grounded,
                "Source FindFloor uses the component world capsule dimensions"
            );
            assert!(
                (position.y - extent.y - 0.0215).abs() < 0.005,
                "Source floor clearance uses the world capsule dimensions"
            );
        }
    }
}

#[test]
fn parent_scaled_capsule_uncrouch_uses_world_clearance() {
    use bevy_als::als::state::Stance;
    for hierarchy in 0..3 {
        let (mut app, actor, _) = scaled_movement_actor(2.0, hierarchy);
        app.add_systems(Update, (move |helper: bevy::transform::helper::TransformHelper, positions: Query<&Position>| {
            let current = helper.compute_global_transform(actor).unwrap().translation();
            let physical = positions.get(actor).unwrap().0;
            assert!(current.distance(physical) < 1e-4,
                "Source crouch pre-physics component position differs: current={current:?} physical={physical:?}");
        }).after(bevy_tnua::TnuaPipelineSystems::Motors));
        app.world_mut()
            .get_mut::<AlsStateRequest>(actor)
            .unwrap()
            .stance = Some(Stance::Crouching);
        for _ in 0..5 {
            app.update();
        }
        assert_eq!(
            *app.world().get::<Stance>(actor).unwrap(),
            Stance::Crouching
        );
        let ceiling = app
            .world_mut()
            .spawn((
                RigidBody::Static,
                Collider::cuboid(4.0, 0.2, 4.0),
                Transform::from_xyz(0.0, 2.6, 0.0),
            ))
            .id();
        app.update();
        app.world_mut()
            .get_mut::<AlsStateRequest>(actor)
            .unwrap()
            .stance = Some(Stance::Standing);
        app.update();
        let stance = *app.world().get::<Stance>(actor).unwrap();
        println!(
            "world_uncrouch hierarchy={hierarchy} blocked_stance={stance:?} position={:?}",
            app.world().get::<Position>(actor)
        );
        assert_eq!(
            stance,
            Stance::Crouching,
            "Source UnCrouch must test the full world capsule under a low ceiling"
        );
        app.world_mut().despawn(ceiling);
        for _ in 0..5 {
            app.update();
        }
        assert_eq!(*app.world().get::<Stance>(actor).unwrap(), Stance::Standing);
    }
}

#[test]
fn parent_scaled_capsule_steps_use_the_world_bottom_and_radius() {
    for hierarchy in 0..3 {
        let (mut app, actor, _) = scaled_movement_actor(2.0, hierarchy);
        app.world_mut().spawn((
            RigidBody::Static,
            Collider::cuboid(4.0, 0.4, 1.0),
            Transform::from_xyz(0.0, 0.2, -2.0),
        ));
        for _ in 0..3 {
            app.update();
        }
        app.world_mut().entity_mut(actor).insert((
            LinearVelocity(Vec3::NEG_Z * 3.75),
            AlsInputCommand {
                move_dir: Vec2::Y,
                ..default()
            },
        ));
        let mut highest = 0.0_f32;
        for _ in 0..60 {
            app.update();
            highest = highest.max(app.world().get::<Position>(actor).unwrap().y);
        }
        let position = app.world().get::<Position>(actor).unwrap().0;
        println!("world_step hierarchy={hierarchy} highest={highest} position={position:?}");
        assert!(
            highest > 2.2 && position.z < -3.0,
            "Source StepUp uses the world capsule bottom and radius"
        );
    }
}

#[test]
fn ordinary_steps_respect_source_component_permissions() {
    for frame_seconds in [1.0_f32 / 60.0, 0.1] {
        for (child, body_permission, collider_permission, expected) in [
            (false, None, None, true),
            (false, Some(true), None, true),
            (false, Some(false), None, false),
            (true, None, None, true),
            (true, Some(true), None, true),
            (true, Some(false), None, false),
            (true, Some(false), Some(true), true),
            (true, Some(true), Some(false), false),
        ] {
            let (mut app, actor, _) = movement_actor(false);
            app.world_mut().spawn((
                RigidBody::Static,
                Collider::cuboid(20.0, 0.2, 30.0),
                Transform::from_xyz(0.0, -0.1, 0.0),
            ));
            let body = app
                .world_mut()
                .spawn((RigidBody::Static, Transform::from_xyz(0.0, 0.2, -1.0)))
                .id();
            if let Some(allows_step_up) = body_permission {
                app.world_mut()
                    .entity_mut(body)
                    .insert(bevy_als::als::mantle::AlsMantleTargetPermission { allows_step_up });
            }
            let collider = if child {
                app.world_mut()
                    .spawn((
                        Collider::cuboid(4.0, 0.4, 1.0),
                        Transform::default(),
                        ChildOf(body),
                    ))
                    .id()
            } else {
                app.world_mut()
                    .entity_mut(body)
                    .insert(Collider::cuboid(4.0, 0.4, 1.0));
                body
            };
            if let Some(allows_step_up) = collider_permission {
                app.world_mut()
                    .entity_mut(collider)
                    .insert(bevy_als::als::mantle::AlsMantleTargetPermission { allows_step_up });
            }
            for _ in 0..30 {
                app.update();
            }
            assert_eq!(app.world().get::<ColliderOf>(collider).unwrap().body, body);
            app.world_mut()
                .get_mut::<AlsInputCommand>(actor)
                .unwrap()
                .move_dir = Vec2::Y;
            app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
                Duration::from_secs_f32(frame_seconds),
            ));
            let mut peak_height = 0.0_f32;
            for _ in 0..(1.0 / frame_seconds).round() as usize {
                app.update();
                peak_height = peak_height.max(app.world().get::<Position>(actor).unwrap().y);
            }
            let position = app.world().get::<Position>(actor).unwrap().0;
            println!(
                "step_permission frame={frame_seconds} child={child} body={body_permission:?} collider={collider_permission:?} expected={expected} position={position:?} peak_height={peak_height}"
            );
            if expected {
                assert!(
                    position.z < -1.5 && peak_height > 1.28,
                    "Allowed low steps must remain traversable"
                );
            } else {
                assert!(
                    position.z > -0.3 && peak_height < 1.0,
                    "Source CanStepUp rejects forbidden components"
                );
            }
        }
    }
}

#[test]
fn ordinary_parent_space_preserves_world_pose_after_ground_and_air_physics() {
    use bevy::ecs::system::RunSystemOnce;
    for mode in [LocomotionState::Grounded, LocomotionState::Air] {
        let (mut app, actor, _) = movement_actor(false);
        app.world_mut().spawn((
            RigidBody::Static,
            Collider::cuboid(20.0, 0.2, 30.0),
            Transform::from_xyz(0.0, -0.1, 0.0),
        ));
        for _ in 0..30 {
            app.update();
        }
        let mut world = *app.world().get::<Transform>(actor).unwrap();
        if mode == LocomotionState::Air {
            world.translation.y = 10.0;
        }
        let parent_transform = Transform::from_xyz(5.0, 2.0, -3.0)
            .with_rotation(Quat::from_rotation_y(std::f32::consts::FRAC_PI_2))
            .with_scale(Vec3::splat(2.0));
        let parent = app.world_mut().spawn(parent_transform).id();
        app.world_mut().entity_mut(actor).insert((
            ChildOf(parent),
            Position(world.translation),
            Rotation(world.rotation),
            GlobalTransform::from(world),
            GlobalTransform::from(world).reparented_to(&GlobalTransform::from(parent_transform)),
            LinearVelocity::ZERO,
            mode,
            AlsStateRequest::default().with_locomotion_state(mode),
        ));
        app.world_mut()
            .get_mut::<AlsInputCommand>(actor)
            .unwrap()
            .move_dir = Vec2::Y;
        app.add_systems(Update, (move |helper: bevy::transform::helper::TransformHelper, positions: Query<&Position>| {
            let current = helper.compute_global_transform(actor).unwrap().translation();
            let physical = positions.get(actor).unwrap().0;
            assert!(current.distance(physical) < 1e-4,
                "Source pre-physics component position differs: current={current:?} physical={physical:?}");
        }).after(bevy_tnua::TnuaPipelineSystems::Motors));
        for _ in 0..4 {
            app.update();
            let rendered = app.world().get::<GlobalTransform>(actor).unwrap();
            let physical = app.world().get::<Rotation>(actor).unwrap().0;
            let position = app.world().get::<Position>(actor).unwrap().0;
            assert_eq!(*app.world().get::<LocomotionState>(actor).unwrap(), mode);
            assert!(
                rendered.translation().distance(position) < 1e-4,
                "Source attached actor world position differs: {mode:?}"
            );
            assert!(
                rendered.rotation().angle_between(physical).abs() < 1e-3
                    && physical.angle_between(world.rotation).abs() < 1e-3,
                "Source attached actor world orientation differs after physics: {mode:?}"
            );
            let current_world = app
                .world_mut()
                .run_system_once(move |helper: bevy::transform::helper::TransformHelper| {
                    helper.compute_global_transform(actor).unwrap()
                })
                .unwrap();
            assert!(
                current_world.translation().distance(position) < 1e-4,
                "Source current component world position differs from physics: {mode:?} current={:?} physics={position:?}",
                current_world.translation()
            );
        }
    }
}

#[test]
fn movement_base_transport_preserves_the_current_parent_world_pose() {
    let (mut app, actor, _) = movement_actor(false);
    let base = app
        .world_mut()
        .spawn((
            RigidBody::Kinematic,
            Collider::cuboid(20.0, 0.2, 30.0),
            Transform::from_xyz(0.0, -0.1, 0.0),
        ))
        .id();
    for _ in 0..30 {
        app.update();
    }
    assert_eq!(
        app.world()
            .get::<bevy_als::als::dynamics::AlsMovementBase>(actor)
            .unwrap()
            .entity,
        Some(base)
    );
    let world = *app.world().get::<Transform>(actor).unwrap();
    let parent_transform = Transform::from_scale(Vec3::splat(2.0));
    let parent = app.world_mut().spawn(parent_transform).id();
    app.world_mut().entity_mut(actor).insert((
        ChildOf(parent),
        GlobalTransform::from(world).reparented_to(&GlobalTransform::from(parent_transform)),
    ));
    app.world_mut().get_mut::<Position>(base).unwrap().x += 0.5;
    app.world_mut()
        .get_mut::<Transform>(base)
        .unwrap()
        .translation
        .x += 0.5;
    app.add_systems(Update, (move |helper: bevy::transform::helper::TransformHelper, positions: Query<&Position>| {
        let current = helper.compute_global_transform(actor).unwrap().translation();
        let physical = positions.get(actor).unwrap().0;
        assert!(current.distance(physical) < 1e-4,
            "Source based movement component position differs: current={current:?} physical={physical:?}");
    }).after(bevy_tnua::TnuaPipelineSystems::Motors));
    app.update();
    assert!((app.world().get::<Position>(actor).unwrap().x - 0.5).abs() < 1e-4);
}

#[test]
fn ordinary_actor_clock_preserves_source_motion_and_independent_world_physics() {
    for capped in [false, true] {
        for mode in [LocomotionState::Grounded, LocomotionState::Air] {
            for (dilation, moving_base) in [0.0_f32, 0.5, 1.0, 2.0]
                .into_iter()
                .flat_map(|rate| [(rate, false), (rate, true)])
            {
                if moving_base && mode == LocomotionState::Air {
                    continue;
                }
                let (mut app, actor, settings) = movement_actor(capped);
                let floor = app
                    .world_mut()
                    .spawn((
                        if moving_base {
                            RigidBody::Kinematic
                        } else {
                            RigidBody::Static
                        },
                        Collider::cuboid(20.0, 0.2, 30.0),
                        Transform::from_xyz(0.0, -0.1, 0.0),
                    ))
                    .id();
                for _ in 0..30 {
                    app.update();
                }
                let mut start = app.world().get::<Position>(actor).unwrap().0;
                if mode == LocomotionState::Air {
                    start.y = 10.0;
                }
                app.world_mut().entity_mut(actor).insert((
                    Position(start),
                    Transform::from_translation(start),
                    GlobalTransform::from_translation(start),
                    LinearVelocity(Vec3::new(0.0, 0.0, -3.75)),
                    mode,
                    AlsStateRequest::default().with_locomotion_state(mode),
                    AlsInputCommand {
                        move_dir: Vec2::Y,
                        ..default()
                    },
                    AlsCameraCustomTimeDilation(dilation),
                ));
                if moving_base {
                    app.world_mut()
                        .entity_mut(floor)
                        .insert(LinearVelocity(Vec3::X * 2.0));
                }
                let floor_start = app.world().get::<Position>(floor).unwrap().0;
                let other = app
                    .world_mut()
                    .spawn((
                        RigidBody::Dynamic,
                        Collider::sphere(0.1),
                        Transform::from_xyz(8.0, 20.0, 0.0),
                        LinearVelocity::ZERO,
                        LinearDamping(0.0),
                    ))
                    .id();
                app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
                    Duration::from_secs_f32(0.05),
                ));
                app.update();
                let delta = app.world().get::<Position>(actor).unwrap().0 - start;
                let velocity = app.world().get::<LinearVelocity>(actor).unwrap().0;
                let other_velocity = app.world().get::<LinearVelocity>(other).unwrap().y;
                let actor_seconds = 0.05 * dilation;
                let physics_seconds = if capped {
                    bevy_als::source_physics_timing::SOURCE_MAX_PHYSICS_DELTA.as_secs_f32()
                } else {
                    0.05
                };
                println!(
                    "capped={capped} mode={mode:?} dilation={dilation} moving_base={moving_base} delta={delta:?} velocity={velocity:?} other_y={other_velocity}"
                );
                assert!(
                    (other_velocity + 9.81 * physics_seconds).abs() < 1e-4,
                    "Actor dilation changed independent world physics"
                );
                assert!(
                    (delta.z + 3.75 * actor_seconds).abs() < 0.001,
                    "Ordinary displacement ignored the source actor clock"
                );
                assert!(
                    (velocity.z + 3.75).abs() < 0.001,
                    "Source movement velocity must remain independent of the actor clock scale"
                );
                if moving_base {
                    let floor_delta = app.world().get::<Position>(floor).unwrap().0 - floor_start;
                    assert!((floor_delta.x - 2.0 * physics_seconds).abs() < 0.001);
                    if mode == LocomotionState::Grounded {
                        assert!(
                            (delta.x - floor_delta.x).abs() < 0.001,
                            "Base carry must use the independent world clock"
                        );
                        let actual = app
                            .world()
                            .get::<bevy_als::als::motion::AlsActualVelocity>(actor)
                            .unwrap()
                            .world_velocity;
                        assert!(
                            actual.x.abs() < 0.001,
                            "Base carry entered the character's relative velocity: {actual:?}"
                        );
                    }
                }
                if mode == LocomotionState::Air {
                    assert!((velocity.y + 9.81 * actor_seconds).abs() < 0.001);
                    assert!((delta.y + 0.5 * 9.81 * actor_seconds * actor_seconds).abs() < 0.001);
                }
                app.world_mut()
                    .entity_mut(actor)
                    .insert(AlsCameraCustomTimeDilation(1.0));
                let before_resume = app.world().get::<Position>(actor).unwrap().0;
                app.update();
                let resumed = app.world().get::<Position>(actor).unwrap().0 - before_resume;
                assert!(
                    (resumed.z + 3.75 * 0.05).abs() < 0.001,
                    "Clock changes must preserve source movement velocity"
                );
                if mode == LocomotionState::Grounded && !moving_base {
                    app.world_mut()
                        .entity_mut(actor)
                        .insert(AlsCameraCustomTimeDilation(dilation));
                    app.world_mut()
                        .get_mut::<AlsInputCommand>(actor)
                        .unwrap()
                        .jump_pressed = true;
                    app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
                        Duration::from_secs_f32(0.02),
                    ));
                    let before_jump = app.world().get::<Position>(actor).unwrap().0;
                    app.update();
                    let jump = app.world().get::<Position>(actor).unwrap().0 - before_jump;
                    let launch = app.world().get::<LinearVelocity>(actor).unwrap().y;
                    let seconds = 0.02 * dilation;
                    let expected_launch = settings.jump.jump_z_velocity - 9.81 * seconds;
                    println!(
                        "jump capped={capped} dilation={dilation} delta={jump:?} launch={launch}"
                    );
                    assert_eq!(
                        app.world().get::<LocomotionState>(actor),
                        Some(&LocomotionState::Air)
                    );
                    assert!((launch - expected_launch).abs() < 0.001);
                    assert!(
                        (jump.y
                            - (settings.jump.jump_z_velocity * seconds
                                - 0.5 * 9.81 * seconds * seconds))
                            .abs()
                            < 0.001
                    );
                    app.world_mut()
                        .get_mut::<AlsInputCommand>(actor)
                        .unwrap()
                        .jump_pressed = false;
                    if dilation == 0.0 {
                        let frozen = app.world().get::<Position>(actor).unwrap().0;
                        for _ in 0..3 {
                            app.update();
                        }
                        assert!(
                            app.world()
                                .get::<Position>(actor)
                                .unwrap()
                                .0
                                .abs_diff_eq(frozen, 0.0001)
                        );
                    } else {
                        let mut landing_seconds = seconds;
                        for _ in 0..160 {
                            app.update();
                            landing_seconds += seconds;
                            if app.world().get::<LocomotionState>(actor)
                                == Some(&LocomotionState::Grounded)
                            {
                                break;
                            }
                        }
                        assert_eq!(
                            app.world().get::<LocomotionState>(actor),
                            Some(&LocomotionState::Grounded)
                        );
                        assert!(
                            (landing_seconds - 2.0 * settings.jump.jump_z_velocity / 9.81).abs()
                                < 2.0 * seconds + 0.01,
                            "Landing did not use the actor's flight time: {landing_seconds}"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn landing_consumes_source_walking_time_without_resetting_the_iteration_budget() {
    use bevy_als::als::character::AlsMovementParamsHandle;
    for capped in [false, true] {
        for (dilation, step, iterations, input) in [0.0_f32, 0.5, 1.0, 2.0]
            .into_iter()
            .flat_map(|rate| [0.01_f32, 0.025, 0.05, 0.1].map(|step| (rate, step, 8, Vec2::Y)))
            .chain([
                (1.0, 0.05, 1, Vec2::Y),
                (1.0, 0.05, 2, Vec2::Y),
                (1.0, 0.05, 8, Vec2::ZERO),
                (1.0, 0.05, 8, Vec2::Y * 0.5),
            ])
        {
            let (mut app, actor, settings) = movement_actor(capped);
            app.world_mut().spawn((
                RigidBody::Static,
                Collider::cuboid(20.0, 0.2, 30.0),
                Transform::from_xyz(0.0, -0.1, 0.0),
            ));
            for _ in 0..30 {
                app.update();
            }
            let handle = app
                .world()
                .get::<AlsMovementParamsHandle>(actor)
                .unwrap()
                .0
                .clone();
            {
                let mut assets = app.world_mut().resource_mut::<Assets<AlsMovementParams>>();
                let mut settings = assets.get_mut(&handle).unwrap();
                settings.cmc.max_simulation_time_step = step;
                settings.cmc.max_simulation_iterations = iterations;
            }
            let start = Vec3::Y;
            app.world_mut().entity_mut(actor).insert((
                Position(start),
                Transform::from_translation(start),
                GlobalTransform::from_translation(start),
                LinearVelocity(Vec3::new(0.0, -3.0, -3.75)),
                LocomotionState::Air,
                AlsStateRequest::default(),
                AlsInputCommand {
                    move_dir: input,
                    ..default()
                },
                AlsCameraCustomTimeDilation(0.0),
            ));
            app.update();
            app.world_mut()
                .entity_mut(actor)
                .insert(AlsCameraCustomTimeDilation(dilation));
            app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
                Duration::from_secs_f32(0.2),
            ));
            app.update();
            let delta = app.world().get::<Position>(actor).unwrap().0 - start;
            let velocity = app.world().get::<LinearVelocity>(actor).unwrap().0;
            let mode = app.world().get::<LocomotionState>(actor).unwrap();
            println!(
                "landing capped={capped} dilation={dilation} step={step} iterations={iterations} input={input:?} delta={delta:?} velocity={velocity:?} mode={mode:?}"
            );
            if dilation == 0.0 {
                assert!(delta.length() < 0.001);
                assert_eq!(*mode, LocomotionState::Air);
            } else {
                assert_eq!(*mode, LocomotionState::Grounded);
                let floor_distance = app.world().get::<Position>(actor).unwrap().y
                    - settings.crouch.standing_capsule_half_height;
                assert!(
                    (0.018..=0.025).contains(&floor_distance),
                    "Source Walking mode must adjust floor height even without another iteration: {floor_distance}"
                );
                if iterations == 1 {
                    assert!(
                        (-delta.z) < 0.2,
                        "Landing reset the source frame iteration budget"
                    );
                } else if input == Vec2::Y {
                    assert!(
                        (delta.z + 0.75 * dilation).abs() < 0.001,
                        "Landing discarded remaining source walking time"
                    );
                } else {
                    assert!((-delta.z) > 0.105 && (-delta.z) < 0.75);
                    assert!(
                        (-velocity.z) < 3.74,
                        "Remaining walking time must apply grounded input and braking"
                    );
                    if input == Vec2::ZERO {
                        assert!(
                            velocity.z.abs() < 0.001,
                            "Source no-input landing friction must stop this frame"
                        );
                    } else {
                        assert!(
                            (velocity.z + 1.875).abs() < 0.001,
                            "Source analog input must reach its grounded speed limit"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn same_frame_walk_fall_land_runs_the_source_landing_callback() {
    use bevy_als::als::dynamics::AlsLandingBrakingFriction;
    let (mut app, actor, _) = movement_actor(false);
    app.world_mut().spawn((
        RigidBody::Static,
        Collider::cuboid(20.0, 0.2, 30.0),
        Transform::from_xyz(0.0, -0.1, 0.0),
    ));
    app.world_mut().spawn((
        RigidBody::Static,
        Collider::cuboid(4.0, 0.6, 0.1),
        Transform::from_xyz(0.0, 0.3, 0.0),
    ));
    let position = Vec3::Y * 1.524;
    app.world_mut().entity_mut(actor).insert((
        Position(position),
        Transform::from_translation(position),
        GlobalTransform::from_translation(position),
        LinearVelocity::ZERO,
    ));
    for _ in 0..30 {
        app.update();
    }
    app.world_mut().entity_mut(actor).insert((
        LinearVelocity(Vec3::NEG_Z * 3.75),
        AlsInputCommand {
            move_dir: Vec2::Y,
            ..default()
        },
        bevy_als::als_camera::AlsCameraCustomTimeDilation(0.0),
    ));
    app.update();
    assert_eq!(
        *app.world().get::<LocomotionState>(actor).unwrap(),
        LocomotionState::Grounded
    );
    assert_eq!(
        app.world()
            .get::<AlsLandingBrakingFriction>(actor)
            .unwrap()
            .factor,
        0.0
    );
    app.world_mut()
        .resource_mut::<Messages<bevy_als::als::transition::StateTransitionEvent>>()
        .clear();
    app.world_mut()
        .entity_mut(actor)
        .insert(bevy_als::als_camera::AlsCameraCustomTimeDilation(2.0));
    app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
        std::time::Duration::from_secs_f32(0.25),
    ));
    app.update();
    let events = app
        .world_mut()
        .resource_mut::<Messages<bevy_als::als::transition::StateTransitionEvent>>()
        .drain()
        .collect::<Vec<_>>();
    let mode = *app.world().get::<LocomotionState>(actor).unwrap();
    let braking = *app.world().get::<AlsLandingBrakingFriction>(actor).unwrap();
    println!(
        "same_frame mode={mode:?} braking={braking:?} events={events:?} position={:?}",
        app.world().get::<Position>(actor)
    );
    assert_eq!(mode, LocomotionState::Grounded);
    assert_eq!(
        events
            .iter()
            .filter(|event| event.entity == actor
                && event.dimension == bevy_als::als::transition::Dimension::LocomotionState)
            .count(),
        2
    );
    assert_eq!(
        braking.factor, 0.5,
        "Source NotifyLocomotionModeChanged runs the landing callback between Falling and Walking"
    );
}
