use als_core::character::AlsCharacter;
use als_core::gait::{AlsGaitAmount, GaitWeights};
use bevy::prelude::*;
use bevy_animation_graph::core::plugin::AnimationGraphSet;

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
        app.configure_sets(PostUpdate, AlsGaitLayerSet.before(AnimationGraphSet::Final))
            .add_systems(PostUpdate, gait_blend_driver.in_set(AlsGaitLayerSet));
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
    use als_core::character::{AlsCharacter, AlsMovementParamsHandle};
    use als_core::gait::{
        AlsDesiredGait, AlsGaitAmount, AlsMaxAllowedGait, GaitWeights, refresh_gait_system,
    };
    use als_core::input::AlsInputCommand;
    use als_core::motion::AlsViewYaw;
    use als_core::params::AlsMovementParams;
    use als_core::state::{Gait, LocomotionState, RotationMode, Stance};
    use als_core::transition::StateTransitionEvent;
    use avian3d::prelude::LinearVelocity;

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
                LinearVelocity(Vec3::Z * 1.2),
                AlsMovementParamsHandle(params),
            ))
            .id();
        let avatar = app
            .world_mut()
            .spawn((ChildOf(character), AlsCharacterAvatar { character }))
            .id();

        app.update();

        let amount = app.world().get::<AlsGaitAmount>(character).unwrap().0;
        let gait = *app.world().get::<Gait>(character).unwrap();
        let weights = app.world().get::<AlsGaitBlend>(avatar).unwrap().weights;
        println!(
            "[BF102.03] backward_gait={gait:?} gait_amount={amount:.6} animation_running_amount={:.6} animation_sprint_amount={:.6}",
            weights.running_amount(),
            weights.sprint_w
        );
        assert_eq!(gait, Gait::Running);
        assert!((amount - 1.2).abs() < 1e-6);
        assert_eq!(weights, GaitWeights::from_gait_amount(amount));
    }
}
