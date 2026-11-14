use std::time::{Duration, Instant};

use bevy::ecs::message::MessageWriter;
use bevy::prelude::*;
use bevy::window::ExitCondition;
use bevy_als::extensions::AlsExtPlugin;
use bevy_animation_graph::AnimationGraphPlugin;
use bevy_animation_graph::core::{
    animation_graph::{AnimationGraph, TimeUpdate},
    animation_graph_player::AnimationGraphPlayer,
    edge_data::DataValue,
    plugin::AnimationGraphSet,
    pose::Pose,
    skeleton::Skeleton,
};

const GRAPH_PATH: &str = "Validation/Graphs/ab_als/view.animgraph.ron";
const SKELETON_PATH: &str = "ALS/Character/SKM_Als.skn.ron";

#[derive(Resource)]
struct AssetsToProbe {
    graph: Handle<AnimationGraph>,
    skeleton: Handle<Skeleton>,
}

#[derive(Resource)]
struct Deadline(Instant);

#[derive(Resource, Default)]
struct Probe {
    player: Option<Entity>,
}

fn main() {
    let mut app = App::new();
    app.add_plugins((
        DefaultPlugins
            .set(AssetPlugin {
                file_path: "assets".into(),
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
    .init_resource::<Probe>()
    .add_systems(Startup, load_assets)
    .add_systems(Update, (verify_static, timeout))
    .add_systems(PostUpdate, observe.after(AnimationGraphSet::Final));
    app.run();
}

fn load_assets(mut commands: Commands, assets: Res<AssetServer>) {
    commands.insert_resource(AssetsToProbe {
        graph: assets.load(GRAPH_PATH),
        skeleton: assets.load(SKELETON_PATH),
    });
    commands.insert_resource(Deadline(Instant::now() + Duration::from_secs(20)));
    info!("[VIEW-REDIRECT-CONTRACT] loading redirect marker");
}

fn fail(_exit: &mut MessageWriter<AppExit>, message: &str) {
    warn!("[VIEW-REDIRECT-CONTRACT][VERDICT] {message} -> FAIL");
    std::process::exit(6);
}

fn verify_static(
    mut commands: Commands,
    server: Res<AssetServer>,
    assets: Res<AssetsToProbe>,
    graphs: Res<Assets<AnimationGraph>>,
    mut probe: ResMut<Probe>,
    mut exit: MessageWriter<AppExit>,
) {
    if probe.player.is_some()
        || !server.is_loaded_with_dependencies(&assets.graph)
        || !server.is_loaded_with_dependencies(&assets.skeleton)
    {
        return;
    }
    let Some(graph) = graphs.get(&assets.graph) else {
        return;
    };
    let root = graph.nodes.is_empty()
        && graph.io_spec.has_output_time()
        && graph.io_spec.get_output_data(&"pose".to_owned()).is_some();
    let source = std::fs::read_to_string("assets/Validation/Graphs/ab_als/view.animgraph.ron")
        .unwrap_or_default();
    let redirect = !source.contains("ClipNode")
        && !source.contains("Curve")
        && !source.contains("BlendNode")
        && !source.contains("LayeredGraphNode")
        && !source.contains("AB_Als_Monolithic");
    info!(
        "[VIEW-REDIRECT-CONTRACT] root={} redirect={}",
        root, redirect
    );
    if !(root && redirect) {
        fail(&mut exit, "static redirect contract");
        return;
    }
    let input = Pose {
        timestamp: 1.75,
        skeleton: assets.skeleton.clone(),
        ..default()
    };
    let mut player =
        AnimationGraphPlayer::new(assets.skeleton.clone()).with_graph(assets.graph.clone());
    player.set_input_data("view_pose", DataValue::Pose(input));
    player.pause();
    player.queue_time_update(TimeUpdate::Delta(0.0));
    probe.player = Some(
        commands
            .spawn((
                Name::new("view_redirect_contract_probe"),
                Children::default(),
                player,
            ))
            .id(),
    );
}

fn observe(
    probe: Res<Probe>,
    players: Query<&AnimationGraphPlayer>,
    mut exit: MessageWriter<AppExit>,
) {
    let Some(entity) = probe.player else {
        return;
    };
    let Ok(player) = players.get(entity) else {
        return;
    };
    if let Some(error) = player.get_error() {
        fail(&mut exit, &format!("runtime error={error:?}"));
        return;
    }
    let Some(DataValue::Pose(pose)) = player.get_outputs().get("pose") else {
        return;
    };
    if (pose.timestamp - 1.75).abs() > 1e-5 {
        fail(&mut exit, "identity time");
        return;
    }
    info!("[VIEW-REDIRECT-CONTRACT] identity pose/time preserved");
    info!("[VIEW-REDIRECT-CONTRACT][VERDICT] redirect=true -> PASS");
    exit.write(AppExit::Success);
}

fn timeout(deadline: Res<Deadline>, mut exit: MessageWriter<AppExit>) {
    if Instant::now() >= deadline.0 {
        fail(&mut exit, "timeout");
    }
}
