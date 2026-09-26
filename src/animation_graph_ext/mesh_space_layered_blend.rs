use std::collections::{HashMap, HashSet};

use bevy::prelude::*;
use bevy_animation_graph::core::{
    animation_graph::TimeUpdate,
    animation_node::{NodeLike, ReflectNodeLike},
    context::{new_context::NodeContext, spec_context::SpecContext},
    edge_data::{DataSpec, bone_mask::BoneMask},
    errors::GraphError,
    interpolation::linear::LinearInterpolator,
    pose::{BoneId, BonePose, Pose},
    skeleton::Skeleton,
};

const WEIGHT_THRESHOLD: f32 = 1e-6;

#[derive(Reflect, Clone, Debug, Default)]
#[reflect(Default, NodeLike)]
#[type_path = "als_ext"]
pub struct MeshSpaceLayeredBlendNode;

impl MeshSpaceLayeredBlendNode {
    pub const FACTOR: &'static str = "factor";
    pub const IN_POSE_A: &'static str = "pose_a";
    pub const IN_TIME_A: &'static str = "time_a";
    pub const IN_POSE_B: &'static str = "pose_b";
    pub const IN_TIME_B: &'static str = "time_b";
    pub const IN_BONE_MASK: &'static str = "bone_mask";
    pub const OUT_POSE: &'static str = "pose";
}

fn hierarchy_order(skeleton: &Skeleton) -> Vec<BoneId> {
    fn visit(
        skeleton: &Skeleton,
        bone: BoneId,
        visited: &mut HashSet<BoneId>,
        order: &mut Vec<BoneId>,
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

fn local_rotation(pose: &Pose, skeleton: &Skeleton, bone: BoneId) -> Quat {
    pose.paths
        .get(&bone)
        .and_then(|index| pose.bones.get(*index))
        .and_then(|pose| pose.rotation)
        .or_else(|| {
            skeleton
                .default_transforms(bone)
                .map(|transform| transform.local.rotation)
        })
        .unwrap_or(Quat::IDENTITY)
}

pub(crate) fn layered_bone_hierarchy(skeleton: &Skeleton) -> Vec<(BoneId, Option<usize>)> {
    let order = hierarchy_order(skeleton);
    let indices: HashMap<_, _> = order.iter().enumerate().map(|(index, bone)| (*bone, index)).collect();
    order.into_iter().map(|bone| {
        let parent = skeleton.parent(&bone).and_then(|parent| indices.get(&parent).copied());
        (bone, parent)
    }).collect()
}

fn mesh_rotations(pose: &Pose, skeleton: &Skeleton, hierarchy: &[(BoneId, Option<usize>)]) -> Vec<Quat> {
    let mut rotations: Vec<Quat> = Vec::with_capacity(hierarchy.len());
    for (bone, parent) in hierarchy {
        let local = local_rotation(pose, skeleton, *bone);
        let mesh = parent
            .and_then(|parent| rotations.get(parent).copied())
            .map(|parent| parent * local)
            .unwrap_or(local);
        rotations.push(mesh);
    }
    rotations
}

pub fn apply_mesh_space_layered_blend(
    base: &mut Pose,
    overlay: &Pose,
    alpha: f32,
    bone_mask: &BoneMask,
    skeleton: &Skeleton,
) {
    if alpha.clamp(0.0, 1.0) <= WEIGHT_THRESHOLD {
        return;
    }
    let hierarchy = layered_bone_hierarchy(skeleton);
    apply_mesh_space_layered_blend_with_hierarchy(base, overlay, alpha, bone_mask, skeleton, &hierarchy);
}

pub(crate) fn apply_mesh_space_layered_blend_with_hierarchy(
    base: &mut Pose,
    overlay: &Pose,
    alpha: f32,
    bone_mask: &BoneMask,
    skeleton: &Skeleton,
    hierarchy: &[(BoneId, Option<usize>)],
) {
    let alpha = alpha.clamp(0.0, 1.0);
    if alpha <= WEIGHT_THRESHOLD {
        return;
    }

    let source_mesh = mesh_rotations(base, skeleton, hierarchy);
    let overlay_mesh = mesh_rotations(overlay, skeleton, hierarchy);
    LinearInterpolator {
        bone_mask: bone_mask.clone(),
    }
    .interpolate_pose(base, overlay, alpha);

    let mut output_mesh = Vec::with_capacity(hierarchy.len());

    for (index, &(bone, parent)) in hierarchy.iter().enumerate() {
        let mask_weight = bone_mask.bone_weight(&bone).clamp(0.0, 1.0);
        let weight = (alpha * mask_weight).clamp(0.0, 1.0);
        let rotation = if weight <= WEIGHT_THRESHOLD {
            source_mesh[index]
        } else {
            let source_rotation = source_mesh[index];
            let overlay_rotation = overlay_mesh[index];
            source_rotation.lerp(overlay_rotation, weight).normalize()
        };
        output_mesh.push(rotation);

        if weight <= WEIGHT_THRESHOLD
            && !base.paths.contains_key(&bone)
            && !overlay.paths.contains_key(&bone)
            && parent.is_none_or(|parent| {
                output_mesh.get(parent) == source_mesh.get(parent)
            })
        {
            continue;
        }
        let local = parent
            .and_then(|parent| output_mesh.get(parent).copied())
            .map(|parent| parent.inverse() * rotation)
            .unwrap_or(rotation)
            .normalize();
        if let Some(index) = base.paths.get(&bone).copied() {
            base.bones[index].rotation = Some(local);
        } else {
            base.add_bone(
                BonePose {
                    rotation: Some(local),
                    ..default()
                },
                bone,
            );
        }
    }
}

fn apply_local_layered_blend(base: &mut Pose, overlay: &Pose, alpha: f32, bone_mask: &BoneMask) {
    LinearInterpolator {
        bone_mask: bone_mask.clone(),
    }
    .interpolate_pose(base, overlay, alpha.clamp(0.0, 1.0));
}

impl NodeLike for MeshSpaceLayeredBlendNode {
    fn display_name(&self) -> String {
        "ALS Mesh Space Layered Blend".into()
    }

    fn duration(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        ctx.set_duration_fwd(ctx.duration_back(Self::IN_TIME_A)?);
        Ok(())
    }

    fn update(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        let update = ctx.time_update_fwd()?;
        ctx.set_time_update_back(Self::IN_TIME_A, update);
        let mut base = ctx.data_back(Self::IN_POSE_A)?.into_pose()?;
        let alpha = ctx.data_back(Self::FACTOR)?.as_f32()?;
        ctx.set_time_update_back(Self::IN_TIME_B, TimeUpdate::Absolute(base.timestamp));
        let overlay = ctx.data_back(Self::IN_POSE_B)?.into_pose()?;
        let bone_mask = ctx.data_back(Self::IN_BONE_MASK)?.into_bone_mask()?;
        let skeletons = &ctx.graph_context.resources.skeleton_assets;
        let base_skeleton = skeletons.get(&base.skeleton);
        let overlay_skeleton = skeletons.get(&overlay.skeleton);
        if base_skeleton.is_none() {
            base.skeleton = overlay.skeleton.clone();
        }
        if let Some(skeleton) = base_skeleton.or(overlay_skeleton) {
            apply_mesh_space_layered_blend(&mut base, &overlay, alpha, &bone_mask, skeleton);
        } else {
            apply_local_layered_blend(&mut base, &overlay, alpha, &bone_mask);
        }
        ctx.set_time(base.timestamp);
        ctx.set_data_fwd(Self::OUT_POSE, base);
        Ok(())
    }

    fn spec(&self, mut ctx: SpecContext) -> Result<(), GraphError> {
        ctx.add_input_data(Self::FACTOR, DataSpec::F32)
            .add_input_data(Self::IN_BONE_MASK, DataSpec::BoneMask)
            .add_input_data(Self::IN_POSE_A, DataSpec::Pose)
            .add_input_time(Self::IN_TIME_A)
            .add_input_data(Self::IN_POSE_B, DataSpec::Pose)
            .add_input_time(Self::IN_TIME_B)
            .add_output_data(Self::OUT_POSE, DataSpec::Pose)
            .add_output_time();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use bevy_animation_graph::core::animation_clip::EntityPath;

    use super::*;

    fn close_rotation(left: Quat, right: Quat) -> bool {
        (1.0 - left.dot(right).abs()).abs() < 1e-5
    }

    fn skeleton() -> (Skeleton, BoneId, BoneId) {
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
    fn layer_mask_interpolates_selected_rotation_in_mesh_space() {
        let (skeleton, root, child) = skeleton();
        let root_rotation = Quat::from_rotation_y(std::f32::consts::FRAC_PI_2);
        let overlay_rotation = Quat::from_rotation_x(std::f32::consts::FRAC_PI_2);
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
        let mut overlay = base.clone();
        let root_index = overlay.paths[&root];
        overlay.bones[root_index].rotation = Some(Quat::IDENTITY);
        let child_index = overlay.paths[&child];
        overlay.bones[child_index].rotation = Some(overlay_rotation);
        let child_path = skeleton.id_to_path(child).unwrap().clone();
        let mut mask = BoneMask::none();
        mask.add_bone_weight(child, child_path, 1.0);

        apply_mesh_space_layered_blend(&mut base, &overlay, 1.0, &mask, &skeleton);

        let output = base.get_bone(child).and_then(|bone| bone.rotation).unwrap();
        let expected = root_rotation.inverse() * overlay_rotation;
        assert!(close_rotation(output, expected));
        assert!(!close_rotation(output, overlay_rotation));
    }

    #[test]
    fn layer_mask_keeps_unselected_local_rotation() {
        let (skeleton, root, child) = skeleton();
        let source = Quat::from_rotation_z(0.4);
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
                rotation: Some(source),
                ..default()
            },
            child,
        );
        let mut overlay = base.clone();
        let child_index = overlay.paths[&child];
        overlay.bones[child_index].rotation = Some(Quat::from_rotation_x(1.0));

        apply_mesh_space_layered_blend(&mut base, &overlay, 1.0, &BoneMask::none(), &skeleton);

        let output = base.get_bone(child).and_then(|bone| bone.rotation).unwrap();
        assert!(close_rotation(output, source));
    }

    #[test]
    fn unselected_child_preserves_mesh_rotation_when_its_parent_is_blended() {
        let (skeleton, root, child) = skeleton();
        for explicit_child in [false, true] {
            for alpha in [0.25, 1.0] {
                let parent_rotation = Quat::from_rotation_y(0.2);
                let child_rotation = if explicit_child { Quat::from_rotation_z(0.4) } else { Quat::IDENTITY };
                let mut base = Pose::default();
                base.add_bone(BonePose { rotation: Some(parent_rotation), ..default() }, root);
                if explicit_child {
                    base.add_bone(BonePose { rotation: Some(child_rotation), ..default() }, child);
                }
                let mut overlay = base.clone();
                let index = overlay.paths[&root];
                overlay.bones[index].rotation = Some(Quat::from_rotation_x(1.1));
                let mut mask = BoneMask::none();
                mask.add_bone_weight(root, skeleton.id_to_path(root).unwrap().clone(), 1.0);
                apply_mesh_space_layered_blend(&mut base, &overlay, alpha, &mask, &skeleton);
                let parent = local_rotation(&base, &skeleton, root);
                let child = local_rotation(&base, &skeleton, child);
                assert!(close_rotation(parent * child, parent_rotation * child_rotation),
                    "unselected mesh rotation: explicit_child={explicit_child} alpha={alpha}");
            }
        }
        let mut empty = Pose::default();
        apply_mesh_space_layered_blend(&mut empty, &Pose::default(), 1.0, &BoneMask::none(), &skeleton);
        assert!(empty.paths.is_empty());
    }

    #[test]
    fn local_fallback_interpolates_selected_rotation() {
        let (_, root, child) = skeleton();
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
                rotation: Some(Quat::IDENTITY),
                ..default()
            },
            child,
        );
        let mut overlay = base.clone();
        let child_index = overlay.paths[&child];
        let target = Quat::from_rotation_y(0.7);
        overlay.bones[child_index].rotation = Some(target);
        let mut mask = BoneMask::none();
        mask.add_bone_weight(child, EntityPath::default().child("child"), 1.0);

        apply_local_layered_blend(&mut base, &overlay, 1.0, &mask);

        let output = base.get_bone(child).and_then(|bone| bone.rotation).unwrap();
        assert!(close_rotation(output, target));
    }
}
