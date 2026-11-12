use std::collections::HashMap;

use bevy::prelude::*;
use bevy_animation_graph::core::{
    animation_graph::TimeUpdate,
    animation_node::{NodeLike, ReflectNodeLike},
    context::{new_context::NodeContext, spec_context::SpecContext},
    edge_data::DataSpec,
    errors::GraphError,
    pose::{BonePose, Pose},
    skeleton::Skeleton,
};
use serde::{Deserialize, Serialize};

use crate::extensions::{RichCurveKey, sample_rich_curve};

const WEIGHT_THRESHOLD: f32 = 1e-5;

pub fn curve_a_input(curve: &str) -> String {
    format!("curve_a_{curve}")
}

pub fn curve_b_input(curve: &str) -> String {
    format!("curve_b_{curve}")
}

#[derive(Reflect, Clone, Debug, Default, PartialEq)]
#[reflect(Default)]
pub struct BoneBlendScale {
    pub bone: String,
    pub scale: f32,
}

#[derive(Reflect, Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[reflect(Default, Serialize)]
pub enum AlphaBlendMode {
    #[default]
    Linear,
    Cubic,
    HermiteCubic,
    QuadraticInOut,
    Custom,
}

#[derive(Reflect, Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[reflect(Default, Serialize)]
pub enum BlendProfileMode {
    TimeFactor,
    #[default]
    WeightFactor,
}

#[derive(Reflect, Clone, Debug, Default, PartialEq)]
#[reflect(Default)]
pub struct SourceBlendProfile {
    pub profile_mode: BlendProfileMode,
    pub bone_scales: Vec<BoneBlendScale>,
}

#[derive(Reflect, Clone, Debug, Default)]
#[reflect(Default, NodeLike)]
#[type_path = "als_ext"]
pub struct ProfiledBlendNode {
    pub curve_names: Vec<String>,
    pub bone_scales: Vec<BoneBlendScale>,
    pub profile_mode: BlendProfileMode,
    pub alpha_blend_mode: AlphaBlendMode,
    pub custom_curve: Vec<RichCurveKey>,
}

impl ProfiledBlendNode {
    pub const FACTOR: &'static str = "factor";
    pub const IN_POSE_A: &'static str = "pose_a";
    pub const IN_TIME_A: &'static str = "time_a";
    pub const IN_POSE_B: &'static str = "pose_b";
    pub const IN_TIME_B: &'static str = "time_b";
    pub const OUT_POSE: &'static str = "pose";
}

fn profile_curve_value(source: f32, target: f32, alpha: f32) -> f32 {
    let source = source * (1.0 - alpha);
    if alpha > crate::extensions::source_slot::MONTAGE_EVALUATION_WEIGHT_THRESHOLD {
        source + target * alpha
    } else {
        source
    }
}

pub fn profile_target_weight(alpha: f32, scale: f32) -> f32 {
    profile_weights(alpha, scale).1
}

fn profile_weights(alpha: f32, scale: f32) -> (f32, f32) {
    let alpha = alpha.clamp(0.0, 1.0);
    let target = (alpha * scale).max(WEIGHT_THRESHOLD);
    let source = if scale > WEIGHT_THRESHOLD { (1.0 - alpha) / scale } else { 1.0 };
    normalize_profile_weights(source.max(WEIGHT_THRESHOLD), target)
}

fn normalize_profile_weights(source: f32, target: f32) -> (f32, f32) {
    let total = target + source;
    if total <= WEIGHT_THRESHOLD {
        (0.5, 0.5)
    } else if (total - 1.0).abs() <= WEIGHT_THRESHOLD {
        (source, target)
    } else {
        (source / total, target / total)
    }
}

pub fn transition_alpha(
    alpha: f32,
    blend_mode: AlphaBlendMode,
    custom_curve: &[RichCurveKey],
) -> f32 {
    let alpha = alpha.clamp(0.0, 1.0);
    let value = match blend_mode {
        AlphaBlendMode::Linear => alpha,
        AlphaBlendMode::Cubic => {
            let squared = alpha * alpha;
            let cubed = squared * alpha;
            -2.0 * cubed + 3.0 * squared
        }
        AlphaBlendMode::HermiteCubic => alpha * alpha * (3.0 - 2.0 * alpha),
        AlphaBlendMode::QuadraticInOut => {
            if alpha < 0.5 {
                2.0 * alpha * alpha
            } else {
                1.0 - (-2.0 * alpha + 2.0).powi(2) * 0.5
            }
        }
        AlphaBlendMode::Custom => match (custom_curve.first(), custom_curve.last()) {
            (Some(first), Some(last)) => {
                sample_rich_curve(custom_curve, first.t + (last.t - first.t) * alpha)
            }
            _ => alpha,
        },
    };
    value.clamp(0.0, 1.0)
}

pub(crate) fn advance_alpha_blend(alpha: &mut f32, remaining: &mut f32, delta: f32) {
    if *remaining > delta {
        let blend_delta = 1.0 - *alpha;
        *alpha += (blend_delta / *remaining) * delta;
        *remaining -= delta;
        *alpha = alpha.clamp(0.0, 1.0);
    } else {
        *remaining = 0.0;
        *alpha = 1.0;
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct MontageBlend {
    pub(crate) duration: f32,
    remaining: f32,
    alpha: f32,
    from: f32,
    to: f32,
    mode: AlphaBlendMode,
}

impl MontageBlend {
    pub(crate) fn new(from: f32, to: f32, duration: f32, mode: AlphaBlendMode) -> Self {
        let duration = duration.max(0.0);
        Self {
            duration,
            remaining: duration,
            alpha: if duration > 0.0 { 0.0 } else { 1.0 },
            from,
            to,
            mode,
        }
    }

    fn value_at(&self, alpha: f32) -> f32 {
        self.from + (self.to - self.from) * transition_alpha(alpha, self.mode, &[])
    }

    pub(crate) fn weight(&self) -> f32 {
        self.value_at(self.alpha)
    }

    pub(crate) fn advance(&mut self, delta: f32) -> f32 {
        let current = self.weight();
        if current == self.value_at(1.0) { return current; }
        if self.remaining > delta {
            self.alpha = (self.alpha + ((1.0 - self.alpha) / self.remaining) * delta).clamp(0.0, 1.0);
            self.remaining -= delta;
        } else {
            self.alpha = 1.0;
            self.remaining = 0.0;
        }
        self.weight()
    }
}

pub(crate) const MONTAGE_SECTION_END_OFFSET: f32 = 0.000_05;

pub(crate) fn advance_forward_montage_position(
    position: &mut f32, delta: f32, play_rate: f32, length: f32, branching_points: &[f32],
    mut after_step: impl FnMut(f32),
) -> bool {
    let mut time_remaining = delta;
    for _ in 0..9 {
        if time_remaining <= 1e-8 || play_rate <= 1e-8 { break; }
        let mut movement = time_remaining * play_rate;
        let end = *position + movement;
        if let Some(marker) = branching_points.iter().copied().find(|marker| *position < *marker && *marker <= end) {
            movement = marker - *position;
        }
        let to_end = length - *position;
        if movement >= to_end {
            *position += to_end;
            after_step(*position);
            return true;
        }
        if movement <= 0.0 { break; }
        *position += movement;
        time_remaining = (time_remaining - movement / play_rate).max(0.0);
        after_step(*position);
    }
    false
}

impl ProfiledBlendNode {
    fn bone_weights(&self, factor: f32, alpha: f32, scale: f32) -> (f32, f32) {
        if self.profile_mode == BlendProfileMode::WeightFactor {
            return profile_weights(alpha, scale);
        }
        if scale >= 1.0 - WEIGHT_THRESHOLD {
            let target = normalize_profile_weights(alpha, alpha).1;
            return (1.0 - target, target);
        }
        let scale = scale.clamp(0.0, 1.0);
        let begin = if scale > WEIGHT_THRESHOLD { 0.0 } else { 1.0 };
        let current = if scale > WEIGHT_THRESHOLD { (factor / scale).clamp(0.0, 1.0) } else { 1.0 };
        let begin = transition_alpha(begin, self.alpha_blend_mode, &self.custom_curve);
        let current = transition_alpha(current, self.alpha_blend_mode, &self.custom_curve);
        let target = (begin + current * (1.0 - begin)).clamp(WEIGHT_THRESHOLD, 1.0);
        (1.0 - target, target)
    }

    fn blend_pose(
        &self,
        source: &mut Pose,
        target: &Pose,
        factor: f32,
        skeleton: Option<&Skeleton>,
        accumulate_rotations: bool,
    ) -> f32 {
        let alpha = transition_alpha(factor, self.alpha_blend_mode, &self.custom_curve);
        let scales: HashMap<_, _> = self
            .bone_scales
            .iter()
            .map(|scale| (scale.bone.as_str(), scale.scale))
            .collect();
        for (bone, target_index) in &target.paths {
            let scale = skeleton
                .filter(|_| !scales.is_empty())
                .and_then(|skeleton| skeleton.id_to_path(*bone))
                .and_then(|path| {
                    path.parts
                        .last()
                        .and_then(|name| scales.get(name.as_str()).copied())
                });
            let (source_weight, target_weight) =
                scale.map_or((1.0 - alpha, alpha), |scale| self.bone_weights(factor, alpha, scale));
            if let Some(source_index) = source.paths.get(bone) {
                let source = &mut source.bones[*source_index];
                let target = &target.bones[*target_index];
                let weighted_vector = |a: Option<Vec3>, b: Option<Vec3>| match (a, b) {
                    (Some(a), Some(b)) => Some(a * source_weight + b * target_weight),
                    (a, b) => a.or(b),
                };
                source.translation = weighted_vector(source.translation, target.translation);
                source.scale = weighted_vector(source.scale, target.scale);
                source.rotation = if let (Some(a), Some(b)) = (source.rotation, target.rotation) {
                    let a = a * source_weight;
                    let b = b * target_weight;
                    let rotation = if a.dot(b) < 0.0 { a - b } else { a + b };
                    Some(if accumulate_rotations { rotation } else {
                        crate::extensions::mesh_space_additive::normalize_pose_rotation(rotation)
                    })
                } else {
                    source.rotation.or(target.rotation)
                };
                if source.weights.is_some() || target.weights.is_some() {
                    let mut weights = BonePose { weights: source.weights.take(), ..default() };
                    let target = BonePose { weights: target.weights.clone(), ..default() };
                    weights.linear_blend_mut(&target, target_weight);
                    source.weights = weights.weights;
                }
            } else {
                source.add_bone(target.bones[*target_index].clone(), *bone);
            }
        }
        source.root_motion = match (&source.root_motion, &target.root_motion) {
            (Some(a), Some(b)) => Some(a.linear_blend(b, alpha)),
            (Some(a), None) => Some(a.scale(1.0 - alpha)),
            (None, Some(b)) => Some(b.scale(alpha)),
            (None, None) => None,
        };
        alpha
    }
}

impl NodeLike for ProfiledBlendNode {
    fn display_name(&self) -> String {
        "ALS Profiled Blend".into()
    }

    fn duration(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        ctx.set_duration_fwd(ctx.duration_back(Self::IN_TIME_A)?);
        Ok(())
    }

    fn update(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        let update = ctx.time_update_fwd()?;
        ctx.set_time_update_back(Self::IN_TIME_A, update.clone());
        let mut source = ctx.data_back(Self::IN_POSE_A)?.into_pose()?;
        ctx.set_time_update_back(Self::IN_TIME_B, TimeUpdate::Absolute(source.timestamp));
        let target = ctx.data_back(Self::IN_POSE_B)?.into_pose()?;
        let source_context =
            crate::extensions::animation_context::pose_input_context_state(&ctx, Self::IN_POSE_A)
                .unwrap_or_default();
        let target_context =
            crate::extensions::animation_context::pose_input_context_state(&ctx, Self::IN_POSE_B)
                .unwrap_or_default();
        let factor = ctx.data_back(Self::FACTOR)?.as_f32()?;
        let skeleton = ctx
            .graph_context
            .resources
            .skeleton_assets
            .get(&source.skeleton);
        let alpha = self.blend_pose(&mut source, &target, factor, skeleton,
            crate::extensions::interruptible_fsm::in_interruptible_transition_graph(&ctx));
        let mut context = crate::extensions::source_pose_nodes::blend_contexts_with_curve_accumulation([
            (source_context, 1.0 - alpha),
            (target_context, alpha),
        ]);
        for curve in &self.curve_names {
            let source_value = ctx.data_back(curve_a_input(curve))?.as_f32()?;
            let target_value = ctx.data_back(curve_b_input(curve))?.as_f32()?;
            let value = profile_curve_value(source_value, target_value, alpha);
            context.curves.insert(curve.clone().into(), value);
            ctx.set_data_fwd(curve.clone(), value);
        }
        ctx.set_time(source.timestamp);
        ctx.set_data_fwd(Self::OUT_POSE, source);
        *ctx.state_mut_or_else(crate::extensions::AnimationContextState::default)? = context;
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
        for curve in &self.curve_names {
            ctx.add_input_data(curve_a_input(curve), DataSpec::F32)
                .add_input_data(curve_b_input(curve), DataSpec::F32)
                .add_output_data(curve.clone(), DataSpec::F32);
        }
        Ok(())
    }
}

#[cfg(test)]
#[derive(serde::Deserialize)]
pub(crate) struct NativeWeightedRotationCase {
    pub stance: String,
    pub bone: String,
    pub base: [f32; 4],
    pub target: [f32; 4],
    pub weights: [f32; 2],
    pub raw_rotation: [f32; 4],
    pub rotation: [f32; 4],
}

#[cfg(test)]
pub(crate) fn native_weighted_rotation_cases() -> Vec<NativeWeightedRotationCase> {
    #[derive(serde::Deserialize)]
    struct Fixture {
        cases: Vec<NativeWeightedRotationCase>,
    }
    let fixture: Fixture = serde_json::from_str(include_str!(
        "../../docs/audit/bf144/d177/weighted-rotations.replay.json"
    )).unwrap();
    fixture.cases
}

#[cfg(test)]
mod tests {
    use bevy_animation_graph::core::{animation_clip::EntityPath, pose::BonePose};

    use super::*;

    fn skeleton() -> (Skeleton, bevy_animation_graph::core::pose::BoneId) {
        let root_path = EntityPath::default().child("root");
        let foot_path = root_path.child("foot_l");
        let root = root_path.id();
        let foot = foot_path.id();
        let mut skeleton = Skeleton::default();
        skeleton.set_root(root);
        skeleton.add_bone(root_path, Transform::IDENTITY, Transform::IDENTITY);
        skeleton.add_bone(foot_path, Transform::IDENTITY, Transform::IDENTITY);
        (skeleton, foot)
    }

    #[test]
    fn profile_modes_preserve_source_per_bone_and_global_weights() {
        use bevy_animation_graph::core::{
            animation_clip::GraphClip, animation_graph::{AnimationGraph, GraphInputPin}, animation_node::AnimationNode,
            context::{deferred_gizmos::DeferredGizmos, graph_context::QueryOutputTime,
                graph_context_arena::GraphContextArena, new_context::GraphContext, system_resources::SystemResources},
            pose::RootMotionDelta, state_machine::high_level::StateMachine, utils::testing::TestIoEnv,
        };
        fn query(In(handle): In<Handle<AnimationGraph>>, resources: SystemResources) -> (Pose, f32) {
            let graph = resources.animation_graph_assets.get(&handle).unwrap();
            let mut arena = GraphContextArena::new(handle.id());
            let mut gizmos = DeferredGizmos::default();
            let env = TestIoEnv::default();
            let entities = default();
            let inputs = default();
            let context = GraphContext::new(arena.get_toplevel_id(), graph, &mut arena, &resources,
                &env, Entity::PLACEHOLDER, &entities, &mut gizmos, &inputs);
            let output = graph.query_with_context(QueryOutputTime::Forced(TimeUpdate::Delta(0.0)), context).unwrap();
            (output["pose"].as_pose().unwrap().clone(), output["fixture"].as_f32().unwrap())
        }
        let mut world = World::new();
        world.init_resource::<Assets<AnimationGraph>>();
        world.init_resource::<Assets<GraphClip>>();
        world.init_resource::<Assets<Skeleton>>();
        world.init_resource::<Assets<StateMachine>>();
        let (mut skeleton, foot) = skeleton();
        let root = skeleton.root();
        let extra_path = EntityPath::default().child("root").child("target_only");
        let extra = extra_path.id();
        skeleton.add_bone(extra_path, Transform::IDENTITY, Transform::IDENTITY);
        let skeleton = world.resource_mut::<Assets<Skeleton>>().add(skeleton);
        use AlphaBlendMode::{Cubic, Custom, Linear};
        use BlendProfileMode::{TimeFactor, WeightFactor};
        for (profile_mode, scale, factor, mode, expected, global) in [
            (TimeFactor, 0.5, 0.25, Linear, 0.5, 0.25),
            (TimeFactor, 0.0, 0.0, Linear, 1.0, 0.0),
            (TimeFactor, 0.5, 0.0, Linear, 1e-5, 0.0),
            (TimeFactor, 1.0, 0.25, Linear, 0.5, 0.25),
            (TimeFactor, 2.0, 0.25, Linear, 0.5, 0.25),
            (TimeFactor, 1.0, 0.0, Linear, 0.5, 0.0),
            (TimeFactor, 0.5, 0.25, Cubic, 0.5, 0.15625),
            (TimeFactor, 0.5, 0.25, Custom, 0.6, 0.35),
            (TimeFactor, 0.0, 0.25, Custom, 0.96, 0.35),
            (WeightFactor, 0.0, 0.25, Linear, 1e-5 / 1.00001, 0.25),
            (WeightFactor, 1.0, 0.25, Linear, 0.25, 0.25),
            (WeightFactor, 2.0, 0.25, Linear, 4.0 / 7.0, 0.25),
            (WeightFactor, 0.5, 0.25, Linear, 1.0 / 13.0, 0.25),
        ] {
            let custom_curve = if mode == Custom { vec![
                RichCurveKey { t: 0.0, v: 0.2, interpolation: crate::extensions::RichCurveInterpolation::Linear, ..default() },
                RichCurveKey { t: 1.0, v: 0.8, interpolation: crate::extensions::RichCurveInterpolation::Linear, ..default() },
            ] } else { vec![] };
            let node = AnimationNode::new("profile", ProfiledBlendNode { profile_mode,
                bone_scales: vec![BoneBlendScale { bone: "foot_l".into(), scale }],
                alpha_blend_mode: mode, custom_curve, curve_names: vec!["fixture".into()] });
            let mut source = Pose { skeleton: skeleton.clone(), timestamp: 0.2,
                root_motion: Some(RootMotionDelta { translation: Vec3::Y, ..default() }), ..default() };
            source.add_bone(BonePose { translation: Some(Vec3::ZERO), ..default() }, foot);
            source.add_bone(BonePose { translation: Some(Vec3::Y), ..default() }, root);
            let mut target = source.clone();
            target.bones[target.paths[&foot]].translation = Some(Vec3::X);
            target.bones[target.paths[&root]].translation = Some(Vec3::Y * 3.0);
            let rotation = Quat::from_rotation_z(0.4);
            target.bones[target.paths[&root]].rotation = Some(rotation);
            target.add_bone(BonePose { translation: Some(Vec3::Z), ..default() }, extra);
            target.root_motion.as_mut().unwrap().translation = Vec3::Y * 3.0;
            let mut graph = AnimationGraph::new();
            for (pin, value) in [("pose_a", source.into()), ("pose_b", target.into()),
                ("factor", factor.into()), ("curve_a_fixture", 2.0_f32.into()), ("curve_b_fixture", 6.0_f32.into())] {
                graph.set_default_data(GraphInputPin::passthrough(pin), value);
                graph.add_input_data_edge(GraphInputPin::passthrough(pin), node.id, pin);
            }
            for pin in ["time_a", "time_b"] { graph.add_input_time_edge(GraphInputPin::passthrough(pin), node.id, pin); }
            for (pin, spec) in [("pose", DataSpec::Pose), ("fixture", DataSpec::F32)] {
                graph.add_output_data(pin.into(), spec);
                graph.add_output_data_edge(node.id, pin, pin);
            }
            graph.add_output_time();
            graph.add_output_pose_edge(node.id);
            graph.add_node(node);
            let handle = world.resource_mut::<Assets<AnimationGraph>>().add(graph);
            let (output, curve) = world.run_system_cached_with(query, handle).unwrap();
            let actual = output.get_bone(foot).unwrap().translation.unwrap().x;
            assert!((actual - expected).abs() < 1e-6, "{profile_mode:?}, {mode:?}, scale={scale}, factor={factor}: {actual} != {expected}");
            assert!((output.get_bone(root).unwrap().translation.unwrap().y - (1.0 + 2.0 * global)).abs() < 1e-6);
            assert_eq!(output.get_bone(root).unwrap().rotation, Some(rotation));
            assert_eq!(output.get_bone(extra).unwrap().translation, Some(Vec3::Z));
            assert!((output.root_motion.unwrap().translation.y - (1.0 + 2.0 * global)).abs() < 1e-6);
            assert!((curve - (2.0 + 4.0 * global)).abs() < 1e-6);
            assert_eq!(output.timestamp, 0.2);
        }
    }

    #[test]
    fn weight_factor_profile_accelerates_listed_bones() {
        let global = profile_target_weight(0.1, 1.0);
        let quick_feet = profile_target_weight(0.1, 5.0);
        assert!((global - 0.1).abs() < 1e-6);
        assert!((quick_feet - 0.7352941).abs() < 1e-6);
    }

    #[test]
    fn alpha_blend_options_match_unreal_curve_mapping() {
        assert_eq!(transition_alpha(0.9997889995574951, AlphaBlendMode::Cubic, &[]), 1.0);
        assert_eq!(transition_alpha(0.9997889995574951, AlphaBlendMode::HermiteCubic, &[]), 1.0 - f32::EPSILON);
        assert_eq!(transition_alpha(0.9997720122337341, AlphaBlendMode::Cubic, &[]), 1.0 - 2.0 * f32::EPSILON);
        assert!((transition_alpha(0.25, AlphaBlendMode::Cubic, &[]) - 0.15625).abs() < 1e-6);
        assert!((transition_alpha(0.25, AlphaBlendMode::QuadraticInOut, &[]) - 0.125).abs() < 1e-6);
        assert!((transition_alpha(0.75, AlphaBlendMode::QuadraticInOut, &[]) - 0.875).abs() < 1e-6);
        let aim_in = [
            RichCurveKey {
                interpolation: crate::extensions::RichCurveInterpolation::Cubic,
                tangent_mode: crate::extensions::RichCurveTangentMode::User,
                t: 0.2,
                v: 0.25,
                arrive_tangent: 2.5235951,
                arrive_weight: 0.9048326,
                leave_tangent: 2.5235951,
                leave_weight: 0.9048368,
                ..default()
            },
            RichCurveKey {
                t: 1.0,
                v: 1.0,
                ..default()
            },
        ];
        assert!((transition_alpha(0.5, AlphaBlendMode::Custom, &aim_in) - 0.8773595).abs() < 1e-5);
    }

    #[test]
    fn single_key_custom_curve_holds_its_value() {
        for value in [-0.5_f32, 0.7, 1.5] {
            let keys = [RichCurveKey { t: 0.4, v: value, ..default() }];
            for time in [-10.0, 0.0, 0.4, 1.0, 10.0] {
                assert_eq!(sample_rich_curve(&keys, time), value);
                assert_eq!(transition_alpha(time, AlphaBlendMode::Custom, &keys), value.clamp(0.0, 1.0));
            }
        }
        assert_eq!(transition_alpha(0.4, AlphaBlendMode::Custom, &[]), 0.4);
    }

    #[test]
    fn custom_stance_curve_matches_the_source_midpoint() {
        let curve = [
            RichCurveKey {
                interpolation: crate::extensions::RichCurveInterpolation::Cubic,
                t: 0.0,
                v: 0.0,
                ..default()
            },
            RichCurveKey {
                interpolation: crate::extensions::RichCurveInterpolation::Cubic,
                tangent_mode: crate::extensions::RichCurveTangentMode::User,
                t: 0.4,
                v: 0.4,
                arrive_tangent: 2.730308,
                leave_tangent: 2.730308,
                ..default()
            },
            RichCurveKey {
                t: 1.0,
                v: 1.0,
                ..default()
            },
        ];
        assert!((transition_alpha(0.5, AlphaBlendMode::Custom, &curve) - 0.63404917).abs() < 1e-6);
    }

    #[test]
    fn profiled_blend_applies_normalized_target_weight_to_listed_bone() {
        let (skeleton, foot) = skeleton();
        let mut source = Pose::default();
        source.add_bone(
            BonePose {
                translation: Some(Vec3::ZERO),
                ..default()
            },
            foot,
        );
        let mut target = Pose::default();
        target.add_bone(
            BonePose {
                translation: Some(Vec3::X),
                ..default()
            },
            foot,
        );
        ProfiledBlendNode {
            bone_scales: vec![BoneBlendScale {
                bone: "foot_l".to_owned(),
                scale: 5.0,
            }],
            ..default()
        }.blend_pose(&mut source, &target, 0.1, Some(&skeleton), false);
        let value = source.get_bone(foot).unwrap().translation.unwrap().x;
        assert!((value - 0.7352941).abs() < 1e-6);
    }

    #[test]
    fn profiled_curves_keep_the_first_layer_and_gate_later_accumulation() {
        use crate::extensions::{AnimationContextState, SourceCurveName, source_pose_nodes::blend_contexts_with_curve_accumulation};

        let name = SourceCurveName::from("ProbeCurve");
        let context = |value| {
            let mut context = AnimationContextState::default();
            context.curves.insert(name.clone(), value);
            context
        };
        for (alpha, target_is_relevant) in [(1e-6_f32, false), (1e-5, false), (1.1e-5, true), (0.999995, true)] {
            let mut expected = -(1.0 - alpha);
            if target_is_relevant { expected += 90.0 * alpha; }
            assert_eq!(profile_curve_value(-1.0, 90.0, alpha), expected);
            let blended = blend_contexts_with_curve_accumulation([(context(-1.0), 1.0 - alpha), (context(90.0), alpha)]);
            assert_eq!(blended.curves[&name], expected);
        }
    }

    #[test]
    fn profiled_rotation_matches_native_accumulation_before_and_after_normalization() {
        use bevy_animation_graph::core::pose::{BoneId, BonePose};

        let bone = BoneId::default();
        let pose = |rotation| {
            let mut pose = Pose::default();
            pose.add_bone(BonePose { rotation: Some(Quat::from_array(rotation)), ..default() }, bone);
            pose
        };
        for case in native_weighted_rotation_cases() {
            for (accumulate, expected) in [(true, case.raw_rotation), (false, case.rotation)] {
                let mut base = pose(case.base);
                let target = pose(case.target);
                ProfiledBlendNode::default().blend_pose(&mut base, &target, case.weights[1], None, accumulate);
                let actual = base.get_bone(bone).unwrap().rotation.unwrap();
                assert!((actual - Quat::from_array(expected)).length() < 3e-7,
                    "{} {} accumulate={accumulate}: {actual:?}", case.stance, case.bone);
            }
        }
    }

    #[test]
    fn source_profile_keeps_both_native_bone_weights() {
        use serde_json::Value;

        let reference: Value = serde_json::from_str(include_str!(
            "../../docs/audit/bf144/d197/weights.replay.json"
        )).unwrap();
        let (skeleton, foot) = skeleton();
        let vector = |value: &Value| Vec3::from_array(std::array::from_fn(|index| value[index].as_f64().unwrap() as f32));
        let quaternion = |value: &Value| Quat::from_array(std::array::from_fn(|index| value[index].as_f64().unwrap() as f32));
        let transform = |value: &Value| Transform {
            translation: vector(&value["translation"]),
            rotation: quaternion(&value["rotation"]),
            scale: vector(&value["scale"]),
        };
        for case in reference["cases"].as_array().unwrap() {
            let scale = case["scale"].as_f64().unwrap() as f32;
            let factor = case["factor"].as_f64().unwrap() as f32;
            let node = ProfiledBlendNode {
                profile_mode: BlendProfileMode::WeightFactor,
                bone_scales: vec![BoneBlendScale { bone: "foot_l".into(), scale }],
                ..default()
            };
            let bone = if case["bone"] == "default" { skeleton.root() } else { foot };
            if bone == foot {
                assert_eq!(profile_weights(factor, scale), (
                    case["weights"][0].as_f64().unwrap() as f32,
                    case["weights"][1].as_f64().unwrap() as f32,
                ));
            }
            for accumulate in [false, true] {
                let mut source = Pose::default();
                source.add_bone(BonePose::from_transform(transform(&case["source"])), bone);
                let mut target = Pose::default();
                target.add_bone(BonePose::from_transform(transform(&case["target"])), bone);
                node.blend_pose(&mut source, &target, factor, Some(&skeleton), accumulate);
                let expected = transform(&case["expected"]);
                let actual = source.get_bone(bone).unwrap();
                assert!(actual.translation.unwrap().abs_diff_eq(expected.translation, 3e-8), "{}", case["name"]);
                assert!(actual.scale.unwrap().abs_diff_eq(expected.scale, 2.5e-7), "{}", case["name"]);
                let actual = actual.rotation.unwrap();
                let expected = if accumulate { quaternion(&case["raw_rotation"]) } else { expected.rotation };
                assert!((actual - expected).length().min((actual + expected).length()) < 3e-7, "{}", case["name"]);
            }
        }
    }

    #[test]
    fn profile_blend_keeps_sparse_channels_and_morph_weight_interpolation() {
        let (skeleton, foot) = skeleton();
        let node = ProfiledBlendNode {
            profile_mode: BlendProfileMode::WeightFactor,
            bone_scales: vec![BoneBlendScale { bone: "foot_l".into(), scale: 2.0 }],
            ..default()
        };
        let mut source = Pose::default();
        source.add_bone(BonePose { translation: Some(Vec3::X), weights: Some(vec![0.3]), ..default() }, foot);
        let mut target = Pose::default();
        target.add_bone(BonePose { scale: Some(Vec3::Y), weights: Some(vec![0.8, 0.5]), ..default() }, foot);
        let factor = 0.33333;
        node.blend_pose(&mut source, &target, factor, Some(&skeleton), true);
        let actual = source.get_bone(foot).unwrap();
        assert_eq!(actual.translation, Some(Vec3::X));
        assert_eq!(actual.scale, Some(Vec3::Y));
        let target_weight = profile_target_weight(factor, 2.0);
        assert_eq!(actual.weights.as_deref(), Some([0.3 + (0.8 - 0.3) * target_weight, 0.5 * target_weight].as_slice()));
    }

    #[test]
    fn node_registers_node_like_dispatch_key() {
        use bevy::reflect::TypeRegistry;

        let mut registry = TypeRegistry::default();
        registry.register::<ProfiledBlendNode>();
        let registration = registry
            .get(std::any::TypeId::of::<ProfiledBlendNode>())
            .unwrap();
        assert!(
            registration
                .data::<bevy_animation_graph::core::animation_node::ReflectNodeLike>()
                .is_some()
        );
    }
}
