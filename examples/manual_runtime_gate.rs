use std::time::{Duration, Instant};

use avian3d::prelude::*;
use bevy::prelude::*;
use bevy_animation_graph::core::animated_scene::AnimatedSceneInstance;
use bevy_animation_graph::core::animation_graph_player::AnimationGraphPlayer;
use bevy_animation_graph::core::edge_data::DataValue;

use als_core::character::{AlsCharacter, AlsMovementParamsHandle};
use als_core::motion::AlsInputOverride;
use als_core::params::AlsMovementParams;
use als_core::state::{Gait, LocomotionState, OverlayState, RotationMode, Stance};

use bevy_als::anim_probe::AnimProbePlugin;
use bevy_als::playable_character::{
    AlsCharacterBlueprint, AlsPlayableCharacterPlugin, spawn_als_character,
};

const TOTAL_SECS: f64 = 19.0;
const SECONDARY_MOTION_DURATION: f32 = 2.666667;

fn main() {
    let mut app = App::new();
    app.add_plugins((
        DefaultPlugins.set(AssetPlugin {
            file_path: "assets".into(),
            ..default()
        }),
        AlsPlayableCharacterPlugin,
        AnimProbePlugin,
    ))
    .init_resource::<SpawnOnce>()
    .init_resource::<JumpBoosted>()
    .init_resource::<FallLaunched>()
    .init_resource::<DemoDriver>()
    .init_resource::<GateVerdict>()
    .insert_resource(AutoQuit(
        Instant::now() + Duration::from_secs_f64(TOTAL_SECS + 8.0),
    ))
    .add_systems(Startup, setup_scene)
    .add_systems(
        PreUpdate,
        (
            demo_driver,
            launch_fall_for_phase_coverage.after(demo_driver),
        ),
    )
    .add_systems(
        Update,
        (
            spawn_character_once,
            configure_jump_for_phase_coverage.after(spawn_character_once),
            gate_watcher,
            auto_quit,
        ),
    );

    info!("[MANUAL-RUNTIME-GATE] armed total={TOTAL_SECS:.1}s");
    if let AppExit::Error(code) = app.run() {
        std::process::exit(i32::from(code.get()));
    }
}

#[derive(Resource, Default)]
struct SpawnOnce(bool);

#[derive(Resource, Default)]
struct JumpBoosted(bool);

#[derive(Resource, Default)]
struct FallLaunched(bool);

#[derive(Resource)]
struct AutoQuit(Instant);

#[derive(Resource)]
struct DemoDriver {
    phase: usize,
    phase_started_at: f64,
}

impl Default for DemoDriver {
    fn default() -> Self {
        Self {
            phase: usize::MAX,
            phase_started_at: 0.0,
        }
    }
}

#[derive(Resource, Default)]
struct GateVerdict {
    forward_direction: bool,
    backward_direction: bool,
    sideways_direction: bool,
    crouch_pose: bool,
    run_gait: bool,
    airborne: bool,
    jump_samples: u32,
    jump_variant_samples: u32,
    jump_variant: Option<f32>,
    jump_variant_changed: bool,
    standing_marker_samples: u32,
    crouching_marker_samples: u32,
    jump_phase_samples: u32,
    fall_phase_samples: u32,
    land_phase_samples: u32,
    rifle_overlay_samples: u32,
    overlay_switches: u32,
    secondary_continuity_samples: u32,
    secondary_reset_detected: bool,
    last_overlay_index: Option<f32>,
    last_secondary: Option<(f64, f32)>,
    last_log: f64,
    done: bool,
}

fn setup_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let ground = materials.add(Color::srgb(0.22, 0.26, 0.22));
    commands.spawn((
        RigidBody::Static,
        Collider::cuboid(40.0, 0.2, 40.0),
        Transform::from_xyz(0.0, -0.2, 0.0),
        Mesh3d(meshes.add(Cuboid::new(80.0, 0.4, 80.0))),
        MeshMaterial3d(ground),
    ));
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(5.0, 3.0, 7.0).looking_at(Vec3::new(0.0, 1.0, 0.0), Vec3::Y),
    ));
    commands.spawn((
        DirectionalLight {
            illuminance: 80_000.0,
            ..default()
        },
        Transform::from_xyz(8.0, 12.0, 4.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}

fn spawn_character_once(
    mut commands: Commands,
    mut once: ResMut<SpawnOnce>,
    mut params: ResMut<Assets<AlsMovementParams>>,
    mut configs: ResMut<Assets<als_core::control_scheme::AlsControlSchemeConfig>>,
    assets: Res<AssetServer>,
) {
    if once.0 {
        return;
    }
    once.0 = true;
    let mut blueprint = AlsCharacterBlueprint::new(Transform::from_xyz(0.0, 0.95, 0.0));
    blueprint.spawn_camera = false;
    let character =
        spawn_als_character(&mut commands, &assets, &mut params, &mut configs, blueprint);
    commands.entity(character).insert((
        AlsInputOverride::default(),
        Gait::Running,
        RotationMode::ViewDirection,
    ));
    info!("[MANUAL-RUNTIME-GATE] character spawned");
}

fn configure_jump_for_phase_coverage(
    mut boosted: ResMut<JumpBoosted>,
    characters: Query<&AlsMovementParamsHandle>,
    mut params: ResMut<Assets<AlsMovementParams>>,
) {
    if boosted.0 {
        return;
    }
    let Ok(handle) = characters.single() else {
        return;
    };
    let Some(mut values) = params.get_mut(&handle.0) else {
        return;
    };
    values.jump.jump_z_velocity = 15.0;
    boosted.0 = true;
    info!("[MANUAL-RUNTIME-GATE] jump velocity raised for fall phase coverage");
}

fn launch_fall_for_phase_coverage(
    demo: Res<DemoDriver>,
    mut launched: ResMut<FallLaunched>,
    mut characters: Query<(&mut Transform, &mut Position, &mut LinearVelocity), With<AlsCharacter>>,
) {
    if launched.0 || demo.phase != 6 {
        return;
    }
    let Ok((mut transform, mut position, mut velocity)) = characters.single_mut() else {
        return;
    };
    let target = Vec3::new(transform.translation.x, 8.0, transform.translation.z);
    transform.translation = target;
    position.0 = target;
    velocity.0 = Vec3::ZERO;
    launched.0 = true;
    info!("[MANUAL-RUNTIME-GATE] fall launch applied for Fall phase coverage");
}

fn phase_at(t: f64) -> usize {
    match t {
        t if t < 1.5 => 0,
        t if t < 3.0 => 1,
        t if t < 4.5 => 2,
        t if t < 6.5 => 3,
        t if t < 8.5 => 4,
        t if t < 12.0 => 5,
        t if t < 13.5 => 6,
        t if t < 15.0 => 7,
        t if t < 16.5 => 8,
        t if t < 18.0 => 9,
        _ => 10,
    }
}

fn demo_driver(
    time: Res<Time>,
    mut demo: ResMut<DemoDriver>,
    mut characters: Query<(&mut AlsInputOverride, &mut Gait, &mut OverlayState)>,
) {
    let Ok((mut input, mut gait, mut overlay)) = characters.single_mut() else {
        return;
    };
    let now = time.elapsed_secs_f64();
    let phase = phase_at(now);
    let entered = phase != demo.phase;
    if entered {
        demo.phase = phase;
        demo.phase_started_at = now;
    }
    let jump_pulse = now - demo.phase_started_at < 0.5;
    input.0.reset();
    *gait = Gait::Running;
    *overlay = if matches!(phase, 7 | 9) {
        OverlayState::Rifle
    } else {
        OverlayState::Default
    };
    match phase {
        0 => {}
        1 => input.0.move_dir = Vec2::NEG_Y,
        2 => input.0.move_dir = Vec2::X,
        3 => {
            input.0.move_dir = Vec2::NEG_Y;
            input.0.crouch_pressed = entered;
        }
        4 => {
            input.0.move_dir = Vec2::Y;
            input.0.crouch_pressed = entered;
        }
        5 => {
            input.0.move_dir = Vec2::Y;
            input.0.jump_pressed = jump_pulse;
            input.0.jump_held = jump_pulse;
        }
        6 | 7 | 8 | 9 => {}
        _ => input.0.move_dir = Vec2::Y,
    }
}

fn read_pin(player: &AnimationGraphPlayer, name: &str) -> Option<f32> {
    match player.get_input_data(name.to_owned()) {
        Some(DataValue::F32(value)) => Some(value),
        _ => None,
    }
}

fn read_global(player: &AnimationGraphPlayer, name: &str) -> Option<f32> {
    match player.get_global_input_data(name) {
        Some(DataValue::F32(value)) => Some(*value),
        _ => None,
    }
}

fn gate_watcher(
    time: Res<Time>,
    mut verdict: ResMut<GateVerdict>,
    characters: Query<(
        &LinearVelocity,
        &Stance,
        &Gait,
        &LocomotionState,
        &OverlayState,
    )>,
    avatars: Query<(&ChildOf, &AnimatedSceneInstance)>,
    players: Query<&AnimationGraphPlayer>,
    mut exit: MessageWriter<AppExit>,
) {
    if verdict.done {
        return;
    }
    let Ok((velocity, stance, gait, locomotion, overlay)) = characters.single() else {
        return;
    };
    let Some((_, instance)) = avatars.iter().next() else {
        return;
    };
    let Ok(player) = players.get(instance.player_entity()) else {
        return;
    };
    let speed = Vec2::new(velocity.x, velocity.z).length();
    let direction = read_pin(player, "direction_angle").unwrap_or(f32::NAN);
    let stance_weight = read_pin(player, "stance_weight").unwrap_or(f32::NAN);
    let run_weight = read_pin(player, "gait_run_weight").unwrap_or(f32::NAN);
    let jump_left = read_pin(player, "jump_left_weight").unwrap_or(f32::NAN);
    let jump_run = read_pin(player, "jump_run_weight").unwrap_or(f32::NAN);
    let movement_phase = read_global(player, "als_movement_marker_phase").unwrap_or(f32::NAN);
    let fall_phase = read_global(player, "als_locomotion_fall_phase").unwrap_or(f32::NAN);
    let jump_phase = read_global(player, "als_locomotion_jump_phase").unwrap_or(f32::NAN);
    let land_phase = read_global(player, "als_locomotion_land_phase").unwrap_or(f32::NAN);
    let jump_state = read_global(player, "als_locomotion_jump_state_index").unwrap_or(f32::NAN);
    let secondary_phase = read_global(player, "als_secondary_motion_phase").unwrap_or(f32::NAN);
    let overlay_index = read_global(player, "als_root_overlay_index").unwrap_or(f32::NAN);
    let t = time.elapsed_secs_f64();

    if ((1.5..3.0).contains(&t) || (4.8..6.5).contains(&t))
        && speed > 0.5
        && direction.abs() > 150.0
    {
        verdict.backward_direction = true;
    }
    if (3.4..4.5).contains(&t) && speed > 0.5 && direction.abs() > 60.0 && direction.abs() < 120.0 {
        verdict.sideways_direction = true;
    }
    if (5.0..6.5).contains(&t) && *stance == Stance::Crouching && speed > 0.5 {
        verdict.crouch_pose = true;
    }
    if (7.2..8.5).contains(&t) && speed > 2.5 && direction.abs() < 25.0 && *gait == Gait::Running {
        verdict.forward_direction = true;
        verdict.run_gait = true;
    }
    if (1.5..4.5).contains(&t) && speed > 0.5 && movement_phase.is_finite() {
        verdict.standing_marker_samples += 1;
    }
    if (5.0..6.5).contains(&t)
        && *stance == Stance::Crouching
        && speed > 0.5
        && movement_phase.is_finite()
    {
        verdict.crouching_marker_samples += 1;
    }
    let phase = phase_at(t);
    if *locomotion == LocomotionState::Air && matches!(phase, 5 | 6) {
        verdict.airborne = true;
        if phase == 5 {
            verdict.jump_samples += 1;
            if jump_state.is_finite() {
                if jump_phase < 0.9 {
                    if let Some(previous) = verdict.jump_variant {
                        if (previous - jump_state).abs() > 1e-4 {
                            verdict.jump_variant_changed = true;
                        }
                    } else {
                        verdict.jump_variant = Some(jump_state);
                    }
                    verdict.jump_variant_samples += 1;
                }
            } else {
                verdict.jump_variant_changed = true;
            }
            if jump_phase.is_finite() && jump_phase > 1e-4 {
                verdict.jump_phase_samples += 1;
            }
        }
        if fall_phase.is_finite() && fall_phase > 1e-4 {
            verdict.fall_phase_samples += 1;
        }
    }
    if verdict.airborne
        && *locomotion == LocomotionState::Grounded
        && land_phase.is_finite()
        && land_phase > 1e-4
    {
        verdict.land_phase_samples += 1;
    }
    if *overlay == OverlayState::Rifle && overlay_index.is_finite() && secondary_phase.is_finite() {
        verdict.rifle_overlay_samples += 1;
    }
    if overlay_index.is_finite() && secondary_phase.is_finite() {
        if let Some(previous_index) = verdict.last_overlay_index
            && (overlay_index - previous_index).abs() > 1e-4
        {
            verdict.overlay_switches += 1;
            if let Some((previous_time, previous_phase)) = verdict.last_secondary {
                let expected =
                    ((t - previous_time) as f32 / SECONDARY_MOTION_DURATION).clamp(0.0, 1.0);
                let actual = (secondary_phase - previous_phase).rem_euclid(1.0);
                if (actual - expected).abs() <= 0.04 {
                    verdict.secondary_continuity_samples += 1;
                    info!(
                        "[MANUAL-RUNTIME-GATE] secondary switch overlay={previous_index:.1}->{overlay_index:.1} expected_delta={expected:.4} actual_delta={actual:.4} continuous=true"
                    );
                } else {
                    verdict.secondary_reset_detected = true;
                    warn!(
                        "[MANUAL-RUNTIME-GATE] secondary switch overlay={previous_index:.1}->{overlay_index:.1} expected_delta={expected:.4} actual_delta={actual:.4} continuous=false"
                    );
                }
            }
        }
        verdict.last_overlay_index = Some(overlay_index);
        verdict.last_secondary = Some((t, secondary_phase));
    }

    if t - verdict.last_log >= 0.25 {
        verdict.last_log = t;
        info!(
            "[MANUAL-RUNTIME-GATE] t={t:.2} speed={speed:.2} stance={stance:?} gait={gait:?} loco={locomotion:?} overlay={overlay:?} pins(dir={direction:.1},stance={stance_weight:.2},run={run_weight:.2},jump_l={jump_left:.2},jump_run={jump_run:.2}) phases(move={movement_phase:.3},jump={jump_phase:.3},fall={fall_phase:.3},land={land_phase:.3},jump_state={jump_state:.1},secondary={secondary_phase:.3},overlay={overlay_index:.1})"
        );
    }

    if t >= TOTAL_SECS {
        verdict.done = true;
        let jump_variant_locked = verdict.jump_variant_samples >= 2
            && verdict.jump_variant.is_some()
            && !verdict.jump_variant_changed;
        let phase_coverage = verdict.standing_marker_samples >= 2
            && verdict.crouching_marker_samples >= 2
            && verdict.jump_phase_samples >= 2
            && verdict.fall_phase_samples >= 2
            && verdict.land_phase_samples >= 2;
        let secondary_continuity = verdict.rifle_overlay_samples >= 2
            && verdict.overlay_switches >= 2
            && verdict.secondary_continuity_samples >= 2
            && !verdict.secondary_reset_detected;
        let pass = verdict.forward_direction
            && verdict.backward_direction
            && verdict.sideways_direction
            && verdict.crouch_pose
            && verdict.run_gait
            && verdict.airborne
            && jump_variant_locked
            && phase_coverage
            && secondary_continuity;
        info!(
            "[MANUAL-RUNTIME-GATE][VERDICT] forward={} backward={} sideways={} crouch={} run={} airborne={} jump_samples={} jump_variant_samples={} jump_variant={:?} jump_variant_changed={} jump_variant_locked={} standing_marker_samples={} crouching_marker_samples={} jump_phase_samples={} fall_phase_samples={} land_phase_samples={} rifle_overlay_samples={} overlay_switches={} secondary_continuity_samples={} secondary_reset_detected={} -> {}",
            verdict.forward_direction,
            verdict.backward_direction,
            verdict.sideways_direction,
            verdict.crouch_pose,
            verdict.run_gait,
            verdict.airborne,
            verdict.jump_samples,
            verdict.jump_variant_samples,
            verdict.jump_variant,
            verdict.jump_variant_changed,
            jump_variant_locked,
            verdict.standing_marker_samples,
            verdict.crouching_marker_samples,
            verdict.jump_phase_samples,
            verdict.fall_phase_samples,
            verdict.land_phase_samples,
            verdict.rifle_overlay_samples,
            verdict.overlay_switches,
            verdict.secondary_continuity_samples,
            verdict.secondary_reset_detected,
            if pass { "PASS" } else { "FAIL" },
        );
        if pass {
            exit.write(AppExit::Success);
        } else {
            exit.write(AppExit::Error(std::num::NonZeroU8::new(7).unwrap()));
        }
    }
}

fn auto_quit(time: Res<AutoQuit>, mut exit: MessageWriter<AppExit>) {
    if Instant::now() >= time.0 {
        warn!("[MANUAL-RUNTIME-GATE] wall-clock safety quit");
        exit.write(AppExit::Error(std::num::NonZeroU8::new(8).unwrap()));
    }
}
