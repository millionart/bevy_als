use als_core::character::AlsCharacter;
use als_core::motion::AlsRotationResponseSet;
use als_ext::{HorizontalVelocityBlend, horizontal_motion_from_world_velocity};
use avian3d::prelude::{LinearVelocity, PhysicsSystems};
use bevy::prelude::*;

#[derive(Component, Debug, Default, Clone, Copy, PartialEq)]
pub struct AlsActualMotion {
    pub world_velocity: Vec3,
    pub local_velocity: Vec2,
    pub horizontal_speed: f32,
    pub signed_direction_radians: f32,
    pub velocity_blend: HorizontalVelocityBlend,
}

impl AlsActualMotion {
    fn from_physics_velocity(world_velocity: Vec3, character_rotation: Quat) -> Self {
        let motion = horizontal_motion_from_world_velocity(world_velocity, character_rotation);
        Self {
            world_velocity: motion.world_velocity,
            local_velocity: motion.local_velocity,
            horizontal_speed: motion.horizontal_speed,
            signed_direction_radians: motion.signed_direction_radians,
            velocity_blend: motion.velocity_blend,
        }
    }
}

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AlsActualMotionSet;

pub struct AlsActualMotionPlugin;

impl Plugin for AlsActualMotionPlugin {
    fn build(&self, app: &mut App) {
        app.configure_sets(
            FixedPostUpdate,
            AlsActualMotionSet
                .after(PhysicsSystems::Last)
                .after(AlsRotationResponseSet),
        )
        .add_systems(
            FixedPostUpdate,
            snapshot_actual_motion_system.in_set(AlsActualMotionSet),
        );
    }
}

fn snapshot_actual_motion_system(
    mut commands: Commands,
    mut characters: Query<
        (
            Entity,
            &LinearVelocity,
            &Transform,
            Option<&mut AlsActualMotion>,
        ),
        With<AlsCharacter>,
    >,
) {
    for (entity, velocity, transform, current) in &mut characters {
        let next = AlsActualMotion::from_physics_velocity(velocity.0, transform.rotation);
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
    fn snapshot_is_character_local_and_isolated() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_systems(FixedPostUpdate, snapshot_actual_motion_system);
        let first = app
            .world_mut()
            .spawn((
                AlsCharacter,
                Transform::default(),
                LinearVelocity(Vec3::NEG_Z),
            ))
            .id();
        let second = app
            .world_mut()
            .spawn((
                AlsCharacter,
                Transform::from_rotation(Quat::from_rotation_y(std::f32::consts::FRAC_PI_2)),
                LinearVelocity(Vec3::NEG_X),
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
            .insert(LinearVelocity::ZERO);
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
}
