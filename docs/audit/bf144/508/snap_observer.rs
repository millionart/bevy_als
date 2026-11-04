use bevy_als::extensions::source_native_refresh::{AlsSourceRefreshFunction, AlsSourceRefreshNode};
use bevy_animation_graph::core::{
    animation_node::{NodeLike, ReflectNodeLike},
    context::{new_context::NodeContext, spec_context::SpecContext},
    edge_data::DataValue,
    errors::GraphError,
};

fn c508_configure(
    mut graphs: ResMut<Assets<bevy_animation_graph::core::animation_graph::AnimationGraph>>,
    mut installed: Local<bool>,
) {
    if *installed || std::env::var_os("ALS_C508_SNAP").is_none() {
        return;
    }
    for (_, graph) in graphs.iter_mut() {
        for node in graph.nodes.values_mut() {
            if node.id.uuid().to_string() != C508_REFRESH {
                continue;
            }
            let source = node.try_inner_downcast_ref::<AlsSourceRefreshNode>().unwrap().clone();
            assert!(matches!(source.function, AlsSourceRefreshFunction::RefreshGrounded));
            let mut replacement = bevy_animation_graph::core::animation_node::AnimationNode::new(
                node.name.clone(), C508SnapRefresh(source));
            replacement.id = node.id;
            *node = replacement;
            *installed = true;
            return;
        }
    }
}

const C508_REFRESH: &str = "18c00b87-478a-29b9-7505-1daa6d305742";

#[derive(Reflect, Clone, Debug)]
#[reflect(NodeLike)]
struct C508SnapRefresh(AlsSourceRefreshNode);

impl NodeLike for C508SnapRefresh {
    fn update(&self, ctx: NodeContext) -> Result<(), GraphError> {
        use bevy_als::extensions::source_instance::set_source_property;
        if ctx.graph_context.frame_index() == 112 {
            set_source_property(&ctx, "grounded_state.velocity_blend.b_initialization_required", DataValue::Bool(true))?;
        }
        self.0.update(ctx)
    }

    fn spec(&self, ctx: SpecContext) -> Result<(), GraphError> {
        self.0.spec(ctx)
    }

    fn display_name(&self) -> String {
        self.0.display_name()
    }
}
