use bevy::prelude::*;
use bevy_als::extensions::{AlsExtPlugin, CurveAsset};

const STRIDE_WALK_RON: &str =
    "ALS/Data/AnimationInstance/Grounded/CF_Als_StrideBlendAmount_Walk.curve.ron";

#[test]
fn imported_als_stride_blend_table_loads_and_evaluates() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins).add_plugins(AssetPlugin {
        file_path: "assets".into(),
        ..default()
    });
    app.add_plugins(AlsExtPlugin);

    let handle: Handle<CurveAsset> = app.world().resource::<AssetServer>().load(STRIDE_WALK_RON);

    let mut frames = 0;
    let asset = loop {
        app.update();
        if let Some(a) = app.world().resource::<Assets<CurveAsset>>().get(&handle) {
            break a.clone();
        }
        frames += 1;
        assert!(frames < 600, "curve asset did not load within 600 frames");
    };

    assert_eq!(asset.keys.len(), 129);
    assert_eq!(asset.domain(), Some((0.0, 150.0)));

    assert_eq!(asset.sample(0.0), asset.keys[0].v);
    assert_eq!(asset.sample(150.0), asset.keys[128].v);
    assert!((asset.sample(0.0) - 0.2).abs() < 1e-3);
    assert!((asset.sample(150.0) - 1.0).abs() < 1e-6);

    let mid = asset.sample(75.0);
    assert_eq!(mid, asset.keys[64].v);

    let a = &asset.keys[10];
    let b = &asset.keys[11];
    let half_t = (a.t + b.t) * 0.5;
    let half_v = (a.v + b.v) * 0.5;
    assert!((asset.sample(half_t) - half_v).abs() < 1e-6);
}

#[test]
fn loaded_curves_sort_keys_and_reject_non_finite_times() {
    use bevy::asset::io::{
        AssetSourceBuilder, AssetSourceId,
        memory::{Dir, MemoryAssetReader},
    };
    use std::{
        path::Path,
        time::{Duration, Instant},
    };

    let source = Dir::default();
    source.insert_asset_text(
        Path::new("unsorted.curve.ron"),
        "(mode: Linear, keys: [(t: 0.0, v: 0.0), (t: 2.0, v: 2.0), (t: 1.0, v: 1.0)])",
    );
    source.insert_asset_text(
        Path::new("non_finite.curve.ron"),
        "(mode: Linear, keys: [(t: NaN, v: 0.0), (t: 1.0, v: 1.0)])",
    );
    let mut app = App::new();
    app.register_asset_source(
        AssetSourceId::Default,
        AssetSourceBuilder::new(move || Box::new(MemoryAssetReader { root: source.clone() })),
    )
    .add_plugins((MinimalPlugins, AssetPlugin::default(), AlsExtPlugin));
    let handle: Handle<CurveAsset> = app
        .world()
        .resource::<AssetServer>()
        .load("unsorted.curve.ron");
    let invalid: Handle<CurveAsset> = app
        .world()
        .resource::<AssetServer>()
        .load("non_finite.curve.ron");
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        app.update();
        if let Some(curve) = app.world().resource::<Assets<CurveAsset>>().get(&handle) {
            assert_eq!(curve.domain(), Some((0.0, 2.0)));
            assert_eq!(curve.sample(1.5), 1.5);
            break;
        }
        assert!(Instant::now() < deadline, "curve asset did not load");
        std::thread::yield_now();
    }
    loop {
        use bevy::asset::LoadState;

        app.update();
        let server = app.world().resource::<AssetServer>();
        match server.load_state(invalid.id()) {
            LoadState::Failed(error) => {
                assert!(error.to_string().contains("non-finite key time"), "{error}");
                break;
            }
            LoadState::Loaded => panic!("loaded a curve with a non-finite key time"),
            _ => assert!(Instant::now() < deadline, "invalid curve did not finish loading"),
        }
        std::thread::yield_now();
    }
}

#[test]
fn non_finite_curve_samples_preserve_their_meaning() {
    use bevy_als::extensions::{CurveKey, CurveSampleMode};

    let curve = CurveAsset::new(
        CurveSampleMode::Linear,
        vec![CurveKey { t: 0.0, v: 0.0 }, CurveKey { t: 1.0, v: 1.0 }],
    );
    assert!(curve.sample(f32::NAN).is_nan());
    assert_eq!(curve.sample(f32::INFINITY), 1.0);
    assert_eq!(curve.sample(f32::NEG_INFINITY), 0.0);
}
