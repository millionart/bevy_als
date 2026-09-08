use std::{fs, path::Path};

use als_ext::AlsExtPlugin;
use bevy::{
    asset::{AssetPlugin, AssetServer},
    prelude::*,
};
use bevy_animation_graph::{
    builtin_nodes::clip_node::ClipNode,
    core::{
        animation_clip::GraphClip,
        animation_graph::{AnimationGraph, loader::AnimationGraphLoader},
        state_machine::high_level::{StateMachine, loader::StateMachineLoader},
    },
};

fn assets_root() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets")
}

fn generated_files() -> Vec<String> {
    let manifest: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(
            assets_root().join("generated/ALS/Character/AB_Als/installation.manifest.json"),
        )
        .expect("generated installation manifest must be readable"),
    )
    .expect("generated installation manifest must be valid json");
    manifest["files"]
        .as_array()
        .expect("generated installation files must be an array")
        .iter()
        .filter_map(serde_json::Value::as_str)
        .map(str::to_owned)
        .collect()
}

fn generated_getter_graphs(files: &[String]) -> Vec<String> {
    files
        .iter()
        .filter(|path| path.ends_with(".animgraph.ron"))
        .filter(|path| {
            fs::read_to_string(assets_root().join(path))
                .expect("generated graph must be readable")
                .contains("als_ext::SourceAnimationStateGetterNode")
        })
        .cloned()
        .collect()
}

#[test]
fn generated_animation_graph_closure_and_state_getters_load() {
    let files = generated_files();
    let getter_paths = generated_getter_graphs(&files);
    let graph_paths = files
        .iter()
        .filter(|path| path.ends_with(".animgraph.ron"))
        .cloned()
        .collect::<Vec<_>>();
    let fsm_paths = files
        .iter()
        .filter(|path| path.ends_with(".fsm.ron"))
        .cloned()
        .collect::<Vec<_>>();
    assert_eq!(files.len(), 256, "installed closure must be complete");
    assert_eq!(
        getter_paths.len(),
        10,
        "every source getter must materialize"
    );

    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(AssetPlugin {
            file_path: "../../assets".into(),
            ..default()
        })
        .add_plugins(AlsExtPlugin)
        .register_type::<ClipNode>()
        .init_asset::<GraphClip>()
        .init_asset::<AnimationGraph>()
        .init_asset_loader::<AnimationGraphLoader>()
        .init_asset::<StateMachine>()
        .init_asset_loader::<StateMachineLoader>();
    let (graph_handles, fsm_handles) = {
        let server = app.world().resource::<AssetServer>();
        (
            graph_paths
                .iter()
                .map(|path| server.load::<AnimationGraph>(path.clone()))
                .collect::<Vec<_>>(),
            fsm_paths
                .iter()
                .map(|path| server.load::<StateMachine>(path.clone()))
                .collect::<Vec<_>>(),
        )
    };

    for _ in 0..600 {
        app.update();
        let graphs_loaded = graph_handles.iter().all(|handle| {
            app.world()
                .resource::<Assets<AnimationGraph>>()
                .get(handle)
                .is_some()
        });
        let fsms_loaded = fsm_handles.iter().all(|handle| {
            app.world()
                .resource::<Assets<StateMachine>>()
                .get(handle)
                .is_some()
        });
        if graphs_loaded && fsms_loaded {
            return;
        }
    }
    panic!("generated animation graph closure did not load within 600 frames");
}
