use crate::als::character::AlsCharacter;
use crate::als::gait::{AlsGaitAmount, GaitWeights};
use bevy::prelude::*;

use crate::character_assembly::AlsCharacterAvatar;

#[derive(Debug, Clone, Copy, PartialEq, Component)]
pub struct AlsGaitBlend {
    pub weights: GaitWeights,
}

pub fn gait_weights_from_source_curve(gait_amount: f32) -> GaitWeights {
    GaitWeights::from_gait_amount(gait_amount)
}

pub struct GaitLayerPlugin;

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AlsGaitLayerSet;

impl Plugin for GaitLayerPlugin {
    fn build(&self, app: &mut App) {
        let outer = crate::AlsPhysicsSchedule::outer(app);
        app.configure_sets(
            outer,
            AlsGaitLayerSet.before(
                crate::extensions::post_physics_animation::PostPhysicsAnimationSet::Evaluate,
            ),
        )
        .add_systems(outer, gait_blend_driver.in_set(AlsGaitLayerSet));
    }
}

type GaitDriverQuery = (Entity, &'static ChildOf, Option<&'static mut AlsGaitBlend>);

fn gait_blend_driver(
    mut commands: Commands,
    mut avatars: Query<GaitDriverQuery, With<AlsCharacterAvatar>>,
    characters: Query<&AlsGaitAmount, With<AlsCharacter>>,
) {
    for (entity, parent, blend) in avatars.iter_mut() {
        let Ok(gait_amount) = characters.get(parent.parent()) else {
            continue;
        };
        let weights = gait_weights_from_source_curve(gait_amount.0);
        match blend {
            Some(mut state) => {
                state.weights = weights;
            }
            None => {
                commands.entity(entity).insert(AlsGaitBlend { weights });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::als::character::{AlsCharacter, AlsMovementParamsHandle};
    use crate::als::gait::{
        AlsDesiredGait, AlsGaitAmount, AlsMaxAllowedGait, GaitWeights, refresh_gait_system,
    };
    use crate::als::input::AlsInputCommand;
    use crate::als::motion::{AlsActualVelocity, AlsViewYaw};
    use crate::als::params::AlsMovementParams;
    use crate::als::state::{Gait, LocomotionState, RotationMode, Stance};
    use crate::als::transition::StateTransitionEvent;

    #[test]
    fn direction_dependent_gait_amount_reaches_animation_layer() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_resource(Assets::<AlsMovementParams>::default())
            .add_message::<StateTransitionEvent>()
            .add_systems(Update, refresh_gait_system)
            .add_plugins(GaitLayerPlugin);
        let mut params = AlsMovementParams::default();
        params
            .speeds
            .standing
            .allow_direction_dependent_movement_speed = true;
        params.speeds.standing.max_walk_backward_speed = 1.0;
        params.speeds.standing.max_run_backward_speed = 2.0;
        let params = app
            .world_mut()
            .resource_mut::<Assets<AlsMovementParams>>()
            .add(params);
        let character = app
            .world_mut()
            .spawn((
                AlsCharacter,
                LocomotionState::Grounded,
                Stance::Standing,
                RotationMode::ViewDirection,
                AlsViewYaw::default(),
                AlsInputCommand::default(),
                AlsDesiredGait(Gait::Running),
                AlsMaxAllowedGait(Gait::Running),
                AlsGaitAmount::default(),
                Gait::Walking,
                AlsActualVelocity {
                    world_velocity: Vec3::Z * 1.2,
                    horizontal_speed: 1.2,
                },
                crate::actual_motion::AlsActualMotion::default(),
                AlsMovementParamsHandle(params),
            ))
            .id();
        let avatar = app
            .world_mut()
            .spawn((ChildOf(character), AlsCharacterAvatar { character }))
            .id();

        for (speed, expected_gait, expected_amount) in
            [(1.2, Gait::Walking, 1.2), (1.9, Gait::Running, 1.9)]
        {
            app.world_mut()
                .entity_mut(character)
                .insert(AlsActualVelocity {
                    world_velocity: Vec3::Z * speed,
                    horizontal_speed: speed,
                });
            app.update();

            let amount = app.world().get::<AlsGaitAmount>(character).unwrap().0;
            let gait = *app.world().get::<Gait>(character).unwrap();
            let weights = app.world().get::<AlsGaitBlend>(avatar).unwrap().weights;
            assert_eq!(gait, expected_gait, "Backward speed {speed}");
            assert!((amount - expected_amount).abs() < 1e-6);
            assert_eq!(weights, GaitWeights::from_gait_amount(amount));
        }
    }
}
