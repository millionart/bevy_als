use avian3d::parry::{math::Pose, query::contact::contact_support_map_support_map, shape::Cuboid};
use avian3d::prelude::Collider;
use bevy::prelude::*;

#[derive(Resource, Clone, Copy)]
pub(crate) struct SourceConvexMarginSettings {
    pub fraction: f32,
    pub maximum: f32,
    pub minimum: f32,
}

pub(crate) fn source_box_margin(half_extents: Vec3, settings: SourceConvexMarginSettings) -> f32 {
    (2.0 * half_extents.min_element() * settings.fraction).min(settings.maximum)
}

pub(crate) fn source_polygon_margins(
    first: f32,
    first_dynamic: bool,
    second: f32,
    second_dynamic: bool,
    minimum: f32,
) -> [f32; 2] {
    let first = if first_dynamic { first } else { 0.0 };
    let second = if second_dynamic { second } else { 0.0 };
    if first == 0.0 { [0.0, second.max(minimum)] }
    else if second == 0.0 { [first.max(minimum), 0.0] }
    else { [first.min(second); 2] }
}

pub(crate) fn source_box_convex_core_normal(
    box_collider: &Collider,
    box_pose: Pose,
    compound: &Collider,
    compound_pose: Pose,
    shape_index: u32,
    settings: SourceConvexMarginSettings,
) -> Option<Vec3> {
    let box_shape = box_collider.shape_scaled().as_cuboid()?;
    let compound = compound.shape_scaled().as_compound()?;
    let (shape_pose, shape) = compound.shapes().get(shape_index as usize)?;
    let shape = shape.as_support_map()?;
    let margin = source_box_margin(box_shape.half_extents, settings);
    let [margin, _] = source_polygon_margins(margin, true, 0.0, false, settings.minimum);
    let core = Cuboid::new(box_shape.half_extents - Vec3::splat(margin));
    let relative = box_pose.inv_mul(&(compound_pose * *shape_pose));
    let contact = contact_support_map_support_map(&relative, &core, shape, f32::MAX)?;
    let normal = box_pose.rotation * contact.normal1;
    normal.is_normalized().then_some(normal)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vector(value: &serde_json::Value) -> Vec3 {
        Vec3::new(value["x"].as_f64().unwrap() as f32, value["z"].as_f64().unwrap() as f32,
            value["y"].as_f64().unwrap() as f32)
    }

    fn bench_fixture() -> (Collider, Pose, Collider, Vec3, SourceConvexMarginSettings) {
        let sample: serde_json::Value = serde_json::from_str(include_str!(
            "../../docs/audit/bf144/d143/bench-foot.native-sample.json")).unwrap();
        let input = &sample["input"];
        let location = &input["body_translation_cm"];
        let body = Vec3::new(location[0].as_f64().unwrap() as f32, location[2].as_f64().unwrap() as f32,
            location[1].as_f64().unwrap() as f32);
        let lower = vector(&sample["geometry"]["minimum_cm"]);
        let upper = vector(&sample["geometry"]["maximum_cm"]);
        let center = (body + (lower + upper) * 0.5) * 0.01;
        let size = (upper - lower) * 0.01;
        let box_collider = Collider::cuboid(size.x, size.y, size.z);
        let definitions: std::collections::HashMap<String, Vec<Vec<[f32; 3]>>> = serde_json::from_str(
            include_str!("../../assets/ALSExtras/Environment/Meshes/static_convex_collision.runtime.json")).unwrap();
        let name = input["mesh_asset_path"].as_str().unwrap().rsplit('/').next().unwrap();
        let index = input["mesh_shape_index"].as_u64().unwrap() as usize;
        let points = definitions[name][index].iter().copied().map(Vec3::from_array).collect();
        let compound = Collider::compound(vec![(Vec3::ZERO, Quat::IDENTITY, Collider::convex_hull(points).unwrap())]);
        let normal = vector(&sample["points"][0]["world_normal"]);
        let [fraction, maximum, minimum] = include!("../als/settings/convex_margin.rs");
        (box_collider, Pose::from_parts(center, Quat::IDENTITY), compound, normal,
            SourceConvexMarginSettings { fraction, maximum, minimum })
    }

    #[test]
    fn source_box_and_polygon_margins_follow_dynamic_body_and_world_limit() {
        let [fraction, maximum, minimum] = include!("../als/settings/convex_margin.rs");
        let settings = SourceConvexMarginSettings { fraction, maximum, minimum };
        let foot = source_box_margin(Vec3::new(0.06, 0.06, 0.14), settings);
        let hand = source_box_margin(Vec3::new(0.08, 0.055, 0.035), settings);
        assert!((foot - 0.006).abs() < 1e-8);
        assert!((hand - 0.0035).abs() < 1e-8);
        assert_eq!(source_box_margin(Vec3::splat(4.0), settings), maximum);
        assert_eq!(source_polygon_margins(foot, true, 0.02, false, minimum), [foot, 0.0]);
        assert_eq!(source_polygon_margins(foot, false, hand, true, minimum), [0.0, hand]);
        assert_eq!(source_polygon_margins(foot, true, hand, true, minimum), [hand, hand]);
        assert_eq!(source_polygon_margins(foot, false, hand, false, minimum), [0.0, minimum]);
    }

    #[test]
    fn source_box_core_query_matches_native_bench_normal() {
        let (box_collider, box_pose, compound, native_normal, settings) = bench_fixture();
        let core = source_box_convex_core_normal(&box_collider, box_pose, &compound, Pose::IDENTITY, 0, settings).unwrap();
        let full = source_box_convex_core_normal(&box_collider, box_pose, &compound, Pose::IDENTITY, 0,
            SourceConvexMarginSettings { fraction: 0.0, ..settings }).unwrap();
        assert!(core.abs_diff_eq(-native_normal, 0.00002));
        assert!(full.distance(-native_normal) > 0.00004);
        let common = Pose::from_parts(Vec3::new(4.0, 2.0, -3.0), Quat::from_rotation_y(0.6));
        let transformed = source_box_convex_core_normal(&box_collider, common * box_pose, &compound, common, 0, settings).unwrap();
        assert!(transformed.abs_diff_eq(common.rotation * core, 0.00002));
    }

    #[test]
    fn source_box_core_normal_reaches_ragdoll_contact_only() {
        use avian3d::collision::{contact_types::ContactGraph, narrow_phase::NarrowPhaseSystems};
        use avian3d::prelude::{Gravity, LockedAxes, PhysicsPlugins, PhysicsSchedule, Position, RigidBody, SleepingDisabled};
        use bevy::time::TimeUpdateStrategy;
        use bevy_animation_graph::core::ragdoll::definition::BodyMode;
        use crate::extensions::ragdoll_physics_profile::RagdollActiveMode;
        use crate::extensions::source_contact_order::{SourceCompoundOrder, order_source_compound_groups};
        let (box_collider, box_pose, compound, native_normal, settings) = bench_fixture();
        let full = source_box_convex_core_normal(&box_collider, box_pose, &compound, Pose::IDENTITY, 0,
            SourceConvexMarginSettings { fraction: 0.0, ..settings }).unwrap();
        for source_pair in [false, true] {
            let mut app = App::new();
            app.add_plugins((MinimalPlugins, TransformPlugin, PhysicsPlugins::new(PostUpdate)))
                .insert_resource(Gravity(Vec3::ZERO))
                .insert_resource(settings)
                .insert_resource(TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(1.0 / 60.0)))
                .add_systems(PhysicsSchedule, order_source_compound_groups.in_set(NarrowPhaseSystems::Last));
            app.finish();
            app.cleanup();
            let floor = app.world_mut().spawn((RigidBody::Static, compound.clone(), SourceCompoundOrder, Transform::IDENTITY)).id();
            let body = app.world_mut().spawn((RigidBody::Dynamic, LockedAxes::ALL_LOCKED, SleepingDisabled,
                box_collider.clone(), Transform::from_translation(box_pose.translation))).id();
            if source_pair { app.world_mut().entity_mut(body).insert(RagdollActiveMode(BodyMode::Dynamic)); }
            app.update();
            app.update();
            let graph = app.world().resource::<ContactGraph>();
            let (_, pair) = graph.get(body, floor).expect("The source foot did not contact the source convex");
            let normal = pair.manifolds[0].normal * if pair.collider1 == body { 1.0 } else { -1.0 };
            if source_pair { assert!(normal.abs_diff_eq(-native_normal, 0.00002)); }
            else { assert!(normal.abs_diff_eq(full, 0.00002)); }
            assert!(app.world().get::<Position>(body).unwrap().0.abs_diff_eq(box_pose.translation, 1e-6));
        }
    }
}
