use bevy::prelude::*;
use bevy_animation_graph::core::{
    animation_graph::{AnimationGraph, GraphInputPin, TargetPin},
    animation_node::{NodeLike, ReflectNodeLike},
    context::{
        graph_context::QueryOutputTime,
        new_context::{GraphContext, NodeContext},
        spec_context::{NodeInput, NodeOutput, SpecContext},
    },
    edge_data::{DataSpec, DataValue},
    errors::GraphError,
};

use crate::animation_context::{
    AnimationContextInput, AnimationContextInputAlias, AnimationContextIoEnv,
    AnimationContextState, capture_context_curve, context_time_update, graph_output_context_state,
    is_context_curve_input, merge_linked_input_context, merge_missing_context_curves,
    pose_input_context_state, resolve_context_curves, select_context_pose_output,
};
use crate::global_f32_input::{global_f32_or_root_default, global_f32_or_root_default_value};
use crate::source_cached_pose::resolve_cached_pose;

#[derive(Reflect, Clone, Debug, Default)]
#[reflect(Default)]
pub struct GlobalF32InputPin {
    pub pin: String,
    pub key: String,
    pub default_value: f32,
}

#[derive(Reflect, Clone, Debug, Default)]
#[reflect(Default)]
pub struct FixedF32Input {
    pub input: String,
    pub value: f32,
}

#[derive(Reflect, Clone, Debug, Default)]
#[reflect(Default)]
pub struct FixedGlobalF32Input {
    pub input: String,
    pub key: String,
}

#[derive(Reflect, Clone, Debug, Default)]
#[reflect(Default)]
pub struct CachedPoseInput {
    pub input: String,
    pub cache_node: String,
}

#[derive(Reflect, Clone, Debug, Default)]
#[reflect(Default)]
pub struct OutputDataAlias {
    pub child_output: String,
    pub outer_output: String,
}

#[derive(Reflect, Clone, Debug, Default)]
#[reflect(Default)]
pub struct NamedPoseOutput {
    pub node_name: String,
    pub output: String,
    pub outer_output: String,
}

#[derive(Reflect, Clone, Debug, Default)]
#[reflect(Default, NodeLike)]
#[type_path = "als_ext"]
pub struct LayeredGraphNode {
    pub graph: Handle<AnimationGraph>,
    #[reflect(default)]
    pub graph_binding: String,
    pub context_inputs: Vec<AnimationContextInput>,
    pub input_aliases: Vec<AnimationContextInputAlias>,
    pub global_input_pins: Vec<GlobalF32InputPin>,
    pub fixed_f32_inputs: Vec<FixedF32Input>,
    pub fixed_global_f32_inputs: Vec<FixedGlobalF32Input>,
    pub cached_pose_inputs: Vec<CachedPoseInput>,
    pub prewarm_cached_pose_nodes: Vec<String>,
    pub output_aliases: Vec<OutputDataAlias>,
    pub named_pose_output: Option<NamedPoseOutput>,
    pub capture_context: bool,
    pub global_curve_prefixes: Vec<String>,
    pub expose_context_curves: bool,
    pub exposed_output_data: Vec<String>,
    pub hide_context_auxiliary_outputs: bool,
    pub internalize_time: bool,
}

impl LayeredGraphNode {
    fn selected_graph(&self, ctx: &NodeContext) -> Handle<AnimationGraph> {
        if self.graph_binding.is_empty() { return self.graph.clone(); }
        crate::source_gameplay_tag_input::linked_graph_input(ctx.graph_context.root_entity, &self.graph_binding).unwrap_or_else(|| self.graph.clone())
    }

    pub fn new(graph: Handle<AnimationGraph>) -> Self {
        Self {
            graph,
            graph_binding: default(),
            context_inputs: default(),
            input_aliases: default(),
            global_input_pins: default(),
            fixed_f32_inputs: default(),
            fixed_global_f32_inputs: default(),
            cached_pose_inputs: default(),
            prewarm_cached_pose_nodes: default(),
            output_aliases: default(),
            named_pose_output: None,
            capture_context: false,
            global_curve_prefixes: default(),
            expose_context_curves: false,
            exposed_output_data: default(),
            hide_context_auxiliary_outputs: false,
            internalize_time: false,
        }
    }

    fn expose_output(&self, id: &str, data_spec: &DataSpec) -> bool {
        if !self.capture_context {
            return true;
        }
        if id == "pose" {
            return true;
        }
        if self.exposed_output_data.iter().any(|value| value == id) {
            return true;
        }
        if *data_spec == DataSpec::F32 {
            return self.expose_context_curves;
        }
        !self.hide_context_auxiliary_outputs
    }

    fn outer_input(&self, child_input: &str) -> String {
        self.input_aliases
            .iter()
            .find(|alias| alias.child_input == child_input)
            .map(|alias| alias.outer_input.clone())
            .unwrap_or_else(|| child_input.to_owned())
    }

    fn outer_output(&self, child_output: &str) -> String {
        self.output_aliases
            .iter()
            .find(|alias| alias.child_output == child_output)
            .map(|alias| alias.outer_output.clone())
            .unwrap_or_else(|| child_output.to_owned())
    }

    fn exposes_source_pose_alias(output: &str, data_spec: &DataSpec) -> bool {
        *data_spec == DataSpec::Pose && output != "pose"
    }

    fn io_input_aliases(&self) -> bevy::platform::collections::HashMap<String, String> {
        self.input_aliases
            .iter()
            .map(|alias| (alias.child_input.clone(), alias.outer_input.clone()))
            .collect()
    }

    fn merge_input_pose_contexts(
        &self,
        graph: &AnimationGraph,
        ctx: &NodeContext,
        target: &mut AnimationContextState,
    ) {
        for input in graph.io_spec.sorted_inputs() {
            let NodeInput::Data(GraphInputPin::Passthrough(input), DataSpec::Pose) = input else {
                continue;
            };
            if let Some(context) = pose_input_context_state(ctx, &self.outer_input(&input)) {
                merge_linked_input_context(target, &context);
            }
        }
    }

    fn child_data_values(
        &self,
        ctx: &NodeContext,
    ) -> Result<bevy::platform::collections::HashMap<String, DataValue>, GraphError> {
        let mut values = self
            .fixed_f32_inputs
            .iter()
            .map(|input| (input.input.clone(), DataValue::F32(input.value)))
            .collect::<bevy::platform::collections::HashMap<_, _>>();
        for input in &self.global_input_pins {
            values.insert(
                input.pin.clone(),
                DataValue::F32(global_f32_or_root_default_value(
                    ctx,
                    &input.key,
                    input.default_value,
                )?),
            );
        }
        for input in &self.fixed_global_f32_inputs {
            values.insert(
                input.input.clone(),
                DataValue::F32(global_f32_or_root_default(ctx, &input.key)?),
            );
        }
        for input in &self.cached_pose_inputs {
            let mut parent_ctx = ctx.clone();
            values.insert(
                input.input.clone(),
                DataValue::Pose(resolve_cached_pose(&mut parent_ctx, &input.cache_node)?),
            );
        }
        Ok(values)
    }

    fn prewarm_cached_poses(&self, ctx: &NodeContext) -> Result<(), GraphError> {
        for cache_node in &self.prewarm_cached_pose_nodes {
            let mut parent_ctx = ctx.clone();
            resolve_cached_pose(&mut parent_ctx, cache_node)?;
        }
        Ok(())
    }

    fn has_fixed_input(&self, input: &str) -> bool {
        self.fixed_f32_inputs
            .iter()
            .any(|value| value.input == input)
    }

    fn has_global_input(&self, input: &str) -> bool {
        self.global_input_pins
            .iter()
            .any(|value| value.pin == input)
    }

    fn has_hidden_input(&self, input: &str) -> bool {
        self.fixed_global_f32_inputs
            .iter()
            .any(|value| value.input == input)
            || self
                .cached_pose_inputs
                .iter()
                .any(|value| value.input == input)
    }

    fn time_update(
        &self,
        ctx: &NodeContext,
    ) -> Option<bevy_animation_graph::core::animation_graph::TimeUpdate> {
        if self.internalize_time {
            context_time_update(ctx).ok()
        } else {
            ctx.time_update_fwd().ok()
        }
    }

    fn named_pose(
        &self,
        graph: &AnimationGraph,
        ctx: &mut GraphContext<'_>,
        output: &NamedPoseOutput,
    ) -> Result<bevy_animation_graph::core::pose::Pose, GraphError> {
        let node = graph
            .nodes
            .values()
            .find(|node| node.name == output.node_name)
            .ok_or_else(|| GraphError::UnknownErrorNode {
                graph: ctx.context_id,
                node: default(),
                error: format!("named pose source is missing: {}", output.node_name),
            })?;
        let node_id = node.id;
        let key = ctx.state_key;
        if !ctx.node_caches().is_updated(node_id, key)
            && graph.io_spec.get_output_data(&"pose".to_owned()).is_some()
        {
            let update = context_time_update(&ctx.create_node_context(node_id))?;
            ctx.context_mut().query_output_time = QueryOutputTime::from_key(key, update);
            graph.get_data(TargetPin::OutputData("pose".to_owned()), ctx.clone())?;
        }
        if !ctx.node_caches().is_update_started(node_id, key) {
            ctx.node_caches_mut().mark_update_started(node_id, key);
            node.update(
                ctx.create_node_context(node_id)
                    .with_debugging(node.should_debug),
            )?;
            ctx.node_caches_mut().mark_updated(node_id, key);
        } else if !ctx.node_caches().is_updated(node_id, key) {
            return Err(GraphError::UnknownErrorNode {
                graph: ctx.context_id,
                node: node_id,
                error: format!("named pose update is reentrant: {}", output.node_name),
            });
        }
        ctx.node_caches()
            .get_output_data(node_id, key, output.output.clone())
            .ok_or_else(|| GraphError::UnknownErrorNode {
                graph: ctx.context_id,
                node: node_id,
                error: format!("named pose output is missing: {}", output.output),
            })?
            .into_pose()
    }

    fn capture_global_curves(
        &self,
        ctx: &NodeContext,
        state: &mut AnimationContextState,
    ) -> Result<(), GraphError> {
        for prefix in &self.global_curve_prefixes {
            for (key, value) in ctx.graph_context.global_input_data {
                let Some(curve) = key.strip_prefix(prefix) else {
                    continue;
                };
                let value = value.as_f32()?;
                state.curves.insert(curve.to_owned(), value);
                state.curves.insert(source_curve_name(curve), value);
            }
        }
        Ok(())
    }
}

fn source_curve_name(value: &str) -> String {
    value
        .split('_')
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut characters = part.chars();
            let Some(first) = characters.next() else {
                return String::new();
            };
            let mut result = first.to_uppercase().collect::<String>();
            result.push_str(characters.as_str());
            result
        })
        .collect()
}

impl NodeLike for LayeredGraphNode {
    fn duration(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        let graph_handle = self.selected_graph(&ctx);
        let Some(graph) = ctx
            .graph_context
            .resources
            .animation_graph_assets
            .get(&graph_handle)
        else {
            return Ok(());
        };

        let sub_ctx_io = AnimationContextIoEnv {
            parent_ctx: ctx.clone(),
            data_values: self.child_data_values(&ctx)?,
            curve_values: default(),
            input_aliases: self.io_input_aliases(),
            time_update: None,
        };

        let sub_ctx = ctx
            .create_child_context(graph_handle.id(), graph, None)
            .with_io(&sub_ctx_io);

        if self.named_pose_output.is_some() {
            ctx.set_duration_fwd(None);
        } else if graph.io_spec.has_output_time() {
            let duration = graph.get_duration(TargetPin::OutputTime, sub_ctx)?;
            ctx.set_duration_fwd(duration);
        } else {
            ctx.set_duration_fwd(None);
        }

        Ok(())
    }

    fn update(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        let graph_handle = self.selected_graph(&ctx);
        let Some(graph) = ctx
            .graph_context
            .resources
            .animation_graph_assets
            .get(&graph_handle)
        else {
            return Ok(());
        };

        self.prewarm_cached_poses(&ctx)?;

        let time_update = self.time_update(&ctx);
        if !self.internalize_time
            && let Some(time_update) = &time_update
        {
            for pin in graph.io_spec.iter_input_times() {
                if let GraphInputPin::Passthrough(pin_id) = pin {
                    ctx.set_time_update_back(pin_id.clone(), time_update.clone());
                }
            }
        }

        let curve_values = if self.capture_context {
            resolve_context_curves(&ctx, graph, &self.context_inputs)?
        } else {
            default()
        };

        let sub_ctx_io = AnimationContextIoEnv {
            parent_ctx: ctx.clone(),
            data_values: self.child_data_values(&ctx)?,
            curve_values,
            input_aliases: self.io_input_aliases(),
            time_update: self.internalize_time.then(|| time_update.clone()).flatten(),
        };

        let mut sub_ctx = ctx
            .create_child_context(graph_handle.id(), graph, None)
            .with_io(&sub_ctx_io);

        if let Some(output) = &self.named_pose_output {
            let pose = self.named_pose(graph, &mut sub_ctx, output)?;
            ctx.set_time(pose.timestamp);
            ctx.set_data_fwd(output.outer_output.clone(), pose);
            return Ok(());
        }

        if graph.io_spec.has_output_time()
            && let Some(time_update) = time_update
        {
            let key = sub_ctx.state_key;
            sub_ctx.context_mut().query_output_time = QueryOutputTime::from_key(key, time_update);
        }

        let mut context_state = AnimationContextState::default();
        let mut pose_output = None;
        for (id, data_spec) in graph.io_spec.iter_output_data() {
            let value = graph.get_data(TargetPin::OutputData(id.clone()), sub_ctx.clone())?;
            let outer_output = self.outer_output(id);
            if self.capture_context {
                capture_context_curve(&mut context_state, id, &value)?;
            }
            select_context_pose_output(&mut pose_output, id, data_spec);
            if self.expose_output(id, data_spec) {
                if Self::exposes_source_pose_alias(&outer_output, data_spec) {
                    ctx.set_data_fwd("pose", value.clone());
                }
                ctx.set_data_fwd(outer_output, value);
            }
        }
        if self.capture_context {
            if let Some(output) = pose_output
                && let Some(output_context) = graph_output_context_state(graph, &sub_ctx, &output)
            {
                merge_missing_context_curves(&mut context_state, &output_context);
            }
            self.merge_input_pose_contexts(graph, &ctx, &mut context_state);
            self.capture_global_curves(&ctx, &mut context_state)?;
            *ctx.state_mut_or_else(AnimationContextState::default)? = context_state;
        }

        Ok(())
    }

    fn spec(&self, mut ctx: SpecContext) -> Result<(), GraphError> {
        let graph = ctx
            .resources()
            .graph_assets
            .get(&self.graph)
            .ok_or(GraphError::GraphAssetMissing)?;

        for input in graph.io_spec.sorted_inputs() {
            match input {
                NodeInput::Time(GraphInputPin::Passthrough(pin_id)) => {
                    if !self.internalize_time {
                        ctx.add_input_time(pin_id);
                    }
                }
                NodeInput::Data(GraphInputPin::Passthrough(pin_id), data_spec) => {
                    if !self.has_fixed_input(&pin_id)
                        && !self.has_global_input(&pin_id)
                        && !self.has_hidden_input(&pin_id)
                        && !is_context_curve_input(&pin_id, &data_spec, &self.context_inputs)
                    {
                        ctx.add_input_data(self.outer_input(&pin_id), data_spec);
                    }
                }
                _ => {}
            }
        }
        for input in &self.global_input_pins {
            ctx.add_input_data(input.pin.clone(), DataSpec::F32);
        }
        if let Some(output) = &self.named_pose_output {
            ctx.add_output_data(output.outer_output.clone(), DataSpec::Pose);
            return Ok(());
        }

        for output in graph.io_spec.sorted_outputs() {
            match output {
                NodeOutput::Time => {
                    if !self.internalize_time {
                        ctx.add_output_time();
                    }
                }
                NodeOutput::Data(pin_id, data_spec) => {
                    if self.expose_output(&pin_id, &data_spec) {
                        let outer_output = self.outer_output(&pin_id);
                        if Self::exposes_source_pose_alias(&outer_output, &data_spec) {
                            ctx.add_output_data("pose", DataSpec::Pose);
                        }
                        ctx.add_output_data(outer_output, data_spec);
                    }
                }
            }
        }

        Ok(())
    }

    fn display_name(&self) -> String {
        "ALS Layered Graph".into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn instance_graph_binding_switches_and_resets_only_the_linked_subtree() {
        use bevy::{ecs::system::RunSystemOnce, platform::collections::HashMap};
        use bevy_animation_graph::core::{
            animation_clip::GraphClip, animation_graph::TimeUpdate, animation_node::AnimationNode,
            context::{deferred_gizmos::DeferredGizmos, graph_context_arena::GraphContextArena, system_resources::SystemResources},
            skeleton::Skeleton, state_machine::high_level::StateMachine, utils::testing::TestIoEnv,
        };
        use crate::source_gameplay_tag_input::{clear_gameplay_tag_inputs, set_linked_graph_input};
        fn query(In((root, implementation, entities)): In<(Handle<AnimationGraph>, Handle<AnimationGraph>, [Entity;2])>, resources: SystemResources) {
            let graph = resources.animation_graph_assets.get(&root).unwrap();
            let mut arenas = [GraphContextArena::new(root.id()), GraphContextArena::new(root.id())];
            let mut gizmos = DeferredGizmos::default();
            let entity_map = HashMap::new();
            let inputs = HashMap::new();
            let env = TestIoEnv::default();
            for (frame, (selected, expected)) in [(false,0.1),(false,0.2),(true,1.1),(true,1.2),(false,0.1),(false,0.2),(true,1.1)].into_iter().enumerate() {
                set_linked_graph_input(entities[0], "layer", selected.then(|| implementation.clone()));
                set_linked_graph_input(entities[1], "layer", Some(implementation.clone()));
                for index in 0..2 {
                    let arena = &mut arenas[index];
                    let context = GraphContext::new(arena.get_toplevel_id(), graph, arena, &resources, &env, entities[index], &entity_map, &mut gizmos, &inputs);
                    let output = graph.query_with_context(QueryOutputTime::Forced(TimeUpdate::Delta(0.1)), context.clone()).unwrap();
                    let expected = if index == 0 { expected } else { 1.0 + (frame + 1) as f32 * 0.1 };
                    assert!((output["selected"].as_pose().unwrap().timestamp - expected).abs() < 1e-5, "actor {index}, frame {frame}");
                    assert!((output["sibling"].as_pose().unwrap().timestamp - (frame + 1) as f32 * 0.1).abs() < 1e-5);
                    let state = graph_output_context_state(graph, &context, "selected").unwrap();
                    assert_eq!(state.curves["Layer"], if index == 1 || selected { 0.75 } else { 0.25 });
                }
            }
            assert_eq!(arenas[0].iter_context_ids().count(), 4);
            for entity in entities { clear_gameplay_tag_inputs(entity); }
        }
        let mut graphs = Assets::<AnimationGraph>::default();
        let mut clips = Assets::<GraphClip>::default();
        let clip = clips.add(GraphClip { duration:4.0, ..default() });
        let mut implementations = Vec::new();
        for (start, value) in [(0.0,0.25),(1.0,0.75)] {
            let player = AnimationNode::new("player", crate::SourceSequencePlayerNode {
                clip:clip.clone(), start_position:start, expose_start_position_pin:false, expose_play_rate_pin:false, expose_loop_animation_pin:false,
                curves:crate::als_curves::parse_curves(&serde_json::json!({"curves":[{"name":"Layer","times":[0.0],"values":[value],"interps":["CONSTANT"]}]})).unwrap(), ..default()
            });
            let mut inner = AnimationGraph::new();
            inner.add_node(player.clone());
            inner.add_output_data("Pose".to_owned(), DataSpec::Pose);
            inner.add_output_data_edge(player.id, "Pose", "Pose");
            let linked = AnimationNode::new("nested", LayeredGraphNode { capture_context:true, internalize_time:true, ..LayeredGraphNode::new(graphs.add(inner)) });
            let mut outer = AnimationGraph::new();
            outer.add_node(linked.clone());
            outer.add_output_data("Pose".to_owned(), DataSpec::Pose);
            outer.add_output_data_edge(linked.id, "Pose", "Pose");
            implementations.push(graphs.add(outer));
        }
        let selected = AnimationNode::new("layer", LayeredGraphNode { graph_binding:"layer".to_owned(), capture_context:true, internalize_time:true, ..LayeredGraphNode::new(implementations[0].clone()) });
        let sibling = AnimationNode::new("sibling", crate::SourceSequencePlayerNode { clip, expose_start_position_pin:false, expose_play_rate_pin:false, expose_loop_animation_pin:false, ..default() });
        let mut root = AnimationGraph::new();
        for (node, output) in [(&selected,"selected"), (&sibling,"sibling")] {
            root.add_node(node.clone());
            root.add_output_data(output.to_owned(), DataSpec::Pose);
            root.add_output_data_edge(node.id, "Pose", output);
        }
        let root = graphs.add(root);
        let mut world = World::new();
        let entities = [world.spawn_empty().id(), world.spawn_empty().id()];
        world.insert_resource(graphs);
        world.insert_resource(clips);
        world.insert_resource(Assets::<Skeleton>::default());
        world.insert_resource(Assets::<StateMachine>::default());
        world.run_system_once_with(query, (root, implementations[1].clone(), entities)).unwrap();
    }

    #[test]
    fn auxiliary_outputs_can_be_hidden_without_hiding_pose_or_explicit_data() {
        let node = LayeredGraphNode {
            capture_context: true,
            hide_context_auxiliary_outputs: true,
            exposed_output_data: vec!["ragdoll_config".to_owned()],
            ..LayeredGraphNode::new(default())
        };
        assert!(node.expose_output("pose", &DataSpec::Pose));
        assert!(node.expose_output("ragdoll_config", &DataSpec::RagdollConfig));
        assert!(!node.expose_output("ragdoll_dynamic", &DataSpec::Bool));
        assert!(!node.expose_output("PoseMoving", &DataSpec::F32));
    }

    #[test]
    fn global_curve_suffixes_retain_source_curve_pin_casing() {
        assert_eq!(source_curve_name("pose_moving"), "PoseMoving");
        assert_eq!(source_curve_name("foot_left_ik"), "FootLeftIk");
        assert_eq!(source_curve_name("rotation_yaw_speed"), "RotationYawSpeed");
    }

    #[test]
    fn source_pose_output_also_exposes_the_runtime_pose_alias() {
        assert!(LayeredGraphNode::exposes_source_pose_alias(
            "Pose",
            &DataSpec::Pose
        ));
        assert!(!LayeredGraphNode::exposes_source_pose_alias(
            "pose",
            &DataSpec::Pose
        ));
        assert!(!LayeredGraphNode::exposes_source_pose_alias(
            "Pose",
            &DataSpec::F32
        ));
    }
}
