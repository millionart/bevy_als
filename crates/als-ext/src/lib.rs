pub mod als_curves;
pub mod curve;
pub mod ik;
pub mod loader;
pub mod nodes;
pub mod overlay;
pub mod playground;

pub use als_curves::{
    parse_curves, AlsAnimCurves, AlsCurvesHandler, AlsCurvesParseError, AlsExtrapolation,
    AlsFloatCurve, AlsKeyInterp, AlsPoseCurves, ALS_CURVES_EXTENSION, ALS_CURVES_LABEL,
};
pub use curve::{sample_curve, CurveAsset, CurveKey, CurveSampleMode};
pub use loader::{CurveAssetLoader, CurveLoadError};
pub use nodes::{CurveDrivenValueNode, CurveSampleNode};
pub use overlay::{
    build_bone_mask, build_grouped_bone_mask, mask_coverage, HEAD_LAYER_BONES, LOOK_SPINE_BONES,
    TORCH_ARM_RIGHT_BONES, TORCH_RIGHT_HAND_BONES, TORCH_SPINE_UPPER_BONES,
};
pub use playground::{
    is_engine_basic, playground_setup_system, ue_extents_to_bevy, ue_point_to_bevy,
    ue_quat_to_bevy, ue_rotator_to_bevy_quat, ue_yaw_to_bevy_quat, PlaygroundActor,
    PlaygroundBoxCollider, PlaygroundComponent, PlaygroundManifest, PlaygroundManifestPath,
    PlaygroundMovingObject, PlaygroundPlugin, PlaygroundRotator, PlaygroundSpawnPoint,
    PlaygroundStatic,
};

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
            .register_type::<GlobalInput>()
            .register_type::<DataSpecWithOptionalDefault>()
            .register_type::<DataSpec>()
            .register_type::<DataValue>();
        AlsCurvesHandler::register(app);
    }
}
