use als_core::character::AlsCharacter;
use als_core::motion::{AlsActualVelocity, AlsActualVelocitySet, AlsRotationResponseSet};
use als_core::orientation::world_forward_to_yaw;
use als_ext::{HorizontalVelocityBlend, horizontal_motion_from_world_velocity};
use bevy::prelude::*;

pub const GROUNDED_VELOCITY_BLEND_HALF_LIFE_SECONDS: f32 = 0.1;

#[derive(Component, Debug, Default, Clone, Copy, PartialEq)]
pub struct AlsActualMotion {
    pub world_velocity: Vec3,
    pub local_velocity: Vec2,
    pub horizontal_speed: f32,
    pub velocity_yaw_degrees: f32,
    pub signed_direction_radians: f32,
    pub velocity_blend: HorizontalVelocityBlend,
}

#[derive(Component, Debug, Default, Clone, Copy, PartialEq)]
pub struct AlsGroundedVelocityBlend {
    pub value: HorizontalVelocityBlend,
    initialized: bool,
}

impl AlsActualMotion {
    fn from_physics_velocity(world_velocity: Vec3, character_rotation: Quat) -> Self {
        let motion = horizontal_motion_from_world_velocity(world_velocity, character_rotation);
        let local = character_rotation.inverse() * world_velocity;
        let direction = if local.length_squared() > 1e-12 {
            local / local.abs().element_sum()
        } else {
            Vec3::ZERO
        };
        Self {
            world_velocity,
            local_velocity: motion.local_velocity,
            horizontal_speed: motion.horizontal_speed,
            velocity_yaw_degrees: world_forward_to_yaw(world_velocity).to_degrees(),
            signed_direction_radians: motion.signed_direction_radians,
            velocity_blend: HorizontalVelocityBlend {
                forward: (-direction.z).max(0.0),
                backward: direction.z.max(0.0),
                left: (-direction.x).max(0.0),
                right: direction.x.max(0.0),
            },
        }
    }
}

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AlsActualMotionSet;

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AlsGroundedVelocityBlendSet;

pub struct AlsActualMotionPlugin;

impl Plugin for AlsActualMotionPlugin {
    fn build(&self, app: &mut App) {
        app.configure_sets(
            PostUpdate,
            AlsActualMotionSet
                .after(AlsActualVelocitySet)
                .after(AlsRotationResponseSet),
        )
        .add_systems(
            PostUpdate,
            snapshot_actual_motion_system.in_set(AlsActualMotionSet),
        )
        .configure_sets(PostUpdate, AlsGroundedVelocityBlendSet)
        .add_systems(
            PostUpdate,
            refresh_grounded_velocity_blend_system.in_set(AlsGroundedVelocityBlendSet),
        );
    }
}

fn grounded_velocity_blend_alpha(delta_seconds: f32) -> f32 {
    1.0 - 2.0_f32
        .powf(-delta_seconds.max(0.0) / GROUNDED_VELOCITY_BLEND_HALF_LIFE_SECONDS.max(f32::EPSILON))
}

fn damp_grounded_velocity_blend(
    current: HorizontalVelocityBlend,
    target: HorizontalVelocityBlend,
    alpha: f32,
) -> HorizontalVelocityBlend {
    HorizontalVelocityBlend {
        forward: current.forward.lerp(target.forward, alpha),
        backward: current.backward.lerp(target.backward, alpha),
        left: current.left.lerp(target.left, alpha),
        right: current.right.lerp(target.right, alpha),
    }
}

fn refresh_grounded_velocity_blend_system(
    time: Res<Time>,
    mut commands: Commands,
    mut characters: Query<
        (
            Entity,
            &AlsActualMotion,
            Option<&mut AlsGroundedVelocityBlend>,
        ),
        With<AlsCharacter>,
    >,
) {
    for (entity, motion, current) in &mut characters {
        let target = motion.velocity_blend;
        if let Some(mut current) = current {
            current.value = if current.initialized {
                damp_grounded_velocity_blend(
                    current.value,
                    target,
                    grounded_velocity_blend_alpha(time.delta_secs()),
                )
            } else {
                current.initialized = true;
                target
            };
        } else {
            commands.entity(entity).insert(AlsGroundedVelocityBlend {
                value: target,
                initialized: true,
            });
        }
    }
}

fn snapshot_actual_motion_system(
    mut commands: Commands,
    mut characters: Query<
        (
            Entity,
            &AlsActualVelocity,
            &Transform,
            Option<&mut AlsActualMotion>,
        ),
        With<AlsCharacter>,
    >,
) {
    for (entity, velocity, transform, current) in &mut characters {
        let mut next =
            AlsActualMotion::from_physics_velocity(velocity.world_velocity, transform.rotation);
        if next.horizontal_speed < 0.01 {
            next.velocity_yaw_degrees = current
                .as_deref()
                .map_or(0.0, |previous| previous.velocity_yaw_degrees);
        }
        if let Some(mut current) = current {
            *current = next;
        } else {
            commands.entity(entity).insert(next);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standing_source_motion_retains_full_velocity_for_three_axis_blend() {
        let velocity = Vec3::new(1.0, 2.0, -1.0);
        let motion = AlsActualMotion::from_physics_velocity(velocity, Quat::IDENTITY);
        assert_eq!(motion.world_velocity, velocity);
        assert!((motion.horizontal_speed - 2.0_f32.sqrt()).abs() < 1e-6);
        assert!((motion.velocity_blend.forward - 0.25).abs() < 1e-6);
        assert!((motion.velocity_blend.right - 0.25).abs() < 1e-6);
    }

    #[test]
    fn snapshot_is_character_local_and_isolated() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_systems(FixedPostUpdate, snapshot_actual_motion_system);
        let first = app
            .world_mut()
            .spawn((
                AlsCharacter,
                Transform::default(),
                AlsActualVelocity {
                    world_velocity: Vec3::NEG_Z,
                    horizontal_speed: 1.0,
                },
            ))
            .id();
        let second = app
            .world_mut()
            .spawn((
                AlsCharacter,
                Transform::from_rotation(Quat::from_rotation_y(std::f32::consts::FRAC_PI_2)),
                AlsActualVelocity {
                    world_velocity: Vec3::NEG_X,
                    horizontal_speed: 1.0,
                },
            ))
            .id();

        app.world_mut().run_schedule(FixedPostUpdate);

        let first_motion = *app.world().get::<AlsActualMotion>(first).unwrap();
        let second_motion = *app.world().get::<AlsActualMotion>(second).unwrap();
        assert!(first_motion.local_velocity.distance(Vec2::X) < 1e-6);
        assert!(second_motion.local_velocity.distance(Vec2::X) < 1e-6);
        assert_eq!(first_motion.velocity_blend.forward, 1.0);
        assert_eq!(second_motion.velocity_blend.forward, 1.0);

        app.world_mut()
            .entity_mut(first)
            .insert(AlsActualVelocity::default());
        app.world_mut().run_schedule(FixedPostUpdate);

        let stopped_motion = *app.world().get::<AlsActualMotion>(first).unwrap();
        assert_eq!(stopped_motion.horizontal_speed, 0.0);
        assert_eq!(stopped_motion.local_velocity, Vec2::ZERO);
        assert_eq!(
            stopped_motion.velocity_blend,
            HorizontalVelocityBlend::default()
        );
        assert!(second_motion.horizontal_speed > 0.0);
    }

    #[test]
    fn grounded_velocity_blend_uses_source_initialization_and_half_life() {
        let target = HorizontalVelocityBlend {
            forward: 1.0,
            ..Default::default()
        };
        let initial = AlsGroundedVelocityBlend {
            value: HorizontalVelocityBlend::default(),
            initialized: false,
        };
        let initialized = if initial.initialized {
            damp_grounded_velocity_blend(initial.value, target, grounded_velocity_blend_alpha(0.1))
        } else {
            target
        };
        assert_eq!(initialized, target);
        let next = damp_grounded_velocity_blend(
            HorizontalVelocityBlend::default(),
            target,
            grounded_velocity_blend_alpha(0.1),
        );
        assert!((next.forward - 0.5).abs() < 1e-6);
    }
}
