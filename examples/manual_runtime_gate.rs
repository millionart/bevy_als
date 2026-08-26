use std::time::{Duration, Instant};

use avian3d::prelude::*;
use bevy::prelude::*;
use bevy_animation_graph::core::animated_scene::AnimatedSceneInstance;
use bevy_animation_graph::core::animation_graph_player::AnimationGraphPlayer;
use bevy_animation_graph::core::edge_data::DataValue;

use als_core::motion::AlsInputOverride;
use als_core::params::AlsMovementParams;
use als_core::state::{Gait, LocomotionState, RotationMode, Stance};

use bevy_als::anim_probe::AnimProbePlugin;
use bevy_als::playable_character::{
    AlsCharacterBlueprint, AlsPlayableCharacterPlugin, spawn_als_character,
};

const TOTAL_SECS: f64 = 12.0;

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
    .init_resource::<DemoDriver>()
    .init_resource::<GateVerdict>()
    .insert_resource(AutoQuit(
        Instant::now() + Duration::from_secs_f64(TOTAL_SECS + 8.0),
    ))
    .add_systems(Startup, setup_scene)
    .add_systems(PreUpdate, demo_driver)
    .add_systems(Update, (spawn_character_once, gate_watcher, auto_quit));

    info!("[MANUAL-RUNTIME-GATE] armed total={TOTAL_SECS:.1}s");
    app.run();
}

#[derive(Resource, Default)]
struct SpawnOnce(bool);

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
    jump_variant: Option<(bool, bool)>,
    jump_variant_changed: bool,
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

fn phase_at(t: f64) -> usize {
    match t {
        t if t < 1.5 => 0,
        t if t < 3.0 => 1,
        t if t < 4.5 => 2,
        t if t < 6.5 => 3,
        t if t < 8.5 => 4,
        t if t < 11.0 => 5,
        _ => 6,
    }
}

fn demo_driver(
    time: Res<Time>,
    mut demo: ResMut<DemoDriver>,
    mut characters: Query<(&mut AlsInputOverride, &mut Gait)>,
) {
    let Ok((mut input, mut gait)) = characters.single_mut() else {
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
        _ => input.0.move_dir = Vec2::Y,
    }
}

fn read_pin(player: &AnimationGraphPlayer, name: &str) -> Option<f32> {
    match player.get_input_data(name.to_owned()) {
        Some(DataValue::F32(value)) => Some(value),
        _ => None,
    }
}

fn gate_watcher(
    time: Res<Time>,
    mut verdict: ResMut<GateVerdict>,
    characters: Query<(&LinearVelocity, &Stance, &LocomotionState)>,
    avatars: Query<(&ChildOf, &AnimatedSceneInstance)>,
    players: Query<&AnimationGraphPlayer>,
    mut exit: MessageWriter<AppExit>,
) {
    if verdict.done {
        return;
    }
    let Ok((velocity, stance, locomotion)) = characters.single() else {
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
    if (5.0..6.5).contains(&t) && *stance == Stance::Crouching && stance_weight > 0.8 {
        verdict.crouch_pose = true;
    }
    if (7.2..8.5).contains(&t) && speed > 2.5 && direction.abs() < 25.0 && run_weight > 0.8 {
        verdict.forward_direction = true;
        verdict.run_gait = true;
    }
    let phase = phase_at(t);
    if *locomotion == LocomotionState::Air && phase == 5 {
        verdict.airborne = true;
        let sample = (jump_left > 0.8, jump_run > 0.8);
        if let Some(previous) = verdict.jump_variant {
            if previous != sample {
                verdict.jump_variant_changed = true;
            }
        } else {
            verdict.jump_variant = Some(sample);
        }
        verdict.jump_samples += 1;
    }

    if t - verdict.last_log >= 0.25 {
        verdict.last_log = t;
        info!(
            "[MANUAL-RUNTIME-GATE] t={t:.2} speed={speed:.2} stance={stance:?} loco={locomotion:?} pins(dir={direction:.1},stance={stance_weight:.2},run={run_weight:.2},jump_l={jump_left:.2},jump_run={jump_run:.2})"
        );
    }

    if t >= TOTAL_SECS {
        verdict.done = true;
        let jump_variant_locked = verdict.jump_samples >= 2 && !verdict.jump_variant_changed;
        let pass = verdict.forward_direction
            && verdict.backward_direction
            && verdict.sideways_direction
            && verdict.crouch_pose
            && verdict.run_gait
            && verdict.airborne
            && jump_variant_locked;
        info!(
            "[MANUAL-RUNTIME-GATE][VERDICT] forward={} backward={} sideways={} crouch={} run={} airborne={} jump_samples={} jump_variant={:?} jump_variant_changed={} jump_variant_locked={} -> {}",
            verdict.forward_direction,
            verdict.backward_direction,
            verdict.sideways_direction,
            verdict.crouch_pose,
            verdict.run_gait,
            verdict.airborne,
            verdict.jump_samples,
            verdict.jump_variant,
            verdict.jump_variant_changed,
            jump_variant_locked,
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
