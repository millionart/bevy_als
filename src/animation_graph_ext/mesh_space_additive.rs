use std::{collections::{HashMap, HashSet}, sync::Arc};

use bevy::{math::{DQuat, DVec3}, prelude::*};
use bevy_animation_graph::core::{
    animation_graph::TimeUpdate,
    animation_node::{NodeLike, ReflectNodeLike},
    context::{new_context::NodeContext, spec_context::SpecContext},
    edge_data::DataSpec,
    errors::GraphError,
    pose::{BonePose, Pose, RootMotionDelta},
    skeleton::Skeleton,
};

use crate::extensions::{
    raw_sequence_sampling::interpolate_rotation,
    source_compression::source_quaternion_product,
    source_pose::{SourcePose, weighted_add},
};

const ALPHA_THRESHOLD: f32 = 1e-6;

#[derive(Reflect, Clone, Debug, Default)]
#[reflect(Default, NodeLike)]
#[type_path = "als_ext"]
pub struct MeshSpaceAdditiveNode;

impl MeshSpaceAdditiveNode {
    pub const FACTOR: &'static str = "factor";
    pub const IN_POSE_A: &'static str = "pose_a";
    pub const IN_TIME_A: &'static str = "time_a";
    pub const IN_POSE_B: &'static str = "pose_b";
    pub const IN_TIME_B: &'static str = "time_b";
    pub const OUT_POSE: &'static str = "pose";
}

pub(crate) fn source_pose_rotation_product(left: Quat, right: Quat) -> Quat {
    let (x, y, z, w) = (-left.x, -left.z, -left.y, left.w);
    let (a, b, c, d) = (-right.x, -right.z, -right.y, right.w);
    let source_x = w * a + x * d + y * c - z * b;
    let source_y = w * b - x * c + y * d + z * a;
    let source_z = w * c + x * b - y * a + z * d;
    let source_w = w * d - x * a - y * b - z * c;
    Quat::from_xyzw(-source_x, -source_z, -source_y, source_w)
}

pub(crate) fn normalize_pose_rotation(rotation: Quat) -> Quat {
    #[cfg(target_arch = "aarch64")]
    {
        use std::arch::aarch64::{vdupq_n_f32, vgetq_lane_f32, vmulq_f32, vrsqrteq_f32, vrsqrtsq_f32};

        let reciprocal = unsafe {
            let value = vdupq_n_f32(rotation.length_squared());
            let estimate = vrsqrteq_f32(value);
            let estimate = vmulq_f32(vrsqrtsq_f32(value, vmulq_f32(estimate, estimate)), estimate);
            let estimate = vmulq_f32(vrsqrtsq_f32(value, vmulq_f32(estimate, estimate)), estimate);
            vgetq_lane_f32::<0>(estimate)
        };
        rotation * reciprocal
    }
    #[cfg(not(target_arch = "aarch64"))]
    {
        rotation.normalize()
    }
}

fn accumulate_additive_rotation(base: Quat, additive: Quat, alpha: f32) -> Quat {
    if alpha >= 1.0 - 1e-5 {
        return source_pose_rotation_product(additive * alpha, base);
    }
    let identity_sign = if additive.w >= 0.0 { 1.0 } else { -1.0 };
    let weighted = normalize_pose_rotation(Quat::from_xyzw(
        additive.x * alpha,
        additive.y * alpha,
        additive.z * alpha,
        identity_sign * (1.0 - alpha) + additive.w * alpha,
    ));
    source_pose_rotation_product(weighted, base)
}

fn blend_additive_rotation(base: Quat, additive: Quat, alpha: f32) -> Quat {
    normalize_pose_rotation(accumulate_additive_rotation(base, additive, alpha))
}

fn merge_translation(base: Option<Vec3>, additive: Option<Vec3>, alpha: f32) -> Option<Vec3> {
    match (base, additive) {
        (Some(base), Some(additive)) => Some(base + alpha * additive),
        (Some(base), None) => Some(base),
        (None, Some(additive)) => Some(additive),
        (None, None) => None,
    }
}

fn merge_scale(base: Option<Vec3>, additive: Option<Vec3>, alpha: f32) -> Option<Vec3> {
    match (base, additive) {
        (Some(base), Some(additive)) => Some(base * (Vec3::ONE + alpha * additive)),
        (Some(base), None) => Some(base),
        (None, Some(additive)) => Some(Vec3::ONE + alpha * additive),
        (None, None) => None,
    }
}

fn merge_weights(
    base: Option<Vec<f32>>,
    additive: Option<Vec<f32>>,
    alpha: f32,
) -> Option<Vec<f32>> {
    match (base, additive) {
        (Some(base), Some(additive)) => Some(
            base.into_iter()
                .zip(additive)
                .map(|(base, additive)| base + alpha * additive)
                .collect(),
        ),
        (Some(base), None) => Some(base),
        (None, Some(additive)) => Some(additive),
        (None, None) => None,
    }
}

fn merge_non_rotation(base: &mut BonePose, additive: &BonePose, alpha: f32) {
    base.translation = merge_translation(base.translation, additive.translation, alpha);
    base.scale = merge_scale(base.scale, additive.scale, alpha);
    base.weights = merge_weights(base.weights.clone(), additive.weights.clone(), alpha);
}

fn hierarchy_order(skeleton: &Skeleton) -> Vec<bevy_animation_graph::core::pose::BoneId> {
    fn visit(
        skeleton: &Skeleton,
        bone: bevy_animation_graph::core::pose::BoneId,
        visited: &mut HashSet<bevy_animation_graph::core::pose::BoneId>,
        order: &mut Vec<bevy_animation_graph::core::pose::BoneId>,
    ) {
        if !visited.insert(bone) {
            return;
        }
        if let Some(parent) = skeleton.parent(&bone) {
            visit(skeleton, parent, visited, order);
        }
        order.push(bone);
    }

    let mut visited = HashSet::default();
    let mut order = Vec::new();
    visit(skeleton, skeleton.root(), &mut visited, &mut order);
    for bone in skeleton.iter_bones() {
        visit(skeleton, bone, &mut visited, &mut order);
    }
    order
}

pub(crate) fn materialize_missing_reference_bones(base: &mut Pose, skeleton: &Skeleton) {
    for bone in skeleton.iter_bones() {
        if bone == skeleton.root() || base.paths.contains_key(&bone) {
            continue;
        }
        let Some(transform) = skeleton.default_transforms(bone) else {
            continue;
        };
        base.add_bone(BonePose::from_transform(transform.local), bone);
    }
}

fn base_local_rotation(
    base: &Pose,
    skeleton: &Skeleton,
    bone: bevy_animation_graph::core::pose::BoneId,
) -> Quat {
    base.paths
        .get(&bone)
        .and_then(|index| base.bones.get(*index))
        .and_then(|pose| pose.rotation)
        .or_else(|| {
            skeleton
                .default_transforms(bone)
                .map(|transform| transform.local.rotation)
        })
        .unwrap_or(Quat::IDENTITY)
}

fn additive_rotation(
    additive: &Pose,
    bone: bevy_animation_graph::core::pose::BoneId,
) -> Option<Quat> {
    additive
        .paths
        .get(&bone)
        .and_then(|index| additive.bones.get(*index))
        .and_then(|pose| pose.rotation)
}

fn merge_root_motion(
    base: Option<RootMotionDelta>,
    additive: Option<RootMotionDelta>,
    alpha: f32,
) -> Option<RootMotionDelta> {
    match (base, additive) {
        (Some(base), Some(additive)) => Some(base.additive_blend(&additive, alpha)),
        (Some(base), None) => Some(base),
        (None, Some(additive)) => Some(RootMotionDelta::default().additive_blend(&additive, alpha)),
        (None, None) => None,
    }
}

fn accumulate_local_root_motion(base: &mut Pose, additive: &Pose, alpha: f32) {
    if let Some(additive_motion) = &additive.root_motion {
        let motion = base.root_motion.get_or_insert_with(RootMotionDelta::default);
        motion.translation += additive_motion.translation * alpha;
        motion.rotation = blend_additive_rotation(motion.rotation, additive_motion.rotation, alpha);
    }
}

pub(crate) fn accumulate_source_additive(
    base: &mut SourcePose,
    additive: &SourcePose,
    alpha: f32,
    parents: Option<&[Option<usize>]>,
    normalization_passes: usize,
) -> Option<()> {
    if !base.same_layout(additive) {
        return None;
    }
    if alpha <= 1e-5 {
        return Some(());
    }
    if let Some(parents) = parents {
        base.convert_rotations_to_mesh(parents);
    }
    let weight = f64::from(alpha);
    let full = alpha >= 1.0 - 1e-5;
    for ((_, base), &(_, additive)) in base.bones.iter_mut().zip(&additive.bones) {
        let delta = if full {
            base.translation = weighted_add(base.translation, additive.translation, weight);
            base.scale *= weighted_add(DVec3::ONE, additive.scale, weight);
            additive.rotation * weight
        } else {
            base.translation += additive.translation * weight;
            base.scale *= DVec3::ONE + additive.scale * weight;
            interpolate_rotation(DQuat::IDENTITY, additive.rotation, alpha)
        };
        base.rotation = source_quaternion_product(delta, base.rotation);
    }
    if let Some(parents) = parents {
        base.convert_rotations_to_local(parents);
    }
    base.normalize_rotations(normalization_passes);
    Some(())
}

pub(crate) fn apply_source_additive_pose(
    base: &mut Pose,
    additive: &Pose,
    precise: Option<(&SourcePose, &SourcePose)>,
    alpha: f32,
    skeleton: &Skeleton,
    mesh_space: bool,
    outer_normalization: bool,
) -> Option<Arc<SourcePose>> {
    let source = precise.and_then(|(source, delta)| {
        if base.bones.iter().chain(&additive.bones).any(|bone| bone.weights.is_some()) {
            return None;
        }
        let mut result = source.clone();
        let parents = if mesh_space { Some(source.parent_indices(skeleton)?) } else { None };
        accumulate_source_additive(&mut result, delta, alpha, parents.as_deref(), 1 + usize::from(outer_normalization))?;
        Some(result)
    });
    if let Some(source) = source {
        source.project_into(base);
        if mesh_space {
            base.root_motion = merge_root_motion(base.root_motion.take(), additive.root_motion.clone(), alpha);
        } else {
            accumulate_local_root_motion(base, additive, alpha);
        }
        return Some(Arc::new(source));
    }
    if mesh_space {
        apply_mesh_space_additive(base, additive, alpha, skeleton);
    } else {
        apply_local_space_additive(base, additive, alpha, skeleton);
    }
    if outer_normalization {
        crate::extensions::source_pose_nodes::normalize_source_pose_rotations(base, 1);
    }
    None
}

pub fn apply_local_space_additive(base: &mut Pose, additive: &Pose, alpha: f32, skeleton: &Skeleton) {
    if alpha.abs() <= ALPHA_THRESHOLD { return; }
    for (bone, additive_index) in &additive.paths {
        if !base.paths.contains_key(bone)
            && let Some(transform) = skeleton.default_transforms(*bone) {
            base.add_bone(BonePose::from_transform(transform.local), *bone);
        }
        let Some(base_index) = base.paths.get(bone).copied() else { continue; };
        let additive_bone = &additive.bones[*additive_index];
        let base_rotation = base_local_rotation(base, skeleton, *bone);
        let base_bone = &mut base.bones[base_index];
        if let Some(rotation) = additive_bone.rotation {
            base_bone.rotation = Some(blend_additive_rotation(base_rotation, rotation, alpha));
        }
        merge_non_rotation(base_bone, additive_bone, alpha);
    }
    accumulate_local_root_motion(base, additive, alpha);
}

pub fn apply_mesh_space_additive(
    base: &mut Pose,
    additive: &Pose,
    alpha: f32,
    skeleton: &Skeleton,
) {
    if alpha.abs() <= ALPHA_THRESHOLD {
        return;
    }

    materialize_missing_reference_bones(base, skeleton);

    for (bone, additive_index) in &additive.paths {
        let additive_bone = &additive.bones[*additive_index];
        if let Some(base_index) = base.paths.get(bone).copied() {
            merge_non_rotation(&mut base.bones[base_index], additive_bone, alpha);
        } else {
            let mut base_bone = skeleton
                .default_transforms(*bone)
                .map(|transform| BonePose {
                    translation: Some(transform.local.translation),
                    rotation: Some(transform.local.rotation),
                    scale: Some(transform.local.scale),
                    weights: None,
                })
                .unwrap_or_default();
            merge_non_rotation(&mut base_bone, additive_bone, alpha);
            base.add_bone(base_bone, *bone);
        }
    }

    let hierarchy = hierarchy_order(skeleton);
    let mut mesh_rotations: HashMap<bevy_animation_graph::core::pose::BoneId, Quat> =
        HashMap::default();
    for bone in &hierarchy {
        let local = base_local_rotation(base, skeleton, *bone);
        let rotation = skeleton
            .parent(bone)
            .and_then(|parent| mesh_rotations.get(&parent).copied())
            .map(|parent| source_pose_rotation_product(parent, local))
            .unwrap_or(local);
        mesh_rotations.insert(*bone, rotation);
    }

    for bone in &hierarchy {
        if let (Some(rotation), Some(additive_rotation)) = (
            mesh_rotations.get(bone).copied(),
            additive_rotation(additive, *bone),
        ) {
            mesh_rotations.insert(
                *bone,
                accumulate_additive_rotation(rotation, additive_rotation, alpha),
            );
        }
    }

    for bone in hierarchy.iter().rev() {
        let Some(rotation) = mesh_rotations.get(bone).copied() else {
            continue;
        };
        let local = normalize_pose_rotation(skeleton
            .parent(bone)
            .and_then(|parent| mesh_rotations.get(&parent).copied())
            .map(|parent| source_pose_rotation_product(parent.inverse(), rotation))
            .unwrap_or(rotation));
        if let Some(base_index) = base.paths.get(bone).copied() {
            base.bones[base_index].rotation = Some(local);
        }
    }

    base.root_motion = merge_root_motion(
        base.root_motion.clone(),
        additive.root_motion.clone(),
        alpha,
    );
}

pub fn compose_mesh_space_additives<'a>(
    samples: impl IntoIterator<Item = (&'a Pose, f32)>,
) -> Option<Pose> {
    let mut output: Option<Pose> = None;
    for (pose, weight) in samples {
        if weight <= ALPHA_THRESHOLD { continue; }
        let output = output.get_or_insert_with(|| Pose {
            skeleton: pose.skeleton.clone(),
            ..default()
        });
        output.timestamp = pose.timestamp;
        for (bone, index) in &pose.paths {
            let additive = &pose.bones[*index];
            if !output.paths.contains_key(bone) {
                output.add_bone(BonePose {
                    translation: Some(Vec3::ZERO),
                    rotation: Some(Quat::IDENTITY),
                    scale: Some(Vec3::ONE),
                    weights: None,
                }, *bone);
            }
            let index = output.paths[bone];
            let target = &mut output.bones[index];
            if let Some(rotation) = additive.rotation {
                target.rotation = Some(blend_additive_rotation(target.rotation.unwrap(), rotation, weight));
            }
            if target.weights.is_none() {
                target.weights = additive.weights.as_ref().map(|weights| vec![0.0; weights.len()]);
            }
            merge_non_rotation(target, additive, weight);
        }
        output.root_motion = merge_root_motion(output.root_motion.take(), pose.root_motion.clone(), weight);
    }
    if let Some(output) = output.as_mut() {
        for bone in &mut output.bones {
            bone.scale = bone.scale.map(|scale| scale - Vec3::ONE);
        }
    }
    output
}

impl NodeLike for MeshSpaceAdditiveNode {
    fn duration(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        ctx.set_duration_fwd(ctx.duration_back(Self::IN_TIME_A)?);
        Ok(())
    }

    fn update(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        let input = ctx.time_update_fwd()?;
        ctx.set_time_update_back(Self::IN_TIME_A, input);
        let mut base = ctx.data_back(Self::IN_POSE_A)?.into_pose()?;
        let alpha = ctx.data_back(Self::FACTOR)?.as_f32()?;

        if alpha.abs() > ALPHA_THRESHOLD {
            ctx.set_time_update_back(Self::IN_TIME_B, TimeUpdate::Absolute(base.timestamp));
            let additive = ctx.data_back(Self::IN_POSE_B)?.into_pose()?;
            if ctx
                .graph_context
                .resources
                .skeleton_assets
                .get(&base.skeleton)
                .is_none()
            {
                base.skeleton = additive.skeleton.clone();
            }
            let Some(skeleton) = ctx
                .graph_context
                .resources
                .skeleton_assets
                .get(&base.skeleton)
            else {
                return Err(GraphError::SkeletonMissing(ctx.node_id));
            };
            apply_mesh_space_additive(&mut base, &additive, alpha, skeleton);
        }

        ctx.set_time(base.timestamp);
        ctx.set_data_fwd(Self::OUT_POSE, base);
        Ok(())
    }

    fn spec(&self, mut ctx: SpecContext) -> Result<(), GraphError> {
        ctx.add_input_data(Self::FACTOR, DataSpec::F32)
            .add_input_data(Self::IN_POSE_A, DataSpec::Pose)
            .add_input_time(Self::IN_TIME_A)
            .add_input_data(Self::IN_POSE_B, DataSpec::Pose)
            .add_input_time(Self::IN_TIME_B)
            .add_output_data(Self::OUT_POSE, DataSpec::Pose)
            .add_output_time();
        Ok(())
    }

    fn display_name(&self) -> String {
        "ALS Mesh Space Additive".into()
    }
}

#[cfg(test)]
mod tests {
    use bevy_animation_graph::core::animation_clip::EntityPath;

    use super::*;

    fn close_rotation(left: Quat, right: Quat) -> bool {
        (1.0 - left.dot(right).abs()).abs() < 1e-5
    }

    fn test_skeleton() -> (
        Skeleton,
        bevy_animation_graph::core::pose::BoneId,
        bevy_animation_graph::core::pose::BoneId,
    ) {
        let root_path = EntityPath::default().child("root");
        let child_path = root_path.child("child");
        let root = root_path.id();
        let child = child_path.id();
        let mut skeleton = Skeleton::default();
        skeleton.set_root(root);
        skeleton.add_bone(root_path, Transform::IDENTITY, Transform::IDENTITY);
        skeleton.add_bone(
            child_path,
            Transform::from_translation(Vec3::new(0.0, 2.0, 0.0)),
            Transform::IDENTITY,
        );
        (skeleton, root, child)
    }

    #[test]
    #[cfg(all(target_arch = "aarch64", target_os = "macos"))]
    fn mesh_space_additive_uses_source_reciprocal_for_actual_head_chain() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"), "/docs/audit/bf144/d248b1/head-fixture.json"
        ))).unwrap();
        let quaternion = |value: &serde_json::Value| {
            let values = value.as_array().unwrap();
            Quat::from_xyzw(values[0].as_f64().unwrap() as f32, values[1].as_f64().unwrap() as f32,
                values[2].as_f64().unwrap() as f32, values[3].as_f64().unwrap() as f32)
        };
        let nodes = fixture["nodes"].as_array().unwrap();
        let mut mesh: Vec<Quat> = Vec::with_capacity(nodes.len());
        for node in nodes {
            let local = if node["base_rotation"].is_null() { Quat::IDENTITY } else { quaternion(&node["base_rotation"]) };
            mesh.push(mesh.last().map_or(local, |parent| *parent * local));
        }
        for (rotation, node) in mesh.iter_mut().zip(nodes) {
            if !node["additive_rotation"].is_null() {
                let delta = normalize_pose_rotation(quaternion(&node["additive_rotation"]));
                *rotation = normalize_pose_rotation(delta * *rotation);
            }
        }
        let last = mesh.len() - 1;
        let local = normalize_pose_rotation(mesh[last - 1].inverse() * mesh[last]);
        assert_eq!(local, quaternion(&fixture["expected_control_rotation"]));
    }

    #[test]
    fn mesh_space_additive_keeps_source_product_order_through_public_pose() {
        #[derive(serde::Deserialize)]
        struct Fixture {
            cases: Vec<Case>,
        }

        #[derive(serde::Deserialize)]
        struct Case {
            name: String,
            parent: [f32; 4],
            child: [f32; 4],
            mesh: [f32; 4],
        }

        let fixture: Fixture = serde_json::from_str(include_str!(
            "../../docs/audit/bf144/d163c/layered.replay.json"
        )).unwrap();
        let (skeleton, root, child) = test_skeleton();
        let delta = Quat::from_xyzw(0.5, -0.5, 0.5, -0.5);
        let weighted = normalize_pose_rotation(Quat::from_xyzw(0.25, -0.25, 0.25, -0.75));
        let mut additive = Pose::default();
        additive.add_bone(BonePose { rotation: Some(delta), ..default() }, child);
        for case in fixture.cases {
            let parent = Quat::from_array(case.parent);
            let mut base = Pose::default();
            base.add_bone(BonePose { rotation: Some(parent), ..default() }, root);
            base.add_bone(BonePose { rotation: Some(Quat::from_array(case.child)), ..default() }, child);
            let mesh = source_pose_rotation_product(weighted, Quat::from_array(case.mesh));
            let expected = normalize_pose_rotation(source_pose_rotation_product(parent.inverse(), mesh));
            apply_mesh_space_additive(&mut base, &additive, 0.5, &skeleton);
            assert_eq!(base.get_bone(child).unwrap().rotation, Some(expected), "{}", case.name);
        }
    }

    #[test]
    fn additive_pose_uses_the_source_full_weight_rotation_branch() {
        #[derive(serde::Deserialize)]
        struct Fixture {
            cases: Vec<Case>,
        }

        #[derive(serde::Deserialize)]
        struct Case {
            name: String,
            base: [f32; 4],
            delta: [f32; 4],
            weight: f32,
            full: bool,
            base_translation: [f32; 3],
            delta_translation: [f32; 3],
            base_scale: [f32; 3],
            scale_delta: [f32; 3],
            expected_rotation: [f32; 4],
            expected_translation: [f32; 3],
            expected_scale: [f32; 3],
        }

        let fixture: Fixture = serde_json::from_str(include_str!(
            "../../docs/audit/bf144/d170/additive.replay.json"
        )).unwrap();
        let (skeleton, root, _) = test_skeleton();
        for case in fixture.cases {
            let base_rotation = Quat::from_array(case.base);
            let additive_rotation = Quat::from_array(case.delta);
            let raw = accumulate_additive_rotation(base_rotation, additive_rotation, case.weight);
            let full = source_pose_rotation_product(additive_rotation * case.weight, base_rotation);
            assert_eq!(raw == full, case.full, "{}", case.name);
            let mut base = Pose::default();
            base.add_bone(BonePose::from_transform(Transform {
                translation: Vec3::from_array(case.base_translation), rotation: base_rotation,
                scale: Vec3::from_array(case.base_scale),
            }), root);
            let mut additive = Pose::default();
            additive.add_bone(BonePose::from_transform(Transform {
                translation: Vec3::from_array(case.delta_translation), rotation: additive_rotation,
                scale: Vec3::from_array(case.scale_delta),
            }), root);
            for apply in [apply_local_space_additive as fn(&mut Pose, &Pose, f32, &Skeleton), apply_mesh_space_additive] {
                let mut output = base.clone();
                apply(&mut output, &additive, case.weight, &skeleton);
                let bone = output.get_bone(root).unwrap();
                let expected = Quat::from_array(case.expected_rotation);
                let rotation = bone.rotation.unwrap();
                assert!((rotation - expected).length().min((rotation + expected).length()) < 3e-7,
                    "{}: {rotation:?}", case.name);
                assert!(bone.translation.unwrap().distance(Vec3::from_array(case.expected_translation)) < 1e-7);
                assert!(bone.scale.unwrap().distance(Vec3::from_array(case.expected_scale)) < 3e-7);
            }
        }
    }

    #[test]
    fn mesh_component_product_is_not_normalized_early() {
        let base = Quat::from_xyzw(0.0, 0.0, 0.5, 0.75);
        let product = accumulate_additive_rotation(base, Quat::IDENTITY, 1.0);
        assert_eq!(product, base);
        assert_ne!(blend_additive_rotation(base, Quat::IDENTITY, 1.0), product);
    }

    #[test]
    #[cfg(all(target_arch = "aarch64", target_os = "macos"))]
    fn head_mesh_output_drops_historical_component_normalization() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../docs/audit/bf144/d241/head-product-fixture.json"
        )).unwrap();
        let quaternion = |value: &serde_json::Value| {
            let values = value.as_array().unwrap();
            Quat::from_xyzw(values[0].as_f64().unwrap() as f32, values[1].as_f64().unwrap() as f32,
                values[2].as_f64().unwrap() as f32, values[3].as_f64().unwrap() as f32)
        };
        let mut path = EntityPath::default();
        let mut skeleton = Skeleton::default();
        let mut base = Pose::default();
        let mut additive = Pose::default();
        for (index, node) in fixture["nodes"].as_array().unwrap().iter().enumerate() {
            path = path.child(node["name"].as_str().unwrap().to_owned());
            if index == 0 { skeleton.set_root(path.id()); }
            skeleton.add_bone(path.clone(), Transform::IDENTITY, Transform::IDENTITY);
            for (pose, key) in [(&mut base, "base_rotation"), (&mut additive, "additive_rotation")] {
                if !node[key].is_null() {
                    pose.add_bone(BonePose { rotation: Some(quaternion(&node[key])), ..default() }, path.id());
                }
            }
        }
        apply_mesh_space_additive(&mut base, &additive, 1.0, &skeleton);
        assert_ne!(base.get_bone(path.id()).unwrap().rotation.unwrap(), quaternion(&fixture["historical_rotation"]));
    }

    #[test]
    fn composed_mesh_additives_match_sequential_slot_accumulation() {
        let (skeleton, root, child) = test_skeleton();
        let mut base = Pose::default();
        let mut first = Pose::default();
        let mut second = Pose::default();
        for bone in [root, child] {
            base.add_bone(BonePose::from_transform(Transform {
                translation: Vec3::new(0.4, 1.2, -0.8),
                rotation: Quat::from_rotation_z(0.7),
                scale: Vec3::new(0.8, 1.2, 1.1),
            }), bone);
            first.add_bone(BonePose {
                translation: Some(Vec3::new(0.2, -0.1, 0.3)),
                rotation: Some(Quat::from_rotation_x(0.8)),
                scale: Some(Vec3::new(0.1, -0.2, 0.3)),
                weights: Some(vec![0.3]),
            }, bone);
            second.add_bone(BonePose {
                translation: Some(Vec3::new(-0.3, 0.4, 0.1)),
                rotation: Some(-Quat::from_rotation_y(0.6)),
                scale: Some(Vec3::new(-0.2, 0.3, 0.1)),
                weights: Some(vec![-0.1]),
            }, bone);
            base.bones.last_mut().unwrap().weights = Some(vec![0.5]);
        }
        for (left, right) in [(1.0, 0.0), (0.84375, 0.15625), (0.5, 0.5), (0.0, 1.0), (0.2, 0.3)] {
            let combined = compose_mesh_space_additives([(&first, left), (&second, right)]).unwrap();
            let mut expected = base.clone();
            apply_mesh_space_additive(&mut expected, &first, left, &skeleton);
            apply_mesh_space_additive(&mut expected, &second, right, &skeleton);
            let mut actual = base.clone();
            apply_mesh_space_additive(&mut actual, &combined, 1.0, &skeleton);
            for bone in [root, child] {
                let expected = expected.get_bone(bone).unwrap();
                let actual = actual.get_bone(bone).unwrap();
                assert!(close_rotation(expected.rotation.unwrap(), actual.rotation.unwrap()));
                assert!(expected.translation.unwrap().distance(actual.translation.unwrap()) < 1e-6);
                assert!(expected.scale.unwrap().distance(actual.scale.unwrap()) < 1e-6);
                assert!((expected.weights.as_ref().unwrap()[0] - actual.weights.as_ref().unwrap()[0]).abs() < 1e-6);
            }
        }
        assert!(compose_mesh_space_additives([(&first, 0.0)]).is_none());
    }

    #[test]
    fn mesh_space_rotation_keeps_the_additive_axis_in_mesh_space() {
        let (skeleton, root, child) = test_skeleton();
        let root_rotation = Quat::from_rotation_y(std::f32::consts::FRAC_PI_2);
        let additive_rotation = Quat::from_rotation_x(std::f32::consts::FRAC_PI_2);
        let mut base = Pose::default();
        base.add_bone(
            BonePose {
                rotation: Some(root_rotation),
                ..default()
            },
            root,
        );
        base.add_bone(
            BonePose {
                rotation: Some(Quat::IDENTITY),
                ..default()
            },
            child,
        );
        let mut additive = Pose::default();
        additive.add_bone(
            BonePose {
                rotation: Some(additive_rotation),
                ..default()
            },
            child,
        );

        apply_mesh_space_additive(&mut base, &additive, 1.0, &skeleton);

        let output = base.get_bone(child).and_then(|bone| bone.rotation).unwrap();
        let expected = root_rotation.inverse() * additive_rotation * root_rotation;
        assert!(close_rotation(output, expected));
        assert!(!close_rotation(output, additive_rotation));
    }

    #[test]
    fn mesh_space_additive_keeps_translation_additive_in_local_space() {
        let (skeleton, root, child) = test_skeleton();
        let mut base = Pose::default();
        base.add_bone(
            BonePose {
                rotation: Some(Quat::IDENTITY),
                ..default()
            },
            root,
        );
        base.add_bone(
            BonePose {
                translation: Some(Vec3::new(1.0, 2.0, 3.0)),
                rotation: Some(Quat::IDENTITY),
                ..default()
            },
            child,
        );
        let mut additive = Pose::default();
        additive.add_bone(
            BonePose {
                translation: Some(Vec3::new(4.0, 6.0, 8.0)),
                ..default()
            },
            child,
        );

        apply_mesh_space_additive(&mut base, &additive, 0.5, &skeleton);

        assert_eq!(
            base.get_bone(child).and_then(|bone| bone.translation),
            Some(Vec3::new(3.0, 5.0, 7.0))
        );
    }

    #[test]
    fn mesh_space_additive_materializes_missing_reference_bones() {
        let (skeleton, root, child) = test_skeleton();
        let mut additive = Pose::default();
        additive.add_bone(
            BonePose {
                translation: Some(Vec3::new(1.0, 0.0, 0.0)),
                ..default()
            },
            root,
        );
        let mut base = Pose::default();

        apply_mesh_space_additive(&mut base, &additive, 1.0, &skeleton);

        assert_eq!(
            base.get_bone(child).and_then(|bone| bone.translation),
            Some(Vec3::new(0.0, 2.0, 0.0))
        );
    }

    #[test]
    fn additive_reference_fill_preserves_unanimated_scene_root_world_transform() {
        use bevy::animation::{AnimatedBy, AnimationTargetId};
        use bevy_animation_graph::core::{
            animation_graph_player::AnimationGraphPlayer, systems::apply_animation_to_targets,
        };

        let (skeleton, scene_root, joint) = test_skeleton();
        let mut base = Pose::default();
        base.add_bone(
            BonePose::from_transform(Transform::from_xyz(0.0, 2.0, 0.0)),
            joint,
        );
        let mut additive = Pose::default();
        additive.add_bone(
            BonePose {
                rotation: Some(Quat::from_rotation_x(0.2)),
                ..default()
            },
            joint,
        );
        apply_mesh_space_additive(&mut base, &additive, 1.0, &skeleton);

        let mut world = World::new();
        let scene_world =
            Transform::from_xyz(4.0, 1.5, -3.0).with_rotation(Quat::from_rotation_y(0.6));
        let entity = world.spawn(scene_world).id();
        let mut player = AnimationGraphPlayer::new(default()).with_graph(default());
        player.set_default_output_pose(base);
        world.entity_mut(entity).insert((
            player,
            AnimationTargetId(scene_root.id()),
            AnimatedBy(entity),
        ));
        world.run_system_cached(apply_animation_to_targets).unwrap();
        assert_eq!(*world.get::<Transform>(entity).unwrap(), scene_world);
    }

    #[test]
    fn mesh_space_additive_matches_unreal_weighted_transform_accumulation() {
        let (skeleton, root, child) = test_skeleton();
        let base_rotation = Quat::from_rotation_y(0.6);
        let additive_rotation = Quat::from_rotation_x(1.1);
        let mut base = Pose::default();
        base.add_bone(
            BonePose {
                rotation: Some(Quat::IDENTITY),
                ..default()
            },
            root,
        );
        base.add_bone(
            BonePose {
                rotation: Some(base_rotation),
                scale: Some(Vec3::new(2.0, 3.0, 4.0)),
                ..default()
            },
            child,
        );
        let mut additive = Pose::default();
        additive.add_bone(
            BonePose {
                rotation: Some(additive_rotation),
                scale: Some(Vec3::new(0.5, -0.25, 1.0)),
                ..default()
            },
            child,
        );

        apply_mesh_space_additive(&mut base, &additive, 0.25, &skeleton);

        let expected_rotation =
            Quat::IDENTITY.lerp(additive_rotation, 0.25).normalize() * base_rotation;
        let output = base.get_bone(child).unwrap();
        assert!(close_rotation(output.rotation.unwrap(), expected_rotation));
        assert_eq!(output.scale, Some(Vec3::new(2.25, 2.8125, 5.0)));
    }

    #[test]
    fn zero_alpha_keeps_the_base_pose() {
        let (skeleton, root, child) = test_skeleton();
        let mut base = Pose::default();
        base.add_bone(
            BonePose {
                rotation: Some(Quat::from_rotation_z(0.2)),
                ..default()
            },
            root,
        );
        base.add_bone(
            BonePose {
                rotation: Some(Quat::from_rotation_x(0.3)),
                ..default()
            },
            child,
        );
        let original = base.clone();

        apply_mesh_space_additive(&mut base, &Pose::default(), 0.0, &skeleton);

        assert_eq!(base, original);
    }

    #[test]
    fn node_registers_node_like_dispatch_key() {
        use bevy::reflect::TypeRegistry;

        let mut registry = TypeRegistry::default();
        registry.register::<MeshSpaceAdditiveNode>();
        let registration = registry
            .get(std::any::TypeId::of::<MeshSpaceAdditiveNode>())
            .unwrap();
        assert!(
            registration
                .data::<bevy_animation_graph::core::animation_node::ReflectNodeLike>()
                .is_some()
        );
    }
}
