use crate::animation_context::{
    AnimationContextState, merge_asset_samples, merge_sync_players, pose_input_context_state,
};
use crate::gameplay_tag_blend::{GameplayTagBlendMode, advance_gameplay_tag_weights};
use crate::source_pose_nodes::{blend_animation_contexts, blend_source_poses, read_blend_list_child};
use bevy::prelude::*;
use bevy_animation_graph::core::{
    animation_graph::TimeUpdate,
    animation_node::{NodeLike, ReflectNodeLike},
    context::{new_context::NodeContext, spec_context::SpecContext},
    edge_data::DataSpec,
    errors::GraphError,
    pose::Pose,
};
use serde::{Deserialize, Serialize};

use crate::{
    build_branch_bone_mask, build_branch_filter_bone_mask_by_name,
    context_time_update,
};

pub(crate) use crate::source_pose_nodes::SourceGameplayTagBlendState as SourceEnumPoseBlendState;

fn evaluator_delta(ctx: &NodeContext) -> Result<f32, GraphError> {
    Ok(match context_time_update(ctx)? {
        TimeUpdate::Delta(value) => value.max(0.0),
        TimeUpdate::Absolute(_) | TimeUpdate::PercentOfEvent { .. } => 0.0,
    })
}

fn source_enum_alpha(
    state: &mut SourceEnumPoseBlendState,
    selected: bool,
    blend_time: f32,
    delta: f32,
) -> f32 {
    advance_gameplay_tag_weights(
        &mut state.blend, 2, selected as usize, blend_time.max(0.0),
        GameplayTagBlendMode::Linear, delta,
    )[1]
}

#[derive(Reflect, Clone, Debug)]
#[reflect(Default, NodeLike)]
#[type_path = "als_ext"]
pub struct SourceEnumPoseBlendNode {
    #[reflect(default)]
    pub reset_child_on_activation: bool,
    pub active_key: String,
    pub selected_enum_value: f32,
    pub active_enum_default: f32,
    pub default_blend_time: f32,
    pub selected_blend_time: f32,
    pub default_pose_pin: String,
    pub selected_pose_pin: String,
    pub default_blend_time_pin: String,
    pub selected_blend_time_pin: String,
    pub active_enum_pin: String,
    pub output_pin: String,
    pub source_visible_enum_entry: String,
}

impl Default for SourceEnumPoseBlendNode {
    fn default() -> Self {
        Self {
            reset_child_on_activation: false,
            active_key: default(),
            selected_enum_value: 0.0,
            active_enum_default: 0.0,
            default_blend_time: 0.0,
            selected_blend_time: 0.0,
            default_pose_pin: "Default Pose".to_owned(),
            selected_pose_pin: "Selected Pose".to_owned(),
            default_blend_time_pin: "Default Blend Time".to_owned(),
            selected_blend_time_pin: "Selected Blend Time".to_owned(),
            active_enum_pin: "Active Enum Value".to_owned(),
            output_pin: "Pose".to_owned(),
            source_visible_enum_entry: default(),
        }
    }
}

impl SourceEnumPoseBlendNode {
    fn active_value(&self, ctx: &NodeContext) -> Result<f32, GraphError> {
        if let Ok(value) = ctx.data_back(self.active_enum_pin.clone()) {
            return value.as_f32();
        }
        if self.active_key.is_empty() {
            return Ok(self.active_enum_default);
        }
        Ok(
            crate::source_instance::source_input_data(ctx, &self.active_key)
                .and_then(|value| value.as_f32().ok())
                .unwrap_or(self.active_enum_default),
        )
    }
}

impl NodeLike for SourceEnumPoseBlendNode {
    fn update(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        let value = self.active_value(&ctx)?;
        let selected = (value.round() - self.selected_enum_value).abs() <= f32::EPSILON;
        let (blend_time_pin, blend_time) = if selected {
            (&self.selected_blend_time_pin, self.selected_blend_time)
        } else {
            (&self.default_blend_time_pin, self.default_blend_time)
        };
        let blend_time = ctx.data_back(blend_time_pin.clone()).ok()
            .and_then(|value| value.as_f32().ok()).unwrap_or(blend_time);
        let delta = evaluator_delta(&ctx)?;
        let previous = ctx.state_mut_or_else(SourceEnumPoseBlendState::default)?
            .blend.instant_transition_source(selected as usize, blend_time);
        let reset = self.reset_child_on_activation && ctx.state_mut_or_else(SourceEnumPoseBlendState::default)?
            .blend.should_reset_child(2, selected as usize);
        let mut child_contexts = std::mem::take(&mut ctx.state_mut_or_else(SourceEnumPoseBlendState::default)?.child_contexts);
        if let Some(previous) = previous {
            read_blend_list_child(&ctx, if previous == 0 { &self.default_pose_pin } else { &self.selected_pose_pin },
                previous, self.reset_child_on_activation, false, &mut child_contexts)?;
        }
        let alpha = source_enum_alpha(
            ctx.state_mut_or_else(SourceEnumPoseBlendState::default)?,
            selected,
            blend_time,
            delta,
        );
        let mut poses = Vec::new();
        let mut contexts = Vec::new();
        for (index, (pin, weight)) in [
            (&self.default_pose_pin, 1.0 - alpha),
            (&self.selected_pose_pin, alpha),
        ].into_iter().enumerate() {
            if weight <= crate::gameplay_tag_blend::WEIGHT_THRESHOLD {
                continue;
            }
            let (pose, context) = read_blend_list_child(&ctx, pin, index, self.reset_child_on_activation,
                reset && index == selected as usize, &mut child_contexts)?;
            poses.push((pose, weight));
            if let Some(context) = context {
                contexts.push((context, weight));
            }
        }
        let output = blend_source_poses(&poses);
        ctx.set_time(output.timestamp);
        ctx.set_data_fwd(self.output_pin.clone(), output);
        let context = blend_animation_contexts(contexts);
        ctx.state_mut_or_else(SourceEnumPoseBlendState::default)?
            .context = context;
        ctx.state_mut_or_else(SourceEnumPoseBlendState::default)?.child_contexts = child_contexts;
        Ok(())
    }

    fn spec(&self, mut ctx: SpecContext) -> Result<(), GraphError> {
        ctx.add_input_data(self.default_pose_pin.clone(), DataSpec::Pose)
            .add_input_data(self.selected_pose_pin.clone(), DataSpec::Pose)
            .add_input_data(self.default_blend_time_pin.clone(), DataSpec::F32)
            .add_input_data(self.selected_blend_time_pin.clone(), DataSpec::F32)
            .add_input_data(self.active_enum_pin.clone(), DataSpec::F32)
            .add_output_data(self.output_pin.clone(), DataSpec::Pose);
        Ok(())
    }

    fn display_name(&self) -> String {
        "ALS Source Enum Pose Blend".to_owned()
    }
}

#[derive(Reflect, Serialize, Deserialize, Clone, Debug, Default)]
#[reflect(Default, Serialize)]
pub struct SourceBranchFilter {
    pub bone: String,
    pub blend_depth: i32,
}

#[derive(Debug)]
struct SourceLayeredBoneData {
    skeleton: bevy::asset::AssetId<bevy_animation_graph::core::skeleton::Skeleton>,
    skeleton_tick: u32,
    filters: Vec<Vec<(String, i32)>>,
    mask: bevy_animation_graph::core::edge_data::bone_mask::BoneMask,
    sources: bevy::platform::collections::HashMap<bevy_animation_graph::core::pose::BoneId, usize>,
    reference: Pose,
    hierarchy: Vec<(bevy_animation_graph::core::pose::BoneId, Option<usize>)>,
}

#[derive(Reflect, Clone, Debug, Default)]
pub(crate) struct SourceLayeredBoneBlendState {
    pub(crate) context: AnimationContextState,
    #[reflect(ignore)]
    data: Option<std::sync::Arc<SourceLayeredBoneData>>,
}

#[derive(Reflect, Clone, Debug)]
#[reflect(Default, NodeLike)]
#[type_path = "als_ext"]
pub struct SourceLayeredBoneBlendNode {
    pub branch_bones: Vec<String>,
    pub branch_bone_layers: Vec<Vec<String>>,
    pub branch_filters: Vec<SourceBranchFilter>,
    pub branch_filter_layers: Vec<Vec<SourceBranchFilter>>,
    pub blend_weight: f32,
    pub blend_weights: Vec<f32>,
    pub blend_weight_pin_prefix: String,
    pub expose_blend_weight_pins: bool,
    pub mesh_space_rotation_blend: bool,
    pub blend_root_motion_based_on_root_bone: bool,
    pub update_base_pose_first: bool,
    pub base_pose_pin: String,
    pub blend_pose_pin: String,
    pub blend_pose_pins: Vec<String>,
    pub output_pin: String,
}

impl Default for SourceLayeredBoneBlendNode {
    fn default() -> Self {
        Self {
            branch_bones: default(),
            branch_bone_layers: default(),
            branch_filters: default(),
            branch_filter_layers: default(),
            blend_weight: 1.0,
            blend_weights: default(),
            blend_weight_pin_prefix: "BlendWeights_".to_owned(),
            expose_blend_weight_pins: false,
            mesh_space_rotation_blend: true,
            blend_root_motion_based_on_root_bone: true,
            update_base_pose_first: false,
            base_pose_pin: "BasePose".to_owned(),
            blend_pose_pin: "BlendPoses_0".to_owned(),
            blend_pose_pins: default(),
            output_pin: "Pose".to_owned(),
        }
    }
}

impl SourceLayeredBoneBlendNode {
    fn layer_pin(&self, index: usize) -> &str {
        self.blend_pose_pins
            .get(index)
            .map(String::as_str)
            .unwrap_or(&self.blend_pose_pin)
    }

    fn layer_bones(&self, index: usize) -> &[String] {
        self.branch_bone_layers
            .get(index)
            .map(Vec::as_slice)
            .unwrap_or(&self.branch_bones)
    }

    fn layer_filters(&self, index: usize) -> &[SourceBranchFilter] {
        self.branch_filter_layers
            .get(index)
            .map(Vec::as_slice)
            .unwrap_or(&self.branch_filters)
    }

    fn layer_weight(&self, ctx: &NodeContext, index: usize) -> Result<f32, GraphError> {
        let fallback = self
            .blend_weights
            .get(index)
            .copied()
            .unwrap_or(self.blend_weight);
        if !self.expose_blend_weight_pins {
            return Ok(fallback);
        }
        ctx.data_back(format!("{}{}", self.blend_weight_pin_prefix, index))?
            .as_f32()
    }

    fn layer_count(&self) -> usize {
        self.blend_pose_pins.len().max(1)
    }
}

impl NodeLike for SourceLayeredBoneBlendNode {
    fn update(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        let mut bone_data = ctx.state::<SourceLayeredBoneBlendState>().ok().and_then(|state| state.data.clone());
        let weights = (0..self.layer_count()).map(|index| self.layer_weight(&ctx, index))
            .collect::<Result<Vec<_>, _>>()?;
        if !self.update_base_pose_first {
            for (index, weight) in weights.iter().enumerate() {
                if *weight > 1e-5 {
                    ctx.data_back(self.layer_pin(index).to_owned())?;
                }
            }
        }
        let mut base = ctx.data_back(self.base_pose_pin.clone())?.into_pose()?;
        let mut context = pose_input_context_state(&ctx, &self.base_pose_pin).unwrap_or_default();
        let mut overlays = Vec::with_capacity(weights.len());
        for (index, weight) in weights.iter().copied().enumerate() {
            if weight <= 1e-5 {
                overlays.push(None);
                continue;
            }
            let overlay = ctx
                .data_back(self.layer_pin(index).to_owned())?
                .into_pose()?;
            if let Some(overlay_context) = pose_input_context_state(&ctx, self.layer_pin(index)) {
                merge_sync_players(
                    &mut context.sync_players,
                    &overlay_context.sync_players,
                    weight,
                );
                merge_asset_samples(&mut context.asset_samples, &overlay_context.asset_samples, weight);
                context.curves.extend(overlay_context.curves);
                context.curves_are_authoritative |= overlay_context.curves_are_authoritative;
            }
            overlays.push(Some(overlay));
        }
        if overlays.iter().any(Option::is_some) {
            let skeletons = &ctx.graph_context.resources.skeleton_assets;
            if skeletons.get(&base.skeleton).is_none()
                && let Some(overlay) = overlays.iter().flatten().find(|pose| skeletons.get(&pose.skeleton).is_some()) {
                base.skeleton = overlay.skeleton.clone();
            }
            if let Some(skeleton) = skeletons.get(&base.skeleton) {
                let filters = (0..self.layer_count()).map(|index| {
                    let filters = self.layer_filters(index);
                    if filters.is_empty() { self.layer_bones(index).iter().map(|bone| (bone.as_str(), 0)).collect::<Vec<_>>() }
                    else { filters.iter().map(|filter| (filter.bone.as_str(), filter.blend_depth)).collect() }
                }).collect::<Vec<_>>();
                let skeleton_tick = skeletons.last_changed().get();
                if bone_data.as_ref().is_none_or(|data| {
                    data.skeleton != base.skeleton.id() || data.skeleton_tick != skeleton_tick
                        || data.filters.len() != filters.len()
                        || data.filters.iter().zip(&filters).any(|(cached, current)| {
                            cached.iter().map(|(bone, depth)| (bone.as_str(), *depth)).ne(current.iter().copied())
                        })
                }) {
                    let mask = build_branch_filter_bone_mask_by_name(skeleton, &filters.iter().flatten().copied().collect::<Vec<_>>());
                    let mut sources = bevy::platform::collections::HashMap::new();
                    for (index, filters) in filters.iter().enumerate() {
                        let branches = build_branch_bone_mask(skeleton, &filters.iter().map(|(bone, _)| *bone).collect::<Vec<_>>(), 1.0);
                        for bone in branches.get_weights().keys() { sources.insert(*bone, index); }
                    }
                    let mut reference = Pose { skeleton: base.skeleton.clone(), ..default() };
                    crate::mesh_space_additive::materialize_missing_reference_bones(&mut reference, skeleton);
                    bone_data = Some(std::sync::Arc::new(SourceLayeredBoneData {
                        skeleton: base.skeleton.id(), skeleton_tick,
                        filters: filters.iter().map(|filters| filters.iter().map(|(bone, depth)| ((*bone).to_owned(), *depth)).collect()).collect(),
                        mask, sources, reference,
                        hierarchy: crate::mesh_space_layered_blend::layered_bone_hierarchy(skeleton),
                    }));
                }
                let data = bone_data.as_ref().unwrap();
                let mut mask = data.mask.clone();
                let sources = &data.sources;
                let mut target = data.reference.clone();
                for (bone, index) in &target.paths {
                    let reference = target.bones[*index].clone();
                    if let Some(base_index) = base.paths.get(bone).copied() {
                        base.bones[base_index] = reference.overlay(&base.bones[base_index]);
                    } else { base.add_bone(reference.clone(), *bone); }
                    let mut source = sources.get(bone).copied().unwrap_or(0);
                    let weight = (mask.bone_weight(bone) * weights[source]).clamp(0.0, 1.0);
                    let weight = if weight <= 1e-5 { source = 0; 0.0 } else if weight >= 1.0 - 1e-5 { 1.0 } else { weight };
                    mask.update_bone_weight(*bone, weight);
                    if let Some(overlay) = overlays[source].as_ref().and_then(|pose| pose.get_bone(*bone)) {
                        target.bones[*index] = reference.overlay(overlay);
                    }
                }
                if self.mesh_space_rotation_blend {
                    crate::mesh_space_layered_blend::apply_mesh_space_layered_blend_with_hierarchy(&mut base, &target, 1.0, &mask, skeleton, &data.hierarchy);
                } else {
                    for (bone, index) in &base.paths {
                        let weight = mask.bone_weight(bone);
                        if weight <= 1e-5 { continue; }
                        if let Some(target) = target.get_bone(*bone) {
                            base.bones[*index] = if weight >= 1.0 - 1e-5 { target.clone() }
                                else { base.bones[*index].scalar_mult(1.0 - weight).linear_add(&target.scalar_mult(weight)).normalize_quat() };
                        }
                    }
                }
            } else { bone_data = None; }
        }
        ctx.set_time(base.timestamp);
        ctx.set_data_fwd(self.output_pin.clone(), base);
        let state = ctx.state_mut::<SourceLayeredBoneBlendState>()?;
        state.context = context;
        state.data = bone_data;
        Ok(())
    }

    fn spec(&self, mut ctx: SpecContext) -> Result<(), GraphError> {
        ctx.add_input_data(self.base_pose_pin.clone(), DataSpec::Pose);
        for index in 0..self.layer_count() {
            ctx.add_input_data(self.layer_pin(index).to_owned(), DataSpec::Pose);
            if self.expose_blend_weight_pins {
                ctx.add_input_data(
                    format!("{}{}", self.blend_weight_pin_prefix, index),
                    DataSpec::F32,
                );
            }
        }
        ctx.add_output_data(self.output_pin.clone(), DataSpec::Pose);
        Ok(())
    }

    fn display_name(&self) -> String {
        "ALS Source Layered Bone Blend".to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layered_bone_data_tracks_skeleton_and_filters_without_caching_live_inputs() {
        use bevy::platform::collections::HashMap;
        use bevy_animation_graph::core::{
            animation_clip::{EntityPath, GraphClip}, animation_graph::AnimationGraph,
            animation_node::{AnimationNode, dyn_node_like::DynNodeLike},
            context::{deferred_gizmos::DeferredGizmos, graph_context::QueryOutputTime,
                graph_context_arena::GraphContextArena, new_context::GraphContext,
                node_states::StateKey, system_resources::SystemResources},
            edge_data::DataValue, pose::BonePose, skeleton::Skeleton,
            state_machine::high_level::StateMachine, utils::testing::TestIoEnv,
        };
        #[derive(Resource)]
        struct Arenas([GraphContextArena; 2]);
        fn evaluate(In((instance, handle, base, overlay)): In<(usize, Handle<AnimationGraph>, Pose, Pose)>,
            mut arenas: ResMut<Arenas>, resources: SystemResources,
        ) -> (std::sync::Arc<SourceLayeredBoneData>, Pose) {
            let graph = resources.animation_graph_assets.get(&handle).unwrap();
            let node = graph.nodes.values().find(|node| node.name == "blend").unwrap();
            let arena = &mut arenas.0[instance];
            let mut gizmos = DeferredGizmos::default();
            let entities = HashMap::new();
            let env = TestIoEnv::default();
            let inputs = HashMap::from([("base".into(), DataValue::Pose(base)), ("overlay".into(), DataValue::Pose(overlay))]);
            let context = GraphContext::new(arena.get_toplevel_id(), graph, arena,
                &resources, &env, Entity::PLACEHOLDER, &entities, &mut gizmos, &inputs);
            let output = graph.query_with_context(QueryOutputTime::Forced(TimeUpdate::Delta(0.02)), context)
                .unwrap().remove("Pose").unwrap().into_pose().unwrap();
            let data = arena.get_toplevel().node_states.get::<SourceLayeredBoneBlendState>(node.id, StateKey::Default)
                .expect("layered blend does not retain bone data").data.clone().unwrap();
            arena.next_frame();
            (data, output)
        }
        let mut world = World::new();
        world.init_resource::<Assets<AnimationGraph>>();
        world.init_resource::<Assets<GraphClip>>();
        world.init_resource::<Assets<Skeleton>>();
        world.init_resource::<Assets<StateMachine>>();
        let scene = EntityPath::default().child("scene");
        let root = scene.child("root");
        let child = root.child("child");
        let skeletons = [1.0, 5.0].map(|height| {
            let mut skeleton = Skeleton::default();
            skeleton.set_root(scene.id());
            for (path, transform) in [(scene.clone(), Transform::IDENTITY), (root.clone(), Transform::IDENTITY),
                (child.clone(), Transform::from_xyz(0.0, height, 0.0))] {
                skeleton.add_bone(path, transform, transform);
            }
            world.resource_mut::<Assets<Skeleton>>().add(skeleton)
        });
        let skeleton = skeletons[0].clone();
        let mut config = SourceLayeredBoneBlendNode {
            branch_filters: vec![SourceBranchFilter { bone: "child".into(), blend_depth: 0 }],
            blend_weight: 0.5, ..default()
        };
        let blend = AnimationNode::new("blend", config.clone());
        let mut graph = AnimationGraph::new();
        graph.add_node(blend.clone());
        for (key, pin) in [("base", "BasePose"), ("overlay", "BlendPoses_0")] {
            let input = AnimationNode::new(key, crate::GlobalPoseInputNode { key: key.into(), output: "Pose".into() });
            graph.add_node(input.clone());
            graph.add_node_parameter_edge(input.id, "Pose", blend.id, pin);
        }
        graph.add_output_data("Pose".into(), DataSpec::Pose);
        graph.add_output_data_edge(blend.id, "Pose", "Pose");
        let graph = world.resource_mut::<Assets<AnimationGraph>>().add(graph);
        world.insert_resource(Arenas([GraphContextArena::new(graph.id()), GraphContextArena::new(graph.id())]));
        let mut base = Pose { skeleton: skeleton.clone(), ..default() };
        base.add_bone(BonePose::from_transform(Transform::from_xyz(1.0, 0.0, 0.0)), root.id());
        let mut overlay = Pose { skeleton: skeleton.clone(), ..default() };
        overlay.add_bone(BonePose::from_transform(Transform::from_xyz(10.0, 0.0, 0.0)), root.id());
        overlay.add_bone(BonePose::from_transform(Transform::from_xyz(0.0, 9.0, 0.0)), child.id());
        let (first, output) = world.run_system_cached_with(evaluate, (0, graph.clone(), base.clone(), overlay.clone())).unwrap();
        assert_eq!(output.get_bone(child.id()).unwrap().translation.unwrap().y, 5.0);
        let (second_instance, _) = world.run_system_cached_with(evaluate, (1, graph.clone(), base.clone(), overlay.clone())).unwrap();
        assert!(!std::sync::Arc::ptr_eq(&first, &second_instance));
        config.blend_weight = 0.25;
        world.resource_mut::<Assets<AnimationGraph>>().get_mut(&graph).unwrap().nodes.get_mut(&blend.id).unwrap().inner = DynNodeLike::new(config.clone());
        let base_index = base.paths[&root.id()];
        base.bones[base_index].translation = Some(Vec3::X * 5.0);
        let (weight_change, output) = world.run_system_cached_with(evaluate, (0, graph.clone(), base.clone(), overlay.clone())).unwrap();
        assert!(std::sync::Arc::ptr_eq(&first, &weight_change));
        assert_eq!(output.get_bone(root.id()).unwrap().translation.unwrap().x, 5.0);
        assert_eq!(output.get_bone(child.id()).unwrap().translation.unwrap().y, 3.0);
        let reference = Transform::from_xyz(0.0, 3.0, 0.0);
        world.resource_mut::<Assets<Skeleton>>().get_mut(&skeleton).unwrap().add_bone(child.clone(), reference, reference);
        let (skeleton_change, output) = world.run_system_cached_with(evaluate, (0, graph.clone(), base.clone(), overlay.clone())).unwrap();
        assert!(!std::sync::Arc::ptr_eq(&first, &skeleton_change));
        assert_eq!(output.get_bone(child.id()).unwrap().translation.unwrap().y, 4.5);
        assert_eq!(second_instance.reference.get_bone(child.id()).unwrap().translation.unwrap().y, 1.0);
        config.branch_filters[0].bone = "root".into();
        world.resource_mut::<Assets<AnimationGraph>>().get_mut(&graph).unwrap().nodes.get_mut(&blend.id).unwrap().inner = DynNodeLike::new(config);
        let (filter_change, output) = world.run_system_cached_with(evaluate, (0, graph.clone(), base.clone(), overlay.clone())).unwrap();
        assert!(!std::sync::Arc::ptr_eq(&skeleton_change, &filter_change));
        assert_eq!(output.get_bone(root.id()).unwrap().translation.unwrap().x, 6.25);
        let (stable, _) = world.run_system_cached_with(evaluate, (0, graph.clone(), base.clone(), overlay.clone())).unwrap();
        assert!(std::sync::Arc::ptr_eq(&filter_change, &stable));
        base.skeleton = skeletons[1].clone();
        overlay.skeleton = skeletons[1].clone();
        let (different_skeleton, output) = world.run_system_cached_with(evaluate, (0, graph, base, overlay)).unwrap();
        assert!(!std::sync::Arc::ptr_eq(&stable, &different_skeleton));
        assert_eq!(output.get_bone(child.id()).unwrap().translation.unwrap().y, 6.0);
    }

    #[test]
    fn layered_blend_uses_one_source_per_bone_and_source_rotation_math() {
        use bevy::platform::collections::HashMap;
        use bevy_animation_graph::core::{
            animation_clip::{EntityPath, GraphClip}, animation_graph::AnimationGraph,
            animation_node::AnimationNode,
            context::{deferred_gizmos::DeferredGizmos, graph_context::QueryOutputTime,
                graph_context_arena::GraphContextArena, new_context::GraphContext,
                node_states::StateKey, system_resources::SystemResources},
            edge_data::DataValue, pose::{BonePose, Pose, RootMotionDelta}, skeleton::Skeleton,
            state_machine::high_level::StateMachine, utils::testing::TestIoEnv,
        };
        fn query(In((handle, poses)): In<(Handle<AnimationGraph>, Vec<Pose>)>, resources: SystemResources) -> (Pose, AnimationContextState, [bool; 3]) {
            let graph = resources.animation_graph_assets.get(&handle).unwrap();
            let mut arena = GraphContextArena::new(handle.id());
            let mut gizmos = DeferredGizmos::default();
            let inputs = poses.into_iter().enumerate().map(|(index, pose)| (format!("input_{index}"), DataValue::Pose(pose))).collect();
            let entities = HashMap::new();
            let env = TestIoEnv::default();
            let context = GraphContext::new(arena.get_toplevel_id(), graph, &mut arena,
                &resources, &env, Entity::PLACEHOLDER, &entities, &mut gizmos, &inputs);
            let pose = graph.query_with_context(QueryOutputTime::Forced(TimeUpdate::Delta(0.02)), context.clone())
                .unwrap().remove("Pose").unwrap().into_pose().unwrap();
            let output_context = crate::animation_context::graph_output_context_state(graph, &context, "Pose").unwrap();
            let updated = [0, 1, 2].map(|index| {
                let node = graph.nodes.values().find(|node| node.name == format!("input_{index}")).unwrap();
                arena.get_toplevel().node_caches.is_updated(node.id, StateKey::Default)
            });
            (pose, output_context, updated)
        }
        let mut world = World::new();
        world.init_resource::<Assets<AnimationGraph>>();
        world.init_resource::<Assets<GraphClip>>();
        world.init_resource::<Assets<Skeleton>>();
        world.init_resource::<Assets<StateMachine>>();
        let scene = EntityPath::default().child("scene");
        let root = scene.child("root");
        let parent = root.child("parent");
        let paths = [root, parent.clone(), parent.child("child")];
        let mut skeleton = Skeleton::default();
        skeleton.set_root(scene.id());
        skeleton.add_bone(scene, Transform::IDENTITY, Transform::IDENTITY);
        let references = [0, 1, 2].map(|index| Transform::from_xyz(0.0, 0.1 * index as f32, 0.0)
            .with_rotation(Quat::from_rotation_z(0.1 * index as f32)));
        for (path, reference) in paths.iter().zip(references) { skeleton.add_bone(path.clone(), reference, reference); }
        let skeleton = world.resource_mut::<Assets<Skeleton>>().add(skeleton);
        let rotations = [
            [Quat::from_rotation_y(0.3), Quat::from_rotation_x(0.2), Quat::from_rotation_z(0.5)],
            [Quat::from_rotation_y(-0.8), Quat::from_rotation_z(1.2), Quat::from_rotation_x(-0.7)],
            [Quat::from_rotation_x(1.7), Quat::from_rotation_y(-1.5), Quat::from_rotation_z(1.3)],
        ];
        let transforms = [0, 1, 2].map(|pose| [0, 1, 2].map(|bone| Transform {
            translation: Vec3::new((pose * 3 + bone * 2) as f32, pose as f32 * 2.0, bone as f32 * 0.1),
            rotation: rotations[pose][bone], scale: Vec3::splat(1.0 + 0.2 * pose as f32 + 0.1 * bone as f32),
        }));
        let mut poses = transforms.iter().enumerate().map(|(index, transforms)| {
            let mut pose = Pose { skeleton: skeleton.clone(), timestamp: 9.0 + index as f32, ..default() };
            for (path, transform) in paths.iter().zip(transforms) { pose.add_bone(BonePose::from_transform(*transform), path.id()); }
            pose
        }).collect::<Vec<_>>();
        poses[0].root_motion = Some(RootMotionDelta { translation: Vec3::X, rotation: Quat::IDENTITY });
        let mut failures = Vec::new();
        for mesh in [false, true] {
            for (label, filters, weights, sources, effective) in [
                ("overlap", [vec![("parent", 0)], vec![("parent", 0)]], [0.4, 0.6], [0, 1, 1], [0.0, 0.6, 0.6]),
                ("reference_channels", [vec![("parent", 0)], vec![("parent", 0)]], [0.4, 0.6], [0, 1, 1], [0.0, 0.6, 0.6]),
                ("cumulative", [vec![("parent", 4)], vec![("parent", 4)]], [0.4, 0.6], [0, 1, 1], [0.0, 0.3, 0.6]),
                ("negative_depth", [vec![("parent", 0)], vec![("child", -2)]], [0.4, 0.6], [0, 0, 1], [0.0, 0.4, 0.3]),
                ("cleared_ancestor", [vec![("root", 0)], vec![("root", -1), ("child", 0)]], [0.4, 0.6], [0, 0, 1], [0.0, 0.0, 0.6]),
                ("inactive_override", [vec![("parent", 0)], vec![("parent", 0)]], [0.4, 0.0], [0, 0, 0], [0.0, 0.0, 0.0]),
                ("inactive_ancestor", [vec![("parent", 0)], vec![("child", 0)]], [0.0, 0.6], [0, 0, 1], [0.0, 0.0, 0.6]),
                ("threshold", [vec![("parent", 4)], vec![("child", 0)]], [0.00002, 0.0], [0, 0, 0], [0.0, 0.0, 0.0]),
                ("full_weight", [vec![("parent", 0)], vec![("root", 0)]], [0.4, 0.999995], [1, 1, 1], [1.0, 1.0, 1.0]),
            ] {
                let mut graph = AnimationGraph::new();
                let blend = AnimationNode::new("blend", SourceLayeredBoneBlendNode {
                    branch_filter_layers: filters.into_iter().map(|filters| filters.into_iter().map(|(bone, blend_depth)| SourceBranchFilter { bone: bone.into(), blend_depth }).collect()).collect(),
                    blend_weights: weights.to_vec(), blend_pose_pins: vec!["BlendPoses_0".into(), "BlendPoses_1".into()],
                    mesh_space_rotation_blend: mesh, ..default()
                });
                graph.add_node(blend.clone());
                graph.add_output_data("Pose".into(), DataSpec::Pose);
                graph.add_output_data_edge(blend.id, "Pose", "Pose");
                for (index, pin) in ["BasePose", "BlendPoses_0", "BlendPoses_1"].into_iter().enumerate() {
                    let key = format!("input_{index}");
                    let input = AnimationNode::new(&key, crate::GlobalPoseInputNode { key: key.clone(), output: "Pose".into() });
                    let curves = AnimationNode::new(format!("curves_{index}"), crate::SourceCurveModifyNode {
                        curve_names: vec!["shared".into(), format!("unique_{index}")], pin_defaults: vec![0.2 + index as f32; 2],
                        alpha: 1.0, ..default()
                    });
                    graph.add_node(input.clone());
                    graph.add_node(curves.clone());
                    graph.add_node_parameter_edge(input.id, "Pose", curves.id, "Source Pose");
                    graph.add_node_parameter_edge(curves.id, "Pose", blend.id, pin);
                }
                let graph = world.resource_mut::<Assets<AnimationGraph>>().add(graph);
                let mut input_poses = poses.clone();
                let mut case_transforms = transforms;
                if label == "reference_channels" {
                    input_poses[0].bones[1].scale = None;
                    case_transforms[0][1].scale = references[1].scale;
                    input_poses[2].bones[1].translation = None;
                    case_transforms[2][1].translation = references[1].translation;
                    input_poses[0].paths.remove(&paths[2].id());
                    case_transforms[0][2] = references[2];
                }
                let (output, context, updated) = world.run_system_cached_with(query, (graph, input_poses)).unwrap();
                let active = [true, weights[0] > 1e-5, weights[1] > 1e-5];
                let last_active = active.iter().rposition(|value| *value).unwrap();
                let curves_match = context.curves.len() == 1 + active.iter().filter(|value| **value).count()
                    && context.curves["shared"] == 0.2 + last_active as f32
                    && active.iter().enumerate().all(|(index, active)| context.curves.get(&format!("unique_{index}")) == active.then_some(&(0.2 + index as f32)));
                if updated != active || !curves_match || output.timestamp != 9.0 || output.skeleton != skeleton {
                    failures.push(format!("{label} mesh={mesh}: context or child lifecycle mismatch"));
                }
                if effective[0] == 0.0 && output.root_motion.as_ref().map(|motion| motion.translation) != Some(Vec3::X) {
                    failures.push(format!("{label} mesh={mesh}: base root motion was lost"));
                }
                let mut source_mesh = Quat::IDENTITY;
                let mut target_mesh = Quat::IDENTITY;
                let mut parent_output = Quat::IDENTITY;
                for bone in 0..3 {
                    let source = case_transforms[0][bone];
                    let target = if active[sources[bone] + 1] { case_transforms[sources[bone] + 1][bone] } else { references[bone] };
                    source_mesh *= source.rotation;
                    target_mesh *= target.rotation;
                    let rotation = if mesh { source_mesh.lerp(target_mesh, effective[bone]).normalize() }
                        else { source.rotation.lerp(target.rotation, effective[bone]).normalize() };
                    let expected = Transform {
                        translation: source.translation.lerp(target.translation, effective[bone]),
                        rotation: if mesh { (parent_output.inverse() * rotation).normalize() } else { rotation },
                        scale: source.scale.lerp(target.scale, effective[bone]),
                    };
                    parent_output = rotation;
                    let actual = output.get_bone(paths[bone].id()).unwrap().to_transform();
                    if actual.translation.distance(expected.translation) > 1e-6 || actual.scale.distance(expected.scale) > 1e-6
                        || 1.0 - actual.rotation.dot(expected.rotation).abs() > 1e-6 {
                        failures.push(format!("{label} mesh={mesh} bone={bone}: actual={actual:?}, expected={expected:?}"));
                    }
                }
            }
        }
        assert!(failures.is_empty(), "{}", failures.join("\n"));
    }

    #[test]
    fn source_enum_pose_blend_initializes_to_the_selected_source_entry() {
        let mut state = SourceEnumPoseBlendState::default();
        assert_eq!(source_enum_alpha(&mut state, true, 0.1, 0.0), 1.0);
        assert_eq!(source_enum_alpha(&mut state, false, 0.1, 0.0), 1.0);
        assert!((source_enum_alpha(&mut state, false, 0.1, 0.02) - 0.8).abs() < 1e-6);
        assert!((source_enum_alpha(&mut state, false, 0.1, 0.02) - 0.6).abs() < 1e-6);
        assert!(source_enum_alpha(&mut state, false, 0.1, 0.06) < 1e-6);
        assert!((source_enum_alpha(&mut state, true, 0.1, 0.05) - 0.5).abs() < 1e-6);
        assert!(source_enum_alpha(&mut state, false, 0.1, 0.05) < 1e-6);
    }

    #[test]
    fn source_blend_children_follow_source_update_order_and_relevance() {
        use bevy::platform::collections::HashMap;
        use bevy_animation_graph::core::{
            animation_clip::GraphClip, animation_graph::{AnimationGraph, NodeId},
            animation_node::AnimationNode,
            context::{deferred_gizmos::DeferredGizmos, graph_context::QueryOutputTime,
                graph_context_arena::GraphContextArena, new_context::GraphContext,
                node_states::StateKey, system_resources::SystemResources},
            edge_data::DataValue, pose::Pose, skeleton::Skeleton,
            state_machine::high_level::StateMachine, utils::testing::TestIoEnv,
        };
        use crate::source_instance::{SourceInstanceProperties, SourcePropertyAssignment, SourcePropertyWriteNode};
        fn query(In((handle, active)): In<(Handle<AnimationGraph>, Vec<f32>)>, resources: SystemResources) -> Vec<(f32, [bool; 2])> {
            let graph = resources.animation_graph_assets.get(&handle).unwrap();
            let mut arena = GraphContextArena::new(handle.id());
            let mut gizmos = DeferredGizmos::default();
            let entities = HashMap::new();
            let env = TestIoEnv::default();
            active.into_iter().map(|active| {
                let inputs = HashMap::from([("input".into(), DataValue::Pose(Pose::default())), ("active".into(), DataValue::F32(active))]);
                let context = GraphContext::new(arena.get_toplevel_id(), graph, &mut arena,
                    &resources, &env, Entity::PLACEHOLDER, &entities, &mut gizmos, &inputs);
                graph.query_with_context(QueryOutputTime::Forced(TimeUpdate::Delta(0.02)), context).unwrap();
                let state = arena.get_toplevel().node_states.get::<SourceInstanceProperties>(NodeId::default(), StateKey::Default).unwrap();
                (state.values["order"].as_f32().unwrap(), ["child_0", "child_1"].map(|key| state.write_frames.get(key) == Some(&arena.frame_index())))
            }).collect()
        }
        let mut world = World::new();
        world.insert_resource(Assets::<AnimationGraph>::default());
        world.insert_resource(Assets::<GraphClip>::default());
        world.insert_resource(Assets::<Skeleton>::default());
        world.insert_resource(Assets::<StateMachine>::default());
        for (kind, selected, expected) in [("layered", false, 1.0), ("layered", true, 2.0), ("layered_zero", false, 1.0), ("enum", false, 1.0), ("enum", true, 2.0)] {
            let mut graph = AnimationGraph::new();
            let layered = kind.starts_with("layered");
            let blend = if layered {
                AnimationNode::new("blend", SourceLayeredBoneBlendNode {
                    update_base_pose_first: selected,
                    blend_weight: if kind == "layered_zero" { 0.0 } else { 1.0 },
                    ..default()
                })
            } else {
                AnimationNode::new("blend", SourceEnumPoseBlendNode {
                    active_key: "active".into(), selected_enum_value: 1.0, ..default()
                })
            };
            graph.add_node(blend.clone());
            graph.add_output_data("Pose".into(), DataSpec::Pose);
            graph.add_output_data_edge(blend.id, "Pose", "Pose");
            let input = AnimationNode::new("input", crate::GlobalPoseInputNode { key: "input".into(), output: "Pose".into() });
            graph.add_node(input.clone());
            for (index, pin) in if layered { ["BasePose", "BlendPoses_0"] } else { ["Default Pose", "Selected Pose"] }.into_iter().enumerate() {
                let child = AnimationNode::new(format!("child_{index}"), SourcePropertyWriteNode {
                    assignments: vec![
                        SourcePropertyAssignment { property: "order".into(), value: DataValue::F32(index as f32 + 1.0) },
                        SourcePropertyAssignment { property: format!("child_{index}"), value: DataValue::Bool(true) },
                    ],
                    ..default()
                });
                graph.add_node(child.clone());
                graph.add_node_parameter_edge(input.id, "Pose", child.id, "Source");
                graph.add_node_parameter_edge(child.id, "Pose", blend.id, pin);
            }
            let handle = world.resource_mut::<Assets<AnimationGraph>>().add(graph);
            let active = if layered { vec![0.0] } else { vec![selected as u8 as f32, !selected as u8 as f32, !selected as u8 as f32, selected as u8 as f32, selected as u8 as f32] };
            let output = world.run_system_cached_with(query, (handle, active.clone())).unwrap();
            assert_eq!(output[0].0, expected, "{kind} {selected}");
            if !layered {
                for (index, (order, updated)) in output.into_iter().enumerate() {
                    let changed = index > 0 && active[index - 1] != active[index];
                    assert_eq!(order, active[index] + 1.0);
                    assert_eq!(updated, [changed || active[index] == 0.0, changed || active[index] == 1.0]);
                }
            }
        }
    }

    #[test]
    fn source_enum_pose_blend_can_preserve_source_pin_names() {
        let node = SourceEnumPoseBlendNode {
            default_pose_pin: "BlendPose_0".to_owned(),
            selected_pose_pin: "BlendPose_1".to_owned(),
            default_blend_time_pin: "BlendTime_0".to_owned(),
            selected_blend_time_pin: "BlendTime_1".to_owned(),
            active_enum_pin: "ActiveEnumValue".to_owned(),
            ..default()
        };
        assert_eq!(node.selected_pose_pin, "BlendPose_1");
        assert_eq!(node.active_enum_pin, "ActiveEnumValue");
    }

    #[test]
    fn source_layered_bone_blend_defaults_preserve_source_pin_names() {
        let node = SourceLayeredBoneBlendNode::default();
        assert_eq!(node.base_pose_pin, "BasePose");
        assert_eq!(node.blend_pose_pin, "BlendPoses_0");
        assert!(node.mesh_space_rotation_blend);
    }

    #[test]
    fn source_layered_bone_blend_keeps_each_source_layer_pin_and_mask() {
        let node = SourceLayeredBoneBlendNode {
            branch_bone_layers: vec![vec!["thigh_l".to_owned()], vec!["thigh_r".to_owned()]],
            branch_filter_layers: vec![
                vec![SourceBranchFilter {
                    bone: "thigh_l".to_owned(),
                    blend_depth: 3,
                }],
                vec![SourceBranchFilter {
                    bone: "thigh_r".to_owned(),
                    blend_depth: -1,
                }],
            ],
            blend_weights: vec![1.0, 0.5],
            blend_pose_pins: vec!["BlendPoses_0".to_owned(), "BlendPoses_1".to_owned()],
            ..default()
        };
        assert_eq!(node.layer_count(), 2);
        assert_eq!(node.layer_pin(0), "BlendPoses_0");
        assert_eq!(node.layer_pin(1), "BlendPoses_1");
        assert_eq!(node.layer_bones(0), ["thigh_l"]);
        assert_eq!(node.layer_bones(1), ["thigh_r"]);
        assert_eq!(node.layer_filters(0)[0].blend_depth, 3);
        assert_eq!(node.layer_filters(1)[0].blend_depth, -1);
    }
}
