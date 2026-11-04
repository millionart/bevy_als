use bevy_als::als::state::LocomotionState;
use bevy_als::extensions::{
    source_instance::{SourceInstanceProperties, set_source_property, source_input_data, source_player_property},
    source_native_refresh::{AlsSourceRefreshFunction, AlsSourceRefreshNode},
};
use bevy_animation_graph::core::{
    animation_graph::{AnimationGraph, NodeId},
    animation_graph_player::AnimationGraphPlayer,
    animation_node::{AnimationNode, NodeLike, ReflectNodeLike},
    context::{new_context::NodeContext, node_states::StateKey, spec_context::SpecContext},
    edge_data::DataValue,
    errors::GraphError,
};

const C509_HALF_LIFE: &str = "in_air_source.lean_interpolation_half_life";
const C509_PENDING: &str = "animation.pending_update";

fn c509_config_input(
    probe: Res<C458Probe>,
    avatars: Query<
        &bevy_animation_graph::core::animated_scene::AnimatedSceneInstance,
        With<bevy_als::character_assembly::AlsCharacterAvatar>,
    >,
    mut players: Query<&mut AnimationGraphPlayer>,
) {
    if !matches!(probe.frame, 33 | 45) {
        return;
    }
    let Ok(value) = std::env::var("ALS_C509_HALF_LIFE") else { return; };
    let value = value.parse::<f32>().unwrap();
    assert!(value == 0.0 || value == 0.6);
    for avatar in &avatars {
        if let Ok(mut player) = players.get_mut(avatar.player_entity()) {
            assert_eq!(player.get_global_input_data(C509_HALF_LIFE).unwrap().as_f32().unwrap(), 0.2);
            player.set_global_input_data(C509_HALF_LIFE, value);
        }
    }
}

fn c509_install_ground_control(mut graphs: ResMut<Assets<AnimationGraph>>, mut installed: Local<bool>) {
    if *installed || std::env::var_os("ALS_C509_GROUND_SNAP").is_none() {
        return;
    }
    for (_, graph) in graphs.iter_mut() {
        for node in graph.nodes.values_mut() {
            if node.id.uuid().to_string() != "18c00b87-478a-29b9-7505-1daa6d305742" {
                continue;
            }
            let source = node.try_inner_downcast_ref::<AlsSourceRefreshNode>().unwrap().clone();
            assert!(matches!(source.function, AlsSourceRefreshFunction::RefreshGrounded));
            let mut replacement = AnimationNode::new(node.name.clone(), C509GroundSnap(source));
            replacement.id = node.id;
            *node = replacement;
            *installed = true;
            return;
        }
    }
}

#[derive(Reflect, Clone, Debug)]
#[reflect(NodeLike)]
struct C509GroundSnap(AlsSourceRefreshNode);

impl NodeLike for C509GroundSnap {
    fn update(&self, ctx: NodeContext) -> Result<(), GraphError> {
        if ctx.graph_context.frame_index() != 112 {
            return self.0.update(ctx);
        }
        let previous = source_input_data(&ctx, C509_PENDING).unwrap();
        assert!(!previous.as_bool()?);
        set_source_property(&ctx, C509_PENDING, DataValue::Bool(true))?;
        self.0.update(ctx.clone())?;
        set_source_property(&ctx, C509_PENDING, previous)
    }

    fn spec(&self, ctx: SpecContext) -> Result<(), GraphError> {
        self.0.spec(ctx)
    }

    fn display_name(&self) -> String {
        self.0.display_name()
    }
}

fn c509_state(player: &AnimationGraphPlayer, locomotion: LocomotionState) -> serde_json::Value {
    let raw = |key| player.get_global_input_data(key);
    let scalar = |key| raw(key).and_then(|value| value.as_f32().ok());
    let vector = |key| raw(key).and_then(|value| value.as_vec3().ok());
    let keys = ["lean_state.forward_amount", "lean_state.right_amount"];
    let amounts = keys.map(|key| source_player_property(player, key).and_then(|value| value.as_f32().ok()));
    let write_frame = player.get_context_arena().and_then(|arena| {
        arena.get_toplevel().node_states
            .get::<SourceInstanceProperties>(NodeId::default(), StateKey::Default)
            .ok()
            .and_then(|state| state.write_frames.get(keys[0]).copied())
    });
    let goal = if locomotion == LocomotionState::Grounded {
        vector("locomotion_state.acceleration_world_space").zip(vector("locomotion_state.velocity_world_space"))
            .map(|(acceleration, velocity)| bevy_als::als::animation::acceleration_amount(
                acceleration, velocity,
                raw("locomotion_state.rotation_quaternion_world_space")
                    .and_then(|value| value.as_quat().ok()).unwrap_or(Quat::IDENTITY),
                scalar("locomotion_state.max_acceleration").unwrap_or_default(),
                scalar("locomotion_state.max_braking_deceleration").unwrap_or_default(),
            ))
    } else {
        raw("in_air_source.lean_target").and_then(|value| value.as_vec2().ok())
    };
    serde_json::json!({
        "amounts":amounts,
        "write_frame":write_frame,
        "pending":raw(C509_PENDING).and_then(|value| value.as_bool().ok()),
        "air_half_life":scalar(C509_HALF_LIFE),
        "goal":goal.map(|value| value.to_array())
    })
}
