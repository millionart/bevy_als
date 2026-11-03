use avian3d::prelude::*;
use bevy::{ecs::system::RunSystemOnce, platform::collections::HashMap, prelude::*};
use bevy_als::extensions::{
    AlsExtPlugin, SourceAnimationInputs,
    animation_context::AnimationContextState,
    control_rig_hierarchy::{RigSpace, RigTransform},
    control_rig_program::{RigDefinition, RigValueRef},
    control_rig_query::{RigQueryContext, RigQueryWorld},
    post_physics_animation::evaluate_players,
    root_graph_boundaries::{
        ControlRigBoundaryNode, RIG_CAPTURE_KEY, RIG_GAME_WORLD_KEY, RigInputBinding,
        current_control_rig,
    },
};
use bevy_animation_graph::{
    AnimationGraphPlugin,
    core::{
        animation_clip::EntityPath,
        animation_graph::AnimationGraph,
        animation_graph_player::AnimationGraphPlayer,
        animation_node::{AnimationNode, NodeLike, ReflectNodeLike},
        context::{new_context::NodeContext, spec_context::SpecContext},
        edge_data::{DataSpec, DataValue},
        errors::GraphError,
        pose::{BonePose, Pose},
        skeleton::Skeleton,
    },
};
use serde_json::{Value, json};
use std::{path::Path, sync::Arc};

#[derive(Reflect, Clone, Debug)]
#[reflect(NodeLike)]
struct RecordedInput {
    curves: Vec<String>,
}

impl NodeLike for RecordedInput {
    fn display_name(&self) -> String {
        "Recorded pose and curves".into()
    }

    fn update(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        let data = ctx.graph_context.global_input_data;
        let pose = data.get("fixture.pose").cloned().unwrap().into_pose()?;
        let mut state = AnimationContextState::default();
        for name in &self.curves {
            if let Some(DataValue::F32(value)) = data.get(format!("fixture.curve.{name}").as_str())
            {
                state.curves.insert(name.clone().into(), *value);
            }
        }
        ctx.set_time(pose.timestamp);
        ctx.set_data_fwd("Pose", pose);
        *ctx.state_mut_or_else(AnimationContextState::default)? = state;
        Ok(())
    }

    fn spec(&self, mut ctx: SpecContext) -> Result<(), GraphError> {
        ctx.add_output_data("Pose", DataSpec::Pose);
        Ok(())
    }
}

fn read(path: &str) -> Value {
    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
}

fn local(value: RigTransform) -> Transform {
    Transform {
        translation: value.translation.as_vec3(),
        rotation: value.rotation.as_quat(),
        scale: value.scale.as_vec3(),
    }
}

fn binding_value(value: &Value) -> DataValue {
    let (kind, value) = value.as_object().unwrap().iter().next().unwrap();
    match kind.as_str() {
        "Bool" => value.as_bool().unwrap().into(),
        "F32" => (value.as_f64().unwrap() as f32).into(),
        "Vec3" => {
            Vec3::from_array(std::array::from_fn(|i| value[i].as_f64().unwrap() as f32)).into()
        }
        "Quat" => {
            Quat::from_array(std::array::from_fn(|i| value[i].as_f64().unwrap() as f32)).into()
        }
        _ => panic!("Unexpected binding type {kind}"),
    }
}

fn player(skeleton: Handle<Skeleton>, graph: Handle<AnimationGraph>) -> AnimationGraphPlayer {
    AnimationGraphPlayer::new(skeleton).with_graph(graph)
}

fn main() {
    let label = std::env::args().nth(1).unwrap();
    let root = Path::new("docs/audit/bf144/492");
    let inputs = read("docs/audit/bf144/459/inputs.json");
    let native = read("docs/audit/bf144/491/lifecycle-switch-restored.json");
    let frames = &inputs["frames"].as_array().unwrap()[..8];
    let definition: RigDefinition = serde_json::from_value(inputs["definition"].clone()).unwrap();
    let root_register =
        inputs["definition"]["program"]["instructions"][153]["Copy"]["output"]["register"]
            .as_u64()
            .unwrap() as usize;
    assert_eq!(root_register, 395);
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        AssetPlugin::default(),
        bevy::mesh::MeshPlugin,
        TransformPlugin,
        bevy::gizmos::GizmoPlugin,
        AnimationGraphPlugin::default(),
        AlsExtPlugin,
    ));
    app.finish();
    app.cleanup();

    let mut skeleton = Skeleton::default();
    let mut paths: Vec<Vec<String>> = Vec::new();
    for bone in &definition.bones {
        let mut path = bone.parent.map(|p| paths[p].clone()).unwrap_or_default();
        path.push(bone.name.clone());
        skeleton.add_bone(
            EntityPath::from(path.clone()),
            local(bone.initial_local),
            local(bone.initial_global),
        );
        paths.push(path);
    }
    let bone_ids: Vec<_> = paths
        .into_iter()
        .map(|path| EntityPath::from(path).id())
        .collect();
    skeleton.set_root(bone_ids[0]);
    let skeleton = app
        .world_mut()
        .resource_mut::<Assets<Skeleton>>()
        .add(skeleton);
    let curve_names: Vec<_> = frames[1]["curves"]
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect();
    let curve_keys: Vec<_> = curve_names
        .iter()
        .map(|name| format!("fixture.curve.{name}"))
        .collect();
    let input_bindings = frames[0]["bindings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|binding| RigInputBinding {
            external: binding["external"].as_str().unwrap().to_owned(),
            path: binding["path"]
                .as_array()
                .unwrap()
                .iter()
                .map(|i| i.as_u64().unwrap() as usize)
                .collect(),
            property: binding["property"].as_str().unwrap().to_owned(),
        })
        .collect();
    let source = AnimationNode::new(
        "source",
        RecordedInput {
            curves: curve_names.clone(),
        },
    );
    let rig = AnimationNode::new(
        "rig",
        ControlRigBoundaryNode {
            definition: Some(definition.clone()),
            input_bindings,
            internalize_time: true,
            capture_context: true,
            ..default()
        },
    );
    let mut graph = AnimationGraph::new();
    for node in [&source, &rig] {
        graph.add_node(node.clone());
    }
    graph.add_node_parameter_edge(source.id, "Pose", rig.id, "source_pose");
    graph.add_output_data("pose".into(), DataSpec::Pose);
    graph.add_output_data_edge(rig.id, "Pose", "pose");
    let graph = app
        .world_mut()
        .resource_mut::<Assets<AnimationGraph>>()
        .add(graph);

    let mut colliders = HashMap::default();
    for x in [-20.0, 0.0] {
        let entity = app.world_mut().spawn_empty().id();
        colliders.insert(
            entity,
            (
                Position(Vec3::new(x, -0.25, -20.0)),
                Rotation::default(),
                Collider::cuboid(20.0, 0.5, 20.0),
                CollisionLayers::ALL,
            ),
        );
    }
    let geometry = Arc::new(RigQueryWorld { colliders });
    let mut report = Vec::new();
    let mut latch_mismatches = 0;
    for (case_index, case) in [
        "actual",
        "initial-ik",
        "initial-virtual",
        "explicit-recreate",
    ]
    .into_iter()
    .enumerate()
    {
        let source_case = &native[usize::from(case_index >= 2)];
        let entity = app
            .world_mut()
            .spawn((
                player(skeleton.clone(), graph.clone()),
                Name::new("root"),
                Transform::IDENTITY,
            ))
            .id();
        let mut entities = vec![entity];
        for bone in definition.bones.iter().skip(1) {
            let parent = entities[bone.parent.unwrap()];
            entities.push(
                app.world_mut()
                    .spawn((
                        Name::new(bone.name.clone()),
                        local(bone.initial_local),
                        ChildOf(parent),
                    ))
                    .id(),
            );
        }
        let mut last_flag = None;
        let mut rows = Vec::new();
        for (index, frame) in frames.iter().enumerate() {
            let flag =
                case_index == 0 || source_case["frames"][index]["requested"].as_bool().unwrap();
            let changed = last_flag.is_some_and(|old| old != flag);
            let recreate = changed && case == "explicit-recreate";
            if recreate {
                app.world_mut()
                    .entity_mut(entity)
                    .insert(player(skeleton.clone(), graph.clone()));
            }
            last_flag = Some(flag);
            let mut pose = Pose {
                skeleton: skeleton.clone(),
                timestamp: frame["pose"]["timestamp"].as_f64().unwrap() as f32,
                ..default()
            };
            for (bone, &id) in definition.bones.iter().zip(&bone_ids) {
                let bone = frame["pose"]["bones"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|b| b["name"] == bone.name)
                    .unwrap();
                pose.add_bone(
                    BonePose::from_transform(local(serde_json::from_value(bone.clone()).unwrap())),
                    id,
                );
            }
            {
                let mut player = app
                    .world_mut()
                    .get_mut::<AnimationGraphPlayer>(entity)
                    .unwrap();
                player.set_global_input_data("fixture.pose", DataValue::Pose(pose));
                for (name, key) in curve_names.iter().zip(&curve_keys) {
                    player.set_global_input_data(
                        key,
                        frame["curves"][name]
                            .as_f64()
                            .map_or(DataValue::Bool(false), |v| DataValue::F32(v as f32)),
                    );
                }
                for binding in frame["bindings"].as_array().unwrap() {
                    let property = binding["property"].as_str().unwrap();
                    let value = if property == "control_rig_input.b_use_foot_ik_bones" {
                        flag.into()
                    } else if binding["resolved_player_property"].is_null() {
                        binding_value(&binding["global"])
                    } else {
                        binding_value(&binding["resolved_player_property"])
                    };
                    player.set_global_input_data(property, value);
                }
                player.set_global_input_data(
                    "animation.delta_seconds",
                    frame["delta_seconds"].as_f64().unwrap() as f32,
                );
                player.set_global_input_data(RIG_CAPTURE_KEY, true);
                player.set_global_input_data(
                    RIG_GAME_WORLD_KEY,
                    frame["game_world"].as_bool().unwrap(),
                );
            }
            app.world_mut()
                .get_mut::<SourceAnimationInputs>(entity)
                .unwrap()
                .set_rig_query_context(Some(RigQueryContext {
                    world: geometry.clone(),
                    to_world: serde_json::from_value(frame["to_world"].clone()).unwrap(),
                    channel: "ECC_Visibility".into(),
                    filter: default(),
                }));
            app.world_mut()
                .run_system_once(evaluate_players::<()>)
                .unwrap();
            let player = app.world().get::<AnimationGraphPlayer>(entity).unwrap();
            assert!(
                player.get_error().is_none(),
                "Graph error in {case} query {index}: {:?}",
                player.get_error()
            );
            let (_, _, state) =
                current_control_rig(player, app.world().resource::<Assets<AnimationGraph>>())
                    .unwrap();
            assert_eq!(state.initialization_count, 1);
            let context_frame = player.get_context_arena().unwrap().frame_index();
            let mut flag_ref = definition.externals["RigInput"].clone();
            flag_ref.path.push(1);
            assert_eq!(json!(state.value(&flag_ref).unwrap()), json!({"Bool":flag}));
            let root = state
                .value(&RigValueRef {
                    register: root_register,
                    path: vec![],
                })
                .unwrap();
            let foot_root = json!(root);
            let expected_root = if case_index == 0 || case == "explicit-recreate" {
                if flag { "ik_foot_root" } else { "VB foot_root" }
            } else {
                source_case["frames"][index]["foot_root"].as_str().unwrap()
            };
            latch_mismatches += usize::from(foot_root["List"][1]["Name"] != expected_root);
            let mut consumed = Vec::new();
            for (pc, names) in [
                (4, vec![foot_root["List"][1]["Name"].as_str().unwrap()]),
                (18, vec![foot_root["List"][1]["Name"].as_str().unwrap()]),
                (91, vec!["thigh_l", "calf_l", "foot_l"]),
                (121, vec!["thigh_r", "calf_r", "foot_r"]),
            ] {
                if let Some(call) = state.calls.iter().find(|call| call.instruction == pc) {
                    let arguments = json!(&call.arguments);
                    for (i, name) in names.iter().enumerate() {
                        assert_eq!(arguments[i]["List"][1]["Name"], *name);
                    }
                    consumed.push(json!({"instruction":pc,"bones":names}));
                } else {
                    assert!(index == 0 && [91, 121].contains(&pc));
                }
            }
            if case_index == 0 {
                assert_eq!(
                    state.executed_steps,
                    frame["expected_steps"].as_u64().unwrap() as usize
                );
                assert_eq!(
                    state.calls.len(),
                    frame["expected_calls"].as_array().unwrap().len()
                );
                for (call, expected) in state
                    .calls
                    .iter()
                    .zip(frame["expected_calls"].as_array().unwrap())
                {
                    assert_eq!(
                        call.instruction,
                        expected["instruction"].as_u64().unwrap() as usize
                    );
                    assert_eq!(
                        json!(call.bones_before),
                        expected["bones_before_call"],
                        "Default call bones query {index} pc {}",
                        call.instruction
                    );
                    assert_eq!(
                        json!(call.arguments),
                        expected["arguments_after_call"],
                        "Default call args query {index} pc {}",
                        call.instruction
                    );
                }
                for (register, expected) in frame["expected_registers"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .enumerate()
                {
                    assert_eq!(
                        json!(
                            state
                                .value(&RigValueRef {
                                    register,
                                    path: vec![]
                                })
                                .unwrap()
                        ),
                        *expected,
                        "Default query {index} register {register}"
                    );
                }
                let mut hierarchy = state.snapshot_hierarchy().unwrap();
                for (i, expected) in frame["expected_local"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .enumerate()
                {
                    assert_eq!(
                        json!(hierarchy.transform(i, RigSpace::Local, false).unwrap()),
                        expected["local"]
                    );
                }
                let traces=state.traces.iter().map(|sample|json!({"instruction":sample.instruction,"start":sample.start,"end":sample.end,"hit":sample.hit})).collect::<Vec<_>>();
                assert_eq!(json!(traces), frame["expected_traces"]);
            }
            rows.push(json!({"frame":index,"input_flag":flag,"foot_root":foot_root,"consumed":consumed,
                             "initialization_count":state.initialization_count,"steps":state.executed_steps,
                             "context_frame":context_frame,"recreated":recreate,
                             "calls":state.calls.len(),"traces":state.traces.len()}));
            println!("Replayed graph case {case} frame {index}");
        }
        app.world_mut().despawn(entity);
        report.push(json!({"case":case,"frames":rows}));
    }
    std::fs::write(
        root.join(format!("replay-{label}.json")),
        serde_json::to_vec(&json!({"cases":report,"native_latch_mismatches":latch_mismatches}))
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        latch_mismatches, 0,
        "Graph lifetime must preserve the native construction-selected foot root"
    );
}
