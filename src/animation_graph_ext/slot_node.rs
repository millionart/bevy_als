use bevy::prelude::*;
use bevy_animation_graph::core::{
    animation_node::{NodeLike, ReflectNodeLike},
    context::{new_context::NodeContext, spec_context::SpecContext},
    edge_data::{DataSpec, bone_mask::BoneMask},
    errors::GraphError,
    interpolation::linear::LinearInterpolator,
    pose::Pose,
};

pub(crate) fn slot_blend_weight(value: Option<f32>) -> f32 {
    value
        .filter(|value| value.is_finite())
        .unwrap_or_default()
        .clamp(0.0, 1.0)
}

pub fn blend_slot_pose(mut base: Pose, overlay: Option<&Pose>, weight: Option<f32>) -> Pose {
    let timestamp = base.timestamp;
    let weight = slot_blend_weight(weight);
    if weight > 0.0 {
        if let Some(overlay) = overlay {
            LinearInterpolator {
                bone_mask: BoneMask::all(),
            }
            .interpolate_pose(&mut base, overlay, weight);
        }
    }
    base.timestamp = timestamp;
    base
}

#[derive(Reflect, Clone, Debug)]
#[reflect(Default, NodeLike)]
#[type_path = "als_ext"]
pub struct SlotNode {
    pub slot_name: String,
    pub pose_key: String,
    pub weight_key: String,
    pub layer_weight_key: String,
}

impl Default for SlotNode {
    fn default() -> Self {
        Self {
            slot_name: "DefaultSlot".to_owned(),
            pose_key: "slot_pose".to_owned(),
            weight_key: "slot_weight".to_owned(),
            layer_weight_key: "layer_slot_weight".to_owned(),
        }
    }
}

impl SlotNode {
    pub const IN_BASE_POSE: &'static str = "base_pose";
    pub const IN_BASE_TIME: &'static str = "base_time";
    pub const OUT_POSE: &'static str = "pose";
}

impl NodeLike for SlotNode {
    fn display_name(&self) -> String {
        format!("ALS Slot {}", self.slot_name)
    }

    fn duration(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        ctx.set_duration_fwd(ctx.duration_back(Self::IN_BASE_TIME)?);
        Ok(())
    }

    fn update(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        let time_update = ctx.time_update_fwd()?;
        ctx.set_time_update_back(Self::IN_BASE_TIME, time_update);
        let base = ctx.data_back(Self::IN_BASE_POSE)?.into_pose()?;
        let slot_weight = ctx
            .graph_context
            .global_input_data
            .get(&self.weight_key)
            .map(|value| value.as_f32())
            .transpose()?;
        let slot_weight = slot_blend_weight(slot_weight);
        let layer_weight = ctx
            .graph_context
            .global_input_data
            .get(&self.layer_weight_key)
            .map(|value| value.as_f32())
            .transpose()?;
        let layer_weight = slot_blend_weight(layer_weight);
        let overlay = if slot_weight > 0.0 && layer_weight > 0.0 {
            ctx.graph_context
                .global_input_data
                .get(&self.pose_key)
                .cloned()
                .map(|value| value.into_pose())
                .transpose()?
        } else {
            None
        };
        let slot_pose = blend_slot_pose(base.clone(), overlay.as_ref(), Some(slot_weight));
        let pose = blend_slot_pose(
            base,
            overlay.as_ref().map(|_| &slot_pose),
            Some(layer_weight),
        );
        ctx.set_time(pose.timestamp);
        ctx.set_data_fwd(Self::OUT_POSE, pose);
        Ok(())
    }

    fn spec(&self, mut ctx: SpecContext) -> Result<(), GraphError> {
        ctx.add_input_data(Self::IN_BASE_POSE, DataSpec::Pose)
            .add_input_time(Self::IN_BASE_TIME)
            .add_output_data(Self::OUT_POSE, DataSpec::Pose)
            .add_output_time();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use bevy_animation_graph::core::{
        animation_clip::EntityPath,
        pose::{BoneId, BonePose, Pose},
    };

    use super::*;

    fn pose(translation: Vec3, timestamp: f32) -> (Pose, BoneId) {
        let path = EntityPath::default().child("root");
        let bone = path.id();
        let mut pose = Pose {
            timestamp,
            ..default()
        };
        pose.add_bone(
            BonePose {
                translation: Some(translation),
                ..default()
            },
            bone,
        );
        (pose, bone)
    }

    #[test]
    fn slot_weight_uses_zero_for_absent_or_non_finite_values() {
        assert_eq!(slot_blend_weight(None), 0.0);
        assert_eq!(slot_blend_weight(Some(f32::NAN)), 0.0);
        assert_eq!(slot_blend_weight(Some(-2.0)), 0.0);
        assert_eq!(slot_blend_weight(Some(2.0)), 1.0);
    }

    #[test]
    fn absent_overlay_and_zero_weight_keep_the_base_pose_exactly() {
        let (base, _) = pose(Vec3::new(1.0, 2.0, 3.0), 2.5);
        let expected = base.clone();
        assert_eq!(blend_slot_pose(base.clone(), None, Some(1.0)), expected);
        let (overlay, _) = pose(Vec3::new(8.0, 9.0, 10.0), 7.0);
        assert_eq!(blend_slot_pose(base, Some(&overlay), Some(0.0)), expected);
    }

    #[test]
    fn slot_pose_blends_linearly_and_preserves_base_time() {
        let (base, bone) = pose(Vec3::ZERO, 2.5);
        let (overlay, _) = pose(Vec3::new(4.0, 2.0, 0.0), 7.0);
        let output = blend_slot_pose(base, Some(&overlay), Some(0.5));
        assert_eq!(output.timestamp, 2.5);
        assert_eq!(
            output.get_bone(bone).and_then(|bone| bone.translation),
            Some(Vec3::new(2.0, 1.0, 0.0))
        );
    }

    #[test]
    fn slot_layer_blend_preserves_the_source_two_stage_order() {
        let (base, bone) = pose(Vec3::ZERO, 2.5);
        let (overlay, _) = pose(Vec3::new(8.0, 0.0, 0.0), 7.0);
        let slot_pose = blend_slot_pose(base.clone(), Some(&overlay), Some(0.5));
        let output = blend_slot_pose(base, Some(&slot_pose), Some(0.25));
        assert_eq!(
            output.get_bone(bone).and_then(|bone| bone.translation),
            Some(Vec3::new(1.0, 0.0, 0.0))
        );
    }

    #[test]
    fn node_registers_node_like_dispatch_key() {
        use bevy::reflect::TypeRegistry;

        let mut registry = TypeRegistry::default();
        registry.register::<SlotNode>();
        assert!(
            registry
                .get(std::any::TypeId::of::<SlotNode>())
                .unwrap()
                .data::<bevy_animation_graph::core::animation_node::ReflectNodeLike>()
                .is_some()
        );
        assert_eq!(
            <SlotNode as bevy::reflect::TypePath>::type_path(),
            "als_ext::SlotNode"
        );
    }
}
