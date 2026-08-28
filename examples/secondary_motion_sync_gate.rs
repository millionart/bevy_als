use std::time::{Duration, Instant};

use als_ext::{AlsExtPlugin, GlobalSyncGroupSampleNode};
use bevy::ecs::message::MessageWriter;
use bevy::prelude::*;
use bevy::window::ExitCondition;
use bevy_animation_graph::AnimationGraphPlugin;
use bevy_animation_graph::core::{
    animation_graph::{AnimationGraph, TimeUpdate},
    animation_graph_player::AnimationGraphPlayer,
    context::node_states::StateKey,
    edge_data::DataValue,
    plugin::AnimationGraphSet,
    pose::Pose,
    skeleton::Skeleton,
};

const SKELETON_PATH: &str = "skeletons/SKM_Als.skn.ron";
const PHASE_KEY: &str = "als_secondary_motion_phase";
const IDLE_DURATION: f32 = 2.666667;
const EPSILON: f32 = 1e-4;
const DIRECT_RUNTIME_COUNT: usize = 13;

#[derive(Clone, Copy)]
struct GraphSpec {
    label: &'static str,
    path: &'static str,
    node: &'static str,
}

const GRAPH_SPECS: [GraphSpec; 21] = [
    GraphSpec {
        label: "default",
        path: "graphs/ab_als/overlays/default.animgraph.ron",
        node: "secondary motion idle loop",
    },
    GraphSpec {
        label: "barrel",
        path: "graphs/ab_als/overlays/barrel.animgraph.ron",
        node: "secondary motion idle loop",
    },
    GraphSpec {
        label: "binoculars",
        path: "graphs/ab_als/overlays/binoculars.animgraph.ron",
        node: "secondary motion idle loop",
    },
    GraphSpec {
        label: "bow_relaxed",
        path: "graphs/ab_als/overlays/bow/state_relaxed.animgraph.ron",
        node: "idle loop",
    },
    GraphSpec {
        label: "bow_ready",
        path: "graphs/ab_als/overlays/bow/state_ready.animgraph.ron",
        node: "idle loop",
    },
    GraphSpec {
        label: "bow_aiming",
        path: "graphs/ab_als/overlays/bow/state_aiming.animgraph.ron",
        node: "idle loop",
    },
    GraphSpec {
        label: "box",
        path: "graphs/ab_als/overlays/box.animgraph.ron",
        node: "secondary motion idle loop",
    },
    GraphSpec {
        label: "feminine",
        path: "graphs/ab_als/overlays/feminine.animgraph.ron",
        node: "secondary motion idle loop",
    },
    GraphSpec {
        label: "hands_tied",
        path: "graphs/ab_als/overlays/hands_tied.animgraph.ron",
        node: "secondary motion idle loop",
    },
    GraphSpec {
        label: "injured",
        path: "graphs/ab_als/overlays/injured.animgraph.ron",
        node: "secondary motion idle loop",
    },
    GraphSpec {
        label: "masculine",
        path: "graphs/ab_als/overlays/masculine.animgraph.ron",
        node: "secondary motion idle loop",
    },
    GraphSpec {
        label: "pistol_one_handed_relaxed",
        path: "graphs/ab_als/overlays/pistol_one_handed/state_relaxed.animgraph.ron",
        node: "idle loop",
    },
    GraphSpec {
        label: "pistol_one_handed_ready",
        path: "graphs/ab_als/overlays/pistol_one_handed/state_ready.animgraph.ron",
        node: "idle loop",
    },
    GraphSpec {
        label: "pistol_one_handed_aiming",
        path: "graphs/ab_als/overlays/pistol_one_handed/state_aiming.animgraph.ron",
        node: "idle loop",
    },
    GraphSpec {
        label: "pistol_two_handed_relaxed",
        path: "graphs/ab_als/overlays/pistol_two_handed/state_relaxed.animgraph.ron",
        node: "idle loop",
    },
    GraphSpec {
        label: "pistol_two_handed_ready",
        path: "graphs/ab_als/overlays/pistol_two_handed/state_ready.animgraph.ron",
        node: "idle loop",
    },
    GraphSpec {
        label: "pistol_two_handed_aiming",
        path: "graphs/ab_als/overlays/pistol_two_handed/state_aiming.animgraph.ron",
        node: "idle loop",
    },
    GraphSpec {
        label: "rifle_relaxed",
        path: "graphs/ab_als/overlays/rifle/state_relaxed.animgraph.ron",
        node: "idle loop",
    },
    GraphSpec {
        label: "rifle_ready",
        path: "graphs/ab_als/overlays/rifle/state_ready.animgraph.ron",
        node: "idle loop",
    },
    GraphSpec {
        label: "rifle_aiming",
        path: "graphs/ab_als/overlays/rifle/state_aiming.animgraph.ron",
        node: "idle loop",
    },
    GraphSpec {
        label: "torch",
        path: "graphs/ab_als/overlays/torch.animgraph.ron",
        node: "secondary motion idle loop",
    },
];

#[derive(Resource)]
struct GateAssets {
    graphs: Vec<Handle<AnimationGraph>>,
    skeleton: Handle<Skeleton>,
}

#[derive(Resource, Default)]
struct Probe {
    players: Vec<RuntimePlayer>,
    phase_index: u8,
}

#[derive(Resource)]
struct Deadline(Instant);

struct RuntimePlayer {
    spec_index: usize,
    entity: Entity,
}

fn main() {
    App::new()
        .add_plugins((
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
        .insert_resource(Deadline(Instant::now() + Duration::from_secs(35)))
        .add_systems(Startup, load_assets)
        .add_systems(Update, (initialize, timeout))
        .add_systems(PostUpdate, observe.after(AnimationGraphSet::Final))
        .run();
}

fn load_assets(mut commands: Commands, assets: Res<AssetServer>) {
    commands.insert_resource(GateAssets {
        graphs: GRAPH_SPECS
            .iter()
            .map(|spec| assets.load(spec.path))
            .collect(),
        skeleton: assets.load(SKELETON_PATH),
    });
    info!("[SECONDARY-MOTION-SYNC] loading 21 overlay graphs");
}

fn static_contract(graph: &AnimationGraph, spec: GraphSpec) -> bool {
    let mut matches = graph.nodes.values().filter(|node| node.name == spec.node);
    let Some(node) = matches.next() else {
        return false;
    };
    matches.next().is_none()
        && node
            .try_inner_downcast_ref::<GlobalSyncGroupSampleNode>()
            .is_some_and(|node| {
                node.key == PHASE_KEY
                    && close(node.leader_duration, IDLE_DURATION)
                    && node.leader_markers.is_empty()
                    && close(node.follower_duration, IDLE_DURATION)
                    && node.follower_markers.is_empty()
                    && node.looped
            })
        && std::fs::read_to_string(format!("assets/{}", spec.path))
            .is_ok_and(|source| source.contains("animations/Base/A_Als_Idle.anim.ron"))
}

fn supports_direct_runtime(spec: GraphSpec) -> bool {
    matches!(
        spec.label,
        "default"
            | "barrel"
            | "binoculars"
            | "bow_relaxed"
            | "box"
            | "feminine"
            | "hands_tied"
            | "injured"
            | "masculine"
            | "pistol_one_handed_relaxed"
            | "pistol_two_handed_relaxed"
            | "rifle_relaxed"
            | "torch"
    )
}

fn initialize(
    mut commands: Commands,
    server: Res<AssetServer>,
    assets: Res<GateAssets>,
    graphs: Res<Assets<AnimationGraph>>,
    mut probe: ResMut<Probe>,
    mut exit: MessageWriter<AppExit>,
) {
    if !probe.players.is_empty()
        || !server.is_loaded_with_dependencies(&assets.skeleton)
        || !assets
            .graphs
            .iter()
            .all(|graph| server.is_loaded_with_dependencies(graph))
    {
        return;
    }
    let static_valid = GRAPH_SPECS
        .iter()
        .zip(&assets.graphs)
        .all(|(spec, handle)| {
            graphs
                .get(handle)
                .is_some_and(|graph| static_contract(graph, *spec))
        });
    if !static_valid {
        fail(&mut exit, "static contract");
        return;
    }
    for (spec_index, (spec, graph)) in GRAPH_SPECS.iter().zip(&assets.graphs).enumerate() {
        if !supports_direct_runtime(*spec) {
            continue;
        }
        let mut player =
            AnimationGraphPlayer::new(assets.skeleton.clone()).with_graph(graph.clone());
        player.set_input_data("base_pose", DataValue::Pose(Pose::default()));
        player.set_global_input_data("als_root_defaults_enabled", true);
        player.set_global_input_data(PHASE_KEY, 0.25_f32);
        player.pause();
        player.queue_time_update(TimeUpdate::Delta(0.0));
        let entity = commands
            .spawn((
                Name::new(format!("secondary_motion_sync_{}", spec.label)),
                Children::default(),
                Transform::IDENTITY,
                player,
            ))
            .id();
        probe.players.push(RuntimePlayer { spec_index, entity });
    }
    info!(
        "[SECONDARY-MOTION-SYNC] static=21/21 direct_runtime={} phase=.25 initialized",
        probe.players.len(),
    );
}

fn node_output(
    player: &AnimationGraphPlayer,
    graph: &AnimationGraph,
    name: &str,
    key: &str,
) -> Option<DataValue> {
    let node_id = graph.nodes.values().find(|node| node.name == name)?.id;
    player
        .get_context_arena()?
        .get_toplevel()
        .node_caches
        .get_output_data(node_id, StateKey::Default, key.to_owned())
}

fn sample_time(player: &AnimationGraphPlayer, graph: &AnimationGraph, name: &str) -> Option<f32> {
    match node_output(player, graph, name, "sample_time")? {
        DataValue::F32(value) => Some(value),
        _ => None,
    }
}

fn node_pose(player: &AnimationGraphPlayer, graph: &AnimationGraph, name: &str) -> Option<Pose> {
    match node_output(player, graph, name, "pose")? {
        DataValue::Pose(value) => Some(value),
        _ => None,
    }
}

fn close(actual: f32, expected: f32) -> bool {
    (actual - expected).abs() <= EPSILON
}

fn pose_is_valid(pose: &Pose) -> bool {
    !pose.bones.is_empty()
        && pose.timestamp.is_finite()
        && pose.bones.iter().all(|bone| {
            bone.translation.is_none_or(Vec3::is_finite)
                && bone.rotation.is_none_or(Quat::is_finite)
                && bone.scale.is_none_or(Vec3::is_finite)
        })
}

fn sample_contract(
    player: &AnimationGraphPlayer,
    graph: &AnimationGraph,
    spec: GraphSpec,
    phase: f32,
) -> Result<usize, String> {
    let sample = sample_time(player, graph, spec.node)
        .ok_or_else(|| format!("{} sample missing", spec.label))?;
    let pose = node_pose(player, graph, spec.node)
        .ok_or_else(|| format!("{} pose missing", spec.label))?;
    let expected = phase.rem_euclid(1.0) * IDLE_DURATION;
    if !close(sample, expected) || !pose_is_valid(&pose) {
        return Err(format!(
            "{} actual={sample:.6} expected={expected:.6} bones={}",
            spec.label,
            pose.bones.len(),
        ));
    }
    Ok(pose.bones.len())
}

fn fail(exit: &mut MessageWriter<AppExit>, message: &str) {
    warn!("[SECONDARY-MOTION-SYNC][VERDICT] {message} -> FAIL");
    exit.write(AppExit::Error(std::num::NonZeroU8::new(8).unwrap()));
}

fn observe(
    mut probe: ResMut<Probe>,
    mut players: Query<&mut AnimationGraphPlayer>,
    graphs: Res<Assets<AnimationGraph>>,
    assets: Res<GateAssets>,
    mut exit: MessageWriter<AppExit>,
) {
    if probe.players.len() != DIRECT_RUNTIME_COUNT {
        return;
    }
    let phase = if probe.phase_index == 0 { 0.25 } else { 0.5 };
    let mut samples = Vec::with_capacity(DIRECT_RUNTIME_COUNT);
    for runtime in &probe.players {
        let spec = GRAPH_SPECS[runtime.spec_index];
        let graph_handle = &assets.graphs[runtime.spec_index];
        let Some(graph) = graphs.get(graph_handle) else {
            return;
        };
        let Ok(player) = players.get_mut(runtime.entity) else {
            fail(&mut exit, "probe player missing");
            return;
        };
        if let Some(error) = player.get_error() {
            fail(
                &mut exit,
                &format!("{} runtime error={error:?}", spec.label),
            );
            return;
        }
        match sample_contract(&player, graph, spec, phase) {
            Ok(bones) => samples.push((spec.label, bones)),
            Err(message) => {
                fail(&mut exit, &message);
                return;
            }
        }
    }
    let expected = phase * IDLE_DURATION;
    info!(
        "[SECONDARY-MOTION-SYNC] phase={phase:.2} sample={expected:.6} graphs={} bones={:?}",
        samples.len(),
        samples,
    );
    if probe.phase_index == 0 {
        for runtime in &probe.players {
            let Ok(mut player) = players.get_mut(runtime.entity) else {
                fail(&mut exit, "probe player update missing");
                return;
            };
            player.set_global_input_data(PHASE_KEY, 0.5_f32);
            player.queue_time_update(TimeUpdate::Delta(0.0));
        }
        probe.phase_index = 1;
        return;
    }
    info!("[SECONDARY-MOTION-SYNC][VERDICT] 13 direct synchronized overlay samples -> PASS");
    exit.write(AppExit::Success);
}

fn timeout(deadline: Res<Deadline>, mut exit: MessageWriter<AppExit>) {
    if Instant::now() >= deadline.0 {
        fail(&mut exit, "timeout");
    }
}
