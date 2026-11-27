use bevy::asset::io::Reader;
use bevy::asset::{AssetLoader, LoadContext};
use bevy::reflect::TypePath;

use crate::extensions::curve::CurveAsset;

#[derive(TypePath)]
pub struct CurveAssetLoader;

impl CurveAssetLoader {
    pub const EXTENSION: &'static str = "curve.ron";
}

impl AssetLoader for CurveAssetLoader {
    type Asset = CurveAsset;
    type Settings = ();
    type Error = CurveLoadError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &Self::Settings,
        _load_context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        let asset: CurveAsset = ron::de::from_bytes(&bytes)?;
        if let Some(index) = asset.keys.iter().position(|key| !key.t.is_finite()) {
            return Err(CurveLoadError::NonFiniteKeyTime(index));
        }
        Ok(CurveAsset::new(asset.mode, asset.keys))
    }

    fn extensions(&self) -> &[&str] {
        &[Self::EXTENSION]
    }
}

#[derive(Debug)]
pub enum CurveLoadError {
    Io(std::io::Error),
    Ron(ron::de::SpannedError),
    NonFiniteKeyTime(usize),
}

impl std::fmt::Display for CurveLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "io error reading curve asset: {e}"),
            Self::Ron(e) => write!(f, "failed to parse curve RON: {e}"),
            Self::NonFiniteKeyTime(index) => write!(f, "non-finite key time at index {index}"),
        }
    }
}

impl std::error::Error for CurveLoadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            Self::Ron(e) => Some(e),
            Self::NonFiniteKeyTime(_) => None,
        }
    }
}

impl From<std::io::Error> for CurveLoadError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

impl From<ron::de::SpannedError> for CurveLoadError {
    fn from(e: ron::de::SpannedError) -> Self {
        Self::Ron(e)
    }
}
