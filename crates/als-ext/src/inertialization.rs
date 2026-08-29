use bevy::prelude::*;
use bevy_animation_graph::core::{
    animation_graph::TimeUpdate,
    animation_node::{NodeLike, ReflectNodeLike},
    context::{new_context::NodeContext, spec_context::SpecContext},
    edge_data::DataSpec,
    errors::GraphError,
    pose::Pose,
};

pub fn inertial_alpha(factor: f32, transition_duration: f32, half_life: f32) -> f32 {
    1.0 - inertial_residual_weight(factor, transition_duration, half_life)
}

pub fn inertial_residual_weight(factor: f32, transition_duration: f32, half_life: f32) -> f32 {
    let factor = factor.clamp(0.0, 1.0);
    if factor >= 1.0 || transition_duration <= f32::EPSILON {
        return 0.0;
    }
    let elapsed = factor.clamp(0.0, 1.0) * transition_duration.max(0.0);
    if half_life <= f32::EPSILON {
        return 1.0 - factor;
    }
    let terminal = 2.0_f32.powf(-transition_duration / half_life);
    let current = 2.0_f32.powf(-elapsed / half_life);
    ((current - terminal) / (1.0 - terminal).max(f32::EPSILON)).clamp(0.0, 1.0)
}

pub fn capture_inertial_residual(source: &Pose, target: &Pose) -> Pose {
    target.difference(source)
}

pub fn apply_inertial_residual(target: &Pose, residual: &Pose, weight: f32) -> Pose {
    if weight <= f32::EPSILON {
        return target.clone();
    }
    target
        .additive_blend(residual, weight.clamp(0.0, 1.0))
        .normalize_quat()
}

#[derive(Reflect, Clone, Debug)]
struct InertializationState {
    initialized: bool,
    previous_factor: f32,
    residual: Pose,
}

impl Default for InertializationState {
    fn default() -> Self {
        Self {
            initialized: false,
            previous_factor: 0.0,
            residual: Pose::default(),
        }
    }
}

#[derive(Reflect, Clone, Debug)]
#[reflect(Default, NodeLike)]
#[type_path = "als_ext"]
pub struct InertializationNode {
    pub transition_duration: f32,
    pub half_life: f32,
}

impl Default for InertializationNode {
    fn default() -> Self {
        Self {
            transition_duration: 0.2,
            half_life: 0.05,
        }
    }
}

impl InertializationNode {
    pub const FACTOR: &'static str = "factor";
    pub const IN_POSE_A: &'static str = "pose_a";
    pub const IN_TIME_A: &'static str = "time_a";
    pub const IN_POSE_B: &'static str = "pose_b";
    pub const IN_TIME_B: &'static str = "time_b";
    pub const OUT_POSE: &'static str = "pose";
}

impl NodeLike for InertializationNode {
    fn display_name(&self) -> String {
        "ALS Inertialization".into()
    }

    fn duration(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        ctx.set_duration_fwd(ctx.duration_back(Self::IN_TIME_A)?);
        Ok(())
    }

    fn update(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        let update = ctx.time_update_fwd()?;
        ctx.set_time_update_back(Self::IN_TIME_A, update.clone());
        let source = ctx.data_back(Self::IN_POSE_A)?.into_pose()?;
        ctx.set_time_update_back(Self::IN_TIME_B, TimeUpdate::Absolute(source.timestamp));
        let target = ctx.data_back(Self::IN_POSE_B)?.into_pose()?;
        let factor = ctx.data_back(Self::FACTOR)?.as_f32()?.clamp(0.0, 1.0);
        let residual = {
            let state = ctx.state_mut::<InertializationState>()?;
            if !state.initialized || factor + 1e-4 < state.previous_factor {
                state.residual = capture_inertial_residual(&source, &target);
                state.initialized = true;
            }
            state.previous_factor = factor;
            state.residual.clone()
        };
        let mut pose = apply_inertial_residual(
            &target,
            &residual,
            inertial_residual_weight(factor, self.transition_duration, self.half_life),
        );
        pose.timestamp = target.timestamp;
        ctx.set_time(pose.timestamp);
        ctx.set_data_fwd(Self::OUT_POSE, pose);
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn residual_weight_reaches_zero_at_transition_end() {
        assert!((inertial_residual_weight(0.0, 0.1, 0.05) - 1.0).abs() < 1e-6);
        assert!((inertial_residual_weight(1.0, 0.1, 0.05)).abs() < 1e-6);
        assert!((inertial_alpha(1.0, 0.1, 0.05) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn captured_residual_starts_at_source_and_ends_at_target() {
        let bone = bevy_animation_graph::core::pose::BoneId::default();
        let mut source = Pose::default();
        source.add_bone(
            bevy_animation_graph::core::pose::BonePose::from_transform(Transform::IDENTITY),
            bone,
        );
        let mut target = Pose::default();
        target.add_bone(
            bevy_animation_graph::core::pose::BonePose::from_transform(Transform::from_rotation(
                Quat::from_rotation_y(std::f32::consts::FRAC_PI_2),
            )),
            bone,
        );
        let residual = capture_inertial_residual(&source, &target);
        let start = apply_inertial_residual(&target, &residual, 1.0);
        let end = apply_inertial_residual(&target, &residual, 0.0);
        let source_rotation = source.bones[0].rotation.unwrap();
        let target_rotation = target.bones[0].rotation.unwrap();
        assert!(start.bones[0].rotation.unwrap().dot(source_rotation).abs() > 0.99999);
        assert!(end.bones[0].rotation.unwrap().dot(target_rotation).abs() > 0.99999);
    }

    #[test]
    fn node_registers_node_like_dispatch_key() {
        use bevy::reflect::TypeRegistry;

        let mut registry = TypeRegistry::default();
        registry.register::<InertializationNode>();
        assert!(
            registry
                .get(std::any::TypeId::of::<InertializationNode>())
                .unwrap()
                .data::<bevy_animation_graph::core::animation_node::ReflectNodeLike>()
                .is_some()
        );
    }
}
