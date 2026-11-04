const C512_APPLIED: &str = "audit.sprint.acceleration_override";
const C512_FORWARD: &str = "standing_source.acceleration_forward";
const C512_REFRESH: &str = "4ae6df60-47a4-b6cd-6db2-b2a8cbb5fbb9";
const C512_SPRINT: &str = "399f03d7-4ff5-edce-96c3-35a0f7a7c922";

fn c512_configure(mut graphs: ResMut<Assets<AnimationGraph>>, mut installed: Local<bool>) {
    if *installed {
        return;
    }
    let Ok(label) = std::env::var("ALS_C512_SPRINT_OVERRIDE") else { return; };
    assert!(matches!(label.as_str(), "acceleration-only" | "cutoff"));
    let data: serde_json::Value = serde_json::from_slice(
        &std::fs::read(format!("docs/audit/bf144/512/control-{label}.json")).unwrap()).unwrap();
    for (_, graph) in graphs.iter_mut() {
        for node in graph.nodes.values_mut() {
            if node.id.uuid().to_string() != C512_REFRESH {
                continue;
            }
            let source = node.try_inner_downcast_ref::<AlsSourceRefreshNode>().unwrap().clone();
            assert!(matches!(source.function, AlsSourceRefreshFunction::RefreshStandingMovement));
            let config = C512StandingInput {
                source,
                frame: data["animation_frame"].as_u64().unwrap(),
                baseline: data["baseline"].as_f64().unwrap() as f32,
                value: data["control"].as_f64().unwrap() as f32,
            };
            let mut replacement = AnimationNode::new(node.name.clone(), config);
            replacement.id = node.id;
            *node = replacement;
            *installed = true;
            return;
        }
    }
}

#[derive(Reflect, Clone, Debug)]
#[reflect(NodeLike)]
struct C512StandingInput {
    source: AlsSourceRefreshNode,
    frame: u64,
    baseline: f32,
    value: f32,
}

impl NodeLike for C512StandingInput {
    fn update(&self, ctx: NodeContext) -> Result<(), GraphError> {
        if ctx.graph_context.frame_index() != self.frame {
            return self.source.update(ctx);
        }
        let old = source_input_data(&ctx, C512_FORWARD).unwrap();
        if self.value == 0.125 {
            assert_eq!(old.as_f32()?, self.baseline);
        } else {
            assert!(old.as_f32()?.abs() < 2.0e-6);
        }
        set_source_property(&ctx, C512_FORWARD, self.value)?;
        self.source.update(ctx.clone())?;
        set_source_property(&ctx, C512_FORWARD, old)?;
        set_source_property(&ctx, C512_APPLIED, self.value)
    }

    fn spec(&self, ctx: SpecContext) -> Result<(), GraphError> {
        self.source.spec(ctx)
    }

    fn display_name(&self) -> String {
        self.source.display_name()
    }
}

fn c512_sprint(
    player: &AnimationGraphPlayer,
    graphs: &Assets<AnimationGraph>,
    skeletons: &Assets<bevy_animation_graph::core::skeleton::Skeleton>,
    assets: &AssetServer,
    before: bool,
) -> serde_json::Value {
    use bevy_als::extensions::source_pose_nodes::SourceTwoWayBlendState;
    let Some(arena) = player.get_context_arena() else { return serde_json::Value::Null; };
    let scalar = |key| source_player_property(player,key).and_then(|v| v.as_f32().ok());
    let keys = ["standing_state.sprint_time","standing_state.sprint_acceleration_amount"];
    let source_state = arena.get_toplevel().node_states
        .get::<SourceInstanceProperties>(NodeId::default(),StateKey::Default).ok();
    let mut mixed = Vec::new();
    if !before {
        for id in arena.iter_context_ids() {
            let context = arena.get_context(id).unwrap();
            if graphs.get(context.get_graph_id()).is_none() { continue; }
            for (node,key) in c458_updated_keys(&context.node_caches) {
                if node.uuid().to_string()!=C512_SPRINT { continue; }
                let state = context.node_states.get::<SourceTwoWayBlendState>(node,key).unwrap();
                let pose = context.node_caches.get_output_data(node,key,"Pose".into())
                    .and_then(|v|v.as_pose().ok().map(|pose|c458_pose_value(pose,skeletons,assets)));
                mixed.push(serde_json::json!({"context":format!("{id:?}"),"key":format!("{key:?}"),
                    "input":state.input_alpha,"alpha":state.blend_alpha,"pose":pose}));
            }
        }
    }
    mixed.sort_by_key(|row|(row["context"].to_string(),row["key"].to_string()));
    serde_json::json!({
        "time":scalar(keys[0]),"acceleration":scalar(keys[1]),
        "written":keys.map(|key|source_state.and_then(|state|state.write_frames.get(key)).copied()),
        "pending":player.get_global_input_data("animation.pending_update").and_then(|v|v.as_bool().ok()),
        "flag":player.get_global_input_data("als_standing_gait_is_sprinting").and_then(|v|v.as_f32().ok()),
        "override":source_state.and_then(|state| {
            (state.write_frames.get(C512_APPLIED)==Some(&arena.frame_index()))
                .then(|| scalar(C512_APPLIED)).flatten()
        }),
        "mixed":mixed
    })
}
