use std::collections::{HashMap, HashSet};

use bevy::prelude::*;
use bevy_animation_graph::core::{
    animation_graph::TimeUpdate,
    animation_node::{NodeLike, ReflectNodeLike},
    context::{new_context::NodeContext, spec_context::SpecContext},
    edge_data::DataSpec,
    errors::GraphError,
    pose::{BonePose, Pose, RootMotionDelta},
    skeleton::Skeleton,
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

fn blend_additive_rotation(base: Quat, additive: Quat, alpha: f32) -> Quat {
    let identity_sign = if additive.w >= 0.0 { 1.0 } else { -1.0 };
    let weighted = Quat::from_xyzw(
        additive.x * alpha,
        additive.y * alpha,
        additive.z * alpha,
        identity_sign * (1.0 - alpha) + additive.w * alpha,
    )
    .normalize();
    (weighted * base).normalize()
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

pub fn apply_mesh_space_additive(
    base: &mut Pose,
    additive: &Pose,
    alpha: f32,
    skeleton: &Skeleton,
) {
    if alpha.abs() <= ALPHA_THRESHOLD {
        return;
    }

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
            .map(|parent| parent * local)
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
                blend_additive_rotation(rotation, additive_rotation, alpha),
            );
        }
    }

    for bone in hierarchy.iter().rev() {
        let Some(rotation) = mesh_rotations.get(bone).copied() else {
            continue;
        };
        let local = skeleton
            .parent(bone)
            .and_then(|parent| mesh_rotations.get(&parent).copied())
            .map(|parent| parent.inverse() * rotation)
            .unwrap_or(rotation)
            .normalize();
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
        skeleton.add_bone(child_path, Transform::IDENTITY, Transform::IDENTITY);
        (skeleton, root, child)
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
