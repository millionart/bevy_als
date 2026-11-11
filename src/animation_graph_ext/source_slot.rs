use bevy::prelude::*;
use bevy_animation_graph::core::{
    animation_node::{NodeLike, ReflectNodeLike},
    context::{new_context::NodeContext, spec_context::SpecContext},
    edge_data::DataSpec,
    errors::GraphError,
};

use crate::extensions::slot_node::{blend_slot_pose, slot_blend_weight};
use crate::extensions::source_gameplay_tag_input::active_slot_input_context;
use crate::extensions::source_pose_nodes::blend_animation_contexts;
use crate::extensions::{
    animation_context::{AnimationContextState, SourceInertializationRequest, merge_inertialization_request, pose_input_context_state},
    mesh_space_additive::apply_mesh_space_additive,
};

pub(crate) const MONTAGE_EVALUATION_WEIGHT_THRESHOLD: f32 = 1e-5;

pub(crate) fn montage_evaluation_denominator(total_weight: f32) -> f32 {
    if total_weight > 1.0 + MONTAGE_EVALUATION_WEIGHT_THRESHOLD { total_weight } else { 1.0 }
}

#[cfg(test)]
#[derive(serde::Deserialize)]
pub(crate) struct MontageEligibilityCase {
    pub weights: Vec<f32>,
    pub eligible: usize,
    pub total: f32,
}

#[cfg(test)]
pub(crate) fn montage_eligibility_cases() -> Vec<MontageEligibilityCase> {
    #[derive(serde::Deserialize)]
    struct Fixture {
        cases: Vec<MontageEligibilityCase>,
    }
    let fixture: Fixture = serde_json::from_str(include_str!(
        "../../docs/audit/bf144/d168/eligibility.replay.json"
    )).unwrap();
    fixture.cases
}

#[cfg(test)]
#[derive(serde::Deserialize)]
pub(crate) struct MontageNormalizationCase {
    pub weights: Vec<f32>,
    pub effective: Vec<f32>,
    pub slot_weight: f32,
    pub source_curve: f32,
    pub base_curve: f32,
}

#[cfg(test)]
pub(crate) fn montage_normalization_cases(slot: &str) -> Vec<MontageNormalizationCase> {
    #[derive(serde::Deserialize)]
    struct Fixture {
        cases: std::collections::HashMap<String, Vec<MontageNormalizationCase>>,
    }
    let mut fixture: Fixture = serde_json::from_str(include_str!(
        "../../docs/audit/bf144/d169/normalization.replay.json"
    )).unwrap();
    fixture.cases.remove(slot).unwrap()
}

#[derive(Reflect, Clone, Debug)]
#[reflect(Default, NodeLike)]
#[type_path = "als_ext"]
pub struct SourceSlotNode {
    pub slot_name: String,
    pub group_name: String,
    pub pose_key: String,
    pub weight_key: String,
    pub layer_weight_key: String,
    pub mesh_space_additive_key: String,
    pub always_update_source_pose: bool,
    pub request_epoch_key: String,
    pub request_duration_key: String,
}

fn source_slot_additive_weight(slot_weight: f32, layer_weight: f32) -> f32 {
    slot_weight * layer_weight
}

fn slot_context(
    base: Option<AnimationContextState>,
    slot: Option<AnimationContextState>,
    weight: f32,
    additive: bool,
) -> Option<AnimationContextState> {
    let Some(slot) = slot else { return base };
    if weight <= 0.0 {
        return base;
    }
    Some(blend_animation_contexts([
        (
            base.unwrap_or_default(),
            if additive { 1.0 } else { 1.0 - weight },
        ),
        (slot, weight),
    ]))
}

impl Default for SourceSlotNode {
    fn default() -> Self {
        Self {
            slot_name: "DefaultSlot".to_owned(),
            group_name: "DefaultGroup".to_owned(),
            pose_key: "slot_pose".to_owned(),
            weight_key: "slot_weight".to_owned(),
            layer_weight_key: "layer_slot_weight".to_owned(),
            mesh_space_additive_key: String::new(),
            always_update_source_pose: false,
            request_epoch_key: String::new(),
            request_duration_key: String::new(),
        }
    }
}

impl SourceSlotNode {
    fn with_inertialization_request(
        &self,
        ctx: &NodeContext,
        mut context: Option<AnimationContextState>,
    ) -> Result<Option<AnimationContextState>, GraphError> {
        let scalar = |key: &str| ctx.graph_context.global_input_data.get(key)
            .map_or(Ok(0.0), |value| value.as_f32());
        if !self.request_epoch_key.is_empty() && scalar(&self.request_epoch_key)? > 0.0 {
            let request = SourceInertializationRequest {
                frame: ctx.graph_context.frame_index(),
                duration: scalar(&self.request_duration_key)?,
                ..default()
            };
            merge_inertialization_request(
                &mut context.get_or_insert_with(AnimationContextState::default).inertialization_request,
                Some(&request),
            );
        }
        Ok(context)
    }

    pub(crate) fn update_weights(&self, ctx: &NodeContext) -> Result<(f32, f32, bool, f32), GraphError> {
        let slot_weight = slot_blend_weight(
            ctx.graph_context
                .global_input_data
                .get(&self.weight_key)
                .map(|value| value.as_f32())
                .transpose()?,
        );
        let slot_weight = if slot_weight > MONTAGE_EVALUATION_WEIGHT_THRESHOLD { slot_weight } else { 0.0 };
        let layer_weight = slot_blend_weight(
            ctx.graph_context
                .global_input_data
                .get(&self.layer_weight_key)
                .map(|value| value.as_f32())
                .transpose()?,
        );
        let mesh_space_additive = if self.mesh_space_additive_key.is_empty() {
            false
        } else {
            ctx.graph_context
                .global_input_data
                .get(&self.mesh_space_additive_key)
                .map(|value| value.as_f32())
                .transpose()?
                .is_some_and(|value| value.is_finite() && value > 0.5)
        };
        let source_weight = if mesh_space_additive || !ctx.graph_context.global_input_data.contains_key(&self.pose_key) {
            1.0
        } else { 1.0 - slot_weight * layer_weight };
        let source_weight = if !self.always_update_source_pose && source_weight <= 1e-5 {
            0.0
        } else { source_weight.max(2e-5) };
        Ok((slot_weight, layer_weight, mesh_space_additive, source_weight))
    }
}

impl NodeLike for SourceSlotNode {
    fn update(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        let (slot_weight, layer_weight, mesh_space_additive, source_weight) = self.update_weights(&ctx)?;
        let effective_weight = slot_weight * layer_weight;
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
        if !mesh_space_additive && 1.0 - effective_weight <= MONTAGE_EVALUATION_WEIGHT_THRESHOLD
            && let Some(pose) = overlay.as_ref()
        {
            if source_weight > 0.0 {
                let _ = crate::extensions::interruptible_fsm::with_machine_weight(source_weight,
                    || ctx.data_back("Source"))?.into_pose()?;
            }
            let slot = active_slot_input_context(ctx.graph_context.root_entity, &self.pose_key);
            let multiple = slot.as_ref().is_some_and(|context| context.slot_has_multiple_montages);
            let context = slot_context(None, slot, effective_weight, false);
            let mut context = self.with_inertialization_request(&ctx, context)?.unwrap_or_default();
            for sample in context.asset_samples.iter_mut().filter(|sample| sample.montage && sample.slot_key.is_empty()) {
                sample.slot_key.clone_from(&self.pose_key);
            }
            ctx.set_time(pose.timestamp);
            let pose = pose.scalar_mult(effective_weight);
            ctx.set_data_fwd("Pose", if multiple { pose.normalize_quat() } else { pose });
            *ctx.state_mut_or_else(AnimationContextState::default)? = context;
            return Ok(());
        }
        let base = crate::extensions::interruptible_fsm::with_machine_weight(source_weight,
            || ctx.data_back("Source"))?.into_pose()?;
        let base_context = pose_input_context_state(&ctx, "Source");
        let context = slot_context(
            base_context,
            overlay
                .as_ref()
                .and_then(|_| active_slot_input_context(ctx.graph_context.root_entity, &self.pose_key)),
            effective_weight,
            mesh_space_additive,
        );
        let context = self.with_inertialization_request(&ctx, context)?;
        let pose = if mesh_space_additive {
            let mut pose = base;
            if let Some(overlay) = overlay.as_ref() {
                let skeleton = ctx
                    .graph_context
                    .resources
                    .skeleton_assets
                    .get(&pose.skeleton)
                    .ok_or(GraphError::SkeletonMissing(ctx.node_id))?;
                let timestamp = pose.timestamp;
                apply_mesh_space_additive(
                    &mut pose,
                    overlay,
                    source_slot_additive_weight(slot_weight, layer_weight),
                    skeleton,
                );
                pose.timestamp = timestamp;
            }
            pose
        } else if layer_weight == 1.0 {
            blend_slot_pose(base, overlay.as_ref(), Some(slot_weight))
        } else {
            let slot_pose = blend_slot_pose(base.clone(), overlay.as_ref(), Some(slot_weight));
            blend_slot_pose(
                base,
                overlay.as_ref().map(|_| &slot_pose),
                Some(layer_weight),
            )
        };
        ctx.set_time(pose.timestamp);
        ctx.set_data_fwd("Pose", pose);
        if let Some(mut context) = context {
            for sample in context.asset_samples.iter_mut().filter(|sample| sample.montage && sample.slot_key.is_empty()) {
                sample.slot_key.clone_from(&self.pose_key);
            }
            *ctx.state_mut_or_else(AnimationContextState::default)? = context;
        }
        Ok(())
    }

    fn spec(&self, mut ctx: SpecContext) -> Result<(), GraphError> {
        ctx.add_input_data("Source", DataSpec::Pose)
            .add_output_data("Pose", DataSpec::Pose);
        Ok(())
    }

    fn display_name(&self) -> String {
        format!("ALS Source Slot {}", self.slot_name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn actual_graph_uses_the_same_slot_weight_for_pose_and_curves() {
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
            pose::{BonePose, Pose},
            skeleton::Skeleton,
            state_machine::high_level::StateMachine,
            utils::testing::TestIoEnv,
        };
        fn query(
            In((handle, entity, base, overlay, weight, idle, epoch)): In<(
                Handle<AnimationGraph>,
                Entity,
                Pose,
                Pose,
                f32,
                bool,
                f32,
            )>,
            resources: SystemResources,
            source_inputs: Query<&crate::extensions::SourceAnimationInputs>,
        ) -> (Pose, AnimationContextState, bool) {
            let graph = resources.animation_graph_assets.get(&handle).unwrap();
            let mut arena = GraphContextArena::new(handle.id());
            let mut gizmos = DeferredGizmos::default();
            let inputs = HashMap::from([
                ("base".to_owned(), DataValue::Pose(base)),
                ("slot_pose".to_owned(), DataValue::Pose(overlay)),
                ("slot_weight".to_owned(), DataValue::F32(weight)),
                ("layer_slot_weight".to_owned(), DataValue::F32(1.0)),
                ("idle".to_owned(), DataValue::F32(if idle { 1.0 } else { 0.0 })),
                ("slot_request".to_owned(), DataValue::F32(epoch)),
                ("slot_request_duration".to_owned(), DataValue::F32(0.2)),
            ]);
            let entities = HashMap::new();
            let env = TestIoEnv::default();
            let context = GraphContext::new(
                arena.get_toplevel_id(),
                graph,
                &mut arena,
                &resources,
                &env,
                entity,
                &entities,
                &mut gizmos,
                &inputs,
            );
            let pose = source_inputs.get(entity).unwrap().evaluate(context.root_entity, || graph
                .query_with_context(
                    QueryOutputTime::Forced(TimeUpdate::Delta(0.1)),
                    context.clone(),
                ))
                .unwrap()
                .remove("Pose")
                .unwrap()
                .into_pose()
                .unwrap();
            let curves =
                crate::extensions::animation_context::graph_output_context_state(graph, &context, "Pose")
                    .unwrap_or_default();
            let base_id = graph
                .nodes
                .values()
                .find(|node| node.name == "base")
                .unwrap()
                .id;
            let updated = arena.get_toplevel().node_caches.is_updated(
                base_id,
                bevy_animation_graph::core::context::node_states::StateKey::Default,
            );
            (pose, curves, updated)
        }
        let mut world = World::new();
        let entity = world.spawn_empty().id();
        let mut graph = AnimationGraph::new();
        let base_node = AnimationNode::new(
            "base",
            crate::extensions::GlobalPoseInputNode {
                key: "base".to_owned(),
                output: "Pose".to_owned(),
            },
        );
        let slot = SourceSlotNode {
            request_epoch_key: "slot_request".into(),
            request_duration_key: "slot_request_duration".into(),
            ..default()
        };
        let slot_node = AnimationNode::new("slot", slot.clone());
        let branch = AnimationNode::new("idle or moving", crate::extensions::source_pose_nodes::SourceTwoWayBlendNode {
            alpha_key: "idle".into(), ..default()
        });
        graph.add_node(base_node.clone());
        graph.add_node(slot_node.clone());
        graph.add_node(branch.clone());
        graph.add_output_data("Pose".to_owned(), DataSpec::Pose);
        graph.add_node_parameter_edge(base_node.id, "Pose", slot_node.id, "Source");
        graph.add_node_parameter_edge(base_node.id, "Pose", branch.id, "A");
        graph.add_node_parameter_edge(slot_node.id, "Pose", branch.id, "B");
        graph.add_output_data_edge(branch.id, "Pose", "Pose");
        let mut always_graph = graph.clone();
        let mut always_slot = AnimationNode::new(
            "slot",
            SourceSlotNode {
                always_update_source_pose: true,
                ..slot
            },
        );
        always_slot.id = slot_node.id;
        always_graph.add_node(always_slot);
        let mut graphs = Assets::<AnimationGraph>::default();
        let handle = graphs.add(graph);
        let always_handle = graphs.add(always_graph);
        world.insert_resource(graphs);
        world.insert_resource(Assets::<GraphClip>::default());
        world.insert_resource(Assets::<Skeleton>::default());
        world.insert_resource(Assets::<StateMachine>::default());
        let bone = EntityPath::default().child("pelvis").id();
        let mut base = Pose::default();
        base.add_bone(
            BonePose {
                translation: Some(Vec3::ZERO),
                rotation: Some(Quat::from_rotation_x(0.5)),
                ..default()
            },
            bone,
        );
        let mut overlay = Pose::default();
        overlay.add_bone(
            BonePose {
                translation: Some(Vec3::X * 2.0),
                rotation: Some(Quat::from_rotation_y(1.8) * 0.8),
                scale: Some(Vec3::new(1.2, 0.75, 1.5)),
                ..default()
            },
            bone,
        );
        let mut context = AnimationContextState {
            curves_are_authoritative: true,
            ..default()
        };
        context.curves.insert("FootLeftIk".into(), 0.8);
        let mut source_inputs = crate::extensions::SourceAnimationInputs::default();
        source_inputs.set_slot_input_context(
            "slot_pose",
            Some(context),
        );
        world.entity_mut(entity).insert(source_inputs);
        for weight in [3e-6, 1e-5] {
            let (pose, context, source_updated) = world.run_system_cached_with(
                query, (handle.clone(), entity, base.clone(), overlay.clone(), weight, true, 0.0),
            ).unwrap();
            assert!(source_updated);
            assert_eq!(pose.get_bone(bone).unwrap().translation, Some(Vec3::ZERO));
            assert_eq!(pose.get_bone(bone).unwrap().rotation, Some(Quat::from_rotation_x(0.5)));
            assert!(!context.curves.contains_key(&crate::extensions::SourceCurveName::from("FootLeftIk")));
        }
        for (idle, weight, epoch, expected) in [
            (true, 0.0, 1.0, Some(0.2)), (true, 1.0, 1.0, Some(0.2)),
            (false, 0.0, 1.0, None), (true, 0.0, 0.0, None),
        ] {
            let (_, context, _) = world.run_system_cached_with(
                query, (handle.clone(), entity, base.clone(), overlay.clone(), weight, idle, epoch),
            ).unwrap();
            assert_eq!(context.inertialization_request.map(|request| request.duration), expected);
        }
        for (handle, always_update, weight) in [
            (handle.clone(), false, 0.6),
            (handle.clone(), false, 1.0),
            (handle.clone(), false, 0.6),
            (always_handle.clone(), true, 1.0),
        ] {
            let (pose, context, source_updated) = world
                .run_system_cached_with(
                    query,
                    (
                        handle.clone(),
                        entity,
                        base.clone(),
                        overlay.clone(),
                        weight,
                        true,
                        0.0,
                    ),
                )
                .unwrap();
            assert!(
                (pose.get_bone(bone).unwrap().translation.unwrap().x - 2.0 * weight).abs() < 1e-6
            );
            let rotation = pose.get_bone(bone).unwrap().rotation.unwrap();
            let expected = if weight == 1.0 {
                Quat::from_rotation_y(1.8) * 0.8
            } else {
                (Quat::from_rotation_x(0.5) * (1.0 - weight)
                    + Quat::from_rotation_y(1.8) * (0.8 * weight)).normalize()
            };
            assert!((rotation - expected).length().min((rotation + expected).length()) < 1e-6);
            assert!((context.curves[&crate::extensions::SourceCurveName::from("FootLeftIk")] - 0.8 * weight).abs() < 1e-6);
            assert_eq!(
                source_updated,
                always_update || weight < 1.0,
                "slot_weight={weight}, always_update={always_update}"
            );
        }
        let near_full = 0.999995;
        for (handle, always_update, multiple) in [
            (handle.clone(), false, false), (handle, false, true), (always_handle, true, false),
        ] {
            let mut inputs = world.get_mut::<crate::extensions::SourceAnimationInputs>(entity).unwrap();
            let mut slot = inputs.slot_input_context("slot_pose").unwrap();
            slot.slot_has_multiple_montages = multiple;
            inputs.set_slot_input_context("slot_pose", Some(slot));
            let (pose, context, source_updated) = world.run_system_cached_with(
                query, (handle, entity, base.clone(), overlay.clone(), near_full, true, 0.0),
            ).unwrap();
            assert_eq!(source_updated, always_update);
            let bone = pose.get_bone(bone).unwrap();
            let raw = (Quat::from_rotation_y(1.8) * 0.8) * near_full;
            assert_eq!(bone.rotation, Some(if multiple { raw.normalize() } else { raw }));
            assert_eq!(bone.translation, Some(Vec3::X * (2.0 * near_full)));
            assert_eq!(bone.scale, Some(Vec3::new(1.2, 0.75, 1.5) * near_full));
            assert_eq!(context.curves[&crate::extensions::SourceCurveName::from("FootLeftIk")], 0.8 * near_full);
        }

    }

    #[test]
    fn source_slot_preserves_source_visible_pin_names() {
        let node = SourceSlotNode::default();
        assert_eq!(node.slot_name, "DefaultSlot");
        assert_eq!(node.group_name, "DefaultGroup");
        assert!(node.mesh_space_additive_key.is_empty());
        assert!(!node.always_update_source_pose);
    }

    #[test]
    fn source_slot_mesh_space_weight_preserves_both_source_stage_weights() {
        assert!((source_slot_additive_weight(0.8, 0.5) - 0.4).abs() < f32::EPSILON);
    }

    #[test]
    fn full_slot_weight_replaces_base_curves_including_an_empty_container() {
        let mut base = AnimationContextState::default();
        base.curves.insert("FootLeftIk".into(), 1.0);
        base.curves_are_authoritative = true;
        let empty = AnimationContextState {
            curves_are_authoritative: true,
            ..default()
        };
        let output = slot_context(Some(base.clone()), Some(empty.clone()), 1.0, false).unwrap();
        assert_eq!(output.curves[&crate::extensions::SourceCurveName::from("FootLeftIk")], 0.0);
        assert!(output.curves_are_authoritative);
        let output = slot_context(Some(base), Some(empty), 0.5, false).unwrap();
        assert_eq!(output.curves[&crate::extensions::SourceCurveName::from("FootLeftIk")], 0.5);
    }
}
