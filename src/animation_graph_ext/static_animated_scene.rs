use bevy::{
    animation::{AnimatedBy, AnimationPlayer, AnimationTargetId},
    asset::{AssetLoader, AssetPath, LoadContext, LoadedAsset, io::Reader},
    ecs::reflect::AppTypeRegistry,
    gltf::Gltf,
    mesh::skinning::SkinnedMesh,
    prelude::*,
    world_serialization::WorldAsset,
};
use bevy_animation_graph::core::{
    animated_scene::AnimatedScene, animation_clip::EntityPath, animation_graph::AnimationGraph,
    animation_graph_player::AnimationGraphPlayer, skeleton::Skeleton,
};
use bevy_gltf::{
    extensions::{ErasedGltfExtensionHandler, GltfExtensionHandler, GltfExtensionHandlers},
    gltf,
};
use serde::Deserialize;
use std::io::{Error, ErrorKind};

#[derive(Clone)]
pub(crate) struct SkeletalModelBindings;

impl SkeletalModelBindings {
    pub(crate) fn register(app: &mut App) {
        app.init_resource::<GltfExtensionHandlers>();
        app.world()
            .resource::<GltfExtensionHandlers>()
            .0
            .write_blocking()
            .push(Box::new(Self));
    }
}

impl GltfExtensionHandler for SkeletalModelBindings {
    fn dyn_clone(&self) -> Box<dyn ErasedGltfExtensionHandler> {
        Box::new(self.clone())
    }

    fn on_scene_completed(
        &mut self,
        context: &mut LoadContext<'_>,
        scene: &gltf::Scene,
        world_root: Entity,
        world: &mut World,
    ) {
        if world
            .try_query_filtered::<Entity, With<AnimationPlayer>>()
            .is_some_and(|mut query| query.iter(world).next().is_some())
        {
            return;
        }
        let Some(root) = skinned_model_root(world_root, world) else {
            return;
        };
        if bind_model_targets(scene, world_root, root, world).is_none() {
            warn!(
                "{}: could not bind the model's animation hierarchy",
                context.path()
            );
        }
    }
}

fn skinned_model_root(scene_root: Entity, world: &World) -> Option<Entity> {
    let root_of = |mut entity| loop {
        let parent = world.get::<ChildOf>(entity)?.parent();
        if parent == scene_root {
            return Some(entity);
        }
        entity = parent;
    };
    let mut meshes = world.try_query::<&SkinnedMesh>()?;
    let mut roots = meshes
        .iter(world)
        .flat_map(|mesh| mesh.joints.iter().copied())
        .map(root_of);
    let root = roots.next()??;
    roots
        .all(|candidate| candidate == Some(root))
        .then_some(root)
}

fn bind_model_targets(
    scene: &gltf::Scene,
    world_root: Entity,
    model_root: Entity,
    world: &mut World,
) -> Option<()> {
    let node_name = |node: &gltf::Node| {
        node.name()
            .map_or_else(|| format!("GltfNode{}", node.index()), str::to_owned)
    };
    let root_name = world.get::<Name>(model_root)?.as_str();
    let node = scene.nodes().find(|node| node_name(node) == root_name)?;
    let mut pending = vec![(node, world_root, EntityPath::default())];
    let mut targets = Vec::new();
    while let Some((node, parent, path)) = pending.pop() {
        let name = node_name(&node);
        let entity = world.get::<Children>(parent)?.iter().find(|&child| {
            world
                .get::<Name>(child)
                .is_some_and(|label| label.as_str() == name)
        })?;
        let path = path.child(name);
        targets.push((entity, AnimationTargetId(path.id().id())));
        pending.extend(node.children().map(|child| (child, entity, path.clone())));
    }
    // The graph loader replaces the Bevy player while preserving the scene's ragdoll settings.
    let root = targets[0].0;
    world.entity_mut(root).insert(AnimationPlayer::default());
    for (entity, id) in targets {
        world.entity_mut(entity).insert((id, AnimatedBy(root)));
    }
    Some(())
}

#[derive(Deserialize)]
struct StaticAnimatedSceneSerial {
    source: AssetPath<'static>,
    animation_graph: AssetPath<'static>,
    skeleton: AssetPath<'static>,
}

#[derive(TypePath)]
pub struct StaticAnimatedSceneLoader {
    registry: AppTypeRegistry,
}

impl FromWorld for StaticAnimatedSceneLoader {
    fn from_world(world: &mut World) -> Self {
        Self {
            registry: AppTypeRegistry(world.resource::<AppTypeRegistry>().0.clone()),
        }
    }
}

impl AssetLoader for StaticAnimatedSceneLoader {
    type Asset = AnimatedScene;
    type Settings = ();
    type Error = Error;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &(),
        context: &mut LoadContext<'_>,
    ) -> Result<AnimatedScene, Error> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        let serial: StaticAnimatedSceneSerial =
            ron::de::from_bytes(&bytes).map_err(Error::other)?;
        let skeleton: LoadedAsset<Skeleton> = context
            .load_builder()
            .load_value(serial.skeleton.clone())
            .await
            .map_err(Error::other)?;
        let source: LoadedAsset<Gltf> = context
            .load_builder()
            .load_value(serial.source.without_label())
            .await
            .map_err(Error::other)?;
        let label = serial
            .source
            .label()
            .ok_or_else(|| Error::new(ErrorKind::InvalidData, "Static scene label is missing"))?;
        let mut scene = source
            .get_labeled(label)
            .and_then(|asset| asset.get::<WorldAsset>())
            .ok_or_else(|| Error::new(ErrorKind::InvalidData, "Static scene is missing"))?
            .clone_with(&self.registry)
            .map_err(Error::other)?;
        let skeleton_handle = context.load(serial.skeleton);
        let graph = context.load(serial.animation_graph);
        bind_static_scene(
            &mut scene.world,
            skeleton.get(),
            skeleton_handle.clone(),
            graph.clone(),
        )?;
        let processed = context.add_labeled_asset("Processed", scene);
        Ok(AnimatedScene {
            source: context.load(serial.source),
            processed_scene: Some(processed),
            animation_graph: graph,
            skeleton: skeleton_handle,
            retargeting: None,
            ragdoll: None,
            ragdoll_bone_map: None,
        })
    }

    fn extensions(&self) -> &[&str] {
        &["static.animscn.ron"]
    }
}

fn bind_static_scene(
    world: &mut World,
    skeleton: &Skeleton,
    skeleton_handle: Handle<Skeleton>,
    graph: Handle<AnimationGraph>,
) -> Result<(), Error> {
    let invalid = || {
        Error::new(
            ErrorKind::InvalidData,
            "Static animated scene does not match its skeleton",
        )
    };
    let path = skeleton.id_to_path(skeleton.root()).ok_or_else(invalid)?;
    if path.parts.len() != 1 {
        return Err(invalid());
    }
    let mut query = world.query::<(Entity, &Name)>();
    let roots = query
        .iter(world)
        .filter(|(_, name)| *name == &path.parts[0])
        .map(|(entity, _)| entity)
        .collect::<Vec<_>>();
    if roots.len() != 1 {
        return Err(invalid());
    }
    let root = roots[0];
    let mut pending = vec![(root, EntityPath::default())];
    let mut count = 0;
    while let Some((entity, parent)) = pending.pop() {
        let name = world.get::<Name>(entity).ok_or_else(invalid)?.clone();
        let path = parent.child(name);
        if skeleton.default_transforms(path.id()).is_none() {
            return Err(invalid());
        }
        if let Some(children) = world.get::<Children>(entity) {
            pending.extend(children.iter().map(|child| (child, path.clone())));
        }
        world
            .entity_mut(entity)
            .insert((AnimationTargetId(path.id().id()), AnimatedBy(root)));
        count += 1;
    }
    if count
        != skeleton
            .iter_bones()
            .filter(|bone| skeleton.default_transforms(*bone).is_some())
            .count()
    {
        return Err(invalid());
    }
    world
        .entity_mut(root)
        .remove::<AnimationPlayer>()
        .insert(AnimationGraphPlayer::new(skeleton_handle).with_graph(graph));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn static_scene_binds_real_bones_without_animation_tracks() {
        let mut world = World::new();
        let root = world.spawn((Name::new("root"), Transform::IDENTITY)).id();
        let child = world
            .spawn((
                Name::new("end"),
                Transform::from_xyz(1.0, 2.0, 3.0),
                ChildOf(root),
            ))
            .id();
        let root_path = EntityPath::default().child("root");
        let child_path = root_path.child("end");
        let mut skeleton = Skeleton::default();
        skeleton.set_root(root_path.id());
        skeleton.add_bone(root_path.clone(), Transform::IDENTITY, Transform::IDENTITY);
        skeleton.add_bone(
            child_path.clone(),
            Transform::from_xyz(1.0, 2.0, 3.0),
            Transform::from_xyz(1.0, 2.0, 3.0),
        );
        bind_static_scene(&mut world, &skeleton, default(), default()).unwrap();
        assert!(world.get::<AnimationGraphPlayer>(root).is_some());
        assert!(
            world
                .get::<bevy::animation::AnimationPlayer>(root)
                .is_none()
        );
        assert_eq!(
            world.get::<AnimationTargetId>(child).unwrap().0,
            child_path.id().id()
        );
        assert_eq!(world.get::<AnimatedBy>(child).unwrap().0, root);
        assert_eq!(
            world.get::<Transform>(child).unwrap().translation,
            Vec3::new(1.0, 2.0, 3.0)
        );
        world.entity_mut(child).insert(Name::new("wrong"));
        assert!(bind_static_scene(&mut world, &skeleton, default(), default()).is_err());
    }
}
