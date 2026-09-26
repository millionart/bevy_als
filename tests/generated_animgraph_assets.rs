use std::{fs, path::Path};

use bevy_als::extensions::{AlsExtPlugin, InterruptibleFsmNode, SourceSlotNode};
use bevy::{
    asset::{AssetPlugin, AssetServer},
    prelude::*,
};
use bevy_animation_graph::{
    builtin_nodes::clip_node::ClipNode,
    core::{
        animation_clip::GraphClip,
        animation_graph::{AnimationGraph, loader::AnimationGraphLoader},
        skeleton::Skeleton,
        state_machine::high_level::{StateMachine, loader::StateMachineLoader},
    },
};

fn assets_root() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("assets")
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

const GENERATED_ROOT_GRAPH: &str = "generated/ALS/Character/AB_Als/AnimGraph.animgraph.ron";
const RAGDOLL_WRAPPER_GRAPH: &str =
    "graphs/runtime/als_generated_ab_als_ragdoll_wrapper.animgraph.ron";
const GENERATED_ANIMATED_SCENE: &str = "animated_scenes/als_generated_ab_als.animscn.ron";

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
fn generated_animation_graph_closure_and_ragdoll_wrapper_load() {
    let files = generated_files();
    let getter_paths = generated_getter_graphs(&files);
    let mut graph_paths = files
        .iter()
        .filter(|path| path.ends_with(".animgraph.ron"))
        .cloned()
        .collect::<Vec<_>>();
    graph_paths.push(RAGDOLL_WRAPPER_GRAPH.to_owned());
    let fsm_paths = files
        .iter()
        .filter(|path| path.ends_with(".fsm.ron"))
        .cloned()
        .collect::<Vec<_>>();
    assert_eq!(files.len(), 256, "installed closure must be complete");
    assert!(
        !files
            .iter()
            .any(|path| path.contains("synthetic_states/AnimStateConduitNode_")),
        "source conduits must lower to entry/transition gates, not placeholder poses"
    );
    let conduit_lowered_transition_paths = files
        .iter()
        .filter(|path| {
            path.contains("SM9_Locomotion_States/T21/Transition_")
                || path.contains("SM9_Locomotion_States/T26/Transition_")
        })
        .collect::<Vec<_>>();
    assert_eq!(
        conduit_lowered_transition_paths.len(),
        2,
        "each source conduit route must materialize a distinct transition path"
    );
    assert_ne!(
        conduit_lowered_transition_paths[0], conduit_lowered_transition_paths[1],
        "conduit-lowered transition paths must remain distinct"
    );
    assert_eq!(
        getter_paths.len(),
        10,
        "every source getter must materialize"
    );

    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(AssetPlugin {
            file_path: "assets".into(),
            ..default()
        })
        .add_plugins(AlsExtPlugin)
        .register_type::<ClipNode>()
        .init_asset::<GraphClip>()
        .init_asset::<Skeleton>()
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
            let mut condition_count = 0;
            let mut slot_count = 0;
            let mut transition_slot_seen = false;
            for handle in &graph_handles {
                let graph = app
                    .world()
                    .resource::<Assets<AnimationGraph>>()
                    .get(handle)
                    .expect("loaded generated graph must exist");
                for node in graph.nodes.values() {
                    if let Some(slot) = node.try_inner_downcast_ref::<SourceSlotNode>() {
                        slot_count += 1;
                        assert!(
                            !slot.mesh_space_additive_key.is_empty(),
                            "every generated source slot needs its additive runtime ABI"
                        );
                        if slot.slot_name == "Transition" {
                            assert_eq!(
                                slot.mesh_space_additive_key, "transition.mesh_space_additive",
                                "the source Transition slot must retain its mesh-space additive ABI"
                            );
                            transition_slot_seen = true;
                        }
                    }
                    let Some(fsm) = node.try_inner_downcast_ref::<InterruptibleFsmNode>() else {
                        continue;
                    };
                    for condition in &fsm.source_transition_conditions {
                        condition_count += 1;
                        assert!(
                            !condition.context_state.uuid().is_nil(),
                            "every generated transition condition needs its own context identity"
                        );
                        for gate in &condition.gates {
                            condition_count += 1;
                            assert!(
                                !gate.context_state.uuid().is_nil(),
                                "every generated transition gate needs its own context identity"
                            );
                        }
                    }
                    for entry in &fsm.source_entry_conditions {
                        for gate in &entry.gates {
                            condition_count += 1;
                            assert!(
                                !gate.context_state.uuid().is_nil(),
                                "every generated entry gate needs its own context identity"
                            );
                        }
                    }
                }
            }
            assert!(
                condition_count > 0,
                "generated closure must preserve source condition graph invocations"
            );
            assert!(
                slot_count > 0,
                "generated closure must preserve source slots"
            );
            assert!(
                transition_slot_seen,
                "generated closure must preserve the source Transition slot"
            );
            return;
        }
    }
    let server = app.world().resource::<AssetServer>();
    let graph_failures = graph_paths
        .iter()
        .zip(&graph_handles)
        .filter_map(|(path, handle)| match server.load_state(handle.id()) {
            bevy::asset::LoadState::Failed(error) => Some(format!("{path}: {error}")),
            _ => None,
        })
        .collect::<Vec<_>>();
    let fsm_failures = fsm_paths
        .iter()
        .zip(&fsm_handles)
        .filter_map(|(path, handle)| match server.load_state(handle.id()) {
            bevy::asset::LoadState::Failed(error) => Some(format!("{path}: {error}")),
            _ => None,
        })
        .collect::<Vec<_>>();
    panic!(
        "generated animation graph closure did not load within 600 frames; graph failures: {graph_failures:?}; fsm failures: {fsm_failures:?}"
    );
}

#[test]
fn generated_root_stays_source_only_while_the_wrapper_adds_ragdoll_config() {
    let root = fs::read_to_string(assets_root().join(GENERATED_ROOT_GRAPH))
        .expect("generated root graph must be readable");
    let wrapper = fs::read_to_string(assets_root().join(RAGDOLL_WRAPPER_GRAPH))
        .expect("runtime ragdoll wrapper must be readable");
    let scene = fs::read_to_string(assets_root().join(GENERATED_ANIMATED_SCENE))
        .expect("generated animated scene must be readable");

    assert!(root.contains("output_data: { \"Result\": Pose }"));
    assert!(!root.contains("ragdoll_config"));
    assert!(!root.contains("OutputData(\"pose\")"));
    assert!(wrapper.contains("capture_context: true"));
    assert!(wrapper.contains("als_ext::RagdollConfigNode"));
    assert!(wrapper.contains("als_ragdolling_dynamic"));
    assert!(wrapper.contains("als_ragdolling_readback"));
    assert!(wrapper.contains("OutputData(\"ragdoll_config\")"));
    assert!(wrapper.contains("OutputData(\"pose\")"));
    assert!(!wrapper.contains("OutputData(\"Result\")"));
    assert!(wrapper.contains("output_data: { \"pose\": Pose, \"ragdoll_config\": RagdollConfig }"));
    assert!(scene.contains(RAGDOLL_WRAPPER_GRAPH));
}
