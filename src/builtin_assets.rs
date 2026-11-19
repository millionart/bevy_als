use bevy::{
    asset::{
        AssetPath, LoadContext,
        io::{
            AssetSourceId,
            embedded::{EMBEDDED, EmbeddedAssetRegistry},
        },
    },
    prelude::{App, Resource},
};
use std::path::{Path, PathBuf};

static ASSETS: &[(&str, &str, &[u8])] = include!(concat!(env!("OUT_DIR"), "/als_assets.rs"));

/// Returns a bundled ALS asset's bytes by its path relative to the asset root.
pub fn als_asset_bytes(path: &str) -> Option<&'static [u8]> {
    let index = ASSETS.binary_search_by_key(&path, |asset| asset.0).ok()?;
    Some(ASSETS[index].2)
}

/// Resolves a bundled ALS asset path or leaves an explicit host asset path intact.
pub fn als_asset_path(path: &str) -> AssetPath<'static> {
    let path = AssetPath::parse(path).into_owned();
    if path.source() == &AssetSourceId::Default
        && path.path().to_str().and_then(als_asset_bytes).is_some()
    {
        path.with_source(EMBEDDED)
    } else {
        path
    }
}

pub(crate) fn dependency_path(context: &LoadContext<'_>, path: &str) -> AssetPath<'static> {
    let path = AssetPath::parse(path).into_owned();
    if path.source() == &AssetSourceId::Default {
        path.with_source(context.path().source().clone_owned())
    } else {
        path
    }
}

#[derive(Resource)]
struct BundledAssetsInstalled;

pub(crate) fn install(app: &mut App) {
    if app.world().contains_resource::<BundledAssetsInstalled>() {
        return;
    }
    let Some(registry) = app.world().get_resource::<EmbeddedAssetRegistry>() else {
        return;
    };
    for &(path, compiled, bytes) in ASSETS {
        registry.insert_asset(PathBuf::from(compiled), Path::new(path), bytes);
    }
    app.insert_resource(BundledAssetsInstalled);
}
