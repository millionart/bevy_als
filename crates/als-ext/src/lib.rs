//! Extension layer over bevy_animation_graph / bevy-tnua: curve assets and nodes, analytic IK, ragdoll config node, ALS float-curve import.

pub mod als_curves;
pub mod blend_space_1d;
pub mod curve;
pub mod curve_bundle;
pub mod curve_weighted_samples;
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
pub mod inertialization;
pub mod inertialized_linked_anim_layer;
pub mod interruptible_fsm;
pub mod layered_graph;
pub mod linked_anim_layer;
pub mod loader;
pub mod local_ref_pose;
pub mod mesh_space_additive;
pub mod mesh_space_difference;
pub mod mesh_space_layered_blend;
pub mod multi_way_blend;
pub mod nodes;
pub mod normalized_blendspace;
pub mod overlay;
pub mod overlay_materials;
pub mod penetration_recovery;
pub mod playground;
pub mod playground_materials;
pub mod profile_blend;
pub mod ragdoll_angular_drive;
pub mod ragdoll_config_node;
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
pub mod state_label_event;
pub mod sync_group_phase;
pub mod time_dilation;
pub mod time_position;

pub use als_curves::{
    ALS_CURVES_EXTENSION, ALS_CURVES_LABEL, AlsAnimCurves, AlsCurvesHandler, AlsCurvesParseError,
    AlsExtrapolation, AlsFloatCurve, AlsKeyInterp, AlsPoseCurves, parse_curves,
};
pub use blend_space_1d::{
    BlendSpace1DNode, BlendSpace1DPoint, blend_weights as blend_space_1d_weights,
    pose_input as blend_space_1d_pose_input, time_input as blend_space_1d_time_input,
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
pub use inertialization::{
    InertializationNode, apply_inertial_residual, capture_inertial_residual, inertial_alpha,
    inertial_residual_weight,
};
pub use inertialized_linked_anim_layer::{
    InertializedLinkedAnimLayerNode, InertializedLinkedLayerActivation,
    inertialized_linked_layer_activation,
};
pub use interruptible_fsm::{InterruptibleFsmNode, InterruptibleFsmSource, InterruptibleFsmState};
pub use layered_graph::LayeredGraphNode;
pub use linked_anim_layer::{ALS_LAYER_CURVE_NAMES, ALS_LAYER_SIGNAL_NAMES, LinkedAnimLayerNode};
pub use loader::{CurveAssetLoader, CurveLoadError};
pub use local_ref_pose::LocalRefPoseNode;
pub use mesh_space_additive::{MeshSpaceAdditiveNode, apply_mesh_space_additive};
pub use mesh_space_difference::{MeshSpaceDifferenceNode, make_mesh_space_difference};
pub use mesh_space_layered_blend::{MeshSpaceLayeredBlendNode, apply_mesh_space_layered_blend};
pub use multi_way_blend::MultiWayBlendNode;
pub use nodes::{ConstantF32Node, CurveDrivenValueNode, CurveSampleNode};
pub use normalized_blendspace::{
    CubicVec2FilterNode, NormalizedBlendSpaceNode, NormalizedBlendSpacePoint,
    blend_normalized_root_motion,
};
pub use overlay::{
    BranchFilter, HEAD_LAYER_BONES, LOOK_SPINE_BONES, TORCH_ARM_RIGHT_BONES,
    TORCH_RIGHT_HAND_BONES, TORCH_SPINE_UPPER_BONES, build_bone_mask, build_branch_bone_mask,
    build_branch_filter_bone_mask, build_grouped_bone_mask, mask_coverage,
};
pub use penetration_recovery::sum_penetration_adjustments;
pub use playground::{
    PLAYGROUND_SHADOW_DEPTH_BIAS, PLAYGROUND_SHADOW_NORMAL_BIAS, PlaygroundActor,
    PlaygroundBoxCollider, PlaygroundColliderKind, PlaygroundCollisionSource, PlaygroundComponent,
    PlaygroundKinematic, PlaygroundKinematicKind, PlaygroundKinematicVelocity, PlaygroundLandscape,
    PlaygroundLandscapeTilesMaterial, PlaygroundManifest, PlaygroundManifestPath,
    PlaygroundMaterialApplied, PlaygroundMaterialRole, PlaygroundMaterialTint,
    PlaygroundMovingCurve, PlaygroundMovingObject, PlaygroundPhysicsDriven, PlaygroundPlugin,
    PlaygroundRotator, PlaygroundSky, PlaygroundStatic, PlaygroundTextFacing, PlaygroundTextFit,
    PlaygroundTextGlyphColor, PlaygroundTextHorizontalAlignment, PlaygroundTextLabel,
    PlaygroundTextLayoutTarget, PlaygroundTextVerticalAlignment, PlaygroundWorldTextMesh,
    is_engine_basic, moving_object_curve_input, moving_object_position, moving_object_velocity,
    playground_collider_kind, playground_moving_object_system, playground_setup_system,
    ue_directional_light_ray_to_bevy, ue_extents_to_bevy, ue_point_to_bevy, ue_quat_to_bevy,
    ue_rotator_to_bevy_quat, ue_scale_to_bevy, ue_yaw_to_bevy_quat,
};
pub use profile_blend::{
    AlphaBlendMode, BoneBlendScale, ProfiledBlendNode, curve_a_input, curve_b_input,
    profile_target_weight, transition_alpha,
};
pub use ragdoll_angular_drive::{
    RagdollAngularDrive, RagdollAngularDriveMetrics, RagdollAngularDrivePlugin,
    RagdollAngularDriveSet, angular_drive_torque,
};
pub use ragdoll_config_node::RagdollConfigNode;
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
            .init_asset::<als_curves::AlsAnimCurves>()
            .init_resource::<SyncGroupPhaseRegistry>()
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
            .register_type::<CubicVec2FilterNode>()
            .register_type::<NormalizedBlendSpaceNode>()
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
            .register_type::<InertializedLinkedAnimLayerNode>()
            .register_type::<InterruptibleFsmNode>()
            .register_type::<TimePositionNode>()
            .register_type::<LinkedAnimLayerNode>()
            .register_type::<LocalRefPoseNode>()
            .register_type::<LayeredGraphNode>()
            .register_type::<SkeletonAwareLayeredGraphNode>()
            .register_type::<RootMotionClipNode>()
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
            RagdollAngularDrivePlugin,
        ));
    }
}
