use als_core::air_jump::{JumpStartVariant, select_jump_start_variant};
use als_core::state::LocomotionState;
use bevy::ecs::hierarchy::ChildOf;
use bevy::prelude::*;
use bevy_animation_graph::core::animated_scene::AnimatedSceneInstance;
use bevy_animation_graph::core::animation_graph_player::AnimationGraphPlayer;
use bevy_animation_graph::core::edge_data::DataValue;

use crate::character_assembly::AlsCharacterAvatar;

fn is_left(variant: JumpStartVariant) -> bool {
    matches!(
        variant,
        JumpStartVariant::WalkLeft | JumpStartVariant::RunLeft
    )
}

fn is_run(variant: JumpStartVariant) -> bool {
    matches!(
        variant,
        JumpStartVariant::RunLeft | JumpStartVariant::RunRight
    )
}

#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct AlsJumpVariantState {
    pub variant: JumpStartVariant,
    pub airborne: bool,
}

pub struct AirJumpLayerPlugin;

impl Plugin for AirJumpLayerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, air_jump_pin_driver);
    }
}

fn next_jump_variant_state(
    current: Option<AlsJumpVariantState>,
    params: &als_core::params::AlsMovementParams,
    gait: als_core::state::Gait,
    horizontal_speed: f32,
    airborne: bool,
) -> AlsJumpVariantState {
    match current {
        Some(current) if airborne && !current.airborne => AlsJumpVariantState {
            variant: select_jump_start_variant(
                params,
                gait,
                horizontal_speed,
                is_left(current.variant),
            ),
            airborne,
        },
        Some(current) => AlsJumpVariantState {
            airborne,
            ..current
        },
        None => AlsJumpVariantState {
            variant: select_jump_start_variant(params, gait, horizontal_speed, false),
            airborne,
        },
    }
}

#[allow(clippy::type_complexity)]
fn air_jump_pin_driver(
    mut commands: Commands,
    characters: Query<
        (
            &als_core::state::Gait,
            &LocomotionState,
            &avian3d::prelude::LinearVelocity,
            &als_core::character::AlsMovementParamsHandle,
        ),
        With<als_core::character::AlsCharacter>,
    >,
    avatars: Query<
        (
            Entity,
            &'static ChildOf,
            &'static AnimatedSceneInstance,
            Option<&'static AlsJumpVariantState>,
        ),
        With<AlsCharacterAvatar>,
    >,
    mut players: Query<&mut AnimationGraphPlayer>,
    params_assets: Res<Assets<als_core::params::AlsMovementParams>>,
) {
    for (entity, child_of, instance, state) in avatars.iter() {
        let Ok((gait, locomotion, vel, handle)) = characters.get(child_of.parent()) else {
            continue;
        };
        let Some(p) = params_assets.get(&handle.0) else {
            continue;
        };
        let hspeed = Vec2::new(vel.x, vel.z).length();
        let airborne = *locomotion == LocomotionState::Air;
        let next = next_jump_variant_state(state.copied(), p, *gait, hspeed, airborne);
        if Some(next) != state.copied() {
            commands.entity(entity).insert(next);
        }
        if let Ok(mut player) = players.get_mut(instance.player_entity()) {
            player.set_input_data(
                "jump_left_weight",
                DataValue::from(if is_left(next.variant) {
                    1.0f32
                } else {
                    0.0f32
                }),
            );
            player.set_input_data(
                "jump_run_weight",
                DataValue::from(if is_run(next.variant) { 1.0f32 } else { 0.0f32 }),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn variant_latches_for_the_entire_airborne_interval() {
        let params = als_core::params::AlsMovementParams::default();
        let initial =
            next_jump_variant_state(None, &params, als_core::state::Gait::Running, 3.75, false);
        assert_eq!(initial.variant, JumpStartVariant::RunLeft);
        let takeoff = next_jump_variant_state(
            Some(initial),
            &params,
            als_core::state::Gait::Running,
            3.75,
            true,
        );
        assert_eq!(takeoff.variant, JumpStartVariant::RunRight);
        let airborne = next_jump_variant_state(
            Some(takeoff),
            &params,
            als_core::state::Gait::Running,
            3.75,
            true,
        );
        assert_eq!(airborne.variant, JumpStartVariant::RunRight);
        let landed = next_jump_variant_state(
            Some(airborne),
            &params,
            als_core::state::Gait::Running,
            3.75,
            false,
        );
        assert_eq!(landed.variant, JumpStartVariant::RunRight);
        let next_takeoff = next_jump_variant_state(
            Some(landed),
            &params,
            als_core::state::Gait::Running,
            3.75,
            true,
        );
        assert_eq!(next_takeoff.variant, JumpStartVariant::RunLeft);
    }
}
