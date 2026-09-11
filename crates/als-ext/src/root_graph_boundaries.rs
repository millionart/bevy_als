use bevy::{platform::collections::HashMap, prelude::*};
use bevy_animation_graph::core::{
    animation_node::{NodeLike, ReflectNodeLike},
    context::{new_context::NodeContext, spec_context::SpecContext},
    edge_data::{DataSpec, DataValue},
    errors::GraphError,
    pose::{BonePose, Pose},
    skeleton::Skeleton,
};

use crate::animation_context::{
    AnimationContextState, context_time_update, merge_missing_context_curves,
    pose_input_context_state, resolve_named_context_curves,
};
use crate::{
    control_rig_hierarchy::{RigHierarchy, RigSpace, RigTransform},
    control_rig_math::RigValue,
    control_rig_program::{RigDefinition, RigValueRef},
    control_rig_units::RigUnitContext,
    source_instance::source_input_data,
};

const SOURCE_TIME: &str = "source_time";
const OUT_POSE: &str = "Pose";
const LEGACY_OUT_POSE: &str = "pose";
pub const RIG_GAME_WORLD_KEY: &str = "control_rig.game_world";

#[derive(Reflect, Clone, Debug)]
pub struct RigInputBinding {
    pub external: String,
    pub path: Vec<usize>,
    pub property: String,
}

#[derive(Clone, Debug)]
struct ControlRigRuntime {
    hierarchy: RigHierarchy,
    values: Vec<RigValue>,
    curves: HashMap<String, f32>,
}

#[derive(Reflect, Clone, Debug, Default)]
pub struct ControlRigNodeState {
    pub context: AnimationContextState,
    pub initialization_count: u64,
    pub executed_steps: usize,
    #[reflect(ignore)]
    runtime: Option<ControlRigRuntime>,
}

fn rig_transform(transform: Transform) -> RigTransform {
    RigTransform {
        rotation: transform.rotation.as_dquat(),
        translation: transform.translation.as_dvec3(),
        scale: transform.scale.as_dvec3(),
    }
}

fn input_value(value: DataValue) -> Result<RigValue, String> {
    match value {
        DataValue::F32(value) => Ok(RigValue::F32(value)),
        DataValue::Bool(value) => Ok(RigValue::Bool(value)),
        DataValue::Vec3(value) => Ok(RigValue::Vector(value.as_dvec3())),
        DataValue::Quat(value) => Ok(RigValue::Quaternion(value.as_dquat())),
        _ => Err("rig external input type is unsupported".to_owned()),
    }
}

fn merge_transferred_input_context(
    transfer_input_curves: bool,
    target: &mut AnimationContextState,
    source: Option<AnimationContextState>,
) {
    if transfer_input_curves && let Some(source) = source {
        merge_missing_context_curves(target, &source);
    }
}

fn pose_spec(
    mut ctx: SpecContext,
    pose_input: &str,
    internalize_time: bool,
) -> Result<(), GraphError> {
    ctx.add_input_data(pose_input, DataSpec::Pose)
        .add_output_data(OUT_POSE, DataSpec::Pose)
        .add_output_data(LEGACY_OUT_POSE, DataSpec::Pose);
    if !internalize_time {
        ctx.add_input_time(SOURCE_TIME).add_output_time();
    }
    Ok(())
}

#[derive(Reflect, Clone, Debug)]
#[reflect(Default, NodeLike)]
#[type_path = "als_ext"]
pub struct ControlRigBoundaryNode {
    pub definition: Option<RigDefinition>,
    pub input_bindings: Vec<RigInputBinding>,
    pub delta_seconds_key: String,
    pub rig_class: String,
    pub alpha_enabled: bool,
    pub transfer_input_pose: bool,
    pub transfer_input_curves: bool,
    pub transfer_pose_in_global_space: bool,
    pub pose_input: String,
    pub curve_names: Vec<String>,
    pub capture_context: bool,
    pub expose_context_curves: bool,
    pub exposed_output_data: Vec<String>,
    pub missing_curve_value: Option<f32>,
    pub internalize_time: bool,
}

impl Default for ControlRigBoundaryNode {
    fn default() -> Self {
        Self {
            definition: None,
            input_bindings: Vec::new(),
            delta_seconds_key: "animation.delta_seconds".to_owned(),
            rig_class: "CR_Als".to_owned(),
            alpha_enabled: true,
            transfer_input_pose: true,
            transfer_input_curves: true,
            transfer_pose_in_global_space: false,
            pose_input: "source_pose".to_owned(),
            curve_names: default(),
            capture_context: false,
            expose_context_curves: false,
            exposed_output_data: default(),
            missing_curve_value: None,
            internalize_time: false,
        }
    }
}

impl ControlRigBoundaryNode {
    fn expose_output(&self, curve: &str) -> bool {
        !self.capture_context
            || self.expose_context_curves
            || self.exposed_output_data.iter().any(|value| value == curve)
    }

    fn solve(
        &self,
        definition: &RigDefinition,
        state: &mut ControlRigNodeState,
        pose: &mut Pose,
        skeleton: &Skeleton,
        inputs: &[(RigValueRef, RigValue)],
        game_world: bool,
        delta_seconds: f64,
        to_world: RigTransform,
        query: Option<&crate::control_rig_query::RigQueryContext>,
    ) -> Result<(), String> {
        state.executed_steps = 0;
        if state.runtime.is_none() {
            state.runtime = Some(ControlRigRuntime {
                hierarchy: RigHierarchy::new(
                    definition.bones.clone(),
                    definition.translation_tolerance,
                )?,
                values: definition.program.initial_values.clone(),
                curves: definition
                    .curves
                    .iter()
                    .filter_map(|(key, value)| value.map(|value| (key.clone(), value)))
                    .collect(),
            });
        }
        let runtime = state.runtime.as_mut().unwrap();
        for (binding, value) in inputs {
            binding.write(
                &mut runtime.values,
                definition.program.literal_count,
                value.clone(),
            )?;
        }
        let execute = |runtime: &mut ControlRigRuntime, entry: &str| {
            let mut trace = |start, end, channel: &str, complex| {
                query
                    .ok_or_else(|| "rig line trace is unavailable".to_owned())?
                    .trace(start, end, channel, complex)
            };
            let mut context = RigUnitContext {
                hierarchy: &mut runtime.hierarchy,
                curves: &runtime.curves,
                game_world,
                construction: false,
                delta_seconds,
                geometry: &definition.geometry,
                to_world,
                line_trace: query
                    .map(|_| &mut trace as &mut crate::control_rig_units::RigLineTrace<'_>),
            };
            definition
                .program
                .execute(entry, &mut runtime.values, |name, args| {
                    context.call(name, args)
                })
                .map_err(|error| {
                    format!(
                        "{error}; operation {:?}",
                        definition.program.instructions.get(error.instruction)
                    )
                })
        };
        if state.initialization_count == 0 {
            execute(runtime, "Construction")?;
            state.initialization_count += 1;
        }
        if !self.alpha_enabled {
            return Ok(());
        }
        if self.transfer_pose_in_global_space {
            return Err("rig global pose transfer is unsupported".to_owned());
        }
        let mut mapping = Vec::with_capacity(definition.bones.len());
        for bone in &definition.bones {
            let mut matches = skeleton.iter_paths().filter(|path| {
                path.parts
                    .last()
                    .is_some_and(|name| name.as_str().eq_ignore_ascii_case(&bone.name))
            });
            let matched = matches.next().map(|path| path.id());
            if matches.next().is_some() {
                return Err(format!("rig bone mapping is ambiguous: {}", bone.name));
            }
            mapping.push(matched);
        }
        if self.transfer_input_pose {
            let mut local = Vec::with_capacity(mapping.len());
            for (index, &id) in mapping.iter().enumerate() {
                let Some(id) = id else {
                    local.push(runtime.hierarchy.transform(index, RigSpace::Local, false)?);
                    continue;
                };
                if let Some(parent) = definition.bones[index].parent
                    && mapping[parent] != skeleton.parent(&id)
                {
                    return Err(format!(
                        "rig local pose parents do not match: {}",
                        definition.bones[index].name
                    ));
                }
                let defaults = skeleton
                    .default_transforms(id)
                    .ok_or("rig skeleton reference is missing")?;
                let transform = pose.get_bone(id).map_or(defaults.local, |bone| {
                    bone.to_transform_with_base(defaults.local)
                });
                local.push(rig_transform(transform));
            }
            runtime.hierarchy.set_input_pose(&local)?;
        }
        if self.transfer_input_curves {
            runtime.curves.clear();
            for name in definition.curves.keys() {
                if let Some(value) = state
                    .context
                    .curves
                    .iter()
                    .find_map(|(key, value)| key.eq_ignore_ascii_case(name).then_some(*value))
                {
                    runtime.curves.insert(name.clone(), value);
                }
            }
        }
        state.executed_steps = execute(runtime, "Forwards Solve")?;
        for (index, id) in mapping.into_iter().enumerate() {
            let Some(id) = id else {
                continue;
            };
            let transform = runtime.hierarchy.transform(index, RigSpace::Local, false)?;
            let mut output = BonePose::from_transform(Transform {
                rotation: transform.rotation.as_quat(),
                translation: transform.translation.as_vec3(),
                scale: transform.scale.as_vec3(),
            });
            if let Some(&index) = pose.paths.get(&id) {
                output.weights = pose.bones[index].weights.clone();
                pose.bones[index] = output;
            } else {
                pose.add_bone(output, id);
            }
        }
        Ok(())
    }
}

impl NodeLike for ControlRigBoundaryNode {
    fn display_name(&self) -> String {
        format!("ALS Control Rig {}", self.rig_class)
    }

    fn duration(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        if self.internalize_time {
            ctx.set_duration_fwd(None);
        } else {
            ctx.set_duration_fwd(ctx.duration_back(SOURCE_TIME)?);
        }
        Ok(())
    }

    fn update(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        let graph_id = ctx.graph_context.context_id;
        let node_id = ctx.node_id;
        let failure = |error: String| GraphError::UnknownErrorNode {
            graph: graph_id,
            node: node_id,
            error: format!("Control rig {}: {error}", self.rig_class),
        };
        let definition = self
            .definition
            .as_ref()
            .ok_or_else(|| failure("target program is missing".to_owned()))?;
        let inputs = self
            .input_bindings
            .iter()
            .map(|binding| {
                let mut target = definition
                    .externals
                    .get(&binding.external)
                    .cloned()
                    .ok_or_else(|| {
                        failure(format!(
                            "external variable is missing: {}",
                            binding.external
                        ))
                    })?;
                target.path.extend_from_slice(&binding.path);
                let value = source_input_data(&ctx, &binding.property)
                    .ok_or_else(|| GraphError::GlobalInputDataMissing(binding.property.clone()))?;
                Ok((target, input_value(value).map_err(&failure)?))
            })
            .collect::<Result<Vec<_>, GraphError>>()?;
        let game_world = match source_input_data(&ctx, RIG_GAME_WORLD_KEY) {
            Some(value) => value.as_bool()?,
            None => false,
        };
        let delta_seconds =
            crate::global_f32_input::global_f32_or_root_default(&ctx, &self.delta_seconds_key)?
                as f64;
        let query =
            crate::source_gameplay_tag_input::rig_query_context(ctx.graph_context.root_entity);
        let update = if self.internalize_time {
            context_time_update(&ctx)?
        } else {
            ctx.time_update_fwd()?
        };
        if !self.internalize_time {
            ctx.set_time_update_back(SOURCE_TIME, update);
        }
        let mut pose = ctx.data_back(self.pose_input.clone())?.into_pose()?;
        if self.transfer_input_pose
            && pose.bones.iter().all(|bone| {
                bone.translation.is_none() && bone.rotation.is_none() && bone.scale.is_none()
            })
        {
            return Err(failure("source pose has no transform channels".to_owned()));
        }
        let context_curves = if self.capture_context {
            resolve_named_context_curves(
                &ctx,
                &self.pose_input,
                &self.curve_names,
                self.missing_curve_value,
            )?
        } else {
            default()
        };
        let mut state = AnimationContextState::default();
        for curve in &self.curve_names {
            let value = if self.capture_context {
                *context_curves
                    .get(curve)
                    .ok_or(GraphError::MissingStateValue)?
            } else {
                ctx.data_back(curve.clone())?.as_f32()?
            };
            state.curves.insert(curve.clone(), value);
            if self.expose_output(curve) {
                ctx.set_data_fwd(curve.clone(), value);
            }
        }
        if self.capture_context {
            merge_transferred_input_context(
                self.transfer_input_curves,
                &mut state,
                pose_input_context_state(&ctx, &self.pose_input),
            );
        }
        let resources = ctx.graph_context.resources;
        let skeleton = resources
            .skeleton_assets
            .get(&pose.skeleton)
            .ok_or(GraphError::SkeletonMissing(ctx.node_id))?;
        let to_world = query
            .as_ref()
            .map(|query| query.to_world)
            .unwrap_or_else(|| {
                ctx.graph_context
                    .resources
                    .transform_query
                    .get(ctx.graph_context.root_entity)
                    .map(|(_, transform)| rig_transform(transform.compute_transform()))
                    .unwrap_or_default()
            });
        let node_state = ctx.state_mut::<ControlRigNodeState>()?;
        node_state.context = state;
        self.solve(
            definition,
            node_state,
            &mut pose,
            skeleton,
            &inputs,
            game_world,
            delta_seconds,
            to_world,
            query.as_ref(),
        )
        .map_err(failure)?;
        ctx.set_time(pose.timestamp);
        ctx.set_data_fwd(OUT_POSE, pose.clone());
        ctx.set_data_fwd(LEGACY_OUT_POSE, pose);
        Ok(())
    }

    fn spec(&self, mut ctx: SpecContext) -> Result<(), GraphError> {
        for curve in &self.curve_names {
            if !self.capture_context {
                ctx.add_input_data(curve.clone(), DataSpec::F32);
            }
            if self.expose_output(curve) {
                ctx.add_output_data(curve.clone(), DataSpec::F32);
            }
        }
        pose_spec(ctx, &self.pose_input, self.internalize_time)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn control_rig_transfer_input_curves_preserves_full_upstream_context() {
        let mut context = AnimationContextState::default();
        context.curves.insert("Explicit".to_owned(), 0.5);
        let mut source = AnimationContextState::default();
        source.curves.insert("Explicit".to_owned(), 0.25);
        source.curves.insert("Transferred".to_owned(), 0.75);
        merge_transferred_input_context(true, &mut context, Some(source));
        assert_eq!(context.curves.get("Explicit"), Some(&0.5));
        assert_eq!(context.curves.get("Transferred"), Some(&0.75));

        let mut ignored = AnimationContextState::default();
        ignored.curves.insert("Ignored".to_owned(), 1.0);
        merge_transferred_input_context(false, &mut context, Some(ignored));
        assert!(!context.curves.contains_key("Ignored"));
    }

    #[test]
    fn control_rig_boundary_exposes_the_source_pose_pin() {
        assert_eq!(OUT_POSE, "Pose");
        assert_eq!(LEGACY_OUT_POSE, "pose");
    }

    #[test]
    fn control_rig_boundary_registers_node_like_dispatch_key() {
        use bevy::reflect::TypeRegistry;

        let mut registry = TypeRegistry::default();
        registry.register::<ControlRigBoundaryNode>();
        assert!(
            registry
                .get(std::any::TypeId::of::<ControlRigBoundaryNode>())
                .unwrap()
                .data::<bevy_animation_graph::core::animation_node::ReflectNodeLike>()
                .is_some()
        );
    }

    #[test]
    fn control_rig_node_solves_the_input_pose_inside_the_graph() {
        use crate::control_rig_program::RigDefinition;
        use bevy::platform::collections::HashMap;
        use bevy_animation_graph::core::{
            animation_clip::{EntityPath, GraphClip},
            animation_graph::{AnimationGraph, TimeUpdate},
            animation_node::AnimationNode,
            context::{
                deferred_gizmos::DeferredGizmos, graph_context::QueryOutputTime,
                graph_context_arena::GraphContextArena, new_context::GraphContext,
                system_resources::SystemResources,
            },
            edge_data::DataValue,
            pose::{BoneId, BonePose, Pose, RootMotionDelta},
            skeleton::Skeleton,
            state_machine::high_level::StateMachine,
            utils::testing::TestIoEnv,
        };
        fn query(
            In((handle, inputs, foot_root)): In<(
                Handle<AnimationGraph>,
                HashMap<String, DataValue>,
                BoneId,
            )>,
            resources: SystemResources,
        ) -> Vec<Pose> {
            let graph = resources.animation_graph_assets.get(&handle).unwrap();
            let rig_node = graph
                .nodes
                .values()
                .find(|node| {
                    node.try_inner_downcast_ref::<ControlRigBoundaryNode>()
                        .is_some()
                })
                .unwrap();
            let definition = rig_node
                .try_inner_downcast_ref::<ControlRigBoundaryNode>()
                .unwrap()
                .definition
                .as_ref()
                .unwrap();
            let mut arenas = [
                GraphContextArena::new(handle.id()),
                GraphContextArena::new(handle.id()),
            ];
            let mut gizmos = DeferredGizmos::default();
            let entities = HashMap::default();
            let env = TestIoEnv::default();
            let mut outputs = Vec::new();
            for (frame, scale) in [Some(2.0), Some(1.0), Some(3.0), None]
                .into_iter()
                .enumerate()
            {
                for (instance, arena) in arenas.iter_mut().enumerate() {
                    let mut inputs = inputs.clone();
                    let mut pose = inputs["source"].clone().into_pose().unwrap();
                    pose.timestamp += frame as f32;
                    let index = pose.paths[&foot_root];
                    pose.bones[index].scale = scale.map(Vec3::splat);
                    pose.bones[index].translation = Some(Vec3::ZERO);
                    inputs.insert("source".into(), DataValue::Pose(pose));
                    let forward = if instance == 0 { 0.5 } else { 0.25 };
                    inputs.insert(
                        "source_frame_input.rig.forward".into(),
                        DataValue::F32(forward),
                    );
                    let context = GraphContext::new(
                        arena.get_toplevel_id(),
                        graph,
                        arena,
                        &resources,
                        &env,
                        Entity::PLACEHOLDER,
                        &entities,
                        &mut gizmos,
                        &inputs,
                    );
                    let output = graph
                        .query_with_context(
                            QueryOutputTime::Forced(TimeUpdate::Delta(0.02)),
                            context,
                        )
                        .unwrap()
                        .remove("Pose")
                        .unwrap()
                        .into_pose()
                        .unwrap();
                    let state = arena
                        .get_toplevel()
                        .node_states
                        .get::<ControlRigNodeState>(
                            rig_node.id,
                            bevy_animation_graph::core::context::node_states::StateKey::Default,
                        )
                        .unwrap();
                    assert_eq!(state.initialization_count, 1);
                    assert!(state.executed_steps > 50);
                    let mut value = definition.externals["RigInput"].clone();
                    value.path.push(3);
                    assert_eq!(
                        value.read(&state.runtime.as_ref().unwrap().values).unwrap(),
                        RigValue::F32(forward)
                    );
                    assert_eq!(state.context.curves["PoseMoving"], 1.0);
                    let foot_height = definition
                        .bones
                        .iter()
                        .find(|bone| bone.name == "foot_l")
                        .unwrap()
                        .initial_global
                        .translation
                        .y;
                    assert_eq!(
                        definition.externals["FootHeight"]
                            .read(&state.runtime.as_ref().unwrap().values)
                            .unwrap(),
                        RigValue::F64(foot_height)
                    );
                    let amount = if instance == 0 { 0.4 } else { 0.2 };
                    let scale = scale.unwrap_or(1.5);
                    assert!(
                        (output.get_bone(foot_root).unwrap().scale.unwrap()
                            - Vec3::new(scale + amount, scale, scale + amount))
                        .length()
                            < 1e-6
                    );
                    outputs.push(output);
                    arena.next_frame();
                    if frame == 1 {
                        arena.next_frame();
                        arena.next_frame();
                    }
                }
            }
            for (case, expected) in [
                (0, "rig line trace is unavailable"),
                (1, "source pose has no transform channels"),
                (2, "rig value assignment types do not match"),
            ] {
                let mut inputs = inputs.clone();
                match case {
                    0 => {
                        inputs.insert("rig.foot_valid".into(), DataValue::Bool(true));
                    }
                    1 => {
                        let mut pose = inputs["source"].clone().into_pose().unwrap();
                        pose.bones.clear();
                        pose.paths.clear();
                        inputs.insert("source".into(), DataValue::Pose(pose));
                    }
                    _ => {
                        inputs.insert("rig.forward".into(), DataValue::Bool(false));
                    }
                }
                let mut arena = GraphContextArena::new(handle.id());
                let context = GraphContext::new(
                    arena.get_toplevel_id(),
                    graph,
                    &mut arena,
                    &resources,
                    &env,
                    Entity::PLACEHOLDER,
                    &entities,
                    &mut gizmos,
                    &inputs,
                );
                let error = graph
                    .query_with_context(QueryOutputTime::Forced(TimeUpdate::Delta(0.02)), context)
                    .unwrap_err();
                let GraphError::UnknownErrorNode { node, error, .. } = error else {
                    panic!("rig failure lost its node identity")
                };
                assert_eq!(node, rig_node.id);
                assert!(error.contains(expected), "{error}");
                if case == 0 {
                    assert!(
                        error.contains("instruction 31")
                            && error.contains("FAlsRigUnit_FootOffsetTrace::Execute")
                    );
                }
            }
            let query_root = Entity::from_raw_u32(812907).unwrap();
            crate::source_gameplay_tag_input::set_rig_query_context(
                query_root,
                Some(crate::control_rig_query::RigQueryContext {
                    world: std::sync::Arc::new(crate::control_rig_query::RigQueryWorld {
                        trees: default(),
                        colliders: default(),
                    }),
                    to_world: default(),
                    channel: "ECC_Visibility".into(),
                    filter: default(),
                }),
            );
            let mut inputs = inputs.clone();
            inputs.insert("rig.foot_valid".into(), DataValue::Bool(true));
            let mut arena = GraphContextArena::new(handle.id());
            let context = GraphContext::new(
                arena.get_toplevel_id(),
                graph,
                &mut arena,
                &resources,
                &env,
                query_root,
                &entities,
                &mut gizmos,
                &inputs,
            );
            let output =
                graph.query_with_context(QueryOutputTime::Forced(TimeUpdate::Delta(0.02)), context);
            crate::source_gameplay_tag_input::set_rig_query_context(query_root, None);
            assert!(output.is_ok(), "{output:?}");
            assert!(crate::source_gameplay_tag_input::rig_query_context(query_root).is_none());
            outputs
        }
        let definition: RigDefinition = serde_json::from_str(include_str!(
            "../../../assets/control_rigs/cr_als.program.json"
        ))
        .unwrap();
        let mut skeleton = Skeleton::default();
        let mut paths: Vec<Vec<String>> = Vec::new();
        for bone in &definition.bones {
            let mut path = bone
                .parent
                .map(|parent| paths[parent].clone())
                .unwrap_or_default();
            path.push(bone.name.clone());
            let mut local = Transform {
                translation: bone.initial_local.translation.as_vec3(),
                rotation: bone.initial_local.rotation.as_quat(),
                scale: bone.initial_local.scale.as_vec3(),
            };
            if bone.name == "VB foot_root" {
                local.scale = Vec3::splat(1.5);
            } else if bone.name == "foot_l" {
                local.translation *= 2.0;
            }
            skeleton.add_bone(EntityPath::from(path.clone()), local, Transform::default());
            paths.push(path);
        }
        skeleton.set_root(EntityPath::from(paths[0].clone()).id());
        let foot_root = EntityPath::from(
            paths[definition
                .bones
                .iter()
                .position(|bone| bone.name == "VB foot_root")
                .unwrap()]
            .clone(),
        )
        .id();
        let mut world = World::new();
        world.insert_resource(Assets::<AnimationGraph>::default());
        world.insert_resource(Assets::<GraphClip>::default());
        world.insert_resource(Assets::<Skeleton>::default());
        world.insert_resource(Assets::<StateMachine>::default());
        let skeleton = world.resource_mut::<Assets<Skeleton>>().add(skeleton);
        let mut pose = Pose {
            skeleton,
            timestamp: 2.5,
            root_motion: Some(RootMotionDelta {
                translation: Vec3::X,
                rotation: Quat::from_rotation_y(0.4),
            }),
            ..default()
        };
        pose.add_bone(
            BonePose {
                scale: Some(Vec3::splat(2.0)),
                weights: Some(vec![0.25]),
                ..default()
            },
            foot_root,
        );
        let mut graph = AnimationGraph::new();
        let source = AnimationNode::new(
            "source",
            crate::GlobalPoseInputNode {
                key: "source".into(),
                output: "Pose".into(),
            },
        );
        let writer = AnimationNode::new(
            "late input write",
            crate::source_instance::SourcePropertyWriteNode {
                assignments: vec![crate::source_instance::SourcePropertyAssignment {
                    property: "rig.forward".into(),
                    value: DataValue::F32(0.9),
                }],
                ..default()
            },
        );
        let curve = AnimationNode::new(
            "curve",
            crate::GlobalF32InputNode {
                key: "moving".into(),
                ..default()
            },
        );
        let rig = AnimationNode::new(
            "Control Rig",
            ControlRigBoundaryNode {
                definition: Some(definition),
                input_bindings: vec![
                    RigInputBinding {
                        external: "RigInput".into(),
                        path: vec![3],
                        property: "rig.forward".into(),
                    },
                    RigInputBinding {
                        external: "RigInput".into(),
                        path: vec![2],
                        property: "rig.foot_valid".into(),
                    },
                ],
                curve_names: vec!["PoseMoving".into(), "FootLeftIk".into()],
                internalize_time: true,
                ..default()
            },
        );
        let foot_curve = AnimationNode::new(
            "foot curve",
            crate::GlobalF32InputNode {
                key: "foot_curve".into(),
            },
        );
        graph.add_node(source.clone());
        graph.add_node(writer.clone());
        graph.add_node(curve.clone());
        graph.add_node(foot_curve.clone());
        graph.add_node(rig.clone());
        graph.add_node_parameter_edge(source.id, "Pose", writer.id, "Source");
        graph.add_node_parameter_edge(writer.id, "Pose", rig.id, "source_pose");
        graph.add_node_parameter_edge(curve.id, "moving", rig.id, "PoseMoving");
        graph.add_node_parameter_edge(foot_curve.id, "foot_curve", rig.id, "FootLeftIk");
        graph.add_output_data("Pose".into(), DataSpec::Pose);
        graph.add_output_data_edge(rig.id, "Pose", "Pose");
        let handle = world.resource_mut::<Assets<AnimationGraph>>().add(graph);
        let inputs = HashMap::from([
            ("source".to_owned(), DataValue::Pose(pose.clone())),
            ("moving".to_owned(), DataValue::F32(1.0)),
            ("rig.forward".to_owned(), DataValue::F32(0.5)),
            ("rig.foot_valid".to_owned(), DataValue::Bool(false)),
            ("foot_curve".to_owned(), DataValue::F32(1.0)),
            ("animation.delta_seconds".to_owned(), DataValue::F32(0.02)),
            (RIG_GAME_WORLD_KEY.to_owned(), DataValue::Bool(true)),
        ]);
        let outputs = world
            .run_system_cached_with(query, (handle, inputs, foot_root))
            .unwrap();
        for (index, output) in outputs.into_iter().enumerate() {
            assert_eq!(output.timestamp, 2.5 + (index / 2) as f32);
            assert_eq!(output.root_motion, pose.root_motion);
            assert_eq!(
                output.get_bone(foot_root).unwrap().weights,
                Some(vec![0.25])
            );
        }
    }

    #[test]
    fn control_rig_node_definition_roundtrips_through_reflection() {
        use bevy::reflect::serde::{ReflectDeserializer, ReflectSerializer};
        use serde::de::DeserializeSeed;

        let definition: RigDefinition = serde_json::from_str(include_str!(
            "../../../assets/control_rigs/cr_als.program.json"
        ))
        .unwrap();
        let node = ControlRigBoundaryNode {
            definition: Some(definition),
            ..default()
        };
        let mut registry = bevy::reflect::TypeRegistry::default();
        registry.register::<ControlRigBoundaryNode>();
        let encoded = ron::ser::to_string(&ReflectSerializer::new(&node, &registry)).unwrap();
        let graph: bevy_animation_graph::core::animation_graph::serial::AnimationGraphDeserializer =
            ron::from_str(include_str!("../../../assets/generated/ALS/Character/AB_Als/AnimGraph.animgraph.ron")).unwrap();
        let generated = graph.nodes.iter().find(|node| node.inner.get_ron().contains("als_ext::ControlRigBoundaryNode")).unwrap();
        for (source, encoded) in [encoded.as_str(), generated.inner.get_ron()].into_iter().enumerate() {
            let mut deserializer = ron::de::Deserializer::from_str(encoded).unwrap();
            let reflected = ReflectDeserializer::new(&registry)
                .deserialize(&mut deserializer)
                .unwrap_or_else(|error| panic!("{}", deserializer.span_error(error)));
            let decoded = ControlRigBoundaryNode::from_reflect(reflected.as_partial_reflect()).unwrap();
            let actual = decoded.definition.expect("generated rig definition is missing");
            let expected = node.definition.as_ref().unwrap();
            assert_eq!(actual.bones.len(), expected.bones.len());
            for (actual, expected) in actual.bones.iter().zip(&expected.bones) {
                assert_eq!(serde_json::to_value(actual).unwrap(), serde_json::to_value(expected).unwrap(), "source {source}, bone {}", actual.name);
            }
            assert_eq!(actual.program, expected.program);
            assert_eq!(actual.externals, expected.externals);
            assert_eq!(actual.curves, expected.curves);
            assert_eq!(actual.translation_tolerance, expected.translation_tolerance);
            assert_eq!(serde_json::to_value(actual.geometry).unwrap(), serde_json::to_value(&expected.geometry).unwrap());
        }
    }
}
