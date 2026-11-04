#[derive(Reflect, Clone, Debug)]
#[reflect(NodeLike)]
struct C503RightInput;

impl NodeLike for C503RightInput {
    fn display_name(&self) -> String {
        "Audit right velocity weight".into()
    }
    fn update(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        use bevy_als::extensions::source_instance::source_input_data;
        let use_control = ctx
            .graph_context
            .global_input_data
            .get("audit.c503_widen")
            .is_some_and(|value| value.as_bool().ok() == Some(true));
        let value = if use_control {
            C503_RIGHT
        } else {
            source_input_data(&ctx, "grounded_state.velocity_blend.right_amount")
                .map_or(Ok(0.0), |value| value.as_f32())?
        };
        ctx.set_data_fwd("Value", value);
        Ok(())
    }
    fn spec(&self, mut ctx: SpecContext) -> Result<(), GraphError> {
        ctx.add_output_data("Value", DataSpec::F32);
        Ok(())
    }
}

use bevy_animation_graph::core::{
    animation_graph::{AnimationGraph, TargetPin},
    animation_graph_player::AnimationGraphPlayer,
    animation_node::{AnimationNode, NodeLike, ReflectNodeLike},
    context::{new_context::NodeContext, spec_context::SpecContext},
    edge_data::DataSpec,
    errors::GraphError,
};

fn c503_bind_and_publish(
    mut graphs: ResMut<Assets<AnimationGraph>>,
    mut installed: Local<bool>,
    mut targets: Local<std::collections::HashSet<String>>,
    probe: Res<C458Probe>,
    avatars: Query<
        &bevy_animation_graph::core::animated_scene::AnimatedSceneInstance,
        With<AlsCharacterAvatar>,
    >,
    mut players: Query<&mut AnimationGraphPlayer>,
) {
    if !*installed {
        for (_, graph) in graphs.iter_mut() {
            let ids = graph
                .nodes
                .keys()
                .filter_map(|id| {
                    let name = id.uuid().to_string();
                    ["41fd7093", "69a33590"]
                        .contains(&&name[..8])
                        .then_some((*id, name))
                })
                .collect::<Vec<_>>();
            for (id, name) in ids {
                if !targets.insert(name) {
                    continue;
                }
                let pin = "DesiredAlphas_3";
                assert!(
                    !graph
                        .edges_inverted
                        .contains_key(&TargetPin::NodeData(id, pin.into()))
                );
                let input = AnimationNode::new("audit D282 right weight", C503RightInput);
                let input_id = input.id;
                graph.add_node(input);
                graph.add_node_parameter_edge(input_id, "Value", id, pin);
            }
        }
        *installed = targets.len() == 2;
    }
    let use_control =
        std::env::var_os("ALS_C503_WIDEN").is_some() && probe.animation_after.len() == 101;
    if use_control {
        assert!(*installed);
    }
    for avatar in &avatars {
        if let Ok(mut player) = players.get_mut(avatar.player_entity()) {
            player.set_global_input_data("audit.c503_widen", use_control);
        }
    }
}
