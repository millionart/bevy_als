use std::collections::{BTreeMap, HashMap};
use std::fs;

use bevy::math::{Quat, Vec3};
use bevy_animation_graph::core::ragdoll::bone_mapping_loader::RagdollBoneMapSerial;
use bevy_animation_graph::core::ragdoll::definition::{
    ColliderMassMode, ColliderShape, JointVariant, Ragdoll,
};
use serde_json::Value;

const RAGDOLL_PATH: &str = "assets/ragdolls/als_manny.rag.ron";
const BONE_MAP_PATH: &str = "assets/ragdoll_bone_maps/als_manny.bm.ron";
const PROFILE_PATH: &str = "assets/ragdolls/pa_als.physics.json";
const SOURCE_PATH: &str = "target/bf109/source_native/PA_Als.ue.json";

fn main() {
    if let Err(error) = verify() {
        eprintln!("[PA-ALS][FAIL] {error}");
        std::process::exit(1);
    }
}

fn verify() -> Result<(), String> {
    let source_path = std::env::var("PA_ALS_SOURCE_PATH").unwrap_or_else(|_| SOURCE_PATH.into());
    let source = read_json(&source_path)?;
    let manifest = read_json(PROFILE_PATH)?;
    let ragdoll: Ragdoll = ron::de::from_str(&fs::read_to_string(RAGDOLL_PATH).map_err(to_error)?)
        .map_err(to_error)?;
    let bone_map: RagdollBoneMapSerial =
        ron::de::from_str(&fs::read_to_string(BONE_MAP_PATH).map_err(to_error)?)
            .map_err(to_error)?;
    let physics = field(&source, "physics_asset")?;
    let source_bodies = array(field(physics, "bodies")?)?;
    let source_constraints = array(field(physics, "constraints")?)?;
    let source_pairs = array(field(physics, "collision_disabled_pairs")?)?;
    let manifest_bodies = array(field(&manifest, "bodies")?)?;
    let manifest_constraints = array(field(&manifest, "constraints")?)?;
    let manifest_pairs = array(field(&manifest, "collision_disabled_pairs")?)?;
    require(source_bodies.len() == 21, "source body count")?;
    require(source_constraints.len() == 20, "source constraint count")?;
    require(
        source_pairs.len() == 49,
        "source collision disabled pair count",
    )?;
    require(
        manifest_bodies.len() == source_bodies.len(),
        "manifest body count",
    )?;
    require(
        manifest_constraints.len() == source_constraints.len(),
        "manifest constraint count",
    )?;
    require(
        manifest_pairs.len() == source_pairs.len(),
        "manifest pair count",
    )?;
    require(
        field(&manifest, "target_axis_mapping")? == &serde_json::json!(["x", "z", "y"]),
        "transform axis mapping",
    )?;
    let objects = array(field(&source, "asset_objects")?)?;
    for (source_body, manifest_body) in source_bodies.iter().zip(manifest_bodies) {
        verify_body(source_body, manifest_body, objects)?;
    }
    for (source_constraint, manifest_constraint) in
        source_constraints.iter().zip(manifest_constraints)
    {
        verify_constraint(source_constraint, manifest_constraint, objects)?;
    }
    let mut source_pair_matrix = source_pairs
        .iter()
        .map(source_pair)
        .collect::<Result<Vec<_>, _>>()?;
    source_pair_matrix.sort_unstable();
    let mut manifest_pair_matrix = manifest_pairs
        .iter()
        .map(manifest_pair)
        .collect::<Result<Vec<_>, _>>()?;
    manifest_pair_matrix.sort_unstable();
    require(
        manifest_pair_matrix == source_pair_matrix,
        "collision disabled pair matrix",
    )?;
    verify_ragdoll(&ragdoll, &bone_map, manifest_bodies, manifest_constraints)?;
    println!(
        "[PA-ALS][MATRIX] bodies={} colliders={} constraints={} collision_disabled_pairs={} transform_fields={} differences=0",
        source_bodies.len(),
        ragdoll.colliders.len(),
        source_constraints.len(),
        source_pairs.len(),
        ragdoll.colliders.len() * 2 + source_constraints.len() * 6
    );
    println!("[PA-ALS][VERDICT] PASS");
    Ok(())
}

fn verify_body(source: &Value, manifest: &Value, objects: &[Value]) -> Result<(), String> {
    let source_index = u8_value(field(source, "index")?)?;
    let bone = string(field(source, "bone_name")?)?;
    require(
        u8_value(field(manifest, "source_index")?)? == source_index,
        "body source index",
    )?;
    require(
        string(field(manifest, "bone_name")?)? == bone,
        "body bone name",
    )?;
    require(
        close(
            number(field(manifest, "calculated_mass_kg")?)?,
            number(field(source, "calculated_mass")?)?,
        ),
        "body calculated mass",
    )?;
    let source_default_instance = property_map(field(source, "default_instance_properties")?)?;
    require(
        json_object_map(field(manifest, "source_default_instance")?)? == source_default_instance,
        "body default instance matrix",
    )?;
    let source_properties = object_properties(objects, "body", bone)?;
    require(
        json_object_map(field(manifest, "source_properties")?)? == source_properties,
        "body source property matrix",
    )?;
    let source_shapes = array(field(source, "shapes")?)?;
    let manifest_shapes = array(field(manifest, "shapes")?)?;
    require(
        source_shapes.len() == manifest_shapes.len(),
        "body shape count",
    )?;
    for (source_shape, manifest_shape) in source_shapes.iter().zip(manifest_shapes) {
        for (key, value) in object(source_shape)? {
            require(field(manifest_shape, key)? == value, "source shape field")?;
        }
        let center = source_vector(field(source_shape, "center")?)?;
        let translation = json_vec3(field(field(manifest_shape, "target")?, "translation_m")?)?;
        require(
            vec_close(
                translation,
                Vec3::new(center.x * 0.01, center.z * 0.01, center.y * 0.01),
            ),
            "shape position transform",
        )?;
    }
    Ok(())
}

fn verify_constraint(source: &Value, manifest: &Value, objects: &[Value]) -> Result<(), String> {
    let source_index = u8_value(field(source, "index")?)?;
    let joint_name = string(field(source, "joint_name")?)?;
    require(
        u8_value(field(manifest, "source_index")?)? == source_index,
        "constraint source index",
    )?;
    require(
        string(field(manifest, "joint_name")?)? == joint_name,
        "constraint joint name",
    )?;
    for key in ["constraint_bone_1", "constraint_bone_2"] {
        require(
            field(manifest, key)? == field(source, key)?,
            "constraint body link",
        )?;
    }
    let source_properties = object_properties(objects, "constraint", joint_name)?;
    require(
        json_object_map(field(manifest, "source_properties")?)? == source_properties,
        "constraint source property matrix",
    )?;
    require(
        json_object_map(field(manifest, "source_instance_properties")?)?
            == property_map(field(source, "instance_properties")?)?,
        "constraint instance matrix",
    )?;
    verify_frame(
        source,
        manifest,
        "position_1",
        "primary_axis_1",
        "secondary_axis_1",
        "frame_1_ue",
        "frame_1",
    )?;
    verify_frame(
        source,
        manifest,
        "position_2",
        "primary_axis_2",
        "secondary_axis_2",
        "frame_2_ue",
        "frame_2",
    )?;
    let source_offset = field(source, "angular_rotation_offset")?;
    let manifest_offset: [f32; 3] = json_array(field(manifest, "angular_rotation_offset_ue")?)?;
    require(
        close(manifest_offset[0], number(field(source_offset, "pitch")?)?)
            && close(manifest_offset[1], number(field(source_offset, "yaw")?)?)
            && close(manifest_offset[2], number(field(source_offset, "roll")?)?),
        "constraint angular rotation offset",
    )?;
    let target_offset = Quat::from_array(json_array(field(
        field(manifest, "target")?,
        "angular_rotation_offset_xyzw",
    )?)?);
    require(
        target_offset.is_finite() && (target_offset.length() - 1.0).abs() <= 1e-4,
        "constraint angular rotation offset transform",
    )?;
    require(
        field(
            field(manifest, "target")?,
            "calibrate_frame_2_to_initial_pose",
        )? == &Value::Bool(true),
        "constraint frame calibration policy",
    )?;
    Ok(())
}

fn verify_frame(
    source: &Value,
    manifest: &Value,
    source_position: &str,
    source_primary: &str,
    source_secondary: &str,
    manifest_frame_ue: &str,
    manifest_frame_target: &str,
) -> Result<(), String> {
    let position = source_vector(field(source, source_position)?)?;
    let primary = source_vector(field(source, source_primary)?)?;
    let secondary = source_vector(field(source, source_secondary)?)?;
    let source_frame = field(manifest, manifest_frame_ue)?;
    require(
        json_vec3(field(source_frame, "translation_cm")?)? == position,
        "constraint source frame translation",
    )?;
    require(
        json_vec3(field(source_frame, "primary_axis")?)? == primary,
        "constraint source primary axis",
    )?;
    require(
        json_vec3(field(source_frame, "secondary_axis")?)? == secondary,
        "constraint source secondary axis",
    )?;
    let target_frame = field(field(manifest, "target")?, manifest_frame_target)?;
    let target_translation = json_vec3(field(target_frame, "translation_m")?)?;
    require(
        vec_close(
            target_translation,
            Vec3::new(position.x * 0.01, position.z * 0.01, position.y * 0.01),
        ),
        "constraint frame position transform",
    )?;
    let rotation = Quat::from_array(json_array(field(target_frame, "rotation_xyzw")?)?);
    require(
        rotation.is_finite() && (rotation.length() - 1.0).abs() <= 1e-4,
        "constraint frame rotation",
    )?;
    Ok(())
}

fn verify_ragdoll(
    ragdoll: &Ragdoll,
    bone_map: &RagdollBoneMapSerial,
    manifest_bodies: &[Value],
    manifest_constraints: &[Value],
) -> Result<(), String> {
    require(ragdoll.bodies.len() == 21, "target ragdoll body count")?;
    require(
        ragdoll.colliders.len() == 22,
        "target ragdoll collider count",
    )?;
    require(ragdoll.joints.len() == 20, "target ragdoll joint count")?;
    require(
        bone_map.bodies_from_bones.len() == 21,
        "target body bone map count",
    )?;
    require(
        bone_map.bones_from_bodies.len() == 23,
        "target readback bone map count",
    )?;
    let bodies = ragdoll
        .bodies
        .iter()
        .map(|(id, body)| (*id, body))
        .collect::<HashMap<_, _>>();
    let labels = ragdoll
        .bodies
        .iter()
        .map(|(id, body)| (*id, body.label.as_str()))
        .collect::<HashMap<_, _>>();
    for body_manifest in manifest_bodies {
        let label = string(field(body_manifest, "bone_name")?)?;
        let body = bodies
            .values()
            .find(|body| body.label == label)
            .ok_or_else(|| format!("missing target body {label}"))?;
        let shapes = array(field(body_manifest, "shapes")?)?;
        require(
            body.colliders.len() == shapes.len(),
            "target body collider count",
        )?;
        let expected_mass = number(field(body_manifest, "calculated_mass_kg")?)?;
        let actual_mass = body
            .colliders
            .iter()
            .map(|id| {
                match ragdoll
                    .colliders
                    .get(id)
                    .ok_or("missing target collider")?
                    .mass_mode
                {
                    ColliderMassMode::Override(value) => Ok(value),
                    ColliderMassMode::ByVolume => Err("target collider uses volume mass"),
                }
            })
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .sum::<f32>();
        require(
            close(actual_mass, expected_mass),
            "target calculated body mass",
        )?;
        for (collider_id, shape_manifest) in body.colliders.iter().zip(shapes) {
            let collider = ragdoll
                .colliders
                .get(collider_id)
                .ok_or("missing target collider")?;
            let target = field(shape_manifest, "target")?;
            let translation = Vec3::from(collider.local_offset.translation);
            let expected_translation = json_vec3(field(target, "translation_m")?)?;
            require(
                vec_close(translation, expected_translation),
                "target collider translation",
            )?;
            let expected_rotation = Quat::from_array(json_array(field(target, "rotation_xyzw")?)?);
            require(
                collider.local_offset.rotation.dot(expected_rotation).abs() >= 1.0 - 1e-4,
                "target collider rotation",
            )?;
            match (string(field(target, "kind")?)?, &collider.shape) {
                ("sphere", ColliderShape::Sphere(shape)) => require(
                    close(shape.radius, number(field(target, "radius_m")?)?),
                    "target sphere radius",
                )?,
                ("box", ColliderShape::Cuboid(shape)) => require(
                    vec_close(
                        shape.half_size,
                        json_vec3(field(target, "half_extents_m")?)?,
                    ),
                    "target box extents",
                )?,
                ("capsule", ColliderShape::Capsule(shape)) => {
                    require(
                        close(shape.radius, number(field(target, "radius_m")?)?),
                        "target capsule radius",
                    )?;
                    require(
                        close(shape.half_length, number(field(target, "half_length_m")?)?),
                        "target capsule length",
                    )?;
                }
                _ => return Err("target collider shape kind".into()),
            }
        }
    }
    for constraint in manifest_constraints {
        let label = string(field(constraint, "joint_name")?)?;
        let joint = ragdoll
            .joints
            .values()
            .find(|joint| joint.label == label)
            .ok_or_else(|| format!("missing target joint {label}"))?;
        let JointVariant::Spherical(value) = &joint.variant else {
            return Err(format!("target joint {label} is not spherical"));
        };
        require(
            labels.get(&value.body1).copied()
                == Some(string(field(constraint, "constraint_bone_1")?)?),
            "target joint child",
        )?;
        require(
            labels.get(&value.body2).copied()
                == Some(string(field(constraint, "constraint_bone_2")?)?),
            "target joint parent",
        )?;
    }
    Ok(())
}

fn read_json(path: &str) -> Result<Value, String> {
    serde_json::from_str(&fs::read_to_string(path).map_err(to_error)?).map_err(to_error)
}

fn field<'a>(value: &'a Value, key: &str) -> Result<&'a Value, String> {
    value.get(key).ok_or_else(|| format!("missing field {key}"))
}

fn array(value: &Value) -> Result<&[Value], String> {
    value
        .as_array()
        .map(Vec::as_slice)
        .ok_or("expected array".into())
}

fn object(value: &Value) -> Result<&serde_json::Map<String, Value>, String> {
    value.as_object().ok_or("expected object".into())
}

fn string(value: &Value) -> Result<&str, String> {
    value.as_str().ok_or("expected string".into())
}

fn number(value: &Value) -> Result<f32, String> {
    value
        .as_f64()
        .map(|value| value as f32)
        .ok_or("expected number".into())
}

fn u8_value(value: &Value) -> Result<u8, String> {
    value
        .as_u64()
        .and_then(|value| u8::try_from(value).ok())
        .ok_or("expected u8".into())
}

fn json_array<const N: usize>(value: &Value) -> Result<[f32; N], String> {
    let values = array(value)?;
    if values.len() != N {
        return Err(format!("expected array length {N}"));
    }
    let mut output = [0.0; N];
    for (index, value) in values.iter().enumerate() {
        output[index] = number(value)?;
    }
    Ok(output)
}

fn json_vec3(value: &Value) -> Result<Vec3, String> {
    Ok(Vec3::from(json_array(value)?))
}

fn source_vector(value: &Value) -> Result<Vec3, String> {
    Ok(Vec3::new(
        number(field(value, "x")?)?,
        number(field(value, "y")?)?,
        number(field(value, "z")?)?,
    ))
}

fn source_pair(value: &Value) -> Result<[u8; 2], String> {
    Ok([
        u8_value(field(value, "body_index_1")?)?,
        u8_value(field(value, "body_index_2")?)?,
    ])
}

fn manifest_pair(value: &Value) -> Result<[u8; 2], String> {
    let values = array(value)?;
    if values.len() != 2 {
        return Err("manifest collision disabled pair length".into());
    }
    Ok([u8_value(&values[0])?, u8_value(&values[1])?])
}

fn property_map(value: &Value) -> Result<BTreeMap<String, String>, String> {
    array(value)?
        .iter()
        .map(|entry| {
            Ok((
                string(field(entry, "name")?)?.to_owned(),
                string(field(entry, "exported_text")?)?.to_owned(),
            ))
        })
        .collect()
}

fn json_object_map(value: &Value) -> Result<BTreeMap<String, String>, String> {
    object(value)?
        .iter()
        .map(|(key, value)| Ok((key.clone(), string(value)?.to_owned())))
        .collect()
}

fn object_properties(
    objects: &[Value],
    kind: &str,
    name: &str,
) -> Result<BTreeMap<String, String>, String> {
    for entry in objects {
        let properties = property_map(field(entry, "properties")?)?;
        match kind {
            "body" if properties.get("BoneName").map(String::as_str) == Some(name) => {
                return Ok(properties);
            }
            "constraint"
                if properties
                    .get("DefaultInstance")
                    .is_some_and(|value| value.contains(&format!("JointName=\"{name}\""))) =>
            {
                return Ok(properties);
            }
            _ => {}
        }
    }
    Err(format!("missing source {kind} properties for {name}"))
}

fn close(left: f32, right: f32) -> bool {
    (left - right).abs() <= 1e-5
}

fn vec_close(left: Vec3, right: Vec3) -> bool {
    left.distance(right) <= 1e-5
}

fn require(condition: bool, name: &str) -> Result<(), String> {
    if condition {
        Ok(())
    } else {
        Err(format!("mismatch: {name}"))
    }
}

fn to_error(error: impl std::fmt::Display) -> String {
    error.to_string()
}
