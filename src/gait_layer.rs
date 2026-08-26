use als_core::character::AlsCharacter;
use als_core::gait::{GaitWeights, SPEED_EMA_HALF_LIFE_SECS, ema_toward};
use als_core::params::AlsMovementParams;
use als_core::state::Stance;
use avian3d::prelude::LinearVelocity;
use bevy::prelude::*;
use bevy_animation_graph::core::plugin::AnimationGraphSet;

use crate::character_assembly::AlsCharacterAvatar;

#[derive(Debug, Clone, Copy, PartialEq, Component)]
pub struct AlsGaitBlend {
    pub speed_ema: f32,
    pub weights: GaitWeights,
}

pub fn advance_gait_blend(
    current: &mut AlsGaitBlend,
    gait: als_core::state::Gait,
    moving: bool,
    target_speed: f32,
    dt: f32,
) {
    current.speed_ema = ema_toward(
        current.speed_ema,
        target_speed,
        dt,
        SPEED_EMA_HALF_LIFE_SECS,
    );
    current.weights = current.weights.advance(gait, moving, dt);
}

fn gait_blend_target_speed(params: &AlsMovementParams, gait: als_core::state::Gait) -> f32 {
    let s = params.stance_speeds(Stance::Standing);
    match gait {
        als_core::state::Gait::Walking => s.max_walk_speed,
        als_core::state::Gait::Running => s.max_run_speed,
        als_core::state::Gait::Sprinting => s.max_sprint_speed,
    }
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
    time: Res<Time>,
    mut commands: Commands,
    mut avatars: Query<GaitDriverQuery, With<AlsCharacterAvatar>>,
    characters: Query<
        (
            &als_core::state::Gait,
            &LinearVelocity,
            &als_core::character::AlsMovementParamsHandle,
        ),
        With<AlsCharacter>,
    >,
    params_assets: Res<Assets<AlsMovementParams>>,
) {
    let dt = time.delta_secs();
    for (entity, parent, blend) in avatars.iter_mut() {
        let Ok((gait, vel, handle)) = characters.get(parent.parent()) else {
            continue;
        };
        let hspeed = Vec2::new(vel.x, vel.z).length();
        let Some(p) = params_assets.get(&handle.0) else {
            continue;
        };
        let moving = hspeed > 0.05;
        let target_speed = if moving {
            hspeed
        } else {
            gait_blend_target_speed(p, *gait)
        };
        match blend {
            Some(mut state) => {
                advance_gait_blend(&mut state, *gait, moving, target_speed, dt);
            }
            None => {
                commands.entity(entity).insert(AlsGaitBlend {
                    speed_ema: target_speed,
                    weights: if moving {
                        GaitWeights::targets(*gait)
                    } else {
                        GaitWeights::default()
                    },
                });
            }
        }
    }
}
