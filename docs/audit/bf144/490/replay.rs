use bevy::{
    animation::AnimationTargetId,
    ecs::{
        schedule::{ScheduleCleanupPolicy, Schedules},
        system::SystemState,
    },
    prelude::*,
    transform::helper::TransformHelper,
    world_serialization::{WorldAsset, WorldInstanceReady, WorldInstanceSpawner},
};
use bevy_als::{
    ab_als_provider::AbAlsProviderSet,
    als::{
        character::AlsCharacter,
        foot_ik::{
            AlsDynamicTransitionState, AlsFootBones, AlsFootLockSettings, AlsFootLockState,
            AlsFootTargetSelection, build_foot_bone_map,
        },
        state::{LocomotionState, RotationMode, Stance},
    },
    character_assembly::{AlsCharacterAssembly, AlsCharacterAvatar},
    control_rig_apply::{AlsControlRigInput, AlsControlRigSettings, ControlRigApplyPlugin},
    extensions::{
        SourceAnimationInputsPlugin,
        control_rig_hierarchy::{RigHierarchy, RigTransform},
        control_rig_math::RigValue,
        control_rig_program::RigDefinition,
        control_rig_units::RigUnitContext,
        source_instance::source_player_property,
    },
    foot_ik_apply::{FootIkApplyPlugin, FootIkApplySet},
    head_layer::AlsHeadLayerSet,
};
use bevy_animation_graph::core::{
    animated_scene::{AnimatedSceneHandle, AnimatedSceneInstance},
    animation_graph_player::AnimationGraphPlayer,
};
use serde_json::{Value, json};
use std::path::Path;

fn array<const N: usize>(value: &Value) -> [f32; N] {
    std::array::from_fn(|i| value[i].as_f64().unwrap() as f32)
}

fn local(value: &Value) -> Transform {
    Transform {
        translation: Vec3::from_array(array(&value["translation"])),
        rotation: Quat::from_array(array(&value["rotation"])),
        scale: Vec3::from_array(array(&value["scale"])),
    }
}

fn global(world: &mut World, entity: Entity) -> GlobalTransform {
    let mut state = SystemState::<TransformHelper>::new(world);
    state
        .get(world)
        .unwrap()
        .compute_global_transform(entity)
        .unwrap()
}

fn fixture(input: &Value, row: &Value) -> (App, Entity, Entity, Entity, Vec<Entity>) {
    let mut app = App::new();
    app.add_plugins(SourceAnimationInputsPlugin).add_plugins((
        MinimalPlugins,
        AssetPlugin::default(),
        TransformPlugin,
        bevy_animation_graph::AnimationGraphPlugin::default(),
    ));
    let chain = &row["observation"]["armature"]["chain"];
    let actor = app
        .world_mut()
        .spawn((
            AlsCharacter,
            LocomotionState::Grounded,
            RotationMode::ViewDirection,
            Stance::Standing,
            AlsDynamicTransitionState::default(),
            local(&chain[1]),
            AlsFootLockSettings {
                allow_foot_lock: false,
                ..default()
            },
        ))
        .id();
    let avatar = app
        .world_mut()
        .spawn((
            AlsCharacterAvatar { character: actor },
            AnimatedSceneHandle::default(),
            local(&chain[0]),
            ChildOf(actor),
        ))
        .id();
    app.world_mut()
        .entity_mut(actor)
        .insert(AlsCharacterAssembly {
            avatar,
            camera: None,
        });
    let player = app
        .world_mut()
        .spawn((
            AnimationGraphPlayer::default(),
            Transform::IDENTITY,
            ChildOf(avatar),
        ))
        .id();
    let mut mesh_parent = player;
    for node in row["scene"].as_array().unwrap().iter().rev() {
        mesh_parent = app
            .world_mut()
            .spawn((local(node), ChildOf(mesh_parent)))
            .id();
    }
    let mut entities = Vec::new();
    for (index, name) in input["names"].as_array().unwrap().iter().enumerate() {
        let name = Name::new(name.as_str().unwrap().to_owned());
        let id = AnimationTargetId::from_name(&name);
        let parent = input["parents"][index]
            .as_u64()
            .map_or(mesh_parent, |i| entities[i as usize]);
        entities.push(
            app.world_mut()
                .spawn((name, id, local(&row["locals"][index]), ChildOf(parent)))
                .id(),
        );
    }
    app.world_mut().trigger(WorldInstanceReady {
        entity: avatar,
        instance_id: WorldInstanceSpawner::default().spawn(Handle::<WorldAsset>::default()),
    });
    app.world_mut().flush();
    assert_eq!(
        app.world()
            .get::<AnimatedSceneInstance>(avatar)
            .unwrap()
            .player_entity(),
        player
    );
    let mut mapping =
        SystemState::<(Query<&Children>, Query<&Name>, Query<&AnimationTargetId>)>::new(
            app.world_mut(),
        );
    let (children, names, targets) = mapping.get(app.world()).unwrap();
    let bones = build_foot_bone_map(player, &children, &names, &targets).unwrap();
    app.world_mut().entity_mut(actor).insert(bones);
    app.world_mut()
        .resource_mut::<Schedules>()
        .remove(PostUpdate);
    app.add_plugins((ControlRigApplyPlugin, FootIkApplyPlugin))
        .configure_sets(PostUpdate, AlsHeadLayerSet.after(AbAlsProviderSet));
    app.world_mut()
        .schedule_scope(PostUpdate, |world, schedule| {
            assert_eq!(
                schedule
                    .remove_systems_in_set(
                        FootIkApplySet::Telemetry,
                        world,
                        ScheduleCleanupPolicy::RemoveSystemsOnly
                    )
                    .unwrap(),
                1
            );
        });
    (app, actor, avatar, player, entities)
}

fn vm_construction(data: &Value) -> Value {
    let definition: RigDefinition = serde_json::from_value(data.clone()).unwrap();
    let mut selection = definition.externals["RigInput"].clone();
    selection.path.push(1);
    let instruction = &data["program"]["instructions"][153]["Copy"]["output"];
    let mut cases = Vec::new();
    for use_ik in [true, false] {
        let mut hierarchy =
            RigHierarchy::new(definition.bones.clone(), definition.translation_tolerance).unwrap();
        let mut values = definition.program.initial_values.clone();
        selection
            .write(
                &mut values,
                definition.program.literal_count,
                RigValue::Bool(use_ik),
            )
            .unwrap();
        let mut context = RigUnitContext {
            hierarchy: &mut hierarchy,
            curves: &default(),
            game_world: true,
            construction: false,
            delta_seconds: 0.0,
            geometry: &definition.geometry,
            to_world: RigTransform::default(),
            line_trace: None,
        };
        let mut visited = Vec::new();
        definition
            .program
            .execute("Construction", &mut values, |pc, name, args| {
                visited.push(pc);
                context.call(name, args)
            })
            .unwrap();
        assert!(visited.contains(&152));
        let key = &values[instruction["register"].as_u64().unwrap() as usize];
        cases.push(json!({"use_ik":use_ik,"foot_root":key,"construction_calls":visited}));
    }
    assert_eq!(
        cases[0]["foot_root"],
        json!({"List":[{"Name":"Bone"},{"Name":"ik_foot_root"}]})
    );
    assert_eq!(
        cases[1]["foot_root"],
        json!({"List":[{"Name":"Bone"},{"Name":"VB foot_root"}]})
    );
    json!(cases)
}

fn main() {
    let output = Path::new("docs/audit/bf144/490");
    let label = std::env::args().nth(1).unwrap();
    let input: Value =
        serde_json::from_slice(&std::fs::read("tools/tmp/bf144/490/input.json").unwrap()).unwrap();
    let vm: Value =
        serde_json::from_slice(&std::fs::read("docs/audit/bf144/459/inputs.json").unwrap())
            .unwrap();
    let mut records = Vec::new();
    let mut mismatches = 0;
    for row in input["rows"].as_array().unwrap() {
        let (mut app, actor, avatar, player, entities) = fixture(&input, row);
        let entity = |name: &str| {
            entities[input["names"]
                .as_array()
                .unwrap()
                .iter()
                .position(|v| v == name)
                .unwrap()]
        };
        for (step, requested) in [None, Some(false), Some(true), None, Some(false)]
            .into_iter()
            .enumerate()
        {
            if let Some(use_ik_bones) = requested {
                app.world_mut()
                    .entity_mut(actor)
                    .insert(AlsControlRigSettings {
                        use_hand_ik_bones: true,
                        use_foot_ik_bones: use_ik_bones,
                    });
            } else {
                app.world_mut()
                    .entity_mut(actor)
                    .remove::<AlsControlRigSettings>();
            }
            if step == 4 {
                app.world_mut()
                    .entity_mut(entity("VB foot_l"))
                    .insert(Name::new("Missing virtual marker"));
            }
            app.world_mut().run_schedule(PostUpdate);
            let use_ik = app
                .world()
                .get::<AlsFootTargetSelection>(actor)
                .unwrap()
                .use_ik_bones;
            let rig = app.world().get::<AlsControlRigInput>(actor).unwrap();
            let graph = app.world().get::<AnimationGraphPlayer>(player).unwrap();
            let field = |name: &str| source_player_property(graph, name).unwrap();
            let published = field("control_rig_input.b_use_foot_ik_bones")
                .as_bool()
                .unwrap();
            let coherent = use_ik == requested.unwrap_or(true)
                && rig.use_foot_ik_bones == use_ik
                && published == use_ik;
            mismatches += usize::from(!coherent);
            assert_eq!(
                field("control_rig_input.b_foot_transforms_valid")
                    .as_bool()
                    .unwrap(),
                true
            );
            for (name, location) in [
                ("left", rig.foot_left_location_armature),
                ("right", rig.foot_right_location_armature),
            ] {
                assert_eq!(
                    field(&format!("control_rig_input.foot_{name}_location"))
                        .as_vec3()
                        .unwrap(),
                    location
                );
            }
            for (name, rotation) in [
                ("left", rig.foot_left_rotation_armature),
                ("right", rig.foot_right_rotation_armature),
            ] {
                assert_eq!(
                    field(&format!("control_rig_input.foot_{name}_rotation"))
                        .as_quat()
                        .unwrap(),
                    rotation
                );
            }
            let states = *app.world().get::<AlsFootLockState>(actor).unwrap();
            let bones = *app.world().get::<AlsFootBones>(actor).unwrap();
            assert!(states.valid && rig.foot_transforms_valid);
            let selected_names = if use_ik {
                ["ik_foot_l", "ik_foot_r"]
            } else if step == 4 {
                ["foot_l", "VB foot_r"]
            } else {
                ["VB foot_l", "VB foot_r"]
            };
            let mesh = global(app.world_mut(), avatar);
            let mut side_rows = Vec::new();
            for (side, state) in [states.left, states.right].into_iter().enumerate() {
                let selected = bones.target(side, use_ik);
                assert_eq!(selected, entity(selected_names[side]));
                let world = global(app.world_mut(), selected);
                assert_eq!(state.target_location, world.translation());
                assert_eq!(state.target_rotation, world.rotation());
                assert_eq!(state.final_location, state.target_location);
                assert_eq!(state.final_rotation, state.target_rotation);
                if use_ik {
                    assert_eq!(
                        json!(state.target_location.to_array()),
                        row["observation"]["targets"][side]["translation"]
                    );
                    assert_eq!(
                        json!(state.target_rotation.to_array()),
                        row["observation"]["targets"][side]["rotation"]
                    );
                }
                if step == 4 && side == 0 {
                    assert_ne!(state.target_location, mesh.translation());
                }
                side_rows.push(json!({"name":selected_names[side],"t":state.target_location,"q":state.target_rotation}));
            }
            records.push(
                json!({"query":row["query"],"step":step,"requested":requested,"selection":use_ik,
                                "published":published,"coherent":coherent,"sides":side_rows,
                                "source_missing_world_t":(step==4).then(||mesh.translation()),
                                "source_missing_world_q":(step==4).then(||mesh.rotation())}),
            );
        }
        println!(
            "Qualified registered selector and publication at query {}",
            row["query"]
        );
    }
    let construction = vm_construction(&vm["definition"]);
    std::fs::write(output.join(format!("replay-{label}.json")),serde_json::to_vec(
        &json!({"records":records,"selection_mismatches":mismatches,"construction":construction})).unwrap()).unwrap();
    assert_eq!(
        mismatches, 0,
        "Source selector and published flag must agree in the same frame"
    );
}
