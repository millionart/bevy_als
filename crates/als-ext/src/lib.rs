pub mod curve;

pub use curve::{CurveAsset, CurveKey, CurveSampleMode};

use bevy::prelude::*;

pub struct AlsExtPlugin;

impl Plugin for AlsExtPlugin {
    fn build(&self, app: &mut App) {
        app.init_asset::<CurveAsset>();
    }
}
