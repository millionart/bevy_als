use std::num::NonZeroU8;

use als_core::character::{
    ALS_CHARACTER_CAPSULE_HALF_HEIGHT, ALS_CHARACTER_CAPSULE_RADIUS, AlsCharacter,
};
use als_core::mantle::{
    AlsMantleControlSettings, AlsMantleStartTimeSettings, AlsMantleState,
    AlsMantleTargetPermission, MantleDetectionInput, MantleKind, MantleRootHeightSample,
    MantleStartTimeMode, auto_mantle_start_allowed, configured_mantle_start_time, detect_mantle,
    mantle_start_allowed, scaled_mantle_capsule_dimensions,
};
use als_core::state::{LocomotionState, OverlayState, Stance};
use avian3d::prelude::*;
use bevy::ecs::message::MessageWriter;
use bevy::prelude::*;

#[derive(Resource, Default)]
struct GateState {
    frame: u32,
    allowed_character: Option<Entity>,
    scaled_character: Option<Entity>,
    denied_character: Option<Entity>,
    done: bool,
}

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, PhysicsPlugins::new(FixedPostUpdate)))
        .init_resource::<GateState>()
        .add_systems(Startup, setup)
        .add_systems(Update, verify)
        .run();
}

fn setup(mut commands: Commands, mut state: ResMut<GateState>) {
    commands.spawn((
        RigidBody::Static,
        Collider::cuboid(30.0, 0.3, 30.0),
        Transform::from_xyz(0.0, -0.15, 0.0),
    ));
    spawn_target(&mut commands, 0.0, 1.0, false);
    spawn_target(&mut commands, 5.0, 2.0, false);
    spawn_target(&mut commands, -5.0, 1.0, true);
    state.allowed_character = Some(commands.spawn(AlsCharacter).id());
    state.scaled_character = Some(commands.spawn(AlsCharacter).id());
    state.denied_character = Some(commands.spawn(AlsCharacter).id());
}

fn spawn_target(commands: &mut Commands, x: f32, scale: f32, denied: bool) {
    let height = 0.8 * scale;
    let target = commands
        .spawn((
            RigidBody::Static,
            Collider::cuboid(2.4 * scale, height, 1.2 * scale),
            avian3d::prelude::CollisionLayers::new([als_core::character::AlsCollisionLayer::Default, als_core::character::AlsCollisionLayer::Visibility], avian3d::prelude::LayerMask::ALL),
            Transform::from_xyz(x, height * 0.5, -1.4 * scale),
        ))
        .id();
    if denied {
        commands.entity(target).insert(AlsMantleTargetPermission {
            allows_step_up: false,
        });
    }
}

fn input_for(x: f32, scale: f32) -> MantleDetectionInput {
    let (capsule_half_height, capsule_radius, capsule_scale) = scaled_mantle_capsule_dimensions(
        ALS_CHARACTER_CAPSULE_HALF_HEIGHT,
        ALS_CHARACTER_CAPSULE_RADIUS,
        Vec3::splat(scale),
    );
    MantleDetectionInput {
        origin: Vec3::new(x, capsule_half_height + 0.05, 0.0),
        actor_yaw: 0.0,
        velocity: Vec3::ZERO,
        input_direction: Vec3::NEG_Z,
        locomotion: LocomotionState::Grounded,
        stance: Stance::Standing,
        overlay: OverlayState::Default,
        capsule_half_height,
        capsule_radius,
        capsule_scale,
    }
}

fn auto_start_settings() -> AlsMantleStartTimeSettings {
    AlsMantleStartTimeSettings {
        low: MantleStartTimeMode::AutoRootHeight {
            samples: vec![
                MantleRootHeightSample {
                    time: 0.0,
                    height: 0.0,
                },
                MantleRootHeightSample {
                    time: 0.75,
                    height: 0.75,
                },
                MantleRootHeightSample {
                    time: 1.5,
                    height: 1.5,
                },
            ],
            sampling_interval: 1.0 / 60.0,
        },
        ..default()
    }
}

fn verify(
    mut state: ResMut<GateState>,
    spatial: SpatialQuery,
    velocities: Query<(&RigidBody, &LinearVelocity, Option<&als_core::dynamics::AlsComponentVelocity>), Without<AlsCharacter>>,
    permissions: Query<Option<&AlsMantleTargetPermission>, Without<AlsCharacter>>,
    colliders: Query<&ColliderOf>,
    mut exit: MessageWriter<AppExit>,
) {
    if state.done {
        return;
    }
    state.frame += 1;
    let (Some(allowed_character), Some(scaled_character), Some(denied_character)) = (
        state.allowed_character,
        state.scaled_character,
        state.denied_character,
    ) else {
        return;
    };
    if state.frame < 12 {
        return;
    }

    let allowed = detect_mantle(
        &spatial,
        allowed_character,
        input_for(0.0, 1.0),
        &velocities,
        &permissions,
        &colliders,
    );
    let scaled = detect_mantle(
        &spatial,
        scaled_character,
        input_for(5.0, 2.0),
        &velocities,
        &permissions,
        &colliders,
    );
    let denied = detect_mantle(
        &spatial,
        denied_character,
        input_for(-5.0, 1.0),
        &velocities,
        &permissions,
        &colliders,
    );

    let Some(allowed_intent) = allowed.intent else {
        if state.frame < 180 {
            return;
        }
        state.done = true;
        println!("[BF102.49][GATE] allowed_intent=false -> FAIL");
        exit.write(AppExit::Error(NonZeroU8::new(49).unwrap()));
        return;
    };
    let Some(scaled_intent) = scaled.intent else {
        if state.frame < 180 {
            return;
        }
        state.done = true;
        println!("[BF102.49][GATE] scaled_intent=false -> FAIL");
        exit.write(AppExit::Error(NonZeroU8::new(49).unwrap()));
        return;
    };
    if !denied.trace.forward_hit.is_some() && state.frame < 180 {
        return;
    }

    let default_control = AlsMantleControlSettings::default();
    let disabled_control = AlsMantleControlSettings {
        allow_mantling: false,
        auto_start_in_air: false,
        ..default()
    };
    let source_settings = AlsMantleStartTimeSettings::default();
    let auto_settings = auto_start_settings();
    let default_start =
        configured_mantle_start_time(None, allowed_intent.kind, allowed_intent.mantling_height);
    let source_start = configured_mantle_start_time(
        Some(&source_settings),
        allowed_intent.kind,
        allowed_intent.mantling_height,
    );
    let auto_start = configured_mantle_start_time(
        Some(&auto_settings),
        allowed_intent.kind,
        allowed_intent.mantling_height,
    );
    let mut mantle_state = AlsMantleState::default();
    mantle_state.begin_with_start_time(
        allowed_intent.kind,
        Vec3::ZERO,
        allowed_intent.target,
        auto_start,
        0.0,
        allowed_intent.yaw_target,
    );
    let height_delta = (allowed_intent.mantling_height - scaled_intent.mantling_height).abs();
    let permission_blocked = denied.trace.forward_hit.is_some() && denied.intent.is_none();
    let scale_preserved = allowed_intent.kind == MantleKind::LowRight
        && scaled_intent.kind == MantleKind::LowRight
        && height_delta <= 0.08;
    let default_phase = (source_start - default_start).abs() <= 1e-6;
    let auto_phase = (auto_start - default_start).abs() > 0.25
        && (auto_start - mantle_state.start_offset).abs() <= 1e-6;
    let controls = mantle_start_allowed(default_control, false)
        && auto_mantle_start_allowed(default_control, false, LocomotionState::Air)
        && !mantle_start_allowed(disabled_control, false)
        && !auto_mantle_start_allowed(disabled_control, false, LocomotionState::Air);
    let pass = controls && permission_blocked && scale_preserved && default_phase && auto_phase;
    state.done = true;
    println!(
        "[BF102.49][GATE] controls={controls} permission_blocked={permission_blocked} heights=({:.4},{:.4}) height_delta={height_delta:.4} kinds=({:?},{:?}) default_start={default_start:.4} source_start={source_start:.4} auto_start={auto_start:.4} state_start={:.4} -> {}",
        allowed_intent.mantling_height,
        scaled_intent.mantling_height,
        allowed_intent.kind,
        scaled_intent.kind,
        mantle_state.start_offset,
        if pass { "PASS" } else { "FAIL" },
    );
    if pass {
        exit.write(AppExit::Success);
    } else {
        exit.write(AppExit::Error(NonZeroU8::new(49).unwrap()));
    }
}
