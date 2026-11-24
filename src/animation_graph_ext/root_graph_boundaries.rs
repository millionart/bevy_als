use std::sync::{Arc, Weak};

use bevy::{asset::AssetId, math::DVec3};
use bevy::prelude::*;
use bevy_animation_graph::core::{
    animation_node::{NodeLike, ReflectNodeLike},
    context::{new_context::NodeContext, spec_context::SpecContext},
    edge_data::{DataSpec, DataValue},
    errors::GraphError,
    id::BoneId,
    pose::{BonePose, Pose},
    skeleton::Skeleton,
};

use crate::extensions::animation_context::{
    AnimationContextState, context_time_update, merge_missing_context_curves,
    pose_input_context_state, resolve_named_context_curves,
};
use crate::extensions::{
    als_curves::SourceCurveKey,
    control_rig_hierarchy::{RigHierarchy, RigSpace, RigTransform},
    control_rig_math::{RigValue, rotation_basis},
    control_rig_program::{RigDefinition, RigValueRef},
    control_rig_units::{RigUnitContext, call_unit},
    source_gameplay_tag_input::active_skeleton_layouts,
    source_instance::source_input_data,
    source_pose::SourceSkeletonLayouts,
};

const SOURCE_TIME: &str = "source_time";
const OUT_POSE: &str = "Pose";
const LEGACY_OUT_POSE: &str = "pose";
pub const RIG_GAME_WORLD_KEY: &str = "control_rig.game_world";
pub const RIG_CAPTURE_KEY: &str = "control_rig.capture";

#[derive(Clone, Debug)]
pub struct RigCallSample {
    pub instruction: usize,
    pub arguments: Vec<RigValue>,
    pub bones_before: Vec<(usize, RigTransform)>,
}

#[derive(Clone, Debug)]
pub struct RigTraceSample {
    pub instruction: usize,
    pub start: bevy::math::DVec3,
    pub end: bevy::math::DVec3,
    pub hit: Option<(bevy::math::DVec3, bevy::math::DVec3)>,
}

#[derive(Reflect, Clone, Debug)]
pub struct RigInputBinding {
    pub external: String,
    pub path: Vec<usize>,
    pub property: String,
}

#[derive(Clone, Debug)]
struct RigBoneBinding {
    skeleton: AssetId<Skeleton>,
    layouts: Weak<SourceSkeletonLayouts>,
    bones: Arc<[Option<BoneId>]>,
}

#[derive(Clone, Debug)]
struct ControlRigRuntime {
    definition: Arc<RigDefinition>,
    binding: Option<RigBoneBinding>,
    hierarchy: RigHierarchy,
    values: Vec<RigValue>,
    curves: crate::extensions::SourceCurveValues,
}

#[derive(Reflect, Clone, Debug, Default)]
pub struct ControlRigNodeState {
    pub context: AnimationContextState,
    pub initialization_count: u64,
    pub executed_steps: usize,
    pub to_world: RigTransform,
    pub elapsed_seconds: f64,
    #[reflect(ignore)]
    pub calls: Vec<RigCallSample>,
    #[reflect(ignore)]
    pub traces: Vec<RigTraceSample>,
    #[reflect(ignore)]
    runtime: Option<ControlRigRuntime>,
    #[reflect(ignore)]
    lod_bypassed: bool,
}

impl ControlRigNodeState {
    pub fn value(&self, reference: &RigValueRef) -> Result<RigValue, String> {
        reference.read(
            &self
                .runtime
                .as_ref()
                .ok_or("rig is not initialized")?
                .values,
        )
    }

    pub fn snapshot_hierarchy(&self) -> Option<RigHierarchy> {
        self.runtime
            .as_ref()
            .map(|runtime| runtime.hierarchy.clone())
    }

    pub fn meters_per_unit(&self) -> f64 {
        self.runtime
            .as_ref()
            .map_or(1.0, |runtime| runtime.definition.geometry.meters_per_unit)
    }
}

pub fn current_control_rig<'a>(
    player: &'a bevy_animation_graph::core::animation_graph_player::AnimationGraphPlayer,
    graphs: &'a Assets<bevy_animation_graph::core::animation_graph::AnimationGraph>,
) -> Option<(
    &'a bevy_animation_graph::core::context::graph_context::GraphState,
    &'a bevy_animation_graph::core::animation_node::AnimationNode,
    &'a ControlRigNodeState,
)> {
    use bevy_animation_graph::core::context::node_states::StateKey;
    let arena = player.get_context_arena()?;
    let mut found = None;
    for id in arena.iter_context_ids() {
        let Some(context) = arena.get_context(id) else {
            continue;
        };
        let Some(graph) = graphs.get(context.get_graph_id()) else {
            continue;
        };
        for node in graph.nodes.values() {
            if node
                .try_inner_downcast_ref::<ControlRigBoundaryNode>()
                .is_some()
                && context.node_caches.is_updated(node.id, StateKey::Default)
                && let Ok(state) = context
                    .node_states
                    .get::<ControlRigNodeState>(node.id, StateKey::Default)
                && !state.lod_bypassed
            {
                if found.is_some() {
                    return None;
                }
                found = Some((context, node, state));
            }
        }
    }
    found
}

fn rig_transform(transform: Transform) -> RigTransform {
    RigTransform {
        rotation: transform.rotation.as_dquat(),
        translation: transform.translation.as_dvec3(),
        scale: transform.scale.as_dvec3(),
    }
}

fn pose_rig_transform(transform: RigTransform, length_scale: f64) -> RigTransform {
    let [x, y, z] = transform.translation.to_array();
    let [sx, sy, sz] = transform.scale.to_array();
    RigTransform {
        translation: DVec3::new(x, z, y) * length_scale,
        rotation: rotation_basis(transform.rotation),
        scale: DVec3::new(sx, sz, sy),
    }
}

fn input_value(value: DataValue, meters_per_unit: f64) -> Result<RigValue, String> {
    match value {
        DataValue::F32(value) => Ok(RigValue::F32(value)),
        DataValue::Bool(value) => Ok(RigValue::Bool(value)),
        DataValue::Vec3(value) => Ok(RigValue::Vector(value.as_dvec3() * meters_per_unit.recip())),
        DataValue::Quat(value) => Ok(RigValue::Quaternion(value.as_dquat())),
        _ => Err("rig external input type is unsupported".to_owned()),
    }
}

fn merge_upstream_pose_context(
    target: &mut AnimationContextState,
    source: Option<AnimationContextState>,
) {
    if let Some(source) = source {
        merge_missing_context_curves(target, &source);
        if target.cached_pose_request_route.is_none() {
            target.cached_pose_request_route = source.cached_pose_request_route;
        }
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
    #[reflect(default)]
    pub lod_threshold: Option<usize>,
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
            lod_threshold: None,
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
        capture: bool,
    ) -> Result<(), String> {
        state.executed_steps = 0;
        state.to_world = to_world;
        state.calls.clear();
        state.traces.clear();
        let meters_per_unit = definition.geometry.meters_per_unit;
        let mut rig_to_world = to_world;
        rig_to_world.translation *= meters_per_unit.recip();
        let rebuild = state
            .runtime
            .as_ref()
            .is_none_or(|runtime| runtime.definition.as_ref() != definition);
        if rebuild {
            state.runtime = Some(ControlRigRuntime {
                definition: Arc::new(definition.clone()),
                binding: None,
                hierarchy: RigHierarchy::new(
                    definition.bones.clone(),
                    definition.translation_tolerance,
                )?,
                values: definition.program.initial_values.clone(),
                curves: definition
                    .curves
                    .iter()
                    .filter_map(|(key, value)| value.map(|value| (key.clone().into(), value)))
                    .collect(),
            });
        }
        let runtime = state.runtime.as_mut().unwrap();
        for (binding, value) in inputs {
            binding.write_in_units(
                &mut runtime.values,
                definition.program.literal_count,
                value.clone(),
                meters_per_unit,
            )?;
        }
        let calls = &mut state.calls;
        let traces = &mut state.traces;
        let mut execute = |runtime: &mut ControlRigRuntime, entry: &str| {
            let capture = capture && entry == "Forwards Solve";
            let instruction = std::cell::Cell::new(0);
            let mut trace = |start, end, channel: &str, complex| {
                let hit = query
                    .ok_or_else(|| "rig line trace is unavailable".to_owned())?
                    .trace_in_units(start, end, channel, complex, meters_per_unit)?;
                if capture {
                    traces.push(RigTraceSample {
                        instruction: instruction.get(),
                        start: start * meters_per_unit,
                        end: end * meters_per_unit,
                        hit: hit.map(|(point, normal)| (point * meters_per_unit, normal)),
                    });
                }
                Ok(hit)
            };
            let mut context = RigUnitContext {
                hierarchy: &mut runtime.hierarchy,
                curves: &runtime.curves,
                game_world,
                construction: false,
                delta_seconds,
                geometry: &definition.geometry,
                to_world: rig_to_world,
                line_trace: query.map(|_| {
                    &mut trace as &mut crate::extensions::control_rig_units::RigLineTrace<'_>
                }),
            };
            definition
                .program
                .execute_in_units(
                    entry,
                    &mut runtime.values,
                    meters_per_unit,
                    |pc, name, args| {
                        let bones_before = if capture {
                            instruction.set(pc);
                            let indices = args
                                .iter()
                                .filter_map(|value| {
                                    context.hierarchy.item_index(value).ok().flatten()
                                })
                                .collect::<Vec<_>>();
                            if indices.is_empty() {
                                Vec::new()
                            } else {
                                let mut snapshot = context.hierarchy.clone();
                                indices
                                    .into_iter()
                                    .filter_map(|index| {
                                        Some((
                                            index,
                                            snapshot
                                                .transform(index, RigSpace::Global, false)
                                                .ok()?,
                                        ))
                                    })
                                    .collect()
                            }
                        } else {
                            Vec::new()
                        };
                        let result = call_unit(&mut context, name, args);
                        if capture {
                            calls.push(RigCallSample {
                                instruction: pc,
                                arguments: args.to_vec(),
                                bones_before,
                            });
                        }
                        result
                    },
                )
                .map_err(|error| {
                    format!(
                        "{error}; operation {:?}",
                        definition.program.instructions.get(error.instruction)
                    )
                })
        };
        if rebuild {
            execute(runtime, "Construction")?;
            state.initialization_count += 1;
        }
        if !self.alpha_enabled {
            return Ok(());
        }
        if self.transfer_pose_in_global_space {
            return Err("rig global pose transfer is unsupported".to_owned());
        }
        let compile_mapping = || {
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
            Ok(mapping)
        };
        let cached;
        let compiled;
        let mapping = if let Some(layouts) = active_skeleton_layouts() {
            let skeleton = pose.skeleton.id();
            if runtime.binding.as_ref().is_none_or(|binding| {
                binding.skeleton != skeleton || binding.layouts.as_ptr() != Arc::as_ptr(&layouts)
            }) {
                runtime.binding = Some(RigBoneBinding {
                    skeleton,
                    layouts: Arc::downgrade(&layouts),
                    bones: Arc::from(compile_mapping()?),
                });
            }
            cached = Arc::clone(&runtime.binding.as_ref().unwrap().bones);
            cached.as_ref()
        } else {
            compiled = compile_mapping()?;
            compiled.as_slice()
        };
        let mut source_pose = state.context.source_pose.take().map(Arc::unwrap_or_clone);
        let source_indices: bevy::platform::collections::HashMap<_, _> = source_pose
            .iter()
            .flat_map(|pose| {
                pose.bones
                    .iter()
                    .enumerate()
                    .map(|(index, (bone, _))| (*bone, index))
            })
            .collect();
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
                if let Some(source) = &source_pose
                    && let Some(&index) = source_indices.get(&id)
                {
                    local.push(pose_rig_transform(
                        source.bones[index].1,
                        source.length_scale / meters_per_unit,
                    ));
                    continue;
                }
                let defaults = skeleton
                    .default_transforms(id)
                    .ok_or("rig skeleton reference is missing")?;
                let transform = pose.get_bone(id).map_or(defaults.local, |bone| {
                    bone.to_transform_with_base(defaults.local)
                });
                let mut transform = rig_transform(transform);
                transform.translation *= meters_per_unit.recip();
                local.push(transform);
            }
            runtime.hierarchy.set_input_pose(&local)?;
        }
        if self.transfer_input_curves {
            runtime.curves.clear();
            for name in definition.curves.keys() {
                if let Some((key, value)) = state.context.curves.get_key_value(&SourceCurveKey(name))
                {
                    runtime.curves.insert(key.clone(), *value);
                }
            }
        }
        state.executed_steps = execute(runtime, "Forwards Solve")?;
        let omitted = crate::extensions::source_gameplay_tag_input::active_omitted_bones(pose.skeleton.id());
        for (index, &id) in mapping.iter().enumerate() {
            let Some(id) = id else {
                continue;
            };
            if omitted.as_ref().is_some_and(|omitted| omitted.contains(&id)) {
                continue;
            }
            let transform = runtime.hierarchy.transform(index, RigSpace::Local, false)?;
            if let Some(source) = &mut source_pose
                && let Some(&index) = source_indices.get(&id)
            {
                source.bones[index].1 =
                    pose_rig_transform(transform, meters_per_unit / source.length_scale);
            }
            let mut output = BonePose::from_transform(Transform {
                rotation: transform.rotation.as_quat(),
                translation: (transform.translation * meters_per_unit).as_vec3(),
                scale: transform.scale.as_vec3(),
            });
            if let Some(&index) = pose.paths.get(&id) {
                output.weights = pose.bones[index].weights.clone();
                pose.bones[index] = output;
            } else {
                pose.add_bone(output, id);
            }
        }
        state.context.source_pose = source_pose
            .filter(|source| source.matches_projection(pose))
            .map(Arc::new);
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
        let meters_per_unit = definition.geometry.meters_per_unit;
        if !meters_per_unit.is_finite() || meters_per_unit <= 0.0 {
            return Err(failure("rig length unit is invalid".to_owned()));
        }
        let lod_allows_rig = crate::extensions::source_gameplay_tag_input::active_lod_allows_control_rig(self.lod_threshold);
        let foot_locations =
            crate::extensions::source_gameplay_tag_input::active_rig_foot_locations_cm(
                ctx.graph_context.root_entity,
            );
        let inputs = self
            .input_bindings
            .iter()
            .filter(|_| lod_allows_rig)
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
                let foot_location =
                    foot_locations.and_then(|locations| match binding.property.as_str() {
                        "control_rig_input.foot_left_location" => Some(locations[0]),
                        "control_rig_input.foot_right_location" => Some(locations[1]),
                        _ => None,
                    });
                if let Some(location) = foot_location {
                    let units_per_centimeter = 0.01 / meters_per_unit;
                    return Ok((
                        target,
                        RigValue::Vector(location.as_dvec3() * units_per_centimeter),
                    ));
                }
                let value = source_input_data(&ctx, &binding.property)
                    .ok_or_else(|| GraphError::GlobalInputDataMissing(binding.property.clone()))?;
                Ok((
                    target,
                    input_value(value, meters_per_unit).map_err(&failure)?,
                ))
            })
            .collect::<Result<Vec<_>, GraphError>>()?;
        let game_world = match source_input_data(&ctx, RIG_GAME_WORLD_KEY) {
            Some(value) => value.as_bool()?,
            None => false,
        };
        let capture = match source_input_data(&ctx, RIG_CAPTURE_KEY) {
            Some(value) => value.as_bool()?,
            None => false,
        };
        let delta_seconds = crate::extensions::global_f32_input::global_f32_or_root_default(
            &ctx,
            &self.delta_seconds_key,
        )? as f64;
        let query = lod_allows_rig.then(|| crate::extensions::source_gameplay_tag_input::active_rig_query_context(ctx.graph_context.root_entity)).flatten();
        let update = if self.internalize_time {
            context_time_update(&ctx)?
        } else {
            ctx.time_update_fwd()?
        };
        if !self.internalize_time {
            ctx.set_time_update_back(SOURCE_TIME, update.clone());
        }
        let (mut pose, notify_aliases) =
            super::pre_rig_sharing::source_pose(&ctx, &self.pose_input, &update)?;
        let preview = crate::extensions::sync_group_phase::source_sync_preview(&ctx);
        if !preview
            && self.transfer_input_pose
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
            state.curves.insert(curve.clone().into(), value);
            if self.expose_output(curve) {
                ctx.set_data_fwd(curve.clone(), value);
            }
        }
        let mut upstream = pose_input_context_state(&ctx, &self.pose_input);
        if let Some(context) = &mut upstream {
            for sample in &mut context.asset_samples {
                if let Some((_, local)) = notify_aliases
                    .iter()
                    .find(|(source, _)| *source == sample.notify_instance_id)
                {
                    sample.notify_instance_id = *local;
                }
            }
        }
        state.source_pose = upstream
            .as_ref()
            .and_then(|context| context.source_pose_for(&pose))
            .cloned();
        merge_upstream_pose_context(&mut state, upstream);
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
        node_state.lod_bypassed = !lod_allows_rig;
        if !preview && lod_allows_rig {
            let started = capture.then(std::time::Instant::now);
            let result = self.solve(
                definition,
                node_state,
                &mut pose,
                skeleton,
                &inputs,
                game_world,
                delta_seconds,
                to_world,
                query.as_ref(),
                capture,
            );
            node_state.elapsed_seconds = started.map_or(0.0, |start| start.elapsed().as_secs_f64());
            result.map_err(failure)?;
        } else if !preview {
            node_state.executed_steps = 0;
            node_state.elapsed_seconds = 0.0;
            node_state.calls.clear();
            node_state.traces.clear();
        }
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
    use crate::extensions::control_rig_program::{RigInstruction, RigProgram};

    fn hot_swap_definition() -> RigDefinition {
        let mut definition: RigDefinition = serde_json::from_str(include_str!(
            "../tests/fixtures/rig/cr_als_program.json"
        ))
        .unwrap();
        definition.program = RigProgram {
            initial_values: vec![RigValue::F32(1.0)],
            literal_count: 1,
            entries: [
                ("Construction".to_owned(), 0),
                ("Forwards Solve".to_owned(), 1),
            ]
            .into(),
            instructions: vec![RigInstruction::Exit, RigInstruction::Exit],
        };
        definition.bones.clear();
        definition.curves.clear();
        definition.externals.clear();
        definition
    }

    #[test]
    fn control_rig_rebinds_bones_after_skeleton_reload() {
        use crate::extensions::{SourceAnimationInputs, control_rig_hierarchy::RigBone};
        use bevy_animation_graph::core::animation_clip::EntityPath;

        let mut definition = hot_swap_definition();
        definition.bones.push(RigBone {
            name: "root".into(),
            parent: None,
            initial_local: default(),
            initial_global: default(),
        });
        let make_skeleton = |armature: &'static str| {
            let mut skeleton = Skeleton::default();
            let path = EntityPath::default().child(armature).child("root");
            let id = path.id();
            skeleton.add_bone(path, Transform::IDENTITY, Transform::IDENTITY);
            skeleton.set_root(id);
            (id, skeleton)
        };
        let (original, skeleton) = make_skeleton("original");
        let mut assets = Assets::default();
        let handle = assets.add(skeleton);
        let node = ControlRigBoundaryNode::default();
        let mut state = ControlRigNodeState::default();
        let mut solve = |assets: &Assets<Skeleton>| {
            let layouts = Arc::new(SourceSkeletonLayouts::new(assets));
            let mut pose = Pose {
                skeleton: handle.clone(),
                ..default()
            };
            SourceAnimationInputs::default().evaluate_with_skeleton_layouts(
                Entity::PLACEHOLDER,
                &layouts,
                None,
                None,
                None,
                || {
                    node.solve(
                        &definition,
                        &mut state,
                        &mut pose,
                        assets.get(&handle).unwrap(),
                        &[],
                        false,
                        0.0,
                        RigTransform::default(),
                        None,
                        false,
                    )
                    .unwrap();
                },
            );
            pose
        };

        assert!(solve(&assets).get_bone(original).is_some());
        let (replacement, skeleton) = make_skeleton("replacement");
        assets.insert(handle.id(), skeleton).unwrap();
        let pose = solve(&assets);
        assert!(pose.get_bone(replacement).is_some());
        assert!(pose.get_bone(original).is_none());
    }

    #[test]
    fn loaded_control_rig_uses_generated_initial_globals() {
        use bevy_animation_graph::core::{
            animated_scene::AnimatedSceneInstance, animation_graph::AnimationGraph,
        };

        let (app, _, avatar) = crate::roll_drive::tests::loaded_roll_character();
        let player = app
            .world()
            .get::<AnimatedSceneInstance>(avatar)
            .unwrap()
            .player_entity();
        let player = app
            .world()
            .get::<bevy_animation_graph::core::animation_graph_player::AnimationGraphPlayer>(player)
            .unwrap();
        let graphs = app.world().resource::<Assets<AnimationGraph>>();
        let (_, node, _) = current_control_rig(player, graphs).unwrap();
        let node = node
            .try_inner_downcast_ref::<ControlRigBoundaryNode>()
            .unwrap();
        let definition: RigDefinition = serde_json::from_str(include_str!(
            "../tests/fixtures/rig/rig_program_centimeters.json"
        ))
        .unwrap();
        assert_eq!(node.definition.as_ref().unwrap().bones, definition.bones);
    }

    #[test]
    fn control_rig_local_transfer_retains_source_centimeters_until_output() {
        use crate::extensions::{control_rig_hierarchy::RigBone, source_pose::SourcePose};
        use bevy_animation_graph::core::animation_clip::EntityPath;

        let mut definition = hot_swap_definition();
        definition.geometry.meters_per_unit = 0.01;
        definition.bones.push(RigBone {
            name: "root".into(),
            parent: None,
            initial_local: default(),
            initial_global: default(),
        });
        let path = EntityPath::default().child("root");
        let root = path.id();
        let mut skeleton = Skeleton::default();
        skeleton.add_bone(path, Transform::IDENTITY, Transform::IDENTITY);
        skeleton.set_root(root);
        let transform = RigTransform {
            translation: DVec3::new(0.1352923745746292, -3.420399962835745, 94.18533515463514),
            ..default()
        };
        let source = Arc::new(SourcePose {
            bones: vec![(root, transform)],
            length_scale: 0.01,
        });
        let mut pose = Pose::default();
        source.project_into(&mut pose);
        let mut state = ControlRigNodeState::default();
        state.context.source_pose = Some(source);
        ControlRigBoundaryNode::default()
            .solve(
                &definition,
                &mut state,
                &mut pose,
                &skeleton,
                &[],
                false,
                0.0,
                RigTransform::default(),
                None,
                false,
            )
            .unwrap();
        let actual = state
            .snapshot_hierarchy()
            .unwrap()
            .transform(0, RigSpace::Local, false)
            .unwrap();
        assert_eq!(actual, pose_rig_transform(transform, 1.0));
        assert_eq!(
            state.context.source_pose_for(&pose).unwrap().bones[0].1,
            transform
        );
    }

    #[test]
    fn control_rig_hot_swap_rebuilds_values_and_hierarchy() {
        let mut definition = hot_swap_definition();
        let node = ControlRigBoundaryNode {
            transfer_input_pose: false,
            transfer_input_curves: false,
            ..default()
        };
        let mut state = ControlRigNodeState::default();
        let mut pose = Pose::default();
        let skeleton = Skeleton::default();
        node.solve(
            &definition,
            &mut state,
            &mut pose,
            &skeleton,
            &[],
            false,
            1.0 / 60.0,
            RigTransform::default(),
            None,
            false,
        )
        .unwrap();
        assert_eq!(
            state
                .value(&RigValueRef {
                    register: 0,
                    path: vec![]
                })
                .unwrap(),
            RigValue::F32(1.0)
        );
        node.solve(
            &definition,
            &mut state,
            &mut pose,
            &skeleton,
            &[],
            false,
            1.0 / 60.0,
            RigTransform::default(),
            None,
            false,
        )
        .unwrap();
        assert_eq!(state.initialization_count, 1);
        let snapshot = state.clone();
        assert!(Arc::ptr_eq(
            &state.runtime.as_ref().unwrap().definition,
            &snapshot.runtime.as_ref().unwrap().definition,
        ));
        let source: RigDefinition = serde_json::from_str(include_str!(
            "../tests/fixtures/rig/cr_als_program.json"
        ))
        .unwrap();
        let bone = source.bones[0].clone();
        definition.program.initial_values[0] = RigValue::F32(2.0);
        definition.bones.push(bone.clone());
        node.solve(
            &definition,
            &mut state,
            &mut pose,
            &skeleton,
            &[],
            false,
            1.0 / 60.0,
            RigTransform::default(),
            None,
            false,
        )
        .unwrap();
        assert_eq!(
            state
                .value(&RigValueRef {
                    register: 0,
                    path: vec![]
                })
                .unwrap(),
            RigValue::F32(2.0)
        );
        assert!(
            state
                .snapshot_hierarchy()
                .unwrap()
                .index(&bone.name)
                .is_some()
        );
        assert_eq!(state.initialization_count, 2);
        assert_eq!(snapshot.initialization_count, 1);
        assert_eq!(
            snapshot.runtime.as_ref().unwrap().values[0],
            RigValue::F32(1.0),
        );
    }

    #[test]
    fn control_rig_hot_swap_rebuilds_register_layout() {
        let mut definition = hot_swap_definition();
        let node = ControlRigBoundaryNode {
            transfer_input_pose: false,
            transfer_input_curves: false,
            ..default()
        };
        let mut state = ControlRigNodeState::default();
        let mut pose = Pose::default();
        let skeleton = Skeleton::default();
        node.solve(
            &definition,
            &mut state,
            &mut pose,
            &skeleton,
            &[],
            false,
            1.0 / 60.0,
            RigTransform::default(),
            None,
            false,
        )
        .unwrap();
        definition.program.initial_values.push(RigValue::F32(3.0));
        definition.program.literal_count = 2;
        node.solve(
            &definition,
            &mut state,
            &mut pose,
            &skeleton,
            &[],
            false,
            1.0 / 60.0,
            RigTransform::default(),
            None,
            false,
        )
        .unwrap();
        assert_eq!(
            state
                .value(&RigValueRef {
                    register: 1,
                    path: vec![]
                })
                .unwrap(),
            RigValue::F32(3.0)
        );
        assert_eq!(state.initialization_count, 2);
    }

    #[test]
    fn disabled_rig_input_curves_keep_noncurve_animation_context() {
        use crate::extensions::animation_context::{
            SourceAssetSample, SourceInertializationRequest, SourceRelevantAsset,
        };
        let mut source = AnimationContextState {
            relevant_asset: Some(SourceRelevantAsset {
                duration: 1.0,
                position: 0.4,
                previous_position: 0.3,
                has_previous_position: true,
                delta_seconds: 0.1,
                playback_delta_seconds: 0.1,
                weight: 1.0,
                ..default()
            }),
            asset_samples: vec![SourceAssetSample {
                identity: "Footstep".into(),
                notify_instance_id: 42,
                clock: SourceRelevantAsset {
                    position: 0.4,
                    previous_position: 0.3,
                    weight: 1.0,
                    ..default()
                },
                ..default()
            }],
            inertialization_request: Some(SourceInertializationRequest {
                frame: 8,
                duration: 0.2,
                ..default()
            }),
            skipped_cached_pose_request: Some(SourceInertializationRequest {
                frame: 8,
                duration: 0.3,
                ..default()
            }),
            ..default()
        };
        source.curves.insert("RigInput".into(), 0.75);
        let mut target = AnimationContextState::default();
        merge_upstream_pose_context(&mut target, Some(source));
        assert_eq!(
            target.asset_samples.len(),
            1,
            "Disabling rig input curves lost the upstream notify sample"
        );
        assert_eq!(target.asset_samples[0].notify_instance_id, 42);
        assert_eq!(target.relevant_asset.unwrap().position, 0.4);
        assert_eq!(target.inertialization_request.unwrap().frame, 8);
        assert_eq!(target.skipped_cached_pose_request.unwrap().duration, 0.3);
    }

    #[test]
    fn rig_curve_transfer_only_controls_hierarchy_input_during_graph_evaluation() {
        use bevy::ecs::system::RunSystemOnce;
        use bevy::platform::collections::HashMap;
        use bevy_animation_graph::core::animation_graph::GraphInputPin;
        use bevy_animation_graph::core::{
            animation_clip::GraphClip,
            animation_graph::{AnimationGraph, TimeUpdate},
            animation_node::AnimationNode,
            context::{
                deferred_gizmos::DeferredGizmos, graph_context::QueryOutputTime,
                graph_context_arena::GraphContextArena, new_context::GraphContext,
                system_resources::SystemResources,
            },
            state_machine::high_level::StateMachine,
        };
        struct PoseInputEnv(Pose);
        impl bevy_animation_graph::core::context::io_env::GraphIoEnv for PoseInputEnv {
            fn get_data_back(
                &self,
                pin: GraphInputPin,
                _: GraphContext,
            ) -> Result<DataValue, GraphError> {
                assert_eq!(pin, GraphInputPin::passthrough("source"));
                Ok(DataValue::Pose(self.0.clone()))
            }
            fn get_duration_back(
                &self,
                _: GraphInputPin,
                _: GraphContext,
            ) -> Result<bevy_animation_graph::core::duration_data::DurationData, GraphError>
            {
                Ok(None)
            }
            fn get_time_fwd(&self, _: GraphContext) -> Result<TimeUpdate, GraphError> {
                Ok(TimeUpdate::Delta(0.02))
            }
        }
        fn query(
            In((handle, skeleton)): In<(Handle<AnimationGraph>, Handle<Skeleton>)>,
            resources: SystemResources,
        ) {
            let graph = resources.animation_graph_assets.get(&handle).unwrap();
            let node = graph
                .nodes
                .values()
                .find(|node| {
                    node.try_inner_downcast_ref::<ControlRigBoundaryNode>()
                        .is_some()
                })
                .unwrap();
            let transfer = node
                .try_inner_downcast_ref::<ControlRigBoundaryNode>()
                .unwrap()
                .transfer_input_curves;
            let mut arena = GraphContextArena::new(handle.id());
            let env = PoseInputEnv(Pose {
                skeleton: skeleton.clone(),
                ..default()
            });
            let entities = HashMap::default();
            let mut gizmos = DeferredGizmos::default();
            let inputs = HashMap::from([
                (
                    "source".into(),
                    DataValue::Pose(Pose {
                        skeleton: skeleton.clone(),
                        ..default()
                    }),
                ),
                ("animation.delta_seconds".into(), DataValue::F32(0.02)),
            ]);
            let source_inputs = crate::extensions::SourceAnimationInputs::default();
            for frame in 0..3 {
                let mut context = GraphContext::new(
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
                let mut upstream = AnimationContextState::default();
                upstream.curves.insert("RigInput".into(), 0.75);
                upstream.relevant_asset =
                    Some(crate::extensions::animation_context::SourceRelevantAsset {
                        duration: 1.0,
                        position: frame as f32 * 0.02,
                        delta_seconds: 0.02,
                        weight: 1.0,
                        ..default()
                    });
                upstream.skipped_cached_pose_request = Some(
                    crate::extensions::animation_context::SourceInertializationRequest {
                        frame,
                        duration: 0.3,
                        ..default()
                    },
                );
                let route = crate::extensions::animation_context::SourceCachedPoseRequestRoute {
                    frame,
                    caller: (
                        context.context_id,
                        node.id,
                        bevy_animation_graph::core::context::node_states::StateKey::Default,
                    ),
                    forward: true,
                };
                upstream.cached_pose_request_route = Some(route);
                upstream.asset_samples.push(
                    crate::extensions::animation_context::SourceAssetSample {
                        identity: "Footstep".into(),
                        notify_instance_id: 42,
                        clock: crate::extensions::animation_context::SourceRelevantAsset {
                            weight: 1.0,
                            position: frame as f32 * 0.02,
                            ..default()
                        },
                        ..default()
                    },
                );
                upstream.inertialization_request = Some(
                    crate::extensions::animation_context::SourceInertializationRequest {
                        frame,
                        duration: 0.2,
                        ..default()
                    },
                );
                for (identity, role) in [
                    (
                        node.id,
                        crate::extensions::sync_group_phase::SourceSyncGroupRole::AlwaysLeader,
                    ),
                    (
                        bevy_animation_graph::core::animation_graph::NodeId::default(),
                        crate::extensions::sync_group_phase::SourceSyncGroupRole::AlwaysFollower,
                    ),
                ] {
                    upstream.sync_players.push(
                        crate::extensions::sync_group_phase::SourceSyncPlayerTick {
                            identity: (context.context_id, identity),
                            key: "Locomotion".into(),
                            role,
                            weight: 1.0,
                            phase: frame as f32 * 0.02,
                            ..default()
                        },
                    );
                }
                crate::extensions::animation_context::write_linked_input_context(
                    &mut context,
                    GraphInputPin::passthrough("source"),
                    upstream,
                )
                .unwrap();
                source_inputs
                    .evaluate(context.root_entity, || {
                        graph.query_with_context(
                            QueryOutputTime::Forced(TimeUpdate::Delta(0.02)),
                            context,
                        )
                    })
                    .unwrap();
                let state = arena
                    .get_toplevel()
                    .node_states
                    .get::<ControlRigNodeState>(
                        node.id,
                        bevy_animation_graph::core::context::node_states::StateKey::Default,
                    )
                    .unwrap();
                assert_eq!(
                    state
                        .runtime
                        .as_ref()
                        .unwrap()
                        .curves
                        .get(&crate::extensions::SourceCurveName::from("RigInput"))
                        .copied(),
                    if transfer { Some(0.75) } else { None }
                );
                assert_eq!(
                    state.context.curves[&crate::extensions::SourceCurveName::from("RigInput")],
                    0.75
                );
                assert_eq!(state.context.asset_samples.len(), 1);
                assert_eq!(state.context.asset_samples[0].notify_instance_id, 42);
                assert_eq!(state.context.sync_players.len(), 2);
                assert_eq!(state.context.sync_players[0].phase, frame as f32 * 0.02);
                assert_eq!(
                    state.context.sync_players[1].role,
                    crate::extensions::sync_group_phase::SourceSyncGroupRole::AlwaysFollower
                );
                assert_eq!(
                    state
                        .context
                        .inertialization_request
                        .as_ref()
                        .unwrap()
                        .frame,
                    frame
                );
                assert_eq!(
                    state
                        .context
                        .skipped_cached_pose_request
                        .as_ref()
                        .unwrap()
                        .frame,
                    frame
                );
                assert_eq!(
                    state.context.relevant_asset.as_ref().unwrap().position,
                    frame as f32 * 0.02
                );
                assert_eq!(state.context.cached_pose_request_route, Some(route));
                arena.next_frame();
            }
        }
        for capture_context in [false, true] {
            for transfer_input_curves in [false, true] {
                let mut world = World::new();
                world.insert_resource(Assets::<AnimationGraph>::default());
                world.insert_resource(Assets::<GraphClip>::default());
                world.insert_resource(Assets::<Skeleton>::default());
                world.insert_resource(Assets::<StateMachine>::default());
                let skeleton = world
                    .resource_mut::<Assets<Skeleton>>()
                    .add(Skeleton::default());
                let mut definition = hot_swap_definition();
                definition.curves.insert("RigInput".into(), None);
                let rig = AnimationNode::new(
                    "Control Rig",
                    ControlRigBoundaryNode {
                        definition: Some(definition),
                        transfer_input_pose: false,
                        transfer_input_curves,
                        capture_context,
                        internalize_time: true,
                        ..default()
                    },
                );
                let source = AnimationNode::new(
                    "Source pose",
                    crate::extensions::source_pose_passthrough::SourcePosePassthroughNode {
                        output_time: false,
                        ..default()
                    },
                );
                let mut graph = AnimationGraph::new();
                graph.add_node(source.clone());
                graph.add_node(rig.clone());
                graph.add_input_data_edge(
                    GraphInputPin::passthrough("source"),
                    source.id,
                    "Source",
                );
                graph.add_node_parameter_edge(source.id, "Pose", rig.id, "source_pose");
                graph.add_output_data("Pose".into(), DataSpec::Pose);
                graph.add_output_data_edge(rig.id, "Pose", "Pose");
                let handle = world.resource_mut::<Assets<AnimationGraph>>().add(graph);
                world
                    .run_system_once_with(query, (handle, skeleton))
                    .unwrap();
            }
        }
    }

    #[test]
    fn control_rig_transfer_input_curves_preserves_full_upstream_context() {
        let mut context = AnimationContextState::default();
        context.curves.insert("Explicit".into(), 0.5);
        let mut source = AnimationContextState::default();
        source.curves.insert("Explicit".into(), 0.25);
        source.curves.insert("Transferred".into(), 0.75);
        merge_upstream_pose_context(&mut context, Some(source));
        assert_eq!(
            context
                .curves
                .get(&crate::extensions::SourceCurveName::from("Explicit")),
            Some(&0.5)
        );
        assert_eq!(
            context
                .curves
                .get(&crate::extensions::SourceCurveName::from("Transferred")),
            Some(&0.75)
        );

        let mut ignored = AnimationContextState::default();
        ignored.curves.insert("Ignored".into(), 1.0);
        merge_upstream_pose_context(&mut context, Some(ignored));
        assert_eq!(
            context
                .curves
                .get(&crate::extensions::SourceCurveName::from("Ignored")),
            Some(&1.0)
        );
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
        use crate::extensions::control_rig_program::RigDefinition;
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
            let mut source_inputs = crate::extensions::SourceAnimationInputs::default();
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
                    let capture = instance == 0 && frame % 2 == 0;
                    inputs.insert(RIG_CAPTURE_KEY.into(), DataValue::Bool(capture));
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
                    let output = source_inputs
                        .evaluate(context.root_entity, || {
                            graph.query_with_context(
                                QueryOutputTime::Forced(TimeUpdate::Delta(0.02)),
                                context,
                            )
                        })
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
                    assert_eq!(!state.calls.is_empty(), capture);
                    assert_eq!(state.elapsed_seconds > 0.0, capture);
                    assert!(state.traces.is_empty());
                    let mut value = definition.externals["RigInput"].clone();
                    value.path.push(3);
                    assert_eq!(state.value(&value).unwrap(), RigValue::F32(forward));
                    assert_eq!(
                        state.context.curves
                            [&crate::extensions::SourceCurveName::from("PoseMoving")],
                        1.0
                    );
                    let foot_height = definition
                        .bones
                        .iter()
                        .find(|bone| bone.name == "foot_l")
                        .unwrap()
                        .initial_global
                        .translation
                        .y;
                    assert_eq!(
                        state.value(&definition.externals["FootHeight"]).unwrap(),
                        RigValue::Centimeters64(
                            foot_height * (definition.geometry.meters_per_unit * 100.0)
                        )
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
                let error = source_inputs
                    .evaluate(context.root_entity, || {
                        graph.query_with_context(
                            QueryOutputTime::Forced(TimeUpdate::Delta(0.02)),
                            context,
                        )
                    })
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
            source_inputs.set_rig_query_context(Some(crate::control_rig_query::RigQueryContext {
                world: std::sync::Arc::new(crate::control_rig_query::RigQueryWorld::default()),
                to_world: default(),
                channel: "ECC_Visibility".into(),
                filter: default(),
            }));
            let mut inputs = inputs.clone();
            inputs.insert("rig.foot_valid".into(), DataValue::Bool(true));
            let mut outcomes = Vec::new();
            for capture in [true, false] {
                inputs.insert(RIG_CAPTURE_KEY.into(), DataValue::Bool(capture));
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
                let output = source_inputs
                    .evaluate(context.root_entity, || {
                        graph.query_with_context(
                            QueryOutputTime::Forced(TimeUpdate::Delta(0.02)),
                            context,
                        )
                    })
                    .unwrap()
                    .remove("Pose")
                    .unwrap()
                    .into_pose()
                    .unwrap();
                let captured = arena
                    .get_toplevel()
                    .node_states
                    .get::<ControlRigNodeState>(
                        rig_node.id,
                        bevy_animation_graph::core::context::node_states::StateKey::Default,
                    )
                    .unwrap();
                assert_eq!(captured.traces.len(), usize::from(capture));
                assert_eq!(captured.elapsed_seconds > 0.0, capture);
                if capture {
                    let trace = &captured.traces[0];
                    assert!(trace.hit.is_none());
                    assert_eq!(trace.start.y, 0.5);
                    assert_eq!(trace.end.y, -0.8);
                    let call = captured
                        .calls
                        .iter()
                        .find(|call| call.instruction == trace.instruction)
                        .unwrap();
                    assert_eq!(call.arguments[6], RigValue::Bool(true));
                    let branches = captured
                        .calls
                        .iter()
                        .filter_map(|call| {
                            let crate::extensions::control_rig_program::RigInstruction::Call {
                                function,
                                ..
                            } = &definition.program.instructions[call.instruction]
                            else {
                                return None;
                            };
                            (function == "FAlsRigVMFunction_IsGameWorld::Execute")
                                .then(|| call.arguments[0].clone())
                        })
                        .collect::<Vec<_>>();
                    assert_eq!(
                        branches,
                        vec![
                            RigValue::Name("True".into()),
                            RigValue::Name("Completed".into())
                        ]
                    );
                    assert!(
                        captured
                            .calls
                            .iter()
                            .flat_map(|call| &call.bones_before)
                            .any(|(index, before)| {
                                captured
                                    .snapshot_hierarchy()
                                    .unwrap()
                                    .transform(*index, RigSpace::Global, false)
                                    .unwrap()
                                    != *before
                            })
                    );
                } else {
                    assert!(captured.calls.is_empty());
                }
                outcomes.push((
                    captured.runtime.as_ref().unwrap().values.clone(),
                    format!("{:?}", captured.runtime.as_ref().unwrap().hierarchy),
                    output.paths.clone(),
                    output
                        .bones
                        .iter()
                        .map(|bone| {
                            (
                                bone.translation,
                                bone.rotation,
                                bone.scale,
                                bone.weights.clone(),
                            )
                        })
                        .collect::<Vec<_>>(),
                    output.timestamp,
                    output.root_motion,
                ));
            }
            assert!(
                outcomes[0] == outcomes[1],
                "capture changed rig values, hierarchy caches, or output pose"
            );
            source_inputs.set_rig_query_context(None);
            assert!(source_inputs.rig_query_context().is_none());
            outputs
        }
        let definition: RigDefinition = serde_json::from_str(include_str!(
            "../tests/fixtures/rig/cr_als_program.json"
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
            crate::extensions::GlobalPoseInputNode {
                key: "source".into(),
                output: "Pose".into(),
            },
        );
        let writer = AnimationNode::new(
            "late input write",
            crate::extensions::source_instance::SourcePropertyWriteNode {
                assignments: vec![
                    crate::extensions::source_instance::SourcePropertyAssignment {
                        property: "rig.forward".into(),
                        value: DataValue::F32(0.9),
                    },
                ],
                ..default()
            },
        );
        let curve = AnimationNode::new(
            "curve",
            crate::extensions::GlobalF32InputNode {
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
            crate::extensions::GlobalF32InputNode {
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
            (RIG_CAPTURE_KEY.to_owned(), DataValue::Bool(true)),
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
            "../tests/fixtures/rig/cr_als_program.json"
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
            ron::from_str(include_str!(
                "../../assets/ALS/Character/AB_Als/AnimGraph.animgraph.ron"
            ))
            .unwrap();
        let generated = graph
            .nodes
            .iter()
            .find(|node| {
                node.inner
                    .get_ron()
                    .contains("als_ext::ControlRigBoundaryNode")
            })
            .unwrap();
        let centimeters: RigDefinition = serde_json::from_str(include_str!(
            "../tests/fixtures/rig/rig_program_centimeters.json"
        ))
        .unwrap();
        let cases = [
            (encoded.as_str(), node.definition.as_ref().unwrap()),
            (generated.inner.get_ron(), &centimeters),
        ];
        for (source, (encoded, expected)) in cases.into_iter().enumerate() {
            let mut deserializer = ron::de::Deserializer::from_str(encoded).unwrap();
            let reflected = ReflectDeserializer::new(&registry)
                .deserialize(&mut deserializer)
                .unwrap_or_else(|error| panic!("{}", deserializer.span_error(error)));
            let decoded =
                ControlRigBoundaryNode::from_reflect(reflected.as_partial_reflect()).unwrap();
            let actual = decoded
                .definition
                .expect("generated rig definition is missing");
            assert_eq!(actual.bones.len(), expected.bones.len());
            for (actual, expected) in actual.bones.iter().zip(&expected.bones) {
                assert_eq!(
                    serde_json::to_value(actual).unwrap(),
                    serde_json::to_value(expected).unwrap(),
                    "source {source}, bone {}",
                    actual.name
                );
            }
            assert_eq!(actual.program, expected.program);
            assert_eq!(actual.externals, expected.externals);
            assert_eq!(actual.curves, expected.curves);
            assert_eq!(actual.translation_tolerance, expected.translation_tolerance);
            assert_eq!(
                serde_json::to_value(actual.geometry).unwrap(),
                serde_json::to_value(&expected.geometry).unwrap()
            );
        }
    }
}
