//! Extension layer over bevy_animation_graph / bevy-tnua: curve assets and nodes, analytic IK, ragdoll config node, ALS float-curve import.

pub mod als_curves;
pub mod animation_context;
pub mod post_physics_animation;
pub mod blend_space_1d;
pub mod crouch_transaction;
pub mod curve;
pub mod curve_bundle;
pub mod curve_weighted_samples;
pub mod control_rig_math;
pub mod control_rig_program;
pub mod control_rig_hierarchy;
pub mod control_rig_ik;
pub mod control_rig_units;
pub mod control_rig_query;
pub mod direct_transition_event;
pub mod distance_match;
pub mod event_lifecycle;
pub mod event_loop;
pub mod event_queue_blend;
pub mod f32_to_vec2;
pub mod f_interp;
pub mod first_update;
pub mod gameplay_tag_blend;
pub mod gameplay_tag_layer_blend;
pub mod global_bone_mask_input;
pub mod global_bool_input;
pub mod global_f32_input;
pub mod global_pose_input;
pub mod ik;
pub mod ik_space;
pub mod inertialization;
pub mod inertialization_passthrough;
pub mod inertialized_linked_anim_layer;
pub mod interruptible_fsm;
pub mod layered_graph;
pub mod linked_anim_layer;
pub mod loader;
pub mod local_ref_pose;
pub mod mesh_space_additive;
pub mod mesh_space_difference;
pub mod mesh_space_layered_blend;
pub mod motion;
pub mod multi_way_blend;
pub mod nodes;
pub mod normalized_blendspace;
pub mod overlay;
pub mod overlay_materials;
pub mod penetration_recovery;
pub mod playground;
pub mod playground_materials;
pub mod projected_decal;
pub mod pose_timestamp;
pub mod profile_blend;
pub mod ragdoll_angular_drive;
pub mod ragdoll_config_node;
pub mod ragdoll_physics_profile;
pub mod ragdolling_tag_blend;
pub mod reference_local_difference;
pub mod reference_mesh_difference;
pub mod reset_on_activation;
pub mod rich_curve;
pub mod root_graph_boundaries;
pub mod root_motion_clip;
pub mod root_motion_defaults;
pub mod skeleton_aware_layered_graph;
pub mod slot_node;
pub mod source_animation_state_getter;
pub mod source_cached_pose;
pub mod source_gameplay_tag_input;
pub mod source_instance;
pub mod source_linked_input_pose;
pub mod source_movement_nodes;
pub mod source_native_refresh;
pub mod source_physics_joint;
pub mod source_pose_nodes;
pub mod static_skeleton;
pub mod static_animated_scene;
pub mod source_pose_passthrough;
pub mod source_scalar_expression;
pub mod source_sequence_evaluator;
pub mod source_sequence_player;
pub mod source_slot;
pub mod source_stop_nodes;
pub mod state_label_event;
pub mod sync_group_phase;
pub mod time_dilation;
pub mod time_position;

pub use als_curves::{
    ALS_CURVES_EXTENSION, ALS_CURVES_LABEL, AlsAnimCurves, AlsCurvesHandler, AlsCurvesParseError,
    AlsExtrapolation, AlsFloatCurve, AlsKeyInterp, AlsPoseCurves, parse_curves,
};
pub use animation_context::{
    AnimationContextInput, AnimationContextInputAlias, AnimationContextState, SourceRelevantAsset,
    capture_context_curve, context_time_update, is_context_curve_input,
    player_animation_context_state, resolve_context_curves, resolve_named_context_curves,
};
pub use blend_space_1d::{
    BlendSpace1DNode, BlendSpace1DPoint, blend_weights as blend_space_1d_weights,
    pose_input as blend_space_1d_pose_input, time_input as blend_space_1d_time_input,
};
pub use crouch_transaction::{
    CapsuleStance, CapsuleStanceCommand, CapsuleStancePhase, CapsuleStanceResult,
    CapsuleStanceTransaction,
};
pub use curve::{CurveAsset, CurveKey, CurveSampleMode, sample_curve};
pub use curve_bundle::{
    CurveBlendNode, CurveComposeMode, CurveComposeNode, CurveModifyNode, CurveSampleSetNode,
    NamedCurve, active_curve_weights, blend_curve_value, compose_curve_value,
    curve_compose_base_input, curve_compose_overlay_input, curve_modify_input, curve_sample_output,
    curve_value_input, curve_weight_input,
};
pub use curve_weighted_samples::CurveWeightedSamplesNode;
pub use direct_transition_event::{DirectTransitionEventNode, direct_transition_index};
pub use distance_match::{
    DistanceMatchNode, default_stop_distance_curve, invert_distance_curve, sample_distance_curve,
};
pub use event_lifecycle::{
    EventClock, EventDispatchContext, EventLifecycle, EventLifecycleSignal, EventPhase,
    PlaybackDirection, StateIntervalBoundary, TimedEvent, TimedEventKind, crossed_time,
};
pub use event_loop::{EventLoopNode, loop_position as event_loop_position};
pub use event_queue_blend::{
    EventQueueBlendNode, IN_EVENTS_A, IN_EVENTS_B, IN_WEIGHT_A, IN_WEIGHT_B, OUT_EVENTS,
    merge_weighted_event_queues, weighted_event_queue,
};
pub use f_interp::{FInterpNode, f_interp_to};
pub use f32_to_vec2::{F32ToVec2Node, f32_to_vec2};
pub use first_update::SuppressFirstUpdateNode;
pub use gameplay_tag_blend::{
    GameplayTagBlendMode, GameplayTagBlendNode, weight_output as gameplay_tag_weight_output,
};
pub use gameplay_tag_layer_blend::GameplayTagLayerBlendNode;
pub use global_bone_mask_input::GlobalBoneMaskInputNode;
pub use global_bool_input::GlobalBoolInputNode;
pub use global_f32_input::{GlobalF32InputNode, ROOT_DEFAULTS_ENABLED_KEY};
pub use global_pose_input::GlobalPoseInputNode;
pub use ik::{
    PoleVector, StableTwoBonePole, blend_ik_rotation, calculate_pole_vector,
    child_world_rotation_after_parent_rotation,
};
pub use ik_space::{
    ArmatureDirection, ArmaturePoint, ArmatureRotation, BoneLocalPoint, ParentPoint,
    WorldDirection, WorldPoint, WorldRotation, armature_direction_from_world,
    armature_point_from_world, armature_rotation_from_world, bone_local_point_from_world,
    parent_point_from_world, world_direction_from_armature, world_point_from_armature,
    world_point_from_bone_local, world_point_from_parent, world_rotation_from_armature,
};
pub use inertialization::InertializationNode;
pub use inertialization_passthrough::InertializationPassthroughNode;
pub use inertialized_linked_anim_layer::{
    GlobalDataInput, InertializedLinkedAnimLayerNode, InertializedLinkedLayerActivation,
    inertialized_linked_layer_activation,
};
pub use interruptible_fsm::{
    InterruptibleFsmNode, InterruptibleFsmSource, InterruptibleFsmState, SourceEntryCondition,
    SourceFsmStateWeight, SourceTransitionCondition, SourceTransitionConditionGate,
    SourceTransitionNotification, StateAliasBinding, StatePoseOutput,
};
pub use layered_graph::{
    CachedPoseInput, FixedF32Input, FixedGlobalF32Input, GlobalF32InputPin, LayeredGraphNode,
    NamedPoseOutput, OutputDataAlias,
};
pub use linked_anim_layer::{ALS_LAYER_CURVE_NAMES, ALS_LAYER_SIGNAL_NAMES, LinkedAnimLayerNode};
pub use loader::{CurveAssetLoader, CurveLoadError};
pub use local_ref_pose::{LocalRefPoseNode, SourceLocalRefPoseNode};
pub use mesh_space_additive::{MeshSpaceAdditiveNode, apply_mesh_space_additive};
pub use mesh_space_difference::{MeshSpaceDifferenceNode, make_mesh_space_difference};
pub use mesh_space_layered_blend::{MeshSpaceLayeredBlendNode, apply_mesh_space_layered_blend};
pub use motion::{
    HorizontalMotion, HorizontalVelocityBlend, horizontal_motion_from_world_velocity,
};
pub use multi_way_blend::MultiWayBlendNode;
pub use nodes::{ConstantF32Node, CurveDrivenValueNode, CurveSampleNode};
pub use normalized_blendspace::{
    CubicVec2AxisFilterNode, CubicVec2FilterNode, NormalizedBlendSpaceNode,
    NormalizedBlendSpacePoint, blend_normalized_root_motion,
};
pub use overlay::{
    BranchFilter, HEAD_LAYER_BONES, LOOK_SPINE_BONES, TORCH_ARM_RIGHT_BONES,
    TORCH_RIGHT_HAND_BONES, TORCH_SPINE_UPPER_BONES, build_bone_mask, build_branch_bone_mask,
    build_branch_filter_bone_mask, build_branch_filter_bone_mask_by_name, build_grouped_bone_mask,
    mask_coverage,
};
pub use penetration_recovery::sum_penetration_adjustments;
pub use playground::{
    PLAYGROUND_SHADOW_DEPTH_BIAS, PLAYGROUND_SHADOW_NORMAL_BIAS, PlaygroundActor,
    PlaygroundBoxCollider, PlaygroundColliderKind, PlaygroundCollisionSource, PlaygroundComponent,
    PlaygroundKinematic, PlaygroundKinematicKind, PlaygroundLandscape,
    PlaygroundLandscapeTilesMaterial, PlaygroundManifest, PlaygroundManifestPath,
    PlaygroundMaterialApplied, PlaygroundMaterialRole, PlaygroundMaterialTint,
    PlaygroundMovingCurve, PlaygroundMovingObject, PlaygroundPhysicsDriven, PlaygroundPlugin,
    PlaygroundRotator, PlaygroundSky, PlaygroundStatic, PlaygroundTextFacing, PlaygroundTextFit,
    PlaygroundTextGlyphColor, PlaygroundTextHorizontalAlignment, PlaygroundTextLabel,
    PlaygroundTextLayoutTarget, PlaygroundTextVerticalAlignment, PlaygroundWorldTextMesh,
    is_engine_basic, moving_object_curve_input, moving_object_position,
    playground_collider_kind, playground_moving_object_system, playground_setup_system,
    ue_directional_light_ray_to_bevy, ue_extents_to_bevy, ue_point_to_bevy, ue_quat_to_bevy,
    ue_rotator_to_bevy_quat, ue_scale_to_bevy, ue_yaw_to_bevy_quat,
};
pub use pose_timestamp::PoseTimestampNode;
pub use profile_blend::{
    AlphaBlendMode, BoneBlendScale, ProfiledBlendNode, curve_a_input, curve_b_input,
    profile_target_weight, transition_alpha,
};
pub use ragdoll_angular_drive::{
    RagdollAngularDrive, RagdollAngularDriveMetrics, RagdollAngularDrivePlugin,
    RagdollAngularDriveSet,
};
pub use ragdoll_config_node::RagdollConfigNode;
pub use ragdoll_physics_profile::{
    RagdollActiveMode, RagdollBodyPhysicsProfile, RagdollColliderPhysicsProfile,
    RagdollCollisionHooks, RagdollCollisionMember, RagdollJointPhysicsProfile,
    RagdollPhysicsHandoffSet, RagdollPhysicsProfile, RagdollPhysicsProfileApplied,
    RagdollPhysicsProfilePlugin, RagdollPhysicsProfileSet,
};
pub use ragdolling_tag_blend::RagdollingTagBlendNode;
pub use reference_local_difference::{ReferenceLocalDifferenceNode, make_local_difference};
pub use reference_mesh_difference::ReferenceMeshDifferenceNode;
pub use reset_on_activation::{ResetOnActivationNode, activation_reset_required};
pub use rich_curve::{
    RichCurveInterpolation, RichCurveKey, RichCurveSampleNode, RichCurveTangentMode,
    RichCurveTangentWeightMode, sample_rich_curve,
};
pub use root_graph_boundaries::ControlRigBoundaryNode;
pub use root_motion_clip::RootMotionClipNode;
pub use root_motion_defaults::{derive_default_root_bone, select_shallowest_animated_path};
pub use skeleton_aware_layered_graph::SkeletonAwareLayeredGraphNode;
pub use slot_node::SlotNode;
pub use source_animation_state_getter::SourceAnimationStateGetterNode;
pub use source_cached_pose::{
    SourceCachedPoseSaveNode, SourceCachedPoseUseNode, SourceContextDefault,
    SourceScopedCachedPoseUseNode, resolve_cached_pose, resolve_scoped_cached_pose,
};
pub use source_gameplay_tag_input::{SourceGameplayTagInputNode, set_gameplay_tag_input};
pub use source_linked_input_pose::SourceLinkedInputPoseNode;
pub use source_movement_nodes::{SourceMultiWayBlendNode, SourceSetHipsDirectionNode};
pub use source_physics_joint::{
    SourceAngularLimit, SourceAngularMotor, SourceAngularSpring, SourceJointSolverSettings,
    SourcePhysicsJoint, SourcePhysicsJointPlugin, SourcePhysicsJointSolverData,
};
pub use source_pose_nodes::{
    SourceAlphaMapping, SourceApplyAdditiveNode, SourceCurveModifyNode, SourceCurvesBlendMode,
    SourceCurvesBlendNode, SourceGameplayTagBlendNode, SourceBoolBlendNode, SourceMakeDynamicAdditiveNode,
    SourceMeshSpaceAdditiveNode, SourceTwoWayBlendNode,
};
pub use source_pose_passthrough::SourcePosePassthroughNode;
pub use source_scalar_expression::{
    SourceScalarDataType, SourceScalarExpressionNode, SourceScalarPin,
};
pub use source_sequence_evaluator::SourceSequenceEvaluatorNode;
pub use source_sequence_player::SourceSequencePlayerNode;
pub use source_slot::SourceSlotNode;
pub use source_stop_nodes::{
    SourceBranchFilter, SourceEnumPoseBlendNode, SourceLayeredBoneBlendNode,
};
pub use state_label_event::{StateLabelEventNode, state_label_index};
pub use sync_group_phase::{
    GlobalSyncGroupSampleNode, SyncGroupPhaseNode, SyncGroupPhaseRegistry, SyncGroupSampleNode,
    SyncMarker, SyncMarkerPhaseInput, SyncMarkerPosition, SyncPhaseInput, SyncPhaseState,
    advance_marker_sync_phase, advance_sync_phase, follower_sample_time,
    marker_phase_from_sample_time, marker_phase_sample_time, marker_position_at_time,
    normalize_phase, phase_to_time, time_for_marker_position, time_to_phase,
};
pub use time_dilation::unscaled_delta_seconds;
pub use time_position::{TimePositionNode, resolve_position};

use bevy::prelude::*;
use bevy_animation_graph::builtin_nodes::global_input::GlobalInput;
use bevy_animation_graph::core::edge_data::{DataSpec, DataSpecWithOptionalDefault, DataValue};

pub struct AlsExtPlugin;

impl Plugin for AlsExtPlugin {
    fn build(&self, app: &mut App) {
        app.register_asset_loader(CurveAssetLoader)
            .init_asset::<CurveAsset>()
            .init_asset_loader::<static_skeleton::StaticSkeletonLoader>()
            .init_asset_loader::<static_animated_scene::StaticAnimatedSceneLoader>()
            .init_asset::<als_curves::AlsAnimCurves>()
            .init_resource::<SyncGroupPhaseRegistry>()
            .register_type::<AnimationContextInput>()
            .register_type::<AnimationContextState>()
            .register_type::<SourceRelevantAsset>()
            .register_type::<BlendSpace1DPoint>()
            .register_type::<BlendSpace1DNode>()
            .register_type::<CurveSampleNode>()
            .register_type::<CurveDrivenValueNode>()
            .register_type::<ConstantF32Node>()
            .register_type::<CurveSampleSetNode>()
            .register_type::<CurveBlendNode>()
            .register_type::<CurveComposeMode>()
            .register_type::<CurveComposeNode>()
            .register_type::<CurveModifyNode>()
            .register_type::<CurveWeightedSamplesNode>()
            .register_type::<DistanceMatchNode>()
            .register_type::<EventQueueBlendNode>()
            .register_type::<EventLoopNode>()
            .register_type::<F32ToVec2Node>()
            .register_type::<FInterpNode>()
            .register_type::<SuppressFirstUpdateNode>()
            .register_type::<ResetOnActivationNode>()
            .register_type::<StateLabelEventNode>()
            .register_type::<SyncGroupPhaseNode>()
            .register_type::<SyncGroupSampleNode>()
            .register_type::<GlobalSyncGroupSampleNode>()
            .register_type::<SlotNode>()
            .register_type::<DirectTransitionEventNode>()
            .register_type::<GlobalBoneMaskInputNode>()
            .register_type::<GlobalBoolInputNode>()
            .register_type::<GlobalF32InputNode>()
            .register_type::<GlobalPoseInputNode>()
            .register_type::<GameplayTagBlendMode>()
            .register_type::<GameplayTagBlendNode>()
            .register_type::<GameplayTagLayerBlendNode>()
            .register_type::<MeshSpaceAdditiveNode>()
            .register_type::<MeshSpaceDifferenceNode>()
            .register_type::<MeshSpaceLayeredBlendNode>()
            .register_type::<MultiWayBlendNode>()
            .register_type::<CubicVec2AxisFilterNode>()
            .register_type::<CubicVec2FilterNode>()
            .register_type::<NormalizedBlendSpaceNode>()
            .register_type::<AnimationContextInputAlias>()
            .register_type::<GlobalF32InputPin>()
            .register_type::<FixedF32Input>()
            .register_type::<FixedGlobalF32Input>()
            .register_type::<CachedPoseInput>()
            .register_type::<NamedPoseOutput>()
            .register_type::<OutputDataAlias>()
            .register_type::<PoseTimestampNode>()
            .register_type::<RagdollConfigNode>()
            .register_type::<RagdollingTagBlendNode>()
            .register_type::<RichCurveInterpolation>()
            .register_type::<RichCurveTangentMode>()
            .register_type::<RichCurveTangentWeightMode>()
            .register_type::<RichCurveKey>()
            .register_type::<RichCurveSampleNode>()
            .register_type::<AlphaBlendMode>()
            .register_type::<BoneBlendScale>()
            .register_type::<ProfiledBlendNode>()
            .register_type::<InertializationNode>()
            .register_type::<InertializationPassthroughNode>()
            .register_type::<InertializedLinkedAnimLayerNode>()
            .register_type::<GlobalDataInput>()
            .register_type::<InterruptibleFsmNode>()
            .register_type::<StateAliasBinding>()
            .register_type::<SourceTransitionCondition>()
            .register_type::<SourceTransitionConditionGate>()
            .register_type::<SourceEntryCondition>()
            .register_type::<SourceTransitionNotification>()
            .register_type::<StatePoseOutput>()
            .register_type::<TimePositionNode>()
            .register_type::<LinkedAnimLayerNode>()
            .register_type::<LocalRefPoseNode>()
            .register_type::<SourceLocalRefPoseNode>()
            .register_type::<LayeredGraphNode>()
            .register_type::<SkeletonAwareLayeredGraphNode>()
            .register_type::<RootMotionClipNode>()
            .register_type::<SourceCachedPoseSaveNode>()
            .register_type::<SourceCachedPoseUseNode>()
            .register_type::<SourceContextDefault>()
            .register_type::<SourceScopedCachedPoseUseNode>()
            .register_type::<SourceAnimationStateGetterNode>()
            .register_type::<SourceGameplayTagInputNode>()
            .register_type::<SourceLinkedInputPoseNode>()
            .register_type::<SourceApplyAdditiveNode>()
            .register_type::<SourceAlphaMapping>()
            .register_type::<SourceCurveModifyNode>()
            .register_type::<SourceCurvesBlendMode>()
            .register_type::<SourceCurvesBlendNode>()
            .register_type::<SourceGameplayTagBlendNode>()
            .register_type::<SourceBoolBlendNode>()
            .register_type::<SourceMakeDynamicAdditiveNode>()
            .register_type::<SourceMeshSpaceAdditiveNode>()
            .register_type::<SourceMultiWayBlendNode>()
            .register_type::<SourceSetHipsDirectionNode>()
            .register_type::<source_instance::SourcePropertyWriteNode>()
            .register_type::<source_native_refresh::AlsSourceRefreshNode>()
            .register_type::<SourcePosePassthroughNode>()
            .register_type::<SourceScalarDataType>()
            .register_type::<SourceScalarExpressionNode>()
            .register_type::<SourceScalarPin>()
            .register_type::<SourceSequenceEvaluatorNode>()
            .register_type::<source_sequence_player::SourceRootLock>()
            .register_type::<SourceSequencePlayerNode>()
            .register_type::<SourceSlotNode>()
            .register_type::<SourceEnumPoseBlendNode>()
            .register_type::<SourceBranchFilter>()
            .register_type::<SourceLayeredBoneBlendNode>()
            .register_type::<SourceTwoWayBlendNode>()
            .register_type::<ControlRigBoundaryNode>()
            .register_type::<ReferenceMeshDifferenceNode>()
            .register_type::<ReferenceLocalDifferenceNode>()
            .register_type::<GlobalInput>()
            .register_type::<DataSpecWithOptionalDefault>()
            .register_type::<DataSpec>()
            .register_type::<DataValue>();
        AlsCurvesHandler::register(app);
        app.add_plugins((
            root_motion_defaults::RootMotionDefaultsPlugin,
            RagdollPhysicsProfilePlugin,
            RagdollAngularDrivePlugin,
        ))
        .add_systems(
            PostUpdate,
            source_gameplay_tag_input::clear_removed_gameplay_tag_inputs,
        );
    }
}
