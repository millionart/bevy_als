use crate::extensions::AnimationContextState;
use bevy::prelude::*;
use serde::Deserialize;
use std::sync::LazyLock;

#[derive(Deserialize)]
pub(crate) struct SourceMontageSettings {
    pub clip: String,
    pub duration: f32,
    pub reference_clip: Option<String>,
    pub curves: crate::extensions::AlsPoseCurves,
}

pub(crate) static MONTAGES: LazyLock<std::collections::HashMap<String, SourceMontageSettings>> =
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
);

pub(crate) fn load_source_montage_assets(mut commands: Commands, server: Res<AssetServer>) {
    commands.insert_resource(SourceMontageAssets(
        MONTAGES
            .iter()
            .map(|(name, settings)| {
                (
                    name.clone(),
                    (
                        server.load(&settings.clip),
                        settings
                            .reference_clip
                            .as_ref()
                            .map(|path| server.load(path)),
                    ),
                )
            })
            .collect(),
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
    let mut pose = crate::extensions::source_sequence_player::sample_pose(clips.get(clip)?, time);
    if let Some(reference) = reference {
        let reference = crate::extensions::source_sequence_player::sample_pose(clips.get(reference)?, 0.0);
        pose = crate::extensions::mesh_space_difference::make_mesh_space_difference(
            &reference,
            &pose,
            skeletons.get(&pose.skeleton)?,
        );
        pose.timestamp = time;
    }
    let mut context = crate::get_up::sampled_curve_context(&settings.curves, time);
    context.asset_samples.push(crate::extensions::animation_context::SourceAssetSample {
        identity: name.to_owned(), clip: clip.clone(), montage: true,
        clock: crate::extensions::SourceRelevantAsset { duration: settings.duration, position: time, weight: 1.0, ..default() },
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
    serde_json::from_str(include_str!("../../../../assets/ALS/Data/AnimationInstance/Grounded/als_standing_curves.json"))
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
    let forward_backward = CURVES.yaw_forward_backward.sample(runtime_angle);
    (
        forward_backward,
        forward_backward,
        CURVES.yaw_left.sample(runtime_angle),
        CURVES.yaw_right.sample(runtime_angle),
    )
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use bevy_animation_graph::core::{animation_clip::GraphClip, skeleton::Skeleton};

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
                eprintln!("native montage assets loaded {loaded}/{}", MONTAGES.len());
                previous = loaded;
            }
            if loaded == MONTAGES.len() {
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "native montage assets did not load within 30 seconds: {loaded}"
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
                let yaw = context.curves[&crate::extensions::SourceCurveName::from("RotationYawSpeed")];
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
