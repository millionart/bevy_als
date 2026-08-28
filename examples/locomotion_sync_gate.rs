use std::time::{Duration, Instant};

use als_ext::{AlsExtPlugin, GlobalF32InputNode};
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

const FALL_PATH: &str = "graphs/ab_als/locomotion/state_fall.animgraph.ron";
const JUMP_PATH: &str = "graphs/ab_als/locomotion/state_jump_left.animgraph.ron";
const LOOP_PATH: &str = "graphs/ab_als/locomotion/state_jump_loop.animgraph.ron";
const FLAIL_PATH: &str = "graphs/ab_als/locomotion/state_jump_flail.animgraph.ron";
const LAND_PATH: &str = "graphs/ab_als/locomotion/state_land.animgraph.ron";
const SKELETON_PATH: &str = "skeletons/SKM_Als.skn.ron";
const EPSILON: f32 = 1e-4;

#[derive(Resource)]
struct GateAssets {
    fall: Handle<AnimationGraph>,
    jump: Handle<AnimationGraph>,
    loop_graph: Handle<AnimationGraph>,
    flail: Handle<AnimationGraph>,
    land: Handle<AnimationGraph>,
    skeleton: Handle<Skeleton>,
}

#[derive(Resource, Default)]
struct Probe {
    fall: Option<Entity>,
    jump: Option<Entity>,
    loop_graph: Option<Entity>,
    flail: Option<Entity>,
    land: Option<Entity>,
    end_checked: bool,
}

#[derive(Resource)]
struct Deadline(Instant);

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
        .insert_resource(Deadline(Instant::now() + Duration::from_secs(25)))
        .add_systems(Startup, load_assets)
        .add_systems(Update, (initialize, timeout))
        .add_systems(PostUpdate, observe.after(AnimationGraphSet::Final))
        .run();
}

fn load_assets(mut commands: Commands, assets: Res<AssetServer>) {
    commands.insert_resource(GateAssets {
        fall: assets.load(FALL_PATH),
        jump: assets.load(JUMP_PATH),
        loop_graph: assets.load(LOOP_PATH),
        flail: assets.load(FLAIL_PATH),
        land: assets.load(LAND_PATH),
        skeleton: assets.load(SKELETON_PATH),
    });
    info!("[LOCOMOTION-SYNC] loading Fall Jump Flail Land graphs");
}

fn player_for(
    graph: &AnimationGraph,
    handle: Handle<AnimationGraph>,
    skeleton: Handle<Skeleton>,
    phase_key: &str,
    phase: f32,
) -> AnimationGraphPlayer {
    let mut player = AnimationGraphPlayer::new(skeleton).with_graph(handle);
    player.set_global_input_data("als_root_defaults_enabled", true);
    for node in graph.nodes.values() {
        if let Some(node) = node.try_inner_downcast_ref::<GlobalF32InputNode>() {
            player.set_global_input_data(node.key.clone(), 0.0_f32);
        }
    }
    player.set_global_input_data(phase_key, phase);
    player.pause();
    player.queue_time_update(TimeUpdate::Delta(0.0));
    player
}

fn initialize(
    mut commands: Commands,
    server: Res<AssetServer>,
    assets: Res<GateAssets>,
    graphs: Res<Assets<AnimationGraph>>,
    mut probe: ResMut<Probe>,
) {
    if probe.fall.is_some()
        || !server.is_loaded_with_dependencies(&assets.fall)
        || !server.is_loaded_with_dependencies(&assets.jump)
        || !server.is_loaded_with_dependencies(&assets.loop_graph)
        || !server.is_loaded_with_dependencies(&assets.flail)
        || !server.is_loaded_with_dependencies(&assets.land)
        || !server.is_loaded_with_dependencies(&assets.skeleton)
    {
        return;
    }
    let (Some(fall), Some(jump), Some(loop_graph), Some(flail), Some(land)) = (
        graphs.get(&assets.fall),
        graphs.get(&assets.jump),
        graphs.get(&assets.loop_graph),
        graphs.get(&assets.flail),
        graphs.get(&assets.land),
    ) else {
        return;
    };
    probe.fall = Some(
        commands
            .spawn((
                Name::new("locomotion_fall_sync_gate"),
                Children::default(),
                player_for(
                    fall,
                    assets.fall.clone(),
                    assets.skeleton.clone(),
                    "als_locomotion_fall_phase",
                    0.2,
                ),
            ))
            .id(),
    );
    probe.jump = Some(
        commands
            .spawn((
                Name::new("locomotion_jump_sync_gate"),
                Children::default(),
                player_for(
                    jump,
                    assets.jump.clone(),
                    assets.skeleton.clone(),
                    "als_locomotion_jump_phase",
                    0.3,
                ),
            ))
            .id(),
    );
    probe.loop_graph = Some(
        commands
            .spawn((
                Name::new("locomotion_loop_sync_gate"),
                Children::default(),
                player_for(
                    loop_graph,
                    assets.loop_graph.clone(),
                    assets.skeleton.clone(),
                    "als_locomotion_flail_phase",
                    0.5,
                ),
            ))
            .id(),
    );
    probe.flail = Some(
        commands
            .spawn((
                Name::new("locomotion_flail_sync_gate"),
                Children::default(),
                player_for(
                    flail,
                    assets.flail.clone(),
                    assets.skeleton.clone(),
                    "als_locomotion_flail_phase",
                    0.5,
                ),
            ))
            .id(),
    );
    probe.land = Some(
        commands
            .spawn((
                Name::new("locomotion_land_sync_gate"),
                Children::default(),
                player_for(
                    land,
                    assets.land.clone(),
                    assets.skeleton.clone(),
                    "als_locomotion_land_phase",
                    0.4,
                ),
            ))
            .id(),
    );
    info!("[LOCOMOTION-SYNC] graph probes initialized");
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

fn sample_pose(player: &AnimationGraphPlayer, graph: &AnimationGraph, name: &str) -> Option<Pose> {
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

fn verify(
    player: &AnimationGraphPlayer,
    graph: &AnimationGraph,
    node: &str,
    expected_time: f32,
) -> Result<usize, String> {
    let time = sample_time(player, graph, node).ok_or_else(|| format!("{node} sample missing"))?;
    let pose = sample_pose(player, graph, node).ok_or_else(|| format!("{node} pose missing"))?;
    if !close(time, expected_time) || !pose_is_valid(&pose) {
        return Err(format!(
            "{node} actual={time:.6} expected={expected_time:.6} bones={}",
            pose.bones.len(),
        ));
    }
    Ok(pose.bones.len())
}

fn fail(exit: &mut MessageWriter<AppExit>, message: &str) {
    warn!("[LOCOMOTION-SYNC][VERDICT] {message} -> FAIL");
    exit.write(AppExit::Error(std::num::NonZeroU8::new(7).unwrap()));
}

fn observe(
    mut probe: ResMut<Probe>,
    mut players: Query<&mut AnimationGraphPlayer>,
    graphs: Res<Assets<AnimationGraph>>,
    assets: Res<GateAssets>,
    mut exit: MessageWriter<AppExit>,
) {
    let (
        Some(fall_entity),
        Some(jump_entity),
        Some(loop_entity),
        Some(flail_entity),
        Some(land_entity),
    ) = (
        probe.fall,
        probe.jump,
        probe.loop_graph,
        probe.flail,
        probe.land,
    )
    else {
        return;
    };
    let (Some(fall_graph), Some(jump_graph), Some(loop_graph), Some(flail_graph), Some(land_graph)) = (
        graphs.get(&assets.fall),
        graphs.get(&assets.jump),
        graphs.get(&assets.loop_graph),
        graphs.get(&assets.flail),
        graphs.get(&assets.land),
    ) else {
        return;
    };
    let Ok([fall, mut jump, loop_player, flail_player, mut land]) = players.get_many_mut([
        fall_entity,
        jump_entity,
        loop_entity,
        flail_entity,
        land_entity,
    ]) else {
        return;
    };
    for player in [&fall, &jump, &loop_player, &flail_player, &land] {
        if let Some(error) = player.get_error() {
            fail(&mut exit, &format!("runtime error={error:?}"));
            return;
        }
    }
    if !probe.end_checked {
        let (fall_bones, jump_bones, loop_bones, flail_bones, land_bones) = match (
            verify(&fall, fall_graph, "fall marker player", 0.2),
            verify(&jump, jump_graph, "jump walk left marker player", 0.5),
            verify(
                &loop_player,
                loop_graph,
                "jump loop marker player",
                0.3333335,
            ),
            verify(
                &flail_player,
                flail_graph,
                "jump flail marker player",
                0.3333335,
            ),
            verify(&land, land_graph, "land light marker player", 0.4666668),
        ) {
            (Ok(fall), Ok(jump), Ok(loop_value), Ok(flail), Ok(land)) => {
                (fall, jump, loop_value, flail, land)
            }
            (fall, jump, loop_value, flail, land) => {
                let message = [fall, jump, loop_value, flail, land]
                    .into_iter()
                    .find_map(Result::err)
                    .unwrap_or_else(|| "initial sample mismatch".to_owned());
                fail(&mut exit, &message);
                return;
            }
        };
        info!(
            "[LOCOMOTION-SYNC] fall=.2 jump=.5 loop=.333333 flail=.333333 land=.466667 bones={fall_bones}/{jump_bones}/{loop_bones}/{flail_bones}/{land_bones}"
        );
        jump.set_global_input_data("als_locomotion_jump_phase", 1.0_f32);
        jump.queue_time_update(TimeUpdate::Delta(0.0));
        land.set_global_input_data("als_locomotion_land_phase", 1.0_f32);
        land.queue_time_update(TimeUpdate::Delta(0.0));
        probe.end_checked = true;
        return;
    }
    let (jump_bones, land_bones) = match (
        verify(&jump, jump_graph, "jump walk left marker player", 1.666667),
        verify(&land, land_graph, "land light marker player", 1.166667),
    ) {
        (Ok(jump), Ok(land)) => (jump, land),
        (jump, land) => {
            let message = [jump, land]
                .into_iter()
                .find_map(Result::err)
                .unwrap_or_else(|| "non-loop end mismatch".to_owned());
            fail(&mut exit, &message);
            return;
        }
    };
    info!("[LOCOMOTION-SYNC] jump_end=1.666667 land_end=1.166667 bones={jump_bones}/{land_bones}");
    info!("[LOCOMOTION-SYNC][VERDICT] Fall/Jump/Flail/Land phases -> PASS");
    exit.write(AppExit::Success);
}

fn timeout(deadline: Res<Deadline>, mut exit: MessageWriter<AppExit>) {
    if Instant::now() >= deadline.0 {
        fail(&mut exit, "timeout");
    }
}
