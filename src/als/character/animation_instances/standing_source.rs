use crate::extensions::AnimationContextState;
use crate::extensions::{
    als_curves::ClipCurves,
    mesh_space_difference::{make_mesh_space_difference, source_pose_difference},
    source_pose::SourcePose,
    source_sequence_player::sample_source_clip_pose,
};
use bevy::prelude::*;
use serde::Deserialize;
use std::sync::{Arc, LazyLock};

#[derive(Deserialize)]
#[doc(hidden)]
pub struct SourceMontageSettings {
    pub clip: String,
    pub duration: f32,
    pub reference_clip: Option<String>,
    pub curves: crate::extensions::AlsPoseCurves,
}

#[doc(hidden)]
pub static MONTAGES: LazyLock<std::collections::HashMap<String, SourceMontageSettings>> =
    LazyLock::new(|| {
        ron::from_str(include_str!(
            "../../../../assets/ALS/Data/AnimationInstance/Grounded/als_standing_montages.ron"
        ))
        .expect("generated native montage settings are valid")
    });

#[derive(Resource)]
pub(crate) struct SourceMontageAssets(
    std::collections::HashMap<
        String,
        (
            Handle<bevy_animation_graph::core::animation_clip::GraphClip>,
            Option<Handle<bevy_animation_graph::core::animation_clip::GraphClip>>,
        ),
    >,
    Option<ClipCurves>,
);

pub(crate) fn load_source_montage_assets(
    mut commands: Commands,
    server: Res<AssetServer>,
    clip_curves: Option<Res<ClipCurves>>,
) {
    commands.insert_resource(SourceMontageAssets(
        MONTAGES
            .iter()
            .map(|(name, settings)| {
                (
                    name.clone(),
                    (
                        server.load(crate::als_asset_path(&settings.clip)),
                        settings
                            .reference_clip
                            .as_ref()
                            .map(|path| server.load(crate::als_asset_path(path))),
                    ),
                )
            })
            .collect(),
        clip_curves.as_deref().cloned(),
    ));
}

pub(crate) fn sample_source_montage(
    name: &str,
    time: f32,
    assets: &SourceMontageAssets,
    clips: &Assets<bevy_animation_graph::core::animation_clip::GraphClip>,
    skeletons: &Assets<bevy_animation_graph::core::skeleton::Skeleton>,
) -> Option<(
    bevy_animation_graph::core::pose::Pose,
    AnimationContextState,
)> {
    let settings = MONTAGES.get(name)?;
    let (clip, reference) = assets.0.get(name)?;
    let time = time.min(settings.duration);
    let sequence = clips.get(clip)?;
    let mut pose = sample_source_clip_pose(sequence, time, skeletons);
    let skeleton = skeletons.get(&pose.skeleton);
    let mut source_pose =
        skeleton.and_then(|skeleton| SourcePose::from_complete_pose(&pose, skeleton));
    if let Some(reference) = reference {
        let reference = sample_source_clip_pose(clips.get(reference)?, 0.0, skeletons);
        let skeleton = skeleton?;
        source_pose = SourcePose::from_complete_pose(&reference, skeleton)
            .zip(source_pose)
            .and_then(|(base, target)| source_pose_difference(&base, &target, skeleton, true));
        pose = make_mesh_space_difference(&reference, &pose, skeleton);
        if let Some(source) = &source_pose {
            source.project_into(&mut pose);
        }
        pose.timestamp = time;
    }
    let curves = if let Some(mut curves) = assets
        .1
        .as_ref()
        .and_then(|curves| curves.sample(sequence, time))
    {
        // Imported GLBs use source yaw, while the montage fallback uses Bevy yaw.
        if let Some(speed) = curves.get_mut(&crate::extensions::SourceCurveName::from(
            "RotationYawSpeed",
        )) {
            *speed = -*speed;
        }
        curves
    } else {
        settings.curves.sample_all(time)
    };
    let mut context = AnimationContextState {
        curves: curves.into(),
        curves_are_authoritative: true,
        source_pose: source_pose.map(Arc::new),
        ..default()
    };
    context
        .asset_samples
        .push(crate::extensions::animation_context::SourceAssetSample {
            identity: name.to_owned(),
            clip: clip.clone(),
            montage: true,
            clock: crate::extensions::SourceRelevantAsset {
                duration: settings.duration,
                position: time,
                weight: 1.0,
                ..default()
            },
            ..default()
        });
    Some((pose, context))
}

#[derive(Deserialize)]
struct StandingCurves {
    velocity_blend_interpolation_half_life: f32,
    stride_walk: crate::extensions::als_curves::AlsFloatCurve,
    stride_run: crate::extensions::als_curves::AlsFloatCurve,
    yaw_forward_backward: crate::extensions::als_curves::AlsFloatCurve,
    yaw_left: crate::extensions::als_curves::AlsFloatCurve,
    yaw_right: crate::extensions::als_curves::AlsFloatCurve,
}

static CURVES: LazyLock<StandingCurves> = LazyLock::new(|| {
    serde_json::from_str(include_str!(
        "../../../../assets/ALS/Data/AnimationInstance/Grounded/als_standing_curves.json"
    ))
    .expect("generated Standing curves are valid")
});

pub(crate) fn velocity_blend_interpolation_half_life() -> f32 {
    CURVES.velocity_blend_interpolation_half_life
}

pub(crate) fn standing_stride_amount(running: f32, speed: f32) -> f32 {
    CURVES
        .stride_walk
        .sample(speed)
        .lerp(CURVES.stride_run.sample(speed), running)
}

pub(crate) fn standing_rotation_yaw_offsets(runtime_angle: f32) -> (f32, f32, f32, f32) {
    let source_angle = -runtime_angle;
    let forward_backward = -CURVES.yaw_forward_backward.sample(source_angle);
    (
        forward_backward,
        forward_backward,
        -CURVES.yaw_left.sample(source_angle),
        -CURVES.yaw_right.sample(source_angle),
    )
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use bevy_animation_graph::core::{animation_clip::GraphClip, skeleton::Skeleton};

    #[test]
    fn standing_yaw_offset_samples_native_key_order() {
        let cases: serde_json::Value = serde_json::from_str(include_str!(
            "../../../tests/fixtures/actions/standing_yaw_curve.json",
        ))
        .unwrap();
        for row in cases.as_array().unwrap() {
            let reference = &row["reference"];
            let angle = reference["view_space_degrees"].as_f64().unwrap() as f32;
            let expected = reference["curve"].as_f64().unwrap() as f32;
            let left = row["left"].as_f64().unwrap() as f32;
            let right = row["right"].as_f64().unwrap() as f32;
            assert_eq!(
                standing_rotation_yaw_offsets(-angle),
                (-expected, -expected, -left, -right),
                "{}",
                row["name"]
            );
        }
    }

    pub(crate) fn loaded_montage_app() -> App {
        use bevy_animation_graph::core::{
            animation_clip::loader::GraphClipLoader, skeleton::loader::SkeletonLoader,
        };
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            bevy::gltf::GltfPlugin::default(),
        ))
        .init_asset::<bevy::world_serialization::WorldAsset>()
        .init_asset::<bevy::animation::AnimationClip>()
        .init_asset::<bevy::mesh::skinning::SkinnedMeshInverseBindposes>()
        .init_asset::<Mesh>()
        .init_asset::<Image>()
        .init_asset::<GraphClip>()
        .init_asset_loader::<GraphClipLoader>()
        .init_asset::<Skeleton>()
        .init_asset_loader::<SkeletonLoader>()
        .add_plugins(crate::extensions::AlsExtPlugin)
        .add_systems(Startup, load_source_montage_assets);
        app.finish();
        app.cleanup();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
        let mut previous = 0;
        loop {
            app.update();
            let assets = app.world().resource::<SourceMontageAssets>();
            let clips = app.world().resource::<Assets<GraphClip>>();
            let skeletons = app.world().resource::<Assets<Skeleton>>();
            let loaded = MONTAGES
                .keys()
                .filter(|name| sample_source_montage(name, 0.4, assets, clips, skeletons).is_some())
                .count();
            if loaded != previous {
                eprintln!(
                    "reference montage assets loaded {loaded}/{}",
                    MONTAGES.len()
                );
                previous = loaded;
            }
            if loaded == MONTAGES.len() {
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "reference montage assets did not load within 30 seconds: {loaded}"
            );
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        app
    }

    #[test]
    fn native_montage_assets_sample_actual_poses_and_signed_curves() {
        let app = loaded_montage_app();
        let assets = app.world().resource::<SourceMontageAssets>();
        let clips = app.world().resource::<Assets<GraphClip>>();
        let skeletons = app.world().resource::<Assets<Skeleton>>();
        for (name, settings) in MONTAGES.iter() {
            let (pose, context) =
                sample_source_montage(name, 0.4, assets, clips, skeletons).unwrap();
            assert_eq!(pose.paths.len(), 79, "{name}");
            assert!(
                pose.bones
                    .iter()
                    .all(|bone| bone.translation.is_none_or(Vec3::is_finite)
                        && bone.rotation.is_none_or(Quat::is_finite)
                        && bone.scale.is_none_or(Vec3::is_finite)),
                "{name}"
            );
            assert!(context.curves_are_authoritative);
            if name.contains("_Turn_") {
                let yaw =
                    context.curves[&crate::extensions::SourceCurveName::from("RotationYawSpeed")];
                assert!(
                    if name.ends_with("_Left") {
                        yaw > 0.0
                    } else {
                        yaw < 0.0
                    },
                    "{name}: {yaw}"
                );
                assert!(settings.reference_clip.is_none());
            } else {
                assert!(settings.reference_clip.is_some());
            }
            assert!(context.curves.values().all(|value| value.is_finite()));
        }
    }
}
