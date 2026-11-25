use std::{any::TypeId, collections::HashSet};

use bevy::asset::{AssetId, Assets};
use bevy_animation_graph::core::{
    animation_graph::AnimationGraph, state_machine::high_level::StateMachine,
};

use crate::extensions::{
    ConstantF32Node, ControlRigBoundaryNode, CubicVec2AxisFilterNode, F32ToVec2Node,
    GlobalBoolInputNode, GlobalF32InputNode, GlobalPoseInputNode, InertializationNode,
    InertializationPassthroughNode, InertializedLinkedAnimLayerNode, InterruptibleFsmNode,
    LayeredGraphNode, LinkedAnimLayerNode, LocalRefPoseNode, NormalizedBlendSpaceNode,
    ProfiledBlendNode, RagdollConfigNode, ReferenceLocalDifferenceNode,
    ReferenceMeshDifferenceNode, RootMotionClipNode, SkeletonAwareLayeredGraphNode,
    SourceAnimationStateGetterNode, SourceApplyAdditiveNode, SourceCachedPoseSaveNode,
    SourceCurveModifyNode, SourceCurvesBlendNode, SourceEnumPoseBlendNode,
    SourceGameplayTagBlendNode, SourceGameplayTagInputNode, SourceLayeredBoneBlendNode,
    SourceLinkedInputPoseNode, SourceMakeDynamicAdditiveNode, SourceMeshSpaceAdditiveNode,
    SourceMultiWayBlendNode, SourcePosePassthroughNode, SourceScalarExpressionNode,
    SourceScopedCachedPoseUseNode, SourceSequenceEvaluatorNode, SourceSequencePlayerNode,
    SourceSetHipsDirectionNode, SourceSlotNode, SourceTwoWayBlendNode, TimePositionNode,
    source_clip::SourceClipNode, source_instance::SourcePropertyWriteNode,
    source_native_refresh::AlsSourceRefreshNode,
};

// These implementations obtain graph data only from the active graph, its inputs or
// contexts supplied by the graph-link nodes handled below. Other node types require
// conservative invalidation because their graph lookups can be opaque.
const GRAPH_LOCAL_NODES: &[TypeId] = &[
    TypeId::of::<AlsSourceRefreshNode>(),
    TypeId::of::<ConstantF32Node>(),
    TypeId::of::<ControlRigBoundaryNode>(),
    TypeId::of::<CubicVec2AxisFilterNode>(),
    TypeId::of::<F32ToVec2Node>(),
    TypeId::of::<GlobalBoolInputNode>(),
    TypeId::of::<GlobalF32InputNode>(),
    TypeId::of::<GlobalPoseInputNode>(),
    TypeId::of::<InertializationNode>(),
    TypeId::of::<InertializationPassthroughNode>(),
    TypeId::of::<LocalRefPoseNode>(),
    TypeId::of::<NormalizedBlendSpaceNode>(),
    TypeId::of::<ProfiledBlendNode>(),
    TypeId::of::<RagdollConfigNode>(),
    TypeId::of::<ReferenceLocalDifferenceNode>(),
    TypeId::of::<ReferenceMeshDifferenceNode>(),
    TypeId::of::<RootMotionClipNode>(),
    TypeId::of::<SourceAnimationStateGetterNode>(),
    TypeId::of::<SourceApplyAdditiveNode>(),
    TypeId::of::<SourceCachedPoseSaveNode>(),
    TypeId::of::<SourceClipNode>(),
    TypeId::of::<SourceCurveModifyNode>(),
    TypeId::of::<SourceCurvesBlendNode>(),
    TypeId::of::<SourceEnumPoseBlendNode>(),
    TypeId::of::<SourceGameplayTagBlendNode>(),
    TypeId::of::<SourceGameplayTagInputNode>(),
    TypeId::of::<SourceLayeredBoneBlendNode>(),
    TypeId::of::<SourceLinkedInputPoseNode>(),
    TypeId::of::<SourceMakeDynamicAdditiveNode>(),
    TypeId::of::<SourceMeshSpaceAdditiveNode>(),
    TypeId::of::<SourceMultiWayBlendNode>(),
    TypeId::of::<SourcePosePassthroughNode>(),
    TypeId::of::<SourcePropertyWriteNode>(),
    TypeId::of::<SourceScalarExpressionNode>(),
    TypeId::of::<SourceScopedCachedPoseUseNode>(),
    TypeId::of::<SourceSequenceEvaluatorNode>(),
    TypeId::of::<SourceSequencePlayerNode>(),
    TypeId::of::<SourceSetHipsDirectionNode>(),
    TypeId::of::<SourceSlotNode>(),
    TypeId::of::<SourceTwoWayBlendNode>(),
    TypeId::of::<TimePositionNode>(),
];

pub(crate) fn reachable_sharing_graphs(
    mut pending: Vec<AssetId<AnimationGraph>>,
    graphs: &Assets<AnimationGraph>,
    states: &Assets<StateMachine>,
) -> Option<HashSet<AssetId<AnimationGraph>>> {
    let mut reachable = HashSet::new();
    let mut visited_machines = HashSet::new();
    while let Some(id) = pending.pop() {
        if !reachable.insert(id) {
            continue;
        }
        let Some(graph) = graphs.get(id) else {
            // Retain the reference so an addition of this graph invalidates sharing.
            continue;
        };
        for node in graph.nodes.values() {
            if let Some(layer) = node.try_inner_downcast_ref::<LayeredGraphNode>() {
                pending.push(layer.graph.id());
            } else if let Some(layer) =
                node.try_inner_downcast_ref::<SkeletonAwareLayeredGraphNode>()
            {
                pending.push(layer.graph.id());
            } else if let Some(layer) = node.try_inner_downcast_ref::<LinkedAnimLayerNode>() {
                pending.push(layer.interface.id());
                pending.extend(layer.implementations.iter().map(|graph| graph.id()));
            } else if let Some(layer) =
                node.try_inner_downcast_ref::<InertializedLinkedAnimLayerNode>()
            {
                pending.push(layer.interface.id());
                pending.extend(layer.implementations.iter().map(|graph| graph.id()));
            } else if let Some(fsm) = node.try_inner_downcast_ref::<InterruptibleFsmNode>() {
                if visited_machines.insert(fsm.fsm.id())
                    && let Some(machine) = states.get(&fsm.fsm)
                {
                    pending.extend(
                        machine
                            .get_low_level_fsm()
                            .states
                            .values()
                            .map(|state| state.graph.id()),
                    );
                }
                for condition in &fsm.source_transition_conditions {
                    pending.push(condition.condition.id());
                    pending.extend(condition.gates.iter().map(|gate| gate.condition.id()));
                }
                for entry in &fsm.source_entry_conditions {
                    pending.extend(entry.gates.iter().map(|gate| gate.condition.id()));
                }
            } else if !GRAPH_LOCAL_NODES.contains(&node.inner_ref().as_any().type_id()) {
                return None;
            }
        }
    }
    Some(reachable)
}

#[cfg(test)]
mod tests {
    use bevy_animation_graph::{
        builtin_nodes::clip_node::ClipNode,
        core::{animation_node::AnimationNode, state_machine::high_level::State},
    };

    use super::*;
    use crate::extensions::SourceTransitionCondition;

    #[test]
    fn sharing_deps_keep_missing_nested_fsm_states_and_conditions() {
        let mut graphs = Assets::default();
        let mut states = Assets::default();
        let dormant = graphs.reserve_handle();
        let condition = graphs.reserve_handle();
        let unrelated = graphs.add(AnimationGraph::new());
        let state = State {
            graph: dormant.clone(),
            ..Default::default()
        };
        let mut fsm = StateMachine::default();
        fsm.set_start_state(state.id);
        fsm.add_state(state);
        fsm.update_low_level_fsm();
        let mut fsm = InterruptibleFsmNode::new(states.add(fsm));
        fsm.source_transition_conditions
            .push(SourceTransitionCondition {
                condition: condition.clone(),
                ..Default::default()
            });
        let mut child = AnimationGraph::new();
        child.add_node(AnimationNode::new("FSM", fsm));
        let child = graphs.add(child);
        let mut root = AnimationGraph::new();
        root.add_node(AnimationNode::new(
            "Layer",
            LayeredGraphNode::new(child.clone()),
        ));
        let root = graphs.add(root);

        let reachable = reachable_sharing_graphs(vec![root.id()], &graphs, &states).unwrap();
        for id in [root.id(), child.id(), dormant.id(), condition.id()] {
            assert!(reachable.contains(&id));
        }
        assert!(!reachable.contains(&unrelated.id()));
    }

    #[test]
    fn opaque_node_requires_conservative_invalidation() {
        let mut graphs = Assets::default();
        let mut root = AnimationGraph::new();
        root.add_node(AnimationNode::new(
            "Extension",
            ClipNode::new(Default::default(), None, None),
        ));
        let root = graphs.add(root);
        assert!(reachable_sharing_graphs(vec![root.id()], &graphs, &Assets::default()).is_none());
    }
}
