pub mod curve;
pub mod loader;
pub mod nodes;

pub use curve::{sample_curve, CurveAsset, CurveKey, CurveSampleMode};
pub use loader::{CurveAssetLoader, CurveLoadError};
pub use nodes::{CurveDrivenValueNode, CurveSampleNode};

use bevy::prelude::*;

pub struct AlsExtPlugin;

impl Plugin for AlsExtPlugin {
    fn build(&self, app: &mut App) {
        app.register_asset_loader(CurveAssetLoader)
            .init_asset::<CurveAsset>()
            .register_type::<CurveSampleNode>()
            .register_type::<CurveDrivenValueNode>();
    }
}
