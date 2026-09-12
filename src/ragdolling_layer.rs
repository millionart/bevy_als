use als_core::character::AlsCharacter;
use als_core::state::AlsNotifyLocomotionAction;
use avian3d::prelude::LinearVelocity;
use bevy::ecs::hierarchy::ChildOf;
use bevy::prelude::*;
use bevy_animation_graph::core::animated_scene::AnimatedSceneInstance;
use bevy_animation_graph::core::animation_graph_player::AnimationGraphPlayer;
use bevy_animation_graph::core::edge_data::DataValue;
use bevy_animation_graph::core::pose::Pose;
use bevy_animation_graph::core::ragdoll::definition::BodyLabel;

use crate::character_assembly::AlsCharacterAvatar;

const PELVIS_LABEL: &str = "pelvis";
const REFERENCE_SPEED_METERS_PER_SECOND: f32 = 10.0;
pub(crate) const RAGDOLLING_FINAL_POSE_KEY: &str = "ragdolling_state.final_ragdoll_pose";
const RAGDOLLING_FLAIL_PLAY_RATE_KEY: &str = "ragdolling_state.flail_play_rate";

#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct AlsRagdollingAnimationState {
    pub flail_play_rate: f32,
}

impl Default for AlsRagdollingAnimationState {
    fn default() -> Self {
        Self {
            flail_play_rate: 1.0,
        }
    }
}

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AlsRagdollingLayerSet;

pub struct RagdollingLayerPlugin;

impl Plugin for RagdollingLayerPlugin {
    fn build(&self, app: &mut App) {
        app.configure_sets(PostUpdate, AlsRagdollingLayerSet.before(als_ext::post_physics_animation::PostPhysicsAnimationSet::Evaluate))
            .add_systems(
                PostUpdate,
                ragdolling_layer_writer.in_set(AlsRagdollingLayerSet),
            );
    }
}

pub fn flail_play_rate_for_speed(speed_meters_per_second: f32) -> f32 {
    (speed_meters_per_second / REFERENCE_SPEED_METERS_PER_SECOND).clamp(0.0, 1.0)
}

fn ragdoll_pelvis_speed(
    player: &AnimationGraphPlayer,
    labels: &Query<&BodyLabel>,
    velocities: &Query<&LinearVelocity>,
) -> Option<f32> {
    let spawned = player.get_spawned_ragdoll()?;
    for body in spawned.bodies.values() {
        if labels.get(*body).is_ok_and(|label| label.0 == PELVIS_LABEL)
            && let Ok(velocity) = velocities.get(*body)
        {
            return Some(velocity.0.length());
        }
    }
    None
}

fn write_globals(player: &mut AnimationGraphPlayer, ragdolling: bool, flail_play_rate: f32) {
    player.set_global_input_data(RAGDOLLING_FLAIL_PLAY_RATE_KEY, flail_play_rate);
    player.set_global_input_data("als_ragdolling_dynamic", ragdolling);
    player.set_global_input_data("als_ragdolling_readback", ragdolling);
}

fn player_pose_with_skeleton(player: &AnimationGraphPlayer) -> Option<Pose> {
    let mut pose = player.get_default_output_pose().cloned()?;
    pose.skeleton = player.skeleton().clone();
    Some(pose)
}

#[allow(clippy::type_complexity)]
fn ragdolling_layer_writer(
    mut commands: Commands,
    mut characters: Query<
        (
            Entity,
            Option<&AlsNotifyLocomotionAction>,
            Option<&mut AlsRagdollingAnimationState>,
        ),
        With<AlsCharacter>,
    >,
    avatars: Query<(&ChildOf, &AnimatedSceneInstance), With<AlsCharacterAvatar>>,
    labels: Query<&BodyLabel>,
    velocities: Query<&LinearVelocity>,
    mut players: Query<&mut AnimationGraphPlayer>,
) {
    for (entity, action, current_state) in &mut characters {
        let ragdolling = action == Some(&AlsNotifyLocomotionAction::Ragdolling);
        let mut state = current_state.as_deref().copied().unwrap_or_default();
        for (parent, instance) in &avatars {
            if parent.parent() != entity {
                continue;
            }
            let Ok(mut player) = players.get_mut(instance.player_entity()) else {
                continue;
            };
            if player
                .get_global_input_data(RAGDOLLING_FINAL_POSE_KEY)
                .is_none()
                && let Some(snapshot) = player_pose_with_skeleton(&player)
            {
                player.set_global_input_data(RAGDOLLING_FINAL_POSE_KEY, DataValue::Pose(snapshot));
            }
            if ragdolling {
                state.flail_play_rate = ragdoll_pelvis_speed(&player, &labels, &velocities)
                    .map(flail_play_rate_for_speed)
                    .unwrap_or_default();
            }
            write_globals(&mut player, ragdolling, state.flail_play_rate);
        }
        if let Some(mut current_state) = current_state {
            *current_state = state;
        } else {
            commands.entity(entity).insert(state);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flail_rate_matches_source_reference_speed() {
        assert_eq!(flail_play_rate_for_speed(-1.0), 0.0);
        assert!((flail_play_rate_for_speed(5.0) - 0.5).abs() < 1e-6);
        assert_eq!(flail_play_rate_for_speed(10.0), 1.0);
        assert_eq!(flail_play_rate_for_speed(25.0), 1.0);
    }

    #[test]
    fn source_animation_state_default_is_one() {
        assert_eq!(AlsRagdollingAnimationState::default().flail_play_rate, 1.0);
    }
}
