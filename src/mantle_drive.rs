use als_core::character::{
    AlsCharacter, AlsCollisionLayerOwner, AlsCollisionLayerOwnership, set_collision_layer_owner,
};
use als_core::mantle::{AlsMantleState, AlsMantleTargetBase, mantle_weight_at};
use avian3d::prelude::{CollisionLayers, LinearVelocity, Rotation};
use bevy::prelude::*;
use bevy_animation_graph::core::animated_scene::AnimatedSceneInstance;
use bevy_animation_graph::core::animation_graph_player::AnimationGraphPlayer;
use bevy_animation_graph::core::edge_data::DataValue;
use bevy_animation_graph::core::plugin::AnimationGraphSet;
use bevy_animation_graph::core::systems::RootMotionOutput;

use crate::character_assembly::AlsCharacterAvatar;
use crate::root_motion_link::{ensure_root_motion_output, read_local_root_motion_delta};

const MANTLE_ROOT_SAMPLER_GRAPH: &str = "graphs/actions.animgraph.ron";

#[derive(Component, Debug, Clone, Copy)]
struct AlsMantleRootSampler {
    character: Entity,
}

pub struct AlsMantleDrivePlugin;

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AlsMantleDriveSet;

impl Plugin for AlsMantleDrivePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                ensure_root_motion_output,
                ensure_mantle_state,
                ensure_mantle_root_samplers,
                mantle_pose_system.after(ensure_mantle_state),
            ),
        );
        app.configure_sets(
            PostUpdate,
            AlsMantleDriveSet.after(AnimationGraphSet::PrePhysics),
        )
        .add_systems(
            PostUpdate,
            prepare_mantle_root_samples.before(AnimationGraphSet::PrePhysics),
        )
        .add_systems(
            PostUpdate,
            mantle_drive_system
                .after(AnimationGraphSet::PrePhysics)
                .after(prepare_mantle_root_samples)
                .in_set(AlsMantleDriveSet),
        );
    }
}

fn ensure_mantle_state(
    mut commands: Commands,
    q: Query<
        Entity,
        (
            With<als_core::character::AlsCharacter>,
            Without<AlsMantleState>,
        ),
    >,
) {
    for entity in &q {
        commands.entity(entity).insert(AlsMantleState::default());
    }
}

fn ensure_mantle_root_samplers(
    asset_server: Option<Res<AssetServer>>,
    avatars: Query<(Entity, &AlsCharacterAvatar, &AnimatedSceneInstance), With<AlsCharacterAvatar>>,
    samplers: Query<&AlsMantleRootSampler>,
    players: Query<&AnimationGraphPlayer>,
    mut commands: Commands,
) {
    let Some(asset_server) = asset_server else {
        return;
    };
    for (avatar, character_avatar, instance) in &avatars {
        if samplers
            .iter()
            .any(|sampler| sampler.character == character_avatar.character)
        {
            continue;
        }
        let Ok(player) = players.get(instance.player_entity()) else {
            continue;
        };
        commands.spawn((
            Name::new("als_mantle_root_sampler"),
            Children::default(),
            ChildOf(avatar),
            AnimationGraphPlayer::new(player.skeleton().clone())
                .with_graph(asset_server.load(MANTLE_ROOT_SAMPLER_GRAPH)),
            RootMotionOutput::default(),
            AlsMantleRootSampler {
                character: character_avatar.character,
            },
        ));
    }
}

fn prepare_mantle_root_samples(
    time: Res<Time>,
    mantles: Query<&AlsMantleState, With<AlsCharacter>>,
    mut samplers: Query<(&AlsMantleRootSampler, &mut AnimationGraphPlayer)>,
) {
    let delta_seconds = time.delta_secs();
    for (sampler, mut player) in &mut samplers {
        let Some(mantle) = mantles
            .get(sampler.character)
            .ok()
            .filter(|mantle| mantle.active)
        else {
            player.set_input_data("mantle_weight", DataValue::from(0.0_f32));
            player.set_input_data("mantle_clip_time", DataValue::from(0.0_f32));
            continue;
        };
        let sample_time = (mantle.time + delta_seconds).min(mantle.action_end());
        player.set_input_data("mantle_weight", DataValue::from(1.0_f32));
        player.set_input_data(
            "mantle_high_w",
            DataValue::from(mantle.kind.is_high() as u8 as f32),
        );
        player.set_input_data(
            "mantle_lr",
            DataValue::from(mantle.kind.graph_low_right_weight()),
        );
        player.set_input_data(
            "mantle_clip_time",
            DataValue::from(mantle.clip_time_at(sample_time)),
        );
    }
}

fn mantle_pose_system(
    mut commands: Commands,
    target_transforms: Query<&GlobalTransform, Without<als_core::character::AlsCharacter>>,
    mut q: Query<
        (
            Entity,
            &mut AlsMantleState,
            &mut LinearVelocity,
            Option<&CollisionLayers>,
            &mut AlsCollisionLayerOwnership,
        ),
        With<AlsCharacter>,
    >,
) {
    for (entity, mut m, mut vel, layers, mut ownership) in &mut q {
        let target_base = mantle_target_base(&m, &target_transforms);
        let target_valid = !m.active || m.refresh_target_base(target_base);
        if !target_valid {
            *vel = LinearVelocity(Vec3::ZERO);
        }
        set_collision_layer_owner(
            &mut commands,
            entity,
            &mut ownership,
            layers.copied(),
            AlsCollisionLayerOwner::Mantle,
            m.active,
        );
    }
}

#[allow(clippy::type_complexity)]
fn mantle_drive_system(
    time: Res<Time>,
    mut commands: Commands,
    mut q: Query<
        (
            Entity,
            &mut AlsMantleState,
            &mut LinearVelocity,
            &mut Transform,
            Option<&mut Rotation>,
            Option<&CollisionLayers>,
            &mut AlsCollisionLayerOwnership,
        ),
        With<AlsCharacter>,
    >,
    target_transforms: Query<&GlobalTransform, Without<als_core::character::AlsCharacter>>,
    avatars: Query<(&AlsCharacterAvatar, &AnimatedSceneInstance)>,
    sampler_root_motion: Query<(&AlsMantleRootSampler, &RootMotionOutput)>,
    mut players: Query<&mut AnimationGraphPlayer>,
) {
    let dt = time.delta_secs();
    for (entity, mut m, mut vel, mut transform, rotation, layers, mut ownership) in &mut q {
        let target_base = mantle_target_base(&m, &target_transforms);
        let target_valid = !m.active || m.refresh_target_base(target_base);
        if !target_valid {
            *vel = LinearVelocity(Vec3::ZERO);
        }
        if !target_valid || !m.active || dt <= 1e-5 {
            set_collision_layer_owner(
                &mut commands,
                entity,
                &mut ownership,
                layers.copied(),
                AlsCollisionLayerOwner::Mantle,
                false,
            );
            continue;
        }

        let Some(instance) = avatars
            .iter()
            .find_map(|(avatar, instance)| (avatar.character == entity).then_some(instance))
        else {
            continue;
        };
        let Some(root_motion) = sampler_root_motion
            .iter()
            .find_map(|(sampler, output)| (sampler.character == entity).then_some(output))
        else {
            continue;
        };
        let previous_time = m.time;
        let current_time = (previous_time + dt).min(m.action_end());
        let (translation_delta, rotation_delta) = read_local_root_motion_delta(Some(root_motion));
        m.accumulate_root_motion(translation_delta, rotation_delta);
        let target_actor_transform = m.target_actor_transform_at(current_time);
        *vel = LinearVelocity((target_actor_transform.translation - transform.translation) / dt);
        transform.rotation = target_actor_transform.rotation;
        if let Some(mut rotation) = rotation {
            rotation.0 = target_actor_transform.rotation;
        }
        m.time = current_time;
        let finishing = current_time >= m.action_end();
        if finishing {
            m.active = false;
        }
        set_collision_layer_owner(
            &mut commands,
            entity,
            &mut ownership,
            layers.copied(),
            AlsCollisionLayerOwner::Mantle,
            m.active,
        );

        let weight = mantle_weight_at(current_time, m.action_end());
        let high_w = if m.kind.is_high() { 1.0 } else { 0.0 };
        let lr = m.kind.graph_low_right_weight();
        if let Ok(mut player) = players.get_mut(instance.player_entity()) {
            player.set_input_data(
                "mantle_clip_time",
                DataValue::from(m.clip_time_at(current_time)),
            );
            player.set_input_data("mantle_weight", DataValue::from(weight));
            player.set_input_data("mantle_high_w", DataValue::from(high_w));
            player.set_input_data("mantle_lr", DataValue::from(lr));
        }
    }
}

fn mantle_target_base(
    state: &AlsMantleState,
    transforms: &Query<&GlobalTransform, Without<als_core::character::AlsCharacter>>,
) -> Option<AlsMantleTargetBase> {
    let base = state.target_base?;
    let transform = transforms.get(base.entity).ok()?;
    Some(AlsMantleTargetBase {
        entity: base.entity,
        translation: transform.translation(),
        rotation: transform.rotation(),
    })
}
