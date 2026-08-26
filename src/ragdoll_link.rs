use als_core::state::LocomotionState;
use avian3d::prelude::Position;
use bevy::prelude::*;
use bevy_animation_graph::core::animated_scene::AnimatedSceneInstance;
use bevy_animation_graph::core::animation_graph_player::AnimationGraphPlayer;
use bevy_animation_graph::core::edge_data::DataValue;
use bevy_animation_graph::core::ragdoll::definition::BodyLabel;
use bevy_animation_graph::core::ragdoll::spawning::SpawnedRagdoll;

use crate::character_assembly::AlsCharacterAvatar;

const PELVIS_LABEL: &str = "pelvis";
const CHARACTER_CENTER_ABOVE_PELVIS: f32 = 0.9;

#[derive(Component, Debug, Default, Clone, Copy)]
pub struct AlsRagdollLink {
    pub prev_ragdoll: bool,
}

pub struct AlsRagdollLinkPlugin;

impl Plugin for AlsRagdollLinkPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                ensure_ragdoll_link,
                ragdoll_link_system.after(ensure_ragdoll_link),
            ),
        );
    }
}

fn ensure_ragdoll_link(
    mut commands: Commands,
    q: Query<
        Entity,
        (
            With<als_core::character::AlsCharacter>,
            Without<AlsRagdollLink>,
        ),
    >,
) {
    for entity in &q {
        commands.entity(entity).insert(AlsRagdollLink::default());
    }
}

fn find_body_by_label(
    spawned: &SpawnedRagdoll,
    labels: &Query<&BodyLabel>,
    label: &str,
) -> Option<Entity> {
    for (_body_id, entity) in spawned.bodies.iter() {
        if let Ok(bl) = labels.get(*entity)
            && bl.0 == label
        {
            return Some(*entity);
        }
    }
    None
}

#[allow(clippy::type_complexity)]
fn ragdoll_link_system(
    mut q: Query<
        (
            Entity,
            &mut Transform,
            &LocomotionState,
            &mut AlsRagdollLink,
        ),
        With<als_core::character::AlsCharacter>,
    >,
    instances: Query<(&ChildOf, &AnimatedSceneInstance), With<AlsCharacterAvatar>>,
    mut players: Query<&mut AnimationGraphPlayer>,
    labels: Query<&BodyLabel>,
    positions: Query<&Position>,
) {
    let Ok((character, mut tf, loco, mut link)) = q.single_mut() else {
        return;
    };
    let is_ragdoll = *loco == LocomotionState::Ragdolling;
    let exiting = link.prev_ragdoll && !is_ragdoll;

    let Some((_, instance)) = instances
        .iter()
        .find(|(child_of, _)| child_of.parent() == character)
    else {
        link.prev_ragdoll = is_ragdoll;
        return;
    };
    let Ok(mut player) = players.get_mut(instance.player_entity()) else {
        link.prev_ragdoll = is_ragdoll;
        return;
    };

    player.set_input_data("ragdoll_dynamic", DataValue::from(is_ragdoll));
    player.set_input_data("ragdoll_readback", DataValue::from(is_ragdoll));

    if exiting
        && let Some(spawned) = player.get_spawned_ragdoll()
        && let Some(body_entity) = find_body_by_label(spawned, &labels, PELVIS_LABEL)
        && let Ok(pos) = positions.get(body_entity)
    {
        tf.translation = Vec3::new(pos.0.x, pos.0.y + CHARACTER_CENTER_ABOVE_PELVIS, pos.0.z);
        info!(
            "[RAGLINK] exit realign to pelvis ({:.2},{:.2},{:.2})",
            tf.translation.x, tf.translation.y, tf.translation.z
        );
    }

    link.prev_ragdoll = is_ragdoll;
}
