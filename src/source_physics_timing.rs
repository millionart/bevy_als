use std::time::Duration;

use avian3d::prelude::PhysicsSystems;
use bevy::prelude::*;
use bevy_animation_graph::core::plugin::AnimationGraphSet;

pub const SOURCE_MAX_PHYSICS_DELTA: Duration = Duration::from_nanos(33_333_333);

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SourcePhysicsTimingSet {
    Clamp,
    Restore,
}

#[derive(Resource, Default)]
struct SourcePhysicsClockRestore {
    previous: Option<Time>,
}

pub struct SourcePhysicsTimingPlugin;

impl Plugin for SourcePhysicsTimingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SourcePhysicsClockRestore>()
            .configure_sets(
                PostUpdate,
                (
                    SourcePhysicsTimingSet::Clamp
                        .after(AnimationGraphSet::PrePhysics)
                        .before(PhysicsSystems::First),
                    SourcePhysicsTimingSet::Restore
                        .after(PhysicsSystems::Last)
                        .before(AnimationGraphSet::PostPhysics),
                ),
            )
            .add_systems(
                PostUpdate,
                clamp_source_physics_delta.in_set(SourcePhysicsTimingSet::Clamp),
            )
            .add_systems(
                PostUpdate,
                restore_source_physics_delta.in_set(SourcePhysicsTimingSet::Restore),
            );
    }
}

fn source_physics_delta(delta: Duration) -> Duration {
    delta.min(SOURCE_MAX_PHYSICS_DELTA)
}

fn clamp_source_physics_delta(
    mut restore: ResMut<SourcePhysicsClockRestore>,
    mut clock: ResMut<Time>,
) {
    let previous = clock.clone();
    let mut capped = previous.clone();
    capped.advance_by(source_physics_delta(previous.delta()));
    restore.previous = Some(previous);
    *clock = capped;
}

fn restore_source_physics_delta(
    mut restore: ResMut<SourcePhysicsClockRestore>,
    mut clock: ResMut<Time>,
) {
    let Some(previous) = restore.previous.take() else {
        return;
    };
    *clock = previous;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_physics_delta_is_a_per_frame_cap_not_a_fixed_timestep() {
        assert_eq!(
            source_physics_delta(Duration::from_millis(12)),
            Duration::from_millis(12)
        );
        assert_eq!(
            source_physics_delta(Duration::from_millis(50)),
            SOURCE_MAX_PHYSICS_DELTA
        );
    }
}
