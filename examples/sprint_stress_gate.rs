use std::time::{Duration, Instant};

use als_camera::AlsCameraRig;
use als_core::character::AlsCharacter;
use als_core::control_scheme::AlsControlSchemeConfig;
use als_core::input::AlsInputCommand;
use als_core::motion::AlsInputOverride;
use als_core::params::AlsMovementParams;
use als_core::state::Gait;
use avian3d::prelude::{Collider, LinearVelocity, PhysicsSystems, Position, RigidBody, Rotation};
use bevy::ecs::message::MessageWriter;
use bevy::prelude::*;
use bevy::window::ExitCondition;
use bevy_animation_graph::core::animated_scene::AnimatedSceneInstance;
use bevy_animation_graph::core::animation_graph::AnimationGraph;
use bevy_animation_graph::core::animation_graph_player::AnimationGraphPlayer;
use bevy_animation_graph::core::edge_data::DataValue;
use bevy_animation_graph::core::plugin::AnimationGraphSet;
use bevy_animation_graph::core::ragdoll::{
    definition::BodyLabel,
    relative_kinematic_body::{RelativeKinematicBody, RelativeKinematicBodyPositionBased},
};

use bevy_als::playable_character::{
    AlsCharacterAvatar, AlsCharacterBlueprint, AlsPlayableCharacterPlugin, spawn_als_character,
};

const STRESS_DURATION_SECS: f32 = 8.0;
const MIN_SPRINT_DURATION_SECS: f32 = 6.0;
const MIN_SPRINT_SPEED: f32 = 5.0;

#[derive(Resource)]
struct Deadline(Instant);

#[derive(Resource)]
struct RootGraph(Handle<AnimationGraph>);

#[derive(Resource, Default)]
struct Probe {
    character: Option<Entity>,
    started_at: Option<f32>,
    complete_actor: bool,
    input_active: bool,
    camera_active: bool,
    pose_finite: bool,
    animation_error: bool,
    graph_error_logged: bool,
    nonfinite_velocity_logged: bool,
    sprint_frames: u32,
    sprint_seconds: f32,
    max_speed: f32,
    next_log_seconds: f32,
    finished: bool,
}

fn main() {
    let mut app = App::new();
    app.add_plugins((
        DefaultPlugins
            .set(AssetPlugin {
                file_path: "assets".into(),
                ..default()
            })
            .set(WindowPlugin {
                primary_window: None,
                exit_condition: ExitCondition::DontExit,
                ..default()
            }),
        AlsPlayableCharacterPlugin,
    ))
    .init_resource::<Probe>()
    .insert_resource(Deadline(Instant::now() + Duration::from_secs(20)))
    .add_systems(Startup, setup)
    .add_systems(Update, spawn_when_ready)
    .add_systems(
        FixedPostUpdate,
        detect_nonfinite_velocity
            .after(AnimationGraphSet::PrePhysics)
            .before(PhysicsSystems::First),
    )
    .add_systems(PostUpdate, observe)
    .add_systems(Update, timeout);
    app.run();
}

fn setup(mut commands: Commands, assets: Res<AssetServer>) {
    commands.spawn((
        RigidBody::Static,
        Collider::cuboid(100.0, 0.1, 100.0),
        Transform::from_xyz(0.0, -0.1, 0.0),
    ));
    commands.insert_resource(RootGraph(assets.load("graphs/ab_als/ab_als.animgraph.ron")));
    info!("[SPRINT-STRESS] waiting for AB_Als recursive dependencies");
}

fn spawn_when_ready(
    mut commands: Commands,
    virtual_time: Res<Time<Virtual>>,
    assets: Res<AssetServer>,
    root_graph: Res<RootGraph>,
    mut params_assets: ResMut<Assets<AlsMovementParams>>,
    mut config_assets: ResMut<Assets<AlsControlSchemeConfig>>,
    mut probe: ResMut<Probe>,
) {
    if probe.character.is_some() || !assets.is_loaded_with_dependencies(root_graph.0.id()) {
        return;
    }
    let character = spawn_als_character(
        &mut commands,
        &assets,
        &mut params_assets,
        &mut config_assets,
        AlsCharacterBlueprint {
            transform: Transform::from_xyz(0.0, 0.95, 0.0),
            spawn_camera: true,
            ..AlsCharacterBlueprint::new(Transform::IDENTITY)
        },
    );
    commands
        .entity(character)
        .insert(AlsInputOverride(AlsInputCommand {
            move_dir: Vec2::NEG_Y,
            sprint_held: true,
            ..default()
        }));
    probe.character = Some(character);
    probe.started_at = Some(virtual_time.elapsed_secs());
    info!("[SPRINT-STRESS] AB_Als ready; complete actor spawned with persistent sprint input");
}

fn pose_is_finite(player: &AnimationGraphPlayer) -> Option<bool> {
    let DataValue::Pose(pose) = player.get_outputs().get("pose")? else {
        return None;
    };
    Some(
        pose.bones.len() >= 70
            && pose.bones.iter().all(|bone| {
                bone.translation.map_or(true, Vec3::is_finite)
                    && bone.rotation.map_or(true, Quat::is_finite)
                    && bone.scale.map_or(true, Vec3::is_finite)
            }),
    )
}

fn detect_nonfinite_velocity(
    mut probe: ResMut<Probe>,
    bodies: Query<(
        Entity,
        &LinearVelocity,
        Option<&Name>,
        Option<&BodyLabel>,
        Option<&ChildOf>,
        Option<&RigidBody>,
        Option<&Position>,
        Option<&Rotation>,
        Option<&RelativeKinematicBody>,
        Option<&RelativeKinematicBodyPositionBased>,
    )>,
) {
    if probe.nonfinite_velocity_logged {
        return;
    }
    for (entity, velocity, name, label, parent, body, position, rotation, relative, target) in
        &bodies
    {
        if velocity.is_finite() {
            continue;
        }
        error!(
            "[SPRINT-STRESS][NONFINITE] entity={entity:?} velocity={:?} name={:?} label={:?} parent={:?} body={body:?} position={position:?} rotation={rotation:?} relative={relative:?} target={target:?}",
            **velocity,
            name.map(Name::as_str),
            label.map(|value| value.0.as_str()),
            parent.map(ChildOf::parent),
        );
        probe.nonfinite_velocity_logged = true;
        return;
    }
}

fn observe(
    virtual_time: Res<Time<Virtual>>,
    mut probe: ResMut<Probe>,
    characters: Query<(&Gait, &AlsInputOverride, &LinearVelocity), With<AlsCharacter>>,
    avatars: Query<(&ChildOf, &AnimatedSceneInstance), With<AlsCharacterAvatar>>,
    players: Query<&AnimationGraphPlayer>,
    cameras: Query<(), (With<Camera3d>, With<AlsCameraRig>)>,
    mut exit: MessageWriter<AppExit>,
) {
    if probe.finished {
        return;
    }
    let Some(character) = probe.character else {
        return;
    };
    let Ok((gait, input, velocity)) = characters.get(character) else {
        return;
    };
    probe.input_active = input.0.sprint_held && input.0.move_dir == Vec2::NEG_Y;
    probe.camera_active = cameras.iter().count() == 1;
    let speed = Vec2::new(velocity.x, velocity.z).length();
    probe.max_speed = probe.max_speed.max(speed);
    if *gait == Gait::Sprinting {
        probe.sprint_frames += 1;
        probe.sprint_seconds += virtual_time.delta_secs();
    }
    let Some((_, instance)) = avatars
        .iter()
        .find(|(parent, _)| parent.parent() == character)
    else {
        return;
    };
    let Ok(player) = players.get(instance.player_entity()) else {
        return;
    };
    probe.complete_actor = true;
    if let Some(graph_error) = player.get_error() {
        probe.animation_error = true;
        if !probe.graph_error_logged {
            error!("[SPRINT-STRESS][GRAPH-ERROR] {graph_error:?}");
            probe.graph_error_logged = true;
        }
    }
    if let Some(finite) = pose_is_finite(player) {
        probe.pose_finite = finite;
    }
    let Some(started_at) = probe.started_at else {
        return;
    };
    let elapsed = virtual_time.elapsed_secs() - started_at;
    if elapsed >= probe.next_log_seconds {
        let ragdoll_bodies = player
            .get_spawned_ragdoll()
            .map_or(0, |spawned| spawned.bodies.len());
        info!(
            "[SPRINT-STRESS] elapsed={elapsed:.2} gait={gait:?} speed={speed:.3} sprint_frames={} sprint_seconds={:.3} input_active={} camera_active={} pose_finite={} animation_error={} ragdoll_bodies={ragdoll_bodies}",
            probe.sprint_frames,
            probe.sprint_seconds,
            probe.input_active,
            probe.camera_active,
            probe.pose_finite,
            probe.animation_error,
        );
        probe.next_log_seconds += 1.0;
    }
    if elapsed < STRESS_DURATION_SECS {
        return;
    }
    let pass = probe.complete_actor
        && probe.input_active
        && probe.camera_active
        && probe.pose_finite
        && !probe.animation_error
        && probe.sprint_seconds >= MIN_SPRINT_DURATION_SECS
        && probe.max_speed >= MIN_SPRINT_SPEED;
    info!(
        "[SPRINT-STRESS][VERDICT] actor={} input={} camera={} pose={} animation_error={} sprint_frames={} sprint_seconds={:.3} max_speed={:.3} -> {}",
        probe.complete_actor,
        probe.input_active,
        probe.camera_active,
        probe.pose_finite,
        probe.animation_error,
        probe.sprint_frames,
        probe.sprint_seconds,
        probe.max_speed,
        if pass { "PASS" } else { "FAIL" },
    );
    probe.finished = true;
    if pass {
        exit.write(AppExit::Success);
    } else {
        exit.write(AppExit::Error(std::num::NonZeroU8::new(20).unwrap()));
    }
}

fn timeout(probe: Res<Probe>, deadline: Res<Deadline>, mut exit: MessageWriter<AppExit>) {
    if !probe.finished && Instant::now() >= deadline.0 {
        error!("[SPRINT-STRESS][VERDICT] timeout -> FAIL");
        exit.write(AppExit::Error(std::num::NonZeroU8::new(21).unwrap()));
    }
}
