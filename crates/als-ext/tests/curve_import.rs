use als_ext::{AlsExtPlugin, CurveAsset};
use bevy::prelude::*;

const STRIDE_WALK_RON: &str =
    "curves/ALS/Data/AnimationInstance/Grounded/CF_Als_StrideBlendAmount_Walk.curve.ron";

#[test]
fn imported_als_stride_blend_table_loads_and_evaluates() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins).add_plugins(AssetPlugin {
        file_path: "../../assets".into(),
        ..default()
    });
    app.add_plugins(AlsExtPlugin);

    let handle: Handle<CurveAsset> = app
        .world()
        .resource::<AssetServer>()
        .load(STRIDE_WALK_RON);

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
