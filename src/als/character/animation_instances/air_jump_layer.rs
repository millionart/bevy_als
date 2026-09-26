use crate::als::air_jump::JumpStartVariant;
use crate::extensions::{SourceSequencePlayerNode, sync_group_phase::source_sync_players};
use bevy::prelude::*;
use bevy_animation_graph::core::{
    animated_scene::AnimatedSceneInstance, animation_graph::AnimationGraph,
    animation_graph_player::AnimationGraphPlayer,
};

use crate::character_assembly::AlsCharacterAvatar;

#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct AlsJumpVariantState {
    pub variant: JumpStartVariant,
    pub airborne: bool,
}

pub struct AirJumpLayerPlugin;

impl Plugin for AirJumpLayerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            PostUpdate,
            capture_jump_variant.after(crate::extensions::post_physics_animation::PostPhysicsAnimationSet::Evaluate),
        );
    }
}

fn source_jump_variant(path: &str) -> Option<JumpStartVariant> {
    match path {
        "animations/Air/Jump/A_Als_Jump_Walk_Left.anim.ron" => Some(JumpStartVariant::WalkLeft),
        "animations/Air/Jump/A_Als_Jump_Walk_Right.anim.ron" => Some(JumpStartVariant::WalkRight),
        "animations/Air/Jump/A_Als_Jump_Run_Left.anim.ron" => Some(JumpStartVariant::RunLeft),
        "animations/Air/Jump/A_Als_Jump_Run_Right.anim.ron" => Some(JumpStartVariant::RunRight),
        _ => None,
    }
}

fn actual_jump_variant(
    player: &AnimationGraphPlayer,
    graphs: &Assets<AnimationGraph>,
) -> Option<JumpStartVariant> {
    let arena = player.get_context_arena()?;
    source_sync_players(arena, graphs, "sync_groups.ab_als_locomotion.jump.phase")?
        .iter()
        .filter(|tick| tick.key == "sync_groups.ab_als_locomotion.jump.phase" && tick.weight > 1e-5)
        .filter_map(|tick| {
            let context = arena.get_context(tick.identity.0)?;
            let node = graphs
                .get(context.get_graph_id())?
                .nodes
                .get(&tick.identity.1)?;
            let sequence = node.try_inner_downcast_ref::<SourceSequencePlayerNode>()?;
            Some((
                source_jump_variant(sequence.clip.path()?.path().to_str()?)?,
                tick.weight,
            ))
        })
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(variant, _)| variant)
}

fn capture_jump_variant(
    mut commands: Commands,
    avatars: Query<
        (Entity, &AnimatedSceneInstance, Option<&AlsJumpVariantState>),
        With<AlsCharacterAvatar>,
    >,
    players: Query<&AnimationGraphPlayer>,
    graphs: Res<Assets<AnimationGraph>>,
) {
    for (entity, instance, previous) in &avatars {
        let Ok(player) = players.get(instance.player_entity()) else {
            continue;
        };
        let actual = actual_jump_variant(player, &graphs);
        if let Some(variant) = actual.or_else(|| previous.map(|state| state.variant)) {
            commands.entity(entity).insert(AlsJumpVariantState {
                variant,
                airborne: actual.is_some(),
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn observed_jump_variants_use_only_the_actual_source_jump_sequences() {
        for variant in [
            JumpStartVariant::WalkLeft,
            JumpStartVariant::WalkRight,
            JumpStartVariant::RunLeft,
            JumpStartVariant::RunRight,
        ] {
            assert_eq!(source_jump_variant(variant.clip_path()), Some(variant));
        }
        assert_eq!(
            source_jump_variant("animations/Air/Fall/A_Als_Fall.anim.ron"),
            None
        );
        assert_eq!(
            actual_jump_variant(&AnimationGraphPlayer::new(default()), &Assets::default()),
            None
        );
    }
}
