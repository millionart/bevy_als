use std::num::NonZeroU8;
use std::time::{Duration, Instant};

use als_core::character::AlsCharacterBundle;
use als_core::control_scheme::AlsControlSchemeConfig;
use als_core::params::AlsMovementParams;
use als_core::state::{OverlayState, RotationMode};
use als_ext::GlobalF32InputNode;
use avian3d::prelude::{Collider, RigidBody};
use bevy::ecs::hierarchy::ChildOf;
use bevy::ecs::message::MessageWriter;
use bevy::prelude::*;
use bevy_animation_graph::core::animated_scene::AnimatedSceneInstance;
use bevy_animation_graph::core::animation_graph::AnimationGraph;
use bevy_animation_graph::core::animation_graph_player::AnimationGraphPlayer;
use bevy_animation_graph::core::context::node_states::StateKey;
use bevy_animation_graph::core::edge_data::DataValue;

use bevy_als::ab_als_root::AlsRootGraphSet;
use bevy_als::character_assembly::{AlsCharacterAvatar, attach_avatar};
use bevy_als::playable_character::AlsPlayableCharacterPlugin;
use bevy_als::standing_layer::AlsStandingLayerSet;

const SPRINT_SOURCE: &str = "als_standing_sprint_acceleration_amount";
const ALLOW_SOURCE: &str = "als_standing_curve_allow_transitions";
const SPRINT_OVERLAY: &str = "als_overlay_sprint_acceleration_amount";
const ALLOW_OVERLAY: &str = "als_overlay_transitions_allowed";
const FULL_WEIGHT_EPSILON: f32 = 1e-4;

#[derive(Resource)]
struct Deadline(Instant);

#[derive(Resource)]
struct GateAssets {
    relaxed: Handle<AnimationGraph>,
    transition_consumers: Vec<Handle<AnimationGraph>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Sprint,
    Blocked,
}

#[derive(Resource)]
struct Probe {
    character: Entity,
    phase: Phase,
    last_values: Option<(f32, f32, f32, f32)>,
    last_sprint_consumer: Option<f32>,
}

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins.set(AssetPlugin {
                file_path: "assets".into(),
                ..default()
            }),
            AlsPlayableCharacterPlugin,
        ))
        .insert_resource(Deadline(Instant::now() + Duration::from_secs(15)))
        .add_systems(Startup, setup)
        .add_systems(
            PostUpdate,
            seed_standing_inputs
                .after(AlsStandingLayerSet)
                .before(AlsRootGraphSet),
        )
        .add_systems(PostUpdate, observe.after(AlsRootGraphSet))
        .add_systems(Update, timeout)
        .run();
}

fn setup(
    mut commands: Commands,
    mut params_assets: ResMut<Assets<AlsMovementParams>>,
    mut config_assets: ResMut<Assets<AlsControlSchemeConfig>>,
    assets: Res<AssetServer>,
) {
    commands.spawn((
        RigidBody::Static,
        Collider::cuboid(40.0, 0.2, 40.0),
        Transform::from_xyz(0.0, -0.1, 0.0),
    ));
    let params = params_assets.add(AlsMovementParams::default());
    let config = config_assets.add(AlsControlSchemeConfig::default());
    let mut bundle = AlsCharacterBundle::new(params, config);
    bundle.transform = Transform::from_xyz(0.0, 0.92, 0.0);
    bundle.overlay_state = OverlayState::Rifle;
    bundle.rotation_mode = RotationMode::ViewDirection;
    let character = commands.spawn(bundle).id();
    attach_avatar(
        &mut commands,
        &assets,
        character,
        "animated_scenes/motion_overlay.animscn.ron",
    );
    commands.insert_resource(GateAssets {
        relaxed: assets.load("graphs/ab_als/overlays/rifle/state_relaxed.animgraph.ron"),
        transition_consumers: [
            "graphs/ab_als/overlays/bow/state_ready.animgraph.ron",
            "graphs/ab_als/overlays/pistol_one_handed/state_ready.animgraph.ron",
            "graphs/ab_als/overlays/pistol_one_handed/state_relaxed.animgraph.ron",
            "graphs/ab_als/overlays/pistol_two_handed/state_ready.animgraph.ron",
            "graphs/ab_als/overlays/pistol_two_handed/state_relaxed.animgraph.ron",
            "graphs/ab_als/overlays/rifle/state_ready.animgraph.ron",
            "graphs/ab_als/overlays/rifle/state_relaxed.animgraph.ron",
        ]
        .into_iter()
        .map(|path| assets.load(path))
        .collect(),
    });
    commands.insert_resource(Probe {
        character,
        phase: Phase::Sprint,
        last_values: None,
        last_sprint_consumer: None,
    });
}

fn seed_standing_inputs(
    probe: Res<Probe>,
    avatars: Query<(&ChildOf, &AnimatedSceneInstance), With<AlsCharacterAvatar>>,
    mut players: Query<&mut AnimationGraphPlayer>,
) {
    for (parent, instance) in &avatars {
        if parent.parent() != probe.character {
            continue;
        }
        let Ok(mut player) = players.get_mut(instance.player_entity()) else {
            continue;
        };
        match probe.phase {
            Phase::Sprint => {
                player.set_global_input_data(SPRINT_SOURCE, 0.35_f32);
                player.set_global_input_data(ALLOW_SOURCE, 1.0_f32);
            }
            Phase::Blocked => {
                player.set_global_input_data(SPRINT_SOURCE, 0.0_f32);
                player.set_global_input_data(ALLOW_SOURCE, -1.0_f32);
            }
        }
    }
}

fn global_f32(player: &AnimationGraphPlayer, key: &str) -> Option<f32> {
    player
        .get_global_input_data(key)
        .and_then(|value| value.as_f32().ok())
        .filter(|value| value.is_finite())
}

fn cached_f32(
    player: &AnimationGraphPlayer,
    graph: &AnimationGraph,
    node_name: &str,
    output: &str,
) -> Option<f32> {
    let node = graph.nodes.values().find(|node| node.name == node_name)?;
    let arena = player.get_context_arena()?;
    arena.iter_context_ids().find_map(|context_id| {
        match arena.get_context(context_id)?.node_caches.get_output_data(
            node.id,
            StateKey::Default,
            output.to_owned(),
        ) {
            Some(DataValue::F32(value)) if value.is_finite() => Some(value),
            _ => None,
        }
    })
}

fn has_global_consumer(graph: &AnimationGraph, node_name: &str, key: &str) -> bool {
    graph.nodes.values().any(|node| {
        node.name == node_name
            && node
                .try_inner_downcast_ref::<GlobalF32InputNode>()
                .is_some_and(|input| input.key == key)
    })
}

fn close(actual: f32, expected: f32) -> bool {
    (actual - expected).abs() < 1e-4
}

fn observe(
    mut probe: ResMut<Probe>,
    assets: Res<GateAssets>,
    graphs: Res<Assets<AnimationGraph>>,
    avatars: Query<(&ChildOf, &AnimatedSceneInstance), With<AlsCharacterAvatar>>,
    players: Query<&AnimationGraphPlayer>,
    mut exit: MessageWriter<AppExit>,
) {
    let Some((_, instance)) = avatars
        .iter()
        .find(|(parent, _)| parent.parent() == probe.character)
    else {
        return;
    };
    let Ok(player) = players.get(instance.player_entity()) else {
        return;
    };
    let Some(relaxed) = graphs.get(&assets.relaxed) else {
        return;
    };
    let (Some(source_sprint), Some(overlay_sprint), Some(source_allow), Some(overlay_allow)) = (
        global_f32(player, SPRINT_SOURCE),
        global_f32(player, SPRINT_OVERLAY),
        global_f32(player, ALLOW_SOURCE),
        global_f32(player, ALLOW_OVERLAY),
    ) else {
        return;
    };
    probe.last_values = Some((source_sprint, overlay_sprint, source_allow, overlay_allow));
    match probe.phase {
        Phase::Sprint => {
            let consumer = cached_f32(
                player,
                relaxed,
                "sprint acceleration amount",
                SPRINT_OVERLAY,
            );
            probe.last_sprint_consumer = consumer;
            if source_sprint.abs() <= 1e-3
                || !close(source_sprint, overlay_sprint)
                || !consumer.is_some_and(|value| close(value, overlay_sprint))
            {
                return;
            }
            info!(
                "[BF102.08] sprint_source={source_sprint:.3} overlay_sprint={overlay_sprint:.3} rifle_consumer={:.3}",
                consumer.unwrap_or_default()
            );
            probe.phase = Phase::Blocked;
        }
        Phase::Blocked => {
            let expected_allowed = (source_allow >= 1.0 - FULL_WEIGHT_EPSILON) as u8 as f32;
            let consumer_bound = has_global_consumer(relaxed, "transitions allowed", ALLOW_OVERLAY);
            let all_transition_consumers_bound = assets.transition_consumers.iter().all(|handle| {
                graphs.get(handle).is_some_and(|graph| {
                    has_global_consumer(graph, "transitions allowed", ALLOW_OVERLAY)
                })
            });
            if source_allow >= 1.0 - FULL_WEIGHT_EPSILON
                || !close(overlay_allow, expected_allowed)
                || !consumer_bound
                || !all_transition_consumers_bound
            {
                return;
            }
            info!(
                "[BF102.08] allow_curve={source_allow:.3} overlay_allowed={overlay_allow:.3} rifle_binding={consumer_bound} transition_consumers=7"
            );
            info!("[BF102.08][VERDICT] standing_and_curve_inputs_reach_rifle -> PASS");
            exit.write(AppExit::Success);
        }
    }
}

fn timeout(deadline: Res<Deadline>, probe: Res<Probe>, mut exit: MessageWriter<AppExit>) {
    if Instant::now() >= deadline.0 {
        error!(
            "[BF102.08][VERDICT] timeout phase={:?} values={:?} sprint_consumer={:?} -> FAIL",
            probe.phase, probe.last_values, probe.last_sprint_consumer,
        );
        exit.write(AppExit::Error(NonZeroU8::new(8).unwrap()));
    }
}
