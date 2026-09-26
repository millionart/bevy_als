use bevy::{
    asset::{AssetLoader, LoadContext, LoadedAsset, io::Reader},
    gltf::Gltf,
    prelude::*,
    world_serialization::WorldAsset,
};
use bevy_animation_graph::core::{
    animation_clip::EntityPath,
    skeleton::{Skeleton, serial::SkeletonSource},
};
use serde::Deserialize;
use std::io::{Error, ErrorKind};

#[derive(Deserialize)]
struct StaticSkeletonSerial {
    source: SkeletonSource,
    root: String,
}

#[derive(Default, TypePath)]
pub struct StaticSkeletonLoader;

impl AssetLoader for StaticSkeletonLoader {
    type Asset = Skeleton;
    type Settings = ();
    type Error = Error;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &Self::Settings,
        context: &mut LoadContext<'_>,
    ) -> Result<Skeleton, Error> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        let serial: StaticSkeletonSerial = ron::de::from_bytes(&bytes).map_err(Error::other)?;
        let SkeletonSource::Gltf { source, label } = serial.source;
        let gltf: LoadedAsset<Gltf> = context
            .load_builder()
            .load_value(source)
            .await
            .map_err(Error::other)?;
        let scene = gltf
            .get_labeled(label)
            .and_then(|asset| asset.get::<WorldAsset>())
            .ok_or_else(|| Error::new(ErrorKind::InvalidData, "Skeleton scene is missing"))?;
        build_skeleton(&scene.world, &serial.root)
    }

    fn extensions(&self) -> &[&str] {
        &["static.skn.ron"]
    }
}

fn build_skeleton(world: &World, root_name: &str) -> Result<Skeleton, Error> {
    let invalid = || Error::new(ErrorKind::InvalidData, "Invalid static skeleton hierarchy");
    let mut names = world.try_query::<(Entity, &Name)>().ok_or_else(invalid)?;
    let mut roots = names
        .iter(world)
        .filter(|(_, name)| name.as_str() == root_name);
    let (root, _) = roots.next().ok_or_else(invalid)?;
    if roots.next().is_some() {
        return Err(invalid());
    }
    let mut skeleton = Skeleton::default();
    skeleton.set_root(EntityPath::default().child(root_name).id());
    let mut pending = vec![(root, EntityPath::default(), Transform::IDENTITY)];
    while let Some((entity, parent, parent_transform)) = pending.pop() {
        let name = world.get::<Name>(entity).ok_or_else(invalid)?;
        let local = *world.get::<Transform>(entity).ok_or_else(invalid)?;
        let path = parent.child(name.clone());
        if skeleton.has_id(&path.id()) {
            return Err(invalid());
        }
        let global = parent_transform * local;
        skeleton.add_bone(path.clone(), local, global);
        if let Some(children) = world.get::<Children>(entity) {
            pending.extend(children.iter().map(|child| (child, path.clone(), global)));
        }
    }
    Ok(skeleton)
}

#[cfg(test)]
mod tests {
    use bevy::prelude::*;
    use bevy_animation_graph::core::skeleton::{Skeleton, loader::SkeletonLoader};
    use std::time::{Duration, Instant};

    #[test]
    fn static_skeleton_preserves_hierarchy_and_rejects_ambiguous_roots() {
        let mut world = World::new();
        let root_transform = Transform::from_xyz(1.0, 2.0, 3.0)
            .with_rotation(Quat::from_rotation_z(std::f32::consts::FRAC_PI_2));
        let root = world.spawn((Name::new("root"), root_transform)).id();
        let local = Transform::from_xyz(0.0, 2.0, 0.0).with_scale(Vec3::splat(2.0));
        world.spawn((Name::new("end"), local, ChildOf(root)));
        let skeleton = super::build_skeleton(&world, "root").unwrap();
        let path = super::EntityPath::default().child("root").child("end");
        let transforms = skeleton.default_transforms(path.id()).unwrap();
        assert_eq!(transforms.local, local);
        assert!((transforms.character.translation - Vec3::new(-1.0, 2.0, 3.0)).length() < 1e-6);
        assert_eq!(transforms.character.scale, Vec3::splat(2.0));
        assert!(
            transforms
                .character
                .rotation
                .abs_diff_eq(root_transform.rotation, 1e-6)
        );
        assert!(super::build_skeleton(&world, "missing").is_err());
        world.spawn((Name::new("root"), Transform::IDENTITY));
        assert!(super::build_skeleton(&world, "root").is_err());
    }

    #[test]
    fn static_camera_skeleton_loads_without_animation_tracks() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets");
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin {
                file_path: root.to_string_lossy().into_owned(),
                ..default()
            },
            bevy::gltf::GltfPlugin::default(),
        ))
        .init_asset::<bevy::world_serialization::WorldAsset>()
        .init_asset::<bevy::animation::AnimationClip>()
        .init_asset::<bevy::mesh::skinning::SkinnedMeshInverseBindposes>()
        .init_asset::<Mesh>()
        .init_asset::<Image>()
        .init_asset::<StandardMaterial>()
        .init_asset::<Skeleton>()
        .init_asset_loader::<SkeletonLoader>()
        .init_asset_loader::<super::StaticSkeletonLoader>();
        app.finish();
        app.cleanup();
        let handle = app
            .world()
            .resource::<AssetServer>()
            .load::<Skeleton>("skeletons/SKM_Als_Camera.static.skn.ron");
        let deadline = Instant::now() + Duration::from_secs(20);
        loop {
            app.update();
            match app
                .world()
                .resource::<AssetServer>()
                .load_state(handle.id())
            {
                bevy::asset::LoadState::Loaded => break,
                bevy::asset::LoadState::Failed(error) => {
                    panic!("Static source skeleton failed: {error}")
                }
                state => assert!(
                    Instant::now() < deadline,
                    "Skeleton load made no progress: {state:?}"
                ),
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        let skeletons = app.world().resource::<Assets<Skeleton>>();
        let skeleton = skeletons.get(&handle).unwrap();
        let root = bevy_animation_graph::core::animation_clip::EntityPath::default()
            .child("cameraboom_root");
        let end = root.child("cameraboom_end");
        assert_eq!(skeleton.root(), root.id());
        assert_eq!(skeleton.parent(&end.id()), Some(root.id()));
        assert_eq!(
            skeleton
                .iter_paths()
                .filter(|path| !path.parts.is_empty())
                .count(),
            2
        );
        for id in [root.id(), end.id()] {
            assert_eq!(
                skeleton.default_transforms(id).unwrap().local,
                Transform::IDENTITY
            );
        }
    }
}
