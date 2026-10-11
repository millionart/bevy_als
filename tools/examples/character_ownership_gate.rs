use std::num::NonZeroU8;
use std::time::{Duration, Instant};

use avian3d::prelude::{Collider, LinearVelocity, RigidBody};
use bevy::asset::AssetPlugin;
use bevy::ecs::message::MessageWriter;
use bevy::prelude::*;
use bevy_animation_graph::core::animated_scene::{AnimatedSceneHandle, AnimatedSceneInstance};
use bevy_animation_graph::core::animation_graph_player::AnimationGraphPlayer;
use bevy_animation_graph::core::edge_data::DataValue;
use bevy_animation_graph::core::plugin::AnimationGraphSet;
use bevy_animation_graph::core::systems::RootMotionOutput;

use bevy_als::als::character::AlsCharacter;
use bevy_als::als::control_scheme::AlsControlSchemeConfig;
use bevy_als::als::mantle::{AlsMantleState, MantleKind};
use bevy_als::als::roll::AlsRollState;
use bevy_als::als::state::{LocomotionState, Stance};
use bevy_als::character_assembly::{
    ALS_LINKED_ANIMATED_SCENE, AlsCharacterAvatar, AlsCharacterBlueprint, spawn_als_character,
};
use bevy_als::mantle_drive::AlsMantleDriveSet;
use bevy_als::playable_character::AlsPlayableCharacterPlugin;

#[derive(Resource)]
struct Deadline(Instant);

#[derive(Resource)]
struct Probe {
    first: Entity,
    second: Entity,
    unrelated: Entity,
    applied: Option<Instant>,
}

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins.set(AssetPlugin {
                file_path: "assets".into(),
                ..default()
            }),
            AlsPlayableCharacterPlugin,
        ))
        .insert_resource(Deadline(Instant::now() + Duration::from_secs(15)))
        .add_systems(Startup, setup)
        .add_systems(Update, (apply_probe_state, verify, timeout))
        .add_systems(
            FixedPostUpdate,
            pin_root_motion
                .after(AnimationGraphSet::PrePhysics)
                .before(AlsMantleDriveSet),
        )
        .run();
}

fn setup(
    mut commands: Commands,
    assets: Res<AssetServer>,
    mut params_assets: ResMut<Assets<bevy_als::als::params::AlsMovementParams>>,
    mut config_assets: ResMut<Assets<AlsControlSchemeConfig>>,
) {
    commands.spawn((
        RigidBody::Static,
        Collider::cuboid(20.0, 0.2, 20.0),
        Transform::from_xyz(0.0, -0.1, 0.0),
    ));
    let mut first_blueprint = AlsCharacterBlueprint::new(Transform::from_xyz(-2.0, 0.92, 0.0));
    first_blueprint.spawn_input = false;
    let first = spawn_als_character(
        &mut commands,
        &assets,
        &mut params_assets,
        &mut config_assets,
        first_blueprint,
    );
    let mut second_blueprint = AlsCharacterBlueprint::new(Transform::from_xyz(2.0, 0.92, 0.0));
    second_blueprint.spawn_input = false;
    let second = spawn_als_character(
        &mut commands,
        &assets,
        &mut params_assets,
        &mut config_assets,
        second_blueprint,
    );
    let unrelated = commands
        .spawn(AnimatedSceneHandle::new(
            assets.load(ALS_LINKED_ANIMATED_SCENE),
        ))
        .id();
    commands.insert_resource(Probe {
        first,
        second,
        unrelated,
        applied: None,
    });
}

fn apply_probe_state(
    mut commands: Commands,
    mut probe: ResMut<Probe>,
    avatars: Query<(&AlsCharacterAvatar, &AnimatedSceneInstance)>,
    instances: Query<&AnimatedSceneInstance>,
    mut rolls: Query<&mut AlsRollState, With<AlsCharacter>>,
    mut mantles: Query<&mut AlsMantleState, With<AlsCharacter>>,
    mut transforms: Query<&mut Transform, With<AlsCharacter>>,
    mut locomotion: Query<&mut LocomotionState, With<AlsCharacter>>,
) {
    if probe.applied.is_some()
        || avatars
            .iter()
            .filter(|(avatar, _)| {
                avatar.character == probe.first || avatar.character == probe.second
            })
            .count()
            != 2
        || instances.get(probe.unrelated).is_err()
    {
        return;
    }
    let (Ok(mut first_roll), Ok(mut second_mantle), Ok(second_transform)) = (
        rolls.get_mut(probe.first),
        mantles.get_mut(probe.second),
        transforms.get_mut(probe.second),
    ) else {
        return;
    };
    first_roll.active = true;
    first_roll.time = 0.25;
    first_roll.direction_yaw = 0.0;
    first_roll.stance_before_roll = Stance::Standing;
    second_mantle.begin(
        MantleKind::High,
        second_transform.translation,
        second_transform.translation,
        1.25,
        0.0,
        0.0,
    );
    if let Ok(mut first_locomotion) = locomotion.get_mut(probe.first) {
        *first_locomotion = LocomotionState::None;
        commands.entity(probe.first).insert(bevy_als::als::state::AlsNotifyLocomotionAction::Ragdolling);
    }
    if let Ok(mut second_locomotion) = locomotion.get_mut(probe.second) {
        *second_locomotion = LocomotionState::Grounded;
    }
    commands.entity(probe.unrelated).insert(RootMotionOutput {
        translation_delta: Vec3::new(0.0, 0.0, -99.0),
        rotation_delta: Quat::IDENTITY,
    });
    probe.applied = Some(Instant::now());
    info!("[BF102.15] two characters and one unrelated animated scene armed");
}

fn pin_root_motion(
    probe: Res<Probe>,
    avatars: Query<(Entity, &AlsCharacterAvatar)>,
    mut outputs: Query<&mut RootMotionOutput>,
) {
    if probe.applied.is_none() {
        return;
    }
    if let Some(second_avatar) = avatars
        .iter()
        .find_map(|(entity, avatar)| (avatar.character == probe.second).then_some(entity))
        && let Ok(mut output) = outputs.get_mut(second_avatar)
    {
        output.translation_delta = Vec3::ZERO;
        output.rotation_delta = Quat::IDENTITY;
    }
    if let Ok(mut output) = outputs.get_mut(probe.unrelated) {
        output.translation_delta = Vec3::new(0.0, 0.0, -99.0);
        output.rotation_delta = Quat::IDENTITY;
    }
}

fn input_f32(player: &AnimationGraphPlayer, name: &str) -> Option<f32> {
    match player.get_input_data(name.to_owned()) {
        Some(DataValue::F32(value)) => Some(value),
        _ => None,
    }
}

fn input_bool(player: &AnimationGraphPlayer, name: &str) -> Option<bool> {
    match player.get_input_data(name.to_owned()) {
        Some(DataValue::Bool(value)) => Some(value),
        _ => None,
    }
}

fn inactive(value: Option<f32>) -> bool {
    value.is_none_or(|value| value.abs() < 1e-4)
}

fn verify(
    probe: Res<Probe>,
    avatars: Query<(&AlsCharacterAvatar, &AnimatedSceneInstance)>,
    instances: Query<&AnimatedSceneInstance>,
    players: Query<&AnimationGraphPlayer>,
    velocities: Query<&LinearVelocity, With<AlsCharacter>>,
    mut exit: MessageWriter<AppExit>,
) {
    let Some(applied) = probe.applied else {
        return;
    };
    if applied.elapsed() < Duration::from_millis(750) {
        return;
    }
    let Some(first_instance) = avatars
        .iter()
        .find_map(|(avatar, instance)| (avatar.character == probe.first).then_some(instance))
    else {
        return;
    };
    let Some(second_instance) = avatars
        .iter()
        .find_map(|(avatar, instance)| (avatar.character == probe.second).then_some(instance))
    else {
        return;
    };
    let Ok(unrelated_instance) = instances.get(probe.unrelated) else {
        return;
    };
    let (Ok(first), Ok(second), Ok(unrelated), Ok(second_velocity)) = (
        players.get(first_instance.player_entity()),
        players.get(second_instance.player_entity()),
        players.get(unrelated_instance.player_entity()),
        velocities.get(probe.second),
    ) else {
        return;
    };
    let first_roll = input_f32(first, "roll_weight");
    let second_roll = input_f32(second, "roll_weight");
    let first_mantle = input_f32(first, "mantle_weight");
    let second_mantle = input_f32(second, "mantle_weight");
    let first_ragdoll = input_bool(first, "ragdoll_dynamic");
    let second_ragdoll = input_bool(second, "ragdoll_dynamic");
    let unrelated_clean = [
        "roll_weight",
        "mantle_weight",
        "ragdoll_dynamic",
        "ragdoll_readback",
    ]
    .iter()
    .all(|name| unrelated.get_input_data((*name).to_owned()).is_none());
    let root_motion_isolated = second_velocity.0.xz().length() < 0.1;
    let passed = first_roll.is_some_and(|value| value > 0.1)
        && inactive(second_roll)
        && inactive(first_mantle)
        && second_mantle.is_some_and(|value| value > 0.1)
        && first_ragdoll == Some(true)
        && second_ragdoll == Some(false)
        && unrelated_clean
        && root_motion_isolated;
    if passed {
        info!(
            "[BF102.15] first roll={:.3} mantle={:?} ragdoll={:?}; second roll={:?} mantle={:.3} ragdoll={:?}; unrelated_clean=true second_speed={:.3}",
            first_roll.unwrap_or_default(),
            first_mantle,
            first_ragdoll,
            second_roll,
            second_mantle.unwrap_or_default(),
            second_ragdoll,
            second_velocity.0.xz().length(),
        );
        info!("[BF102.15][VERDICT] character_owned_action_and_root_motion_paths -> PASS");
        exit.write(AppExit::Success);
    } else {
        error!(
            "[BF102.15][VERDICT] first_roll={first_roll:?} second_roll={second_roll:?} first_mantle={first_mantle:?} second_mantle={second_mantle:?} first_ragdoll={first_ragdoll:?} second_ragdoll={second_ragdoll:?} unrelated_clean={unrelated_clean} second_speed={:.3} -> FAIL",
            second_velocity.0.xz().length(),
        );
        exit.write(AppExit::Error(NonZeroU8::new(15).unwrap()));
    }
}

fn timeout(deadline: Res<Deadline>, mut exit: MessageWriter<AppExit>) {
    if Instant::now() >= deadline.0 {
        error!("[BF102.15][VERDICT] timeout -> FAIL");
        exit.write(AppExit::Error(NonZeroU8::new(16).unwrap()));
    }
}
