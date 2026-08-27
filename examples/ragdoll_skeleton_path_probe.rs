use bevy::prelude::*;
use bevy_animation_graph::AnimationGraphPlugin;
use bevy_animation_graph::core::animation_clip::EntityPath;
use bevy_animation_graph::core::skeleton::Skeleton;

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, AnimationGraphPlugin::default()))
        .init_resource::<ProbeDone>()
        .add_systems(Startup, load_skeleton)
        .add_systems(Update, print_skeleton_paths)
        .run();
}

#[derive(Resource)]
struct ProbeHandle(Handle<Skeleton>);

#[derive(Resource, Default)]
struct ProbeDone(bool);

fn load_skeleton(mut commands: Commands, assets: Res<AssetServer>) {
    commands.insert_resource(ProbeHandle(assets.load("skeletons/SKM_Als.skn.ron")));
}

fn print_skeleton_paths(
    handle: Res<ProbeHandle>,
    skeletons: Res<Assets<Skeleton>>,
    mut done: ResMut<ProbeDone>,
    mut exit: MessageWriter<AppExit>,
) {
    if done.0 {
        return;
    }
    let Some(skeleton) = skeletons.get(&handle.0) else {
        return;
    };
    let root = skeleton
        .id_to_path(skeleton.root())
        .expect("skeleton root path");
    let candidates = [
        "root",
        "GltfNode0/root",
        "Scene0/root",
        "root/pelvis",
        "GltfNode0/root/pelvis",
        "Scene0/root/pelvis",
        "root/pelvis/spine_01/spine_02/spine_03/neck_01/head",
        "GltfNode0/root/pelvis/spine_01/spine_02/spine_03/neck_01/head",
        "Scene0/root/pelvis/spine_01/spine_02/spine_03/neck_01/head",
    ];
    info!("[RAGPATH] root={root}");
    for candidate in candidates {
        let path = EntityPath::from_slashed_string(candidate.into());
        info!(
            "[RAGPATH] path={candidate} present={}",
            skeleton.has_id(&path.id())
        );
    }
    done.0 = true;
    exit.write(AppExit::Success);
}
