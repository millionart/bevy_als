use std::collections::HashSet;

use bevy::asset::Assets;
use bevy::prelude::*;
use bevy_animation_graph::builtin_nodes::clip_node::ClipNode;
use bevy_animation_graph::core::animation_clip::{EntityPath, GraphClip};
use bevy_animation_graph::core::animation_graph::AnimationGraph;
use bevy_animation_graph::core::pose::RootMotionMode;
use bevy_animation_graph::core::skeleton::Skeleton;

pub fn select_shallowest_animated_path<'a>(
    candidates: impl Iterator<Item = (&'a EntityPath, bool)>,
) -> Option<EntityPath> {
    candidates
        .filter(|(_, animated)| *animated)
        .min_by(|a, b| {
            let ka = (a.0.parts.len(), a.0.to_slashed_string());
            let kb = (b.0.parts.len(), b.0.to_slashed_string());
            ka.cmp(&kb)
        })
        .map(|(path, _)| path.clone())
}

pub fn derive_default_root_bone(clip: &GraphClip, skeleton: &Skeleton) -> Option<EntityPath> {
    select_shallowest_animated_path(
        skeleton
            .iter_paths()
            .map(|p| (p, clip.curves.contains_key(&p.id().animation_target_id()))),
    )
}

pub(crate) enum ScanStep {
    Resolved,
    PendingAssets,
}

fn open_struct(node: &mut ClipNode) -> Option<&mut dyn bevy::reflect::structs::Struct> {
    let partial: &mut dyn bevy::reflect::PartialReflect = node;
    partial.reflect_mut().as_struct().ok()
}

fn read_mode_is_disabled(struct_v: &mut dyn bevy::reflect::structs::Struct) -> Option<bool> {
    struct_v
        .field("root_motion_mode")
        .and_then(|f| f.try_downcast_ref::<RootMotionMode>())
        .map(|m| matches!(m, RootMotionMode::Disabled))
}

fn bone_already_set(struct_v: &mut dyn bevy::reflect::structs::Struct) -> Option<bool> {
    struct_v
        .field("root_motion_bone")
        .and_then(|f| f.try_downcast_ref::<Option<EntityPath>>())
        .map(|b| b.is_some())
}

fn write_missing_root_bone(struct_v: &mut dyn bevy::reflect::structs::Struct, bone: EntityPath) {
    if let Some(slot) = struct_v
        .field_mut("root_motion_bone")
        .and_then(|f| f.try_downcast_mut::<Option<EntityPath>>())
    {
        *slot = Some(bone);
    }
}

#[cfg(test)]
fn force_mode_full(struct_v: &mut dyn bevy::reflect::structs::Struct) {
    if let Some(slot) = struct_v
        .field_mut("root_motion_mode")
        .and_then(|f| f.try_downcast_mut::<RootMotionMode>())
    {
        *slot = RootMotionMode::Full;
    }
}

fn step_clip_node(
    clip_node: &mut ClipNode,
    clips: &Assets<GraphClip>,
    skeletons: &Assets<Skeleton>,
) -> ScanStep {
    let Some(struct_v) = open_struct(clip_node) else {
        warn_once!("root motion defaults: ClipNode reflection struct unavailable");
        return ScanStep::Resolved;
    };
    match read_mode_is_disabled(struct_v) {
        Some(true) => return ScanStep::Resolved,
        None => {
            warn_once!("root motion defaults: root_motion_mode field not readable");
            return ScanStep::Resolved;
        }
        Some(false) => {}
    }
    match bone_already_set(struct_v) {
        Some(true) => return ScanStep::Resolved,
        None => {
            warn_once!("root motion defaults: root_motion_bone field not readable");
            return ScanStep::Resolved;
        }
        Some(false) => {}
    }
    let handle = match struct_v
        .field("clip")
        .and_then(|f| f.try_downcast_ref::<Handle<GraphClip>>())
        .cloned()
    {
        Some(h) => h,
        None => {
            warn_once!("root motion defaults: clip field not readable");
            return ScanStep::Resolved;
        }
    };
    let Some(clip) = clips.get(&handle) else {
        return ScanStep::PendingAssets;
    };
    let Some(skeleton) = skeletons.get(&clip.skeleton) else {
        return ScanStep::PendingAssets;
    };
    let Some(bone) = derive_default_root_bone(clip, skeleton) else {
        warn_once!(
            "root motion defaults: no animated skeleton path found for clip {:?}",
            handle.path()
        );
        return ScanStep::Resolved;
    };
    write_missing_root_bone(struct_v, bone);
    info!(
        "root motion defaults: injected derived root bone for clip {:?}",
        handle.path()
    );
    ScanStep::Resolved
}

#[derive(Resource, Default)]
struct ProcessedGraphs(HashSet<AssetId<AnimationGraph>>);

fn inject_default_root_motion_bones(
    graphs: Option<ResMut<Assets<AnimationGraph>>>,
    clips: Option<Res<Assets<GraphClip>>>,
    skeletons: Option<Res<Assets<Skeleton>>>,
    mut processed: ResMut<ProcessedGraphs>,
) {
    let (Some(mut graphs), Some(clips), Some(skeletons)) = (graphs, clips, skeletons) else {
        return;
    };
    for (id, graph) in graphs.iter_mut() {
        if processed.0.contains(&id) {
            continue;
        }
        let mut pending = false;
        for node in graph.nodes.values_mut() {
            let inner: &mut dyn bevy_animation_graph::core::animation_node::NodeLike =
                &mut **node.inner;
            let Some(clip_node) = inner.as_any_mut().downcast_mut::<ClipNode>() else {
                continue;
            };
            if matches!(
                step_clip_node(clip_node, &clips, &skeletons),
                ScanStep::PendingAssets
            ) {
                pending = true;
            }
        }
        if !pending {
            processed.0.insert(id);
        }
    }
}

pub(crate) struct RootMotionDefaultsPlugin;

impl Plugin for RootMotionDefaultsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ProcessedGraphs>()
            .add_systems(Update, inject_default_root_motion_bones);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::animation::AnimationTargetId;

    fn path(parts: &[&str]) -> EntityPath {
        EntityPath::from(parts.iter().map(|s| s.to_string()).collect::<Vec<_>>())
    }

    #[test]
    fn skips_unanimated_shallow_paths() {
        let wrapper = path(&["GltfNode0"]);
        let root = path(&["GltfNode0", "root"]);
        let picked =
            select_shallowest_animated_path(vec![(&wrapper, false), (&root, true)].into_iter());
        assert_eq!(picked, Some(root));
    }

    #[test]
    fn deterministic_tie_break_on_equal_depth() {
        let left = path(&["GltfNode0", "a"]);
        let right = path(&["GltfNode0", "b"]);
        let picked =
            select_shallowest_animated_path(vec![(&right, true), (&left, true)].into_iter());
        assert_eq!(picked, Some(left));
    }

    #[test]
    fn empty_candidates_resolve_to_none() {
        let empty: Vec<(&EntityPath, bool)> = Vec::new();
        assert!(select_shallowest_animated_path(empty.into_iter()).is_none());
    }

    #[test]
    fn disabled_clip_is_resolved_without_assets() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()));
        app.init_asset::<GraphClip>().init_asset::<Skeleton>();
        let mut cn = ClipNode::new(Handle::default(), None, None);
        let clips = app.world().resource::<Assets<GraphClip>>();
        let skeletons = app.world().resource::<Assets<Skeleton>>();
        assert!(matches!(
            step_clip_node(&mut cn, clips, skeletons),
            ScanStep::Resolved
        ));
    }

    #[test]
    fn enabled_clip_pends_until_assets_ready_then_injects_once() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()));
        app.init_asset::<GraphClip>().init_asset::<Skeleton>();

        let target =
            AnimationTargetId::from_names([Name::from("GltfNode0"), Name::from("root")].iter());
        let skel_handle = app.world().resource::<Assets<Skeleton>>().reserve_handle();
        let mut clip = GraphClip {
            duration: 1.0,
            skeleton: skel_handle.clone(),
            ..Default::default()
        };
        clip.curves.insert(target, Vec::new());
        let clip_handle = app
            .world_mut()
            .resource_mut::<Assets<GraphClip>>()
            .add(clip);

        let mut cn = ClipNode::new(clip_handle.clone(), None, None);
        {
            let struct_v = open_struct(&mut cn).expect("struct");
            force_mode_full(struct_v);
        }

        let clips = app.world().resource::<Assets<GraphClip>>();
        let empty_skeletons = app.world().resource::<Assets<Skeleton>>();
        assert!(matches!(
            step_clip_node(&mut cn, clips, empty_skeletons),
            ScanStep::PendingAssets
        ));

        let mut skeleton = Skeleton::default();
        skeleton.add_bone(
            path(&["GltfNode0"]),
            Transform::default(),
            Transform::default(),
        );
        skeleton.add_bone(
            path(&["GltfNode0", "root"]),
            Transform::default(),
            Transform::default(),
        );
        app.world_mut()
            .resource_mut::<Assets<Skeleton>>()
            .insert(skel_handle.id(), skeleton);

        let clips = app.world().resource::<Assets<GraphClip>>();
        let skeletons = app.world().resource::<Assets<Skeleton>>();
        assert!(matches!(
            step_clip_node(&mut cn, clips, skeletons),
            ScanStep::Resolved
        ));

        let struct_v = open_struct(&mut cn).expect("struct");
        assert_eq!(bone_already_set(struct_v), Some(true));

        let derived = {
            let clip = app
                .world()
                .resource::<Assets<GraphClip>>()
                .get(&clip_handle)
                .unwrap();
            let skeleton = app
                .world()
                .resource::<Assets<Skeleton>>()
                .get(skel_handle.id())
                .unwrap();
            derive_default_root_bone(clip, skeleton)
        };
        assert_eq!(derived, Some(path(&["GltfNode0", "root"])));
    }
}
