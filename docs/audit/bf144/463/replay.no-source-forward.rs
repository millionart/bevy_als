use std::sync::Arc;
use avian3d::prelude::*;
use bevy::{math::DVec3, platform::collections::HashMap, prelude::*};
use bevy_als::extensions::{
    control_rig_hierarchy::{RigHierarchy, RigTransform},
    control_rig_math::RigValue,
    control_rig_program::RigDefinition,
    control_rig_query::{RigQueryContext, RigQueryWorld},
    control_rig_units::RigUnitContext,
    SourceCurveValues,
};
use serde_json::{Value, json};

fn vector(v: &Value) -> DVec3 {
    DVec3::from_array(std::array::from_fn(|i| v[i].as_f64().unwrap()))
}

fn query() -> RigQueryContext {
    let mut colliders = HashMap::default();
    let mut world = World::new();
    for x in [-20.0, 0.0] {
        colliders.insert(world.spawn_empty().id(), (
            Position(Vec3::new(x, -0.25, -20.0)), Rotation::default(),
            Collider::cuboid(20.0, 0.5, 20.0), CollisionLayers::ALL,
        ));
    }
    RigQueryContext {
        world: Arc::new(RigQueryWorld { colliders }),
        to_world: RigTransform::default(), channel: "ECC_Visibility".into(),
        filter: SpatialQueryFilter::default(),
    }
}

fn after_hit(args: &[RigValue], world: RigTransform, hit: (DVec3, DVec3), source_normal: bool, source_point: bool) -> Vec<RigValue> {
    let mut result = args.to_vec();
    let RigValue::F32(angle) = args[4] else { panic!("Walkable angle is missing") };
    let RigValue::F32(height) = args[5] else { panic!("Foot height is missing") };
    assert_eq!(args[6], RigValue::Bool(true));
    assert!(world.scale.to_array().iter().all(|value| value.abs() > 1e-8_f32 as f64));
    let normal = if source_normal {
        let q = -world.rotation.xyz();
        let twice_cross = q.cross(hit.1) * 2.0;
        ((hit.1 + twice_cross * world.rotation.w) + q.cross(twice_cross)) * world.scale.recip()
    } else {
        world.inverse_transform_vector(hit.1)
    };
    let slope = normal.y;
    assert!(slope >= angle.to_radians().cos() as f64);
    let slope = slope as f32;
    let correction = if slope > 1e-8 { height / slope - height } else { 0.0 };
    let point = if source_point {
        let v = hit.0 - world.translation;
        let q = -world.rotation.xyz();
        let twice_cross = q.cross(v) * 2.0;
        ((v + twice_cross * world.rotation.w) + q.cross(twice_cross)) * world.scale.recip()
    } else { world.inverse_transform_point(hit.0) };
    result[8] = RigValue::F32((point.y + correction as f64) as f32);
    result[9] = RigValue::Vector(normal);
    result
}

fn main() {
    let root = std::path::Path::new("docs/audit/bf144/463");
    let label = std::env::args().nth(1).unwrap();
    let input: Value = serde_json::from_slice(&std::fs::read(root.join("inputs.json")).unwrap()).unwrap();
    let definition: RigDefinition = serde_json::from_value(input["definition"].clone()).unwrap();
    let mut hierarchy = RigHierarchy::new(definition.bones.clone(), definition.translation_tolerance).unwrap();
    let curves = SourceCurveValues::default();
    let world_query = query();
    let mut outputs = Vec::new();
    let mut source_normal_restored = 0;
    let mut current_offset_source_equal = 0;
    let mut controlled_offset_source_equal = 0;
    let mut changed_offsets = 0;
    let mut full_source_matches = 0;
    for (i, row) in input["rows"].as_array().unwrap().iter().enumerate() {
        let mut args: Vec<RigValue> = serde_json::from_value(row["source_arguments_projected"].clone()).unwrap();
        assert_eq!(json!(&args[..8]), json!(row["current_arguments"].as_array().unwrap()[..8]));
        args[8] = RigValue::F32(123.0);
        args[9] = RigValue::Vector(DVec3::splat(123.0));
        let world: RigTransform = serde_json::from_value(row["current_world"].clone()).unwrap();
        let expected = &row["actual_trace"];
        let mut observed_hit = None;
        let mut trace_count = 0;
        let mut trace = |start, end, channel: &str, complex| {
            assert_eq!(start, vector(&expected["start"]));
            assert_eq!(end, vector(&expected["end"]));
            let hit = world_query.trace(start, end, channel, complex)?;
            assert_eq!(hit, Some((vector(&expected["hit"][0]), vector(&expected["hit"][1]))));
            observed_hit = hit;
            trace_count += 1;
            Ok(hit)
        };
        let mut context = RigUnitContext {
            hierarchy: &mut hierarchy, curves: &curves, game_world: true,
            construction: false, delta_seconds: row["source_delta"].as_f64().unwrap(),
            geometry: &definition.geometry, to_world: world, line_trace: Some(&mut trace),
        };
        context.call("FAlsRigUnit_FootOffsetTrace::Execute", &mut args).unwrap();
        drop(context);
        assert_eq!(trace_count, 1);
        assert_eq!(json!(args), row["current_arguments"]);
        let hit = observed_hit.unwrap();
        assert_eq!(after_hit(&args, world, hit, false, false), args);
        let controlled = after_hit(&args, world, hit, true, false);
        let source: Vec<RigValue> = serde_json::from_value(row["source_arguments_projected"].clone()).unwrap();
        source_normal_restored += usize::from(controlled[9] == source[9]);
        current_offset_source_equal += usize::from(args[8] == source[8]);
        controlled_offset_source_equal += usize::from(controlled[8] == source[8]);
        changed_offsets += usize::from(controlled[8] != args[8]);
        let point_control = after_hit(&args, world, hit, true, true);
        let native = &row["native"];
        let native_point = DVec3::new(native["world_hit_cm"]["x"].as_f64().unwrap() * 0.01, native["world_hit_cm"]["z"].as_f64().unwrap() * 0.01, native["world_hit_cm"]["y"].as_f64().unwrap() * 0.01);
        let native_normal = DVec3::new(native["world_normal"]["x"].as_f64().unwrap(), native["world_normal"]["z"].as_f64().unwrap(), native["world_normal"]["y"].as_f64().unwrap());
        let native_hit_control = after_hit(&args, world, (native_point, native_normal), true, true);
        let RigValue::Vector(foot) = args[0] else { panic!("Foot vector is missing") };
        let RigValue::F32(upward) = args[2] else { panic!("Upward distance is missing") };
        let RigValue::F32(downward) = args[3] else { panic!("Downward distance is missing") };
        let forward = |local: DVec3| {
            let v = world.scale * local;
            let q = world.rotation.xyz();
            let twice_cross = q.cross(v) * 2.0;
            world.transform_point(local)
        };
        let source_forward_start = forward(DVec3::new(foot.x, upward as f64, foot.z));
        let source_forward_end = forward(DVec3::new(foot.x, -(downward as f64), foot.z));
        let source_forward_hit = world_query.trace(source_forward_start, source_forward_end, "ECC_Visibility", true).unwrap().unwrap();
        let source_forward_control = after_hit(&args, world, source_forward_hit, true, true);
        let endpoint = |v: &Value| DVec3::new(v["x"].as_f64().unwrap() * 0.01, v["z"].as_f64().unwrap() * 0.01, v["y"].as_f64().unwrap() * 0.01);
        let native_ray_target_hit = world_query.trace(endpoint(&native["world_start_cm"]), endpoint(&native["world_end_cm"]), "ECC_Visibility", true).unwrap().unwrap();
        let native_ray_target_query = after_hit(&args, world, native_ray_target_hit, true, true);
        let source_world = &row["source_world"];
        assert_eq!(source_world["rotation"]["x"].as_f64().unwrap(), 0.0);
        assert_eq!(source_world["rotation"]["y"].as_f64().unwrap(), 0.0);
        assert_eq!(source_world["scale"]["z"].as_f64().unwrap(), 1.0);
        let source_origin = source_world["translation"]["z"].as_f64().unwrap();
        let native_z_cm = native["world_hit_cm"]["z"].as_f64().unwrap() - source_origin;
        assert_eq!(native_z_cm, native["vm_hit_cm"]["z"].as_f64().unwrap(), "Native vertical inverse transform qualification");
        assert_eq!(native_normal, DVec3::Y);
        let source_cm_output = native_z_cm as f32;
        assert_eq!(source_cm_output as f64, native["source_offset_cm"].as_f64().unwrap(), "Actual source centimeter output");
        let full_source = RigValue::F32((source_cm_output as f64 * 0.01) as f32);
        assert_eq!(full_source, source[8]);
        full_source_matches += usize::from(source_forward_control[8] == source[8] && source_forward_control[9] == source[9]);
        outputs.push(json!({"query": row["query"], "pc": row["pc"], "current": args, "normal_only": controlled, "normal_and_point": point_control, "native_hit_and_inverse": native_hit_control, "source_forward_full_inverse": source_forward_control, "source_forward_start": source_forward_start, "source_forward_end": source_forward_end, "source_forward_hit": source_forward_hit, "native_ray_target_query": native_ray_target_query, "native_ray_target_hit": native_ray_target_hit, "full_source_offset": full_source, "native_vertical_height_cm": native_z_cm, "source": source, "regenerated_hit": hit, "native_hit_runtime": native_point, "actual_world_start": expected["start"], "actual_world_end": expected["end"], "native_world_start_cm": native["world_start_cm"], "native_world_end_cm": native["world_end_cm"]}));
        if i % 64 == 0 { println!("Replayed actual foot trace unit {i}"); }
    }
    let result = json!({"gate": "ACTUAL636_QUERY_INVERSE_AND_SOURCE_CENTIMETER_OFFSET_FACTORS", "calls": outputs.len(), "actual_requests_and_regenerated_hits_exact": outputs.len(), "public_complete_arguments_exact": outputs.len(), "post_hit_current_model_exact": outputs.len(), "source_normal_restored": source_normal_restored, "all_source_normal_point_forward_matches": full_source_matches, "current_offset_source_equal": current_offset_source_equal, "normal_only_offset_source_equal": controlled_offset_source_equal, "normal_only_changed_offsets": changed_offsets, "source_internal_query_hits_captured": false, "outputs": outputs});
    std::fs::write(root.join(format!("replay-{label}.json")), serde_json::to_vec(&result).unwrap()).unwrap();
    assert_eq!(source_normal_restored, 636, "Actual source normal factor must be retained");
    assert_eq!(full_source_matches, 636, "Actual source point and forward rotation factors must be retained");
    println!("Qualified636 full current calls, native vertical inverse and source centimeter offset factors");
}
