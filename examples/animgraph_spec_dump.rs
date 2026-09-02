use std::{
    env, fs,
    time::{Duration, Instant},
};

use als_ext::AlsExtPlugin;
use bevy::{app::AppExit, ecs::message::MessageWriter, prelude::*, window::ExitCondition};
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
        .insert_resource(Deadline(Instant::now() + Duration::from_secs(30)))
        .add_systems(Startup, load)
        .add_systems(Update, (dump, timeout))
        .run();
}

fn load(mut commands: Commands, assets: Res<AssetServer>, root: Res<DumpAssetRoot>) {
    let graph_path = env::var("ALS_GRAPH_SPEC_PATH")
        .unwrap_or_else(|_| "graphs/ab_als/stances/standing.animgraph.ron".to_owned());
    let graph_source = fs::read_to_string(format!("{}/{graph_path}", root.0))
        .expect("graph source must be readable");
    commands.insert_resource(DumpAsset {
        graph: assets.load(graph_path.clone()),
        graph_path,
        graph_source,
    });
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
    dump_asset: Res<DumpAsset>,
    mut exit: MessageWriter<AppExit>,
) {
    if !assets.is_loaded_with_dependencies(&dump_asset.graph) {
        return;
    }
    let Some(graph) = graph_assets.get(&dump_asset.graph) else {
        return;
    };
    let resources = SpecResources {
        graph_assets: &graph_assets,
        fsm_assets: &fsm_assets,
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
    let output = Dump {
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
    };
    let encoded = serde_json::to_string_pretty(&output).expect("JSON serialization must work");
    if let Ok(path) = env::var("ALS_GRAPH_SPEC_OUTPUT") {
        fs::write(path, encoded).expect("spec dump output must be writable");
    } else {
        println!("{encoded}");
    }
    exit.write(AppExit::Success);
}

fn timeout(deadline: Res<Deadline>, mut exit: MessageWriter<AppExit>) {
    if Instant::now() >= deadline.0 {
        eprintln!("animgraph spec dump timed out");
        exit.write(AppExit::Error(std::num::NonZeroU8::new(9).unwrap()));
    }
}
