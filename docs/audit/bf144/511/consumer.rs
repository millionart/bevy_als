const C511_FORWARD: &str = "lean_state.forward_amount";
const C511_FORWARD_GUID: &str = "8bf8140b-6f73-578b-9715-8961d134047d";
const C511_POSITION_GUID: &str = "6c1fffa0-d2ee-5aa2-99f8-54338425771f";
const C511_LEAN_GUID: &str = "34f57700-443d-7c77-5457-2cb100b5b922";
const C511_APPLY_GUID: &str = "7c742ec2-4a57-89f7-5699-fd9f6869c86c";
const C511_SPRINT_GUID: &str = "399f03d7-4ff5-edce-96c3-35a0f7a7c922";

fn c511_configure(mut graphs: ResMut<Assets<AnimationGraph>>, mut installed: Local<bool>) {
    if *installed || std::env::var_os("ALS_C511_LOCAL_CLAMP").is_none() {
        return;
    }
    for (_, graph) in graphs.iter_mut() {
        for node in graph.nodes.values_mut() {
            if node.id.uuid().to_string() != C511_FORWARD_GUID {
                continue;
            }
            let source = node.try_inner_downcast_ref::<bevy_als::extensions::global_f32_input::GlobalF32InputNode>().unwrap().clone();
            assert_eq!(source.key, C511_FORWARD);
            let mut replacement = AnimationNode::new(node.name.clone(), C511LeanForward(source));
            replacement.id = node.id;
            *node = replacement;
            *installed = true;
            return;
        }
    }
}

#[derive(Reflect, Clone, Debug)]
#[reflect(NodeLike)]
struct C511LeanForward(bevy_als::extensions::global_f32_input::GlobalF32InputNode);

impl NodeLike for C511LeanForward {
    fn update(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        if ctx.graph_context.frame_index() != 141 {
            return self.0.update(ctx);
        }
        let value = source_input_data(&ctx, C511_FORWARD)
            .ok_or_else(|| GraphError::GlobalInputDataMissing(C511_FORWARD.to_owned()))?
            .as_f32()?;
        assert_eq!(value, 0.4051828980445862_f32);
        ctx.set_data_fwd(C511_FORWARD, 0.4051828682422638_f32);
        Ok(())
    }

    fn spec(&self, mut ctx: SpecContext) -> Result<(), GraphError> {
        ctx.add_output_data(C511_FORWARD, bevy_animation_graph::core::edge_data::DataSpec::F32);
        Ok(())
    }

    fn display_name(&self) -> String {
        "Audit Standing Lean forward input".to_owned()
    }
}

fn c511_consumer(
    player: &AnimationGraphPlayer,
    graphs: &Assets<AnimationGraph>,
    skeletons: &Assets<bevy_animation_graph::core::skeleton::Skeleton>,
    assets: &AssetServer,
) -> serde_json::Value {
    use bevy_als::extensions::animation_context::node_context_state;
    let Some(arena) = player.get_context_arena() else { return serde_json::Value::Null; };
    let scalar = |key| source_player_property(player, key).and_then(|value| value.as_f32().ok());
    let mut nodes = Vec::new();
    for id in arena.iter_context_ids() {
        let context = arena.get_context(id).unwrap();
        if graphs.get(context.get_graph_id()).is_none() { continue; }
        for (node, key) in c458_updated_keys(&context.node_caches) {
            let guid = node.uuid().to_string();
            if ![C511_POSITION_GUID,C511_LEAN_GUID,C511_APPLY_GUID,C511_SPRINT_GUID].contains(&guid.as_str()) {
                continue;
            }
            let cache = &context.node_caches;
            let pose = cache.get_output_data(node, key, "Pose".into())
                .or_else(|| cache.get_output_data(node, key, "pose".into()))
                .and_then(|value| value.as_pose().ok().map(|pose| c458_pose_value(pose,skeletons,assets)));
            let coordinates = cache.get_output_data(node,key,"value".into())
                .and_then(|value| value.as_vec2().ok()).map(|v| v.to_array());
            let samples = node_context_state(&context.node_states,key,node).map(|state| {
                state.asset_samples.iter().map(|sample| serde_json::json!({
                    "identity":sample.identity,"position":sample.clock.position,"weight":sample.clock.weight
                })).collect::<Vec<_>>()
            });
            nodes.push(serde_json::json!({"guid":guid,"context":format!("{id:?}"),"coordinates":coordinates,
                "pose":pose,"samples":samples}));
        }
    }
    nodes.sort_by_key(|node| (node["guid"].to_string(),node["context"].to_string()));
    serde_json::json!({"nodes":nodes,"sprint_time":scalar("standing_state.sprint_time"),
        "sprint_acceleration":scalar("standing_state.sprint_acceleration_amount"),
        "running_alpha_input":scalar("pose_state.unweighted_gait_running_amount"),
        "sprinting":player.get_global_input_data("als_standing_gait_is_sprinting").and_then(|value| value.as_f32().ok())})
}
