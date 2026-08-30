use als_core::character::AlsCharacter;
use als_core::motion::AlsViewYaw;
use als_core::movement_direction::{
    AlsMovementDirectionState, normalize_angle180, unwrap_angle_continuous, update_direction,
};
use als_core::orientation::world_forward_to_yaw;
use als_core::state::{Gait, RotationMode};
use bevy::ecs::hierarchy::ChildOf;
use bevy::prelude::*;
use bevy_animation_graph::core::animated_scene::AnimatedSceneInstance;
use bevy_animation_graph::core::animation_graph_player::AnimationGraphPlayer;
use bevy_animation_graph::core::edge_data::DataValue;

use crate::actual_motion::AlsActualMotion;
use crate::character_assembly::AlsCharacterAvatar;

pub const DIRECTION_MIN_SPEED: f32 = 0.01;

pub fn velocity_yaw_deg(vel: Vec3) -> f32 {
    world_forward_to_yaw(vel).to_degrees()
}

pub fn view_space_direction_angle(velocity_yaw: f32, view_yaw: f32) -> f32 {
    let d = velocity_yaw - view_yaw;
    let mut a = d;
    while a > 180.0 {
        a -= 360.0;
    }
    while a < -180.0 {
        a += 360.0;
    }
    a
}

fn graph_direction_angle(raw: f32) -> f32 {
    normalize_angle180(raw)
}

pub struct DirectionLayerPlugin;

impl Plugin for DirectionLayerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, direction_angle_driver);
    }
}

type DirectionAvatarQuery = (
    Entity,
    &'static ChildOf,
    Option<&'static mut AlsMovementDirectionState>,
    &'static AnimatedSceneInstance,
);

fn direction_angle_driver(
    mut commands: Commands,
    characters: Query<(&AlsActualMotion, &AlsViewYaw, &RotationMode, &Gait), With<AlsCharacter>>,
    mut avatars: Query<DirectionAvatarQuery, With<AlsCharacterAvatar>>,
    mut players: Query<&mut AnimationGraphPlayer>,
) {
    for (entity, child_of, state, instance) in avatars.iter_mut() {
        let Ok((motion, view_yaw, rotation_mode, gait)) = characters.get(child_of.parent()) else {
            continue;
        };
        let hspeed = motion.horizontal_speed;
        if hspeed < DIRECTION_MIN_SPEED {
            continue;
        }
        let raw = match rotation_mode {
            RotationMode::VelocityDirection => 0.0,
            _ => {
                if *gait == Gait::Sprinting {
                    0.0
                } else {
                    view_space_direction_angle(velocity_yaw_deg(motion.world_velocity), view_yaw.0)
                }
            }
        };
        match state {
            Some(mut s) => {
                let continuous = unwrap_angle_continuous(s.angle, raw);
                s.direction = update_direction(Some(s.direction), raw);
                s.angle = continuous;
            }
            None => {
                let dir = update_direction(None, raw);
                commands.entity(entity).insert(AlsMovementDirectionState {
                    direction: dir,
                    angle: raw,
                });
            }
        }
        let angle = graph_direction_angle(raw);
        if let Ok(mut player) = players.get_mut(instance.player_entity()) {
            player.set_input_data("direction_angle", DataValue::from(angle));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn velocity_yaw_matches_body_convention() {
        assert!((velocity_yaw_deg(Vec3::NEG_Z) - 0.0).abs() < 1e-4);
        assert!((velocity_yaw_deg(Vec3::X) + 90.0).abs() < 1e-4);
        assert!((velocity_yaw_deg(Vec3::Z).abs() - 180.0).abs() < 1e-4);
    }

    #[test]
    fn view_space_angle_subtracts_view() {
        assert!((view_space_direction_angle(180.0, 0.0) - 180.0).abs() < 1e-4);
        assert!((view_space_direction_angle(45.0, 45.0) - 0.0).abs() < 1e-4);
        assert!((view_space_direction_angle(-90.0, 45.0) + 135.0).abs() < 1e-4);
    }

    #[test]
    fn graph_angle_stays_in_the_cycle_blend_domain() {
        assert!((graph_direction_angle(-630.0) - 90.0).abs() < 1e-4);
        assert!((graph_direction_angle(540.0) - 180.0).abs() < 1e-4);
    }
}
