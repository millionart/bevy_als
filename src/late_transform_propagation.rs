use avian3d::physics_transform::PhysicsTransformSystems;
use bevy::{
    ecs::{
        change_detection::Tick,
        lifecycle::RemovedComponentEntity,
        message::MessageCursor,
        schedule::{AnonymousSet, InternedSystemSet, IntoSystemSet, ScheduleConfigs},
        system::{ScheduleSystem, SystemChangeTick},
    },
    prelude::*,
    transform::{TransformPlugin, TransformSystems, systems},
};
use bevy_animation_graph::core::plugin::{AnimationGraphCorePlugin, AnimationGraphSet};

use crate::extensions::post_physics_animation::PostPhysicsAnimationPlugin;

#[derive(Default, Resource)]
struct TransformPassInputs {
    tick: Option<Tick>,
    detachments: MessageCursor<RemovedComponentEntity>,
    readback_is_ordered: bool,
}

pub(crate) struct LateTransformPropagationPlugin;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, SystemSet)]
struct AvianTransformPropagation;

impl Plugin for LateTransformPropagationPlugin {
    fn build(&self, _app: &mut App) {}

    fn finish(&self, app: &mut App) {
        if !app.is_plugin_added::<TransformPlugin>() {
            return;
        }
        let capture = capture_transform_inputs.before(TransformSystems::Propagate);
        let capture = if app.is_plugin_added::<AnimationGraphCorePlugin>() {
            capture.after(AnimationGraphSet::Final)
        } else {
            capture
        };
        let readback_is_ordered = app.is_plugin_added::<PostPhysicsAnimationPlugin>();
        app.insert_resource(TransformPassInputs {
            readback_is_ordered,
            ..default()
        })
        .add_systems(PostUpdate, capture);

        if readback_is_ordered && let Some(pass) = avian_transform_pass(app) {
            app.configure_sets(
                PostUpdate,
                (
                    pass.in_set(AvianTransformPropagation),
                    AvianTransformPropagation.run_if(late_transform_inputs_changed::<false>),
                ),
            );
        }
    }
}

fn avian_transform_pass(app: &mut App) -> Option<InternedSystemSet> {
    app.get_schedule(PostUpdate)?
        .graph()
        .system_sets
        .contains(PhysicsTransformSystems::Propagate)
        .then_some(())?;
    app.world_mut()
        .schedule_scope(PostUpdate, |world, schedule| {
            schedule.initialize(world).ok()?;
            let graph = schedule.graph();
            let physics = graph
                .systems_in_set(PhysicsTransformSystems::Propagate.intern())
                .ok()?;
            let standard = [
                systems::mark_dirty_trees.into_system_set().intern(),
                systems::propagate_parent_transforms
                    .into_system_set()
                    .intern(),
                systems::sync_simple_transforms.into_system_set().intern(),
            ];
            let mut members = Vec::with_capacity(standard.len());
            for system in standard {
                let mut matching = graph
                    .systems_in_set(system)
                    .ok()?
                    .iter()
                    .filter(|key| physics.contains(*key));
                members.push(*matching.next()?);
                if matching.next().is_some() {
                    return None;
                }
            }
            // Target the original chain so host systems in the public physics set still run.
            graph.system_sets.iter().find_map(|(_, set, _)| {
                let set = (set as &dyn std::any::Any)
                    .downcast_ref::<AnonymousSet>()?
                    .intern();
                let group = graph.systems_in_set(set).ok()?;
                (group.len() == members.len()
                    && members.iter().all(|member| group.contains(member)))
                .then_some(set)
            })
        })
}

pub(crate) fn late_transform_propagation<const AFTER_READBACK: bool>()
-> ScheduleConfigs<ScheduleSystem> {
    (
        capture_transform_inputs,
        systems::mark_dirty_trees,
        systems::propagate_parent_transforms,
        systems::sync_simple_transforms,
    )
        .chain()
        .run_if(late_transform_inputs_changed::<AFTER_READBACK>)
}

fn capture_transform_inputs(
    inputs: Option<ResMut<TransformPassInputs>>,
    tick: SystemChangeTick,
    removed: RemovedComponents<ChildOf>,
) {
    let Some(mut inputs) = inputs else { return };
    // Changes visible at the start of a pass are already part of that pass.
    inputs.tick = Some(tick.this_run());
    inputs.detachments = removed
        .messages()
        .map_or_else(MessageCursor::default, Messages::get_cursor_current);
}

fn late_transform_inputs_changed<const AFTER_READBACK: bool>(
    inputs: Option<Res<TransformPassInputs>>,
    tick: SystemChangeTick,
    changed: Query<
        (Ref<Transform>, Ref<GlobalTransform>, Option<Ref<ChildOf>>),
        Or<(Changed<Transform>, Changed<ChildOf>, Added<GlobalTransform>)>,
    >,
    removed: RemovedComponents<ChildOf>,
) -> bool {
    let Some(inputs) = inputs else { return true };
    if AFTER_READBACK && !inputs.readback_is_ordered {
        return true;
    }
    let Some(boundary) = inputs.tick else {
        return true;
    };
    let now = tick.this_run();
    let changed_since_pass = |tick: Tick| tick.is_newer_than(boundary, now);
    removed
        .messages()
        .is_some_and(|messages| inputs.detachments.len(messages) != 0)
        || changed.iter().any(|(local, global, parent)| {
            changed_since_pass(local.last_changed())
                || changed_since_pass(global.added())
                || parent.is_some_and(|parent| changed_since_pass(parent.last_changed()))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Copy)]
    enum LateWrite {
        Move,
        Reparent,
        Detach,
    }

    #[derive(Resource)]
    struct Hierarchy {
        root: Entity,
        other: Entity,
        child: Entity,
        first: Option<LateWrite>,
        second: Option<LateWrite>,
        passes: [Option<Tick>; 2],
    }

    fn write(world: &mut World, first: bool) {
        let mut hierarchy = world.resource_mut::<Hierarchy>();
        let write = if first {
            hierarchy.first.take()
        } else {
            hierarchy.second.take()
        };
        let (root, other, child) = (hierarchy.root, hierarchy.other, hierarchy.child);
        match write {
            Some(LateWrite::Move) => world.get_mut::<Transform>(root).unwrap().translation.x = 3.0,
            Some(LateWrite::Reparent) => {
                world.entity_mut(child).insert(ChildOf(other));
            }
            Some(LateWrite::Detach) => {
                world.entity_mut(child).remove::<ChildOf>();
            }
            None => {}
        }
    }

    #[test]
    fn consecutive_late_passes_propagate_each_real_change_once() {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            TransformPlugin,
            LateTransformPropagationPlugin,
        ))
        .add_systems(
            PostUpdate,
            (
                |world: &mut World| write(world, true),
                late_transform_propagation::<true>(),
                |inputs: Option<Res<TransformPassInputs>>, mut hierarchy: ResMut<Hierarchy>| {
                    hierarchy.passes[0] = inputs.and_then(|inputs| inputs.tick);
                },
                |world: &mut World| write(world, false),
                late_transform_propagation::<false>(),
                |inputs: Option<Res<TransformPassInputs>>, mut hierarchy: ResMut<Hierarchy>| {
                    hierarchy.passes[1] = inputs.and_then(|inputs| inputs.tick);
                },
            )
                .chain()
                .after(TransformSystems::Propagate),
        );
        let world = app.world_mut();
        let root = world.spawn(Transform::IDENTITY).id();
        let other = world.spawn(Transform::from_xyz(14.0, 0.0, 0.0)).id();
        let child = world
            .spawn((Transform::from_xyz(2.0, 0.0, 0.0), ChildOf(root)))
            .id();
        let leaf = world
            .spawn((Transform::from_xyz(4.0, 0.0, 0.0), ChildOf(child)))
            .id();
        app.insert_resource(Hierarchy {
            root,
            other,
            child,
            first: Some(LateWrite::Move),
            second: None,
            passes: [None; 2],
        });
        app.update();
        assert_eq!(
            app.world()
                .get::<GlobalTransform>(leaf)
                .unwrap()
                .translation(),
            Vec3::X * 9.0,
        );
        app.world_mut()
            .get_mut::<Transform>(root)
            .unwrap()
            .translation
            .x = 0.0;
        app.finish();
        app.cleanup();
        app.world_mut()
            .resource_mut::<TransformPassInputs>()
            .readback_is_ordered = true;

        for (first, second, position) in [
            (None, None, 6.0),
            (Some(LateWrite::Move), None, 9.0),
            (None, Some(LateWrite::Reparent), 20.0),
            (Some(LateWrite::Detach), None, 6.0),
            (None, None, 6.0),
        ] {
            let mut hierarchy = app.world_mut().resource_mut::<Hierarchy>();
            hierarchy.first = first;
            hierarchy.second = second;
            app.update();

            let world = app.world();
            assert_eq!(
                world.get::<GlobalTransform>(leaf).unwrap().translation(),
                Vec3::X * position
            );
            let [first_pass, second_pass] = world.resource::<Hierarchy>().passes;
            assert!(first_pass.is_some());
            assert_eq!(first_pass != second_pass, second.is_some());
        }
    }
}
