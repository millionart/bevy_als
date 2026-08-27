//! Extension layer over bevy_animation_graph / bevy-tnua: curve assets and nodes, analytic IK, ragdoll config node, ALS float-curve import.

pub mod als_curves;
pub mod curve;
pub mod curve_bundle;
pub mod curve_weighted_samples;
pub mod distance_match;
pub mod f_interp;
pub mod first_update;
pub mod gameplay_tag_blend;
pub mod gameplay_tag_layer_blend;
pub mod global_f32_input;
pub mod ik;
pub mod layered_graph;
pub mod linked_anim_layer;
pub mod loader;
pub mod mesh_space_additive;
pub mod multi_way_blend;
pub mod nodes;
pub mod overlay;
pub mod playground;
pub mod profile_blend;
pub mod ragdoll_config_node;
pub mod reset_on_activation;
pub mod rich_curve;
pub mod root_motion_clip;
pub mod root_motion_defaults;
pub mod time_position;

pub use als_curves::{
    ALS_CURVES_EXTENSION, ALS_CURVES_LABEL, AlsAnimCurves, AlsCurvesHandler, AlsCurvesParseError,
    AlsExtrapolation, AlsFloatCurve, AlsKeyInterp, AlsPoseCurves, parse_curves,
};
pub use curve::{CurveAsset, CurveKey, CurveSampleMode, sample_curve};
pub use curve_bundle::{
    CurveBlendNode, CurveModifyNode, CurveSampleSetNode, NamedCurve, active_curve_weights,
    blend_curve_value, curve_modify_input, curve_sample_output, curve_value_input,
    curve_weight_input,
};
pub use curve_weighted_samples::CurveWeightedSamplesNode;
pub use distance_match::{
    DistanceMatchNode, default_stop_distance_curve, invert_distance_curve, sample_distance_curve,
};
pub use f_interp::{FInterpNode, f_interp_to};
pub use first_update::SuppressFirstUpdateNode;
pub use gameplay_tag_blend::{
    GameplayTagBlendMode, GameplayTagBlendNode, weight_output as gameplay_tag_weight_output,
};
pub use gameplay_tag_layer_blend::GameplayTagLayerBlendNode;
pub use global_f32_input::GlobalF32InputNode;
pub use layered_graph::LayeredGraphNode;
pub use linked_anim_layer::{ALS_LAYER_CURVE_NAMES, ALS_LAYER_SIGNAL_NAMES, LinkedAnimLayerNode};
pub use loader::{CurveAssetLoader, CurveLoadError};
pub use mesh_space_additive::{MeshSpaceAdditiveNode, apply_mesh_space_additive};
pub use multi_way_blend::MultiWayBlendNode;
pub use nodes::{ConstantF32Node, CurveDrivenValueNode, CurveSampleNode};
pub use overlay::{
    HEAD_LAYER_BONES, LOOK_SPINE_BONES, TORCH_ARM_RIGHT_BONES, TORCH_RIGHT_HAND_BONES,
    TORCH_SPINE_UPPER_BONES, build_bone_mask, build_grouped_bone_mask, mask_coverage,
};
pub use playground::{
    PlaygroundActor, PlaygroundBoxCollider, PlaygroundComponent, PlaygroundLandscape,
    PlaygroundManifest, PlaygroundManifestPath, PlaygroundMovingObject, PlaygroundMovingOffset,
    PlaygroundPlugin, PlaygroundRotator, PlaygroundSpawnPoint, PlaygroundStatic,
    PlaygroundTextLabel, is_engine_basic, playground_moving_object_system, playground_setup_system,
    ue_extents_to_bevy, ue_point_to_bevy, ue_quat_to_bevy, ue_rotator_to_bevy_quat,
    ue_yaw_to_bevy_quat,
};
pub use profile_blend::{
    AlphaBlendMode, BoneBlendScale, ProfiledBlendNode, curve_a_input, curve_b_input,
    profile_target_weight, transition_alpha,
};
pub use ragdoll_config_node::RagdollConfigNode;
pub use reset_on_activation::{ResetOnActivationNode, activation_reset_required};
pub use rich_curve::{
    RichCurveInterpolation, RichCurveKey, RichCurveSampleNode, RichCurveTangentMode,
    RichCurveTangentWeightMode, sample_rich_curve,
};
pub use root_motion_clip::RootMotionClipNode;
pub use root_motion_defaults::{derive_default_root_bone, select_shallowest_animated_path};
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
            .register_type::<CurveSampleNode>()
            .register_type::<CurveDrivenValueNode>()
            .register_type::<ConstantF32Node>()
            .register_type::<CurveSampleSetNode>()
            .register_type::<CurveBlendNode>()
            .register_type::<CurveModifyNode>()
            .register_type::<CurveWeightedSamplesNode>()
            .register_type::<DistanceMatchNode>()
            .register_type::<FInterpNode>()
            .register_type::<SuppressFirstUpdateNode>()
            .register_type::<ResetOnActivationNode>()
            .register_type::<GlobalF32InputNode>()
            .register_type::<GameplayTagBlendMode>()
            .register_type::<GameplayTagBlendNode>()
            .register_type::<GameplayTagLayerBlendNode>()
            .register_type::<MeshSpaceAdditiveNode>()
            .register_type::<MultiWayBlendNode>()
            .register_type::<RagdollConfigNode>()
            .register_type::<RichCurveInterpolation>()
            .register_type::<RichCurveTangentMode>()
            .register_type::<RichCurveTangentWeightMode>()
            .register_type::<RichCurveKey>()
            .register_type::<RichCurveSampleNode>()
            .register_type::<AlphaBlendMode>()
            .register_type::<BoneBlendScale>()
            .register_type::<ProfiledBlendNode>()
            .register_type::<TimePositionNode>()
            .register_type::<LinkedAnimLayerNode>()
            .register_type::<LayeredGraphNode>()
            .register_type::<RootMotionClipNode>()
            .register_type::<GlobalInput>()
            .register_type::<DataSpecWithOptionalDefault>()
            .register_type::<DataSpec>()
            .register_type::<DataValue>();
        AlsCurvesHandler::register(app);
        app.add_plugins(root_motion_defaults::RootMotionDefaultsPlugin);
    }
}
