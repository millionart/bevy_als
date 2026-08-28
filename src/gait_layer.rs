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
