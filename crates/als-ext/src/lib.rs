//! Extension layer over bevy_animation_graph / bevy-tnua: curve assets and nodes, analytic IK, ragdoll config node, ALS float-curve import.

pub mod als_curves;
pub mod curve;
pub mod distance_match;
pub mod ik;
pub mod layered_graph;
pub mod loader;
pub mod nodes;
pub mod overlay;
pub mod ragdoll_config_node;
pub mod root_motion_clip;
pub mod playground;
pub mod playground_materials;
pub mod overlay_materials;
pub mod root_motion_defaults;
pub mod time_position;

pub use als_curves::{
    parse_curves, AlsAnimCurves, AlsCurvesHandler, AlsCurvesParseError, AlsExtrapolation,
    AlsFloatCurve, AlsKeyInterp, AlsPoseCurves, ALS_CURVES_EXTENSION, ALS_CURVES_LABEL,
};
pub use curve::{sample_curve, CurveAsset, CurveKey, CurveSampleMode};
pub use distance_match::{
    default_stop_distance_curve, invert_distance_curve, sample_distance_curve,
    DistanceMatchNode,
};
pub use layered_graph::LayeredGraphNode;
pub use root_motion_clip::RootMotionClipNode;
pub use loader::{CurveAssetLoader, CurveLoadError};
pub use nodes::{CurveDrivenValueNode, CurveSampleNode};
pub use ragdoll_config_node::RagdollConfigNode;
pub use overlay::{
    build_bone_mask, build_grouped_bone_mask, mask_coverage, HEAD_LAYER_BONES, LOOK_SPINE_BONES,
    TORCH_ARM_RIGHT_BONES, TORCH_RIGHT_HAND_BONES, TORCH_SPINE_UPPER_BONES,
};
pub use playground::{
    is_engine_basic, moving_object_curve_input, moving_object_position, moving_object_velocity,
    playground_moving_object_system, playground_setup_system,
    ue_directional_light_ray_to_bevy, ue_extents_to_bevy, ue_point_to_bevy, ue_quat_to_bevy,
    ue_rotator_to_bevy_quat, ue_scale_to_bevy, ue_yaw_to_bevy_quat, PlaygroundActor,
    PlaygroundBoxCollider, PlaygroundComponent, PlaygroundKinematic, PlaygroundKinematicKind,
    PlaygroundKinematicVelocity,
    PlaygroundLandscape, PlaygroundLandscapeTilesMaterial, PlaygroundManifest,
    PlaygroundManifestPath, PlaygroundMaterialApplied, PlaygroundMovingCurve,
    PlaygroundMaterialRole, PlaygroundMaterialTint, PlaygroundMovingObject, PlaygroundPlugin,
    PlaygroundRotator, PlaygroundSky, PlaygroundSpawnPoint, PlaygroundStatic, PlaygroundTextFacing,
    PlaygroundTextFit, PlaygroundTextGlyphColor, PlaygroundTextHorizontalAlignment, PlaygroundTextLabel,
    PlaygroundTextLayoutTarget, PlaygroundTextVerticalAlignment, PlaygroundWorldTextMesh,
    PLAYGROUND_SHADOW_DEPTH_BIAS, PLAYGROUND_SHADOW_NORMAL_BIAS,
};
pub use root_motion_defaults::{derive_default_root_bone, select_shallowest_animated_path};
pub use time_position::{resolve_position, TimePositionNode};

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
            .register_type::<DistanceMatchNode>()
        .register_type::<DistanceMatchNode>()
        .register_type::<RagdollConfigNode>()
        .register_type::<TimePositionNode>()
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
