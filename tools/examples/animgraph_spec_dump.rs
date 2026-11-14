use std::{
    collections::BTreeMap,
    env, fs,
    time::{Duration, Instant},
};

use bevy::{app::AppExit, ecs::message::MessageWriter, prelude::*, window::ExitCondition};
use bevy_als::extensions::AlsExtPlugin;
use bevy_animation_graph::{
    AnimationGraphPlugin,
    core::{
        animation_graph::AnimationGraph,
        context::spec_context::{NodeInput, NodeOutput, SpecResources},
        state_machine::high_level::StateMachine,
    },
};
use serde::Serialize;

#[derive(Resource)]
struct DumpAsset {
    graph: Handle<AnimationGraph>,
    graph_path: String,
    graph_source: String,
}

#[derive(Resource)]
struct DumpAssets {
    values: Vec<DumpAsset>,
    batch: bool,
}

#[derive(Resource)]
struct DumpAssetRoot(String);

#[derive(Resource)]
struct Deadline(Instant);

#[derive(Serialize)]
struct PinRecord {
    direction: String,
    name: String,
    data_type: Option<String>,
}

#[derive(Serialize)]
struct NodeRecord {
    id: String,
    name: String,
    display_name: String,
    position: [f32; 2],
    inputs: Vec<PinRecord>,
    outputs: Vec<PinRecord>,
}

#[derive(Serialize)]
struct EdgeRecord {
    target: String,
    source: String,
}

#[derive(Serialize)]
struct Dump {
    graph: String,
    graph_source: String,
    nodes: Vec<NodeRecord>,
    edges: Vec<EdgeRecord>,
    graph_outputs: Vec<PinRecord>,
    input_position: [f32; 2],
    output_position: [f32; 2],
}

fn main() {
    let asset_root = env::var("ALS_GRAPH_SPEC_ASSET_ROOT").unwrap_or_else(|_| "assets".to_owned());
    App::new()
        .add_plugins((
            DefaultPlugins
                .set(AssetPlugin {
                    file_path: asset_root.clone().into(),
                    ..default()
                })
                .set(WindowPlugin {
                    primary_window: None,
                    exit_condition: ExitCondition::DontExit,
                    ..default()
                }),
            AnimationGraphPlugin::from_physics_schedule(Update),
            AlsExtPlugin,
        ))
        .insert_resource(DumpAssetRoot(asset_root))
        .insert_resource(Deadline(Instant::now() + Duration::from_secs(60)))
        .add_systems(Startup, load)
        .add_systems(Update, (dump, timeout))
        .run();
}

fn load(mut commands: Commands, assets: Res<AssetServer>, root: Res<DumpAssetRoot>) {
    let batch = env::var_os("ALS_GRAPH_SPEC_PATHS").is_some();
    let graph_paths = if batch {
        serde_json::from_str::<Vec<String>>(
            &env::var("ALS_GRAPH_SPEC_PATHS").expect("batch graph paths must be readable"),
        )
        .expect("batch graph paths must be valid JSON")
    } else {
        vec![env::var("ALS_GRAPH_SPEC_PATH").unwrap_or_else(|_| {
            "Validation/Graphs/ab_als/stances/standing.animgraph.ron".to_owned()
        })]
    };
    assert!(
        !graph_paths.is_empty(),
        "batch graph paths must not be empty"
    );
    let values = graph_paths
        .into_iter()
        .map(|graph_path| DumpAsset {
            graph_source: fs::read_to_string(format!("{}/{graph_path}", root.0))
                .expect("graph source must be readable"),
            graph: assets.load(graph_path.clone()),
            graph_path,
        })
        .collect();
    commands.insert_resource(DumpAssets { values, batch });
}

fn debug_text(value: impl std::fmt::Debug) -> String {
    let encoded = format!("{value:?}");
    serde_json::from_str(&encoded).unwrap_or(encoded)
}

fn node_id_text(value: impl std::fmt::Debug) -> String {
    let encoded = format!("{value:?}");
    encoded
        .strip_prefix("NodeId(")
        .and_then(|value| value.strip_suffix(')'))
        .unwrap_or(&encoded)
        .to_owned()
}

fn input_record(input: NodeInput<impl std::fmt::Debug>) -> PinRecord {
    match input {
        NodeInput::Time(pin) => PinRecord {
            direction: "input".to_owned(),
            name: debug_text(pin),
            data_type: Some("Time".to_owned()),
        },
        NodeInput::Data(pin, data_type) => PinRecord {
            direction: "input".to_owned(),
            name: debug_text(pin),
            data_type: Some(format!("{data_type:?}")),
        },
    }
}

fn output_record(output: NodeOutput) -> PinRecord {
    match output {
        NodeOutput::Time => PinRecord {
            direction: "output".to_owned(),
            name: "time".to_owned(),
            data_type: Some("Time".to_owned()),
        },
        NodeOutput::Data(pin, data_type) => PinRecord {
            direction: "output".to_owned(),
            name: debug_text(pin),
            data_type: Some(format!("{data_type:?}")),
        },
    }
}

fn dump(
    assets: Res<AssetServer>,
    graph_assets: Res<Assets<AnimationGraph>>,
    fsm_assets: Res<Assets<StateMachine>>,
    dump_assets: Res<DumpAssets>,
    mut exit: MessageWriter<AppExit>,
) {
    let mut outputs = BTreeMap::new();
    for dump_asset in &dump_assets.values {
        if let Some((load, dependencies, recursive_dependencies)) =
            assets.get_load_states(&dump_asset.graph)
            && (load.is_failed() || dependencies.is_failed() || recursive_dependencies.is_failed())
        {
            eprintln!(
                "animgraph spec dump asset failed: graph={} load={load:?} dependencies={dependencies:?} recursive={recursive_dependencies:?}",
                dump_asset.graph_path,
            );
            std::process::exit(8);
        }
        if !assets.is_loaded_with_dependencies(&dump_asset.graph) {
            return;
        }
        let Some(graph) = graph_assets.get(&dump_asset.graph) else {
            return;
        };
        outputs.insert(
            dump_asset.graph_path.clone(),
            build_dump(graph, &graph_assets, &fsm_assets, dump_asset),
        );
    }
    let encoded = if dump_assets.batch {
        serde_json::to_string_pretty(&outputs)
    } else {
        serde_json::to_string_pretty(
            &outputs
                .into_values()
                .next()
                .expect("single graph output must exist"),
        )
    }
    .expect("JSON serialization must work");
    if let Ok(path) = env::var("ALS_GRAPH_SPEC_OUTPUT") {
        fs::write(path, encoded).expect("spec dump output must be writable");
    } else {
        println!("{encoded}");
    }
    exit.write(AppExit::Success);
}

fn build_dump(
    graph: &AnimationGraph,
    graph_assets: &Assets<AnimationGraph>,
    fsm_assets: &Assets<StateMachine>,
    dump_asset: &DumpAsset,
) -> Dump {
    let resources = SpecResources {
        graph_assets,
        fsm_assets,
    };
    let mut nodes = graph
        .nodes
        .values()
        .map(|node| {
            let spec = node.new_spec(resources).expect("node spec must resolve");
            NodeRecord {
                id: node_id_text(node.id),
                name: node.name.clone(),
                display_name: node.display_name(),
                position: graph
                    .editor_metadata
                    .node_positions
                    .get(&node.id)
                    .copied()
                    .unwrap_or_default()
                    .to_array(),
                inputs: spec.sorted_inputs().into_iter().map(input_record).collect(),
                outputs: spec
                    .sorted_outputs()
                    .into_iter()
                    .map(output_record)
                    .collect(),
            }
        })
        .collect::<Vec<_>>();
    nodes.sort_by(|left, right| left.id.cmp(&right.id));
    let mut edges = graph
        .edges_inverted
        .iter()
        .map(|(target, source)| EdgeRecord {
            target: format!("{target:?}"),
            source: format!("{source:?}"),
        })
        .collect::<Vec<_>>();
    edges.sort_by(|left, right| left.target.cmp(&right.target));
    Dump {
        graph: dump_asset.graph_path.clone(),
        graph_source: dump_asset.graph_source.clone(),
        nodes,
        edges,
        graph_outputs: graph
            .io_spec
            .sorted_outputs()
            .into_iter()
            .map(output_record)
            .collect(),
        input_position: graph.editor_metadata.input_position.to_array(),
        output_position: graph.editor_metadata.output_position.to_array(),
    }
}

fn timeout(deadline: Res<Deadline>) {
    if Instant::now() >= deadline.0 {
        eprintln!("animgraph spec dump timed out");
        std::process::exit(9);
    }
}
