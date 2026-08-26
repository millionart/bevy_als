use als_core::air_jump::{select_jump_start_variant, JumpStartVariant};
use bevy::ecs::hierarchy::ChildOf;
use bevy::prelude::*;
use bevy_animation_graph::core::animated_scene::AnimatedSceneInstance;
use bevy_animation_graph::core::animation_graph_player::AnimationGraphPlayer;
use bevy_animation_graph::core::edge_data::DataValue;

fn is_left(variant: JumpStartVariant) -> bool {
    matches!(variant, JumpStartVariant::WalkLeft | JumpStartVariant::RunLeft)
}

fn is_run(variant: JumpStartVariant) -> bool {
    matches!(variant, JumpStartVariant::RunLeft | JumpStartVariant::RunRight)
}

#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct AlsJumpVariantState {
    pub variant: JumpStartVariant,
}

pub struct AirJumpLayerPlugin;

impl Plugin for AirJumpLayerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, air_jump_pin_driver);
    }
}

#[allow(clippy::type_complexity)]
fn air_jump_pin_driver(
    mut commands: Commands,
    characters: Query<
        (
            &als_core::state::Gait,
            &avian3d::prelude::LinearVelocity,
            &als_core::character::AlsMovementParamsHandle,
        ),
        With<als_core::character::AlsCharacter>,
    >,
    avatars: Query<(
        Entity,
        &'static ChildOf,
        &'static AnimatedSceneInstance,
        Option<&'static AlsJumpVariantState>,
    )>,
    mut players: Query<&mut AnimationGraphPlayer>,
    params_assets: Res<Assets<als_core::params::AlsMovementParams>>,
) {
    for (entity, child_of, instance, state) in avatars.iter() {
        let Ok((gait, vel, handle)) = characters.get(child_of.parent()) else {
            continue;
        };
        let Some(p) = params_assets.get(&handle.0) else {
            continue;
        };
        let hspeed = Vec2::new(vel.x, vel.z).length();
        let left_planted = match state {
            Some(s) => !is_left(s.variant),
            None => false,
        };
        let variant = select_jump_start_variant(p, *gait, hspeed, left_planted);
        if Some(variant) != state.map(|s| s.variant) {
            commands.entity(entity).insert(AlsJumpVariantState { variant });
        }
        if let Ok(mut player) = players.get_mut(instance.player_entity()) {
            player.set_input_data(
                "jump_left_weight",
                DataValue::from(if is_left(variant) { 1.0f32 } else { 0.0f32 }),
            );
            player.set_input_data(
                "jump_run_weight",
                DataValue::from(if is_run(variant) { 1.0f32 } else { 0.0f32 }),
            );
        }
    }
}
