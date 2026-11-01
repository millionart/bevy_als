#[derive(Resource)]
struct C450BProbe {
    actor: Option<Entity>,
    frame: usize,
    rows: Vec<serde_json::Value>,
    progress: Option<(bool, usize, usize)>,
    camera_rows: Vec<serde_json::Value>,
    motor_rows: Vec<serde_json::Value>,
    animation_before: Vec<serde_json::Value>,
    animation_after: Vec<serde_json::Value>,
}
impl Default for C450BProbe {
    fn default() -> Self { Self { actor: None, frame: 0, rows: Vec::new(), progress: None, camera_rows:Vec::new(),motor_rows:Vec::new(),animation_before:Vec::new(),animation_after:Vec::new() } }
}
fn c450b_progress(value: serde_json::Value) {
    let output = std::env::var("ALS_C450B_OUTPUT").expect("parameter evidence output is required");
    std::fs::write(std::path::Path::new(&output).with_extension("progress.json"), value.to_string()).unwrap();
}
fn c450b_drive(
    mut commands: Commands,
    mut probe: ResMut<C450BProbe>,
    ready: Res<ManualPlaygroundStartupReady>,
    admissions: Query<&AlsPlayerAdmissionComplete>,
    graphs: Res<Assets<bevy_animation_graph::core::animation_graph::AnimationGraph>>,
    mut actors: Query<(&mut bevy_als::als::gait::AlsDesiredGait, &mut bevy_als::als::transition::AlsDesiredStance, &mut bevy_als::als::motion::AlsViewYaw, &mut bevy_als::als::state::RotationMode)>,
) {
    let progress = (ready.passed, admissions.iter().count(), graphs.len());
    if probe.progress != Some(progress) { info!("[C450B] startup_progress={progress:?}"); probe.progress = Some(progress); c450b_progress(serde_json::json!({"startup":progress,"captured":probe.frame})); }
    if !ready.passed || probe.frame >= 240 { return; }
    let Some(admission) = admissions.iter().next() else { return; };
    let Ok((mut gait, mut stance, mut view, mut mode)) = actors.get_mut(admission.character) else { return; };
    probe.actor = Some(admission.character);
    gait.0 = bevy_als::als::state::Gait::Running;
    stance.0 = bevy_als::als::state::Stance::Standing;
    view.0 = -std::f32::consts::FRAC_PI_2;
    *mode = bevy_als::als::state::RotationMode::ViewDirection;
    let moving = (20..60).contains(&probe.frame) || (120..160).contains(&probe.frame);
    commands.entity(admission.character).insert(bevy_als::als::motion::AlsInputOverride(bevy_als::als::input::AlsInputCommand { move_dir: if moving { Vec2::Y } else { Vec2::ZERO }, ..default() }));
}
fn c450b_sample(
    mut probe: ResMut<C450BProbe>,
    time: Res<Time>,
    assets: Res<Assets<AlsMovementParams>>,
    actors: Query<(
        &bevy_als::als::state::LocomotionState,
        &bevy_als::als::state::Stance,
        &bevy_als::als::state::RotationMode,
        &bevy_als::als::motion::AlsViewYaw,
        &bevy_als::als::gait::AlsGaitAmount,
        &bevy_als::als::dynamics::AlsGroundedDynamics,
        &bevy_als::als::dynamics::AlsMovementBaseDynamicsState,
        &bevy_als::als::character::AlsCmcAdapterRuntime,
        &bevy_als::als::character::AlsMovementParamsHandle,
        &bevy_tnua::TnuaController<bevy_als::als::control_scheme::AlsControlScheme>,
        &Position,
        &LinearVelocity,
        &bevy_als::als::dynamics::AlsLandingBrakingFriction,
        (&bevy_als::als::gait::AlsMaxAllowedGait, Option<&bevy_als::als::view::AlsPlayerViewYaw>, Option<&bevy_als::als::motion::AlsInputOverride>, &bevy_als::als::input::AlsInputCommand, Has<bevy_als::als::motion::AlsMovementInputBlocked>),
    )>,
    views: Query<(Entity,&Camera,Ref<GlobalTransform>,&bevy_als::als_camera::AlsCharacterCamera),(With<Camera3d>,With<AlsCameraViewSource>)>,
) {
    let Some(actor) = probe.actor else { return; };
    if probe.frame >= 240 { return; }
    let Ok((locomotion, stance, mode, view, gait, dynamics, base, cmc, handle, controller, position, physical, braking, (allowed, player_view, override_command, command, input_blocked))) = actors.get(actor) else { return; };
    let Some(params) = assets.get(handle.id()) else { return; };
    let thresholds = params.gait_speed_thresholds(*stance, *mode, base.relative_velocity, view.0);
    let expected = params.grounded_dynamics(*stance, *mode, gait.0);
    let maximum = bevy_als::als::dynamics::max_allowed_speed(params, *stance, *mode, allowed.0, base.relative_velocity, view.0);
    let public_gait = bevy_als::als::gait::calculate_gait_amount_for_thresholds(base.relative_velocity.length(), thresholds);
    let horizontal_gait = bevy_als::als::gait::calculate_gait_amount_for_thresholds(base.relative_velocity.with_y(0.0).length(), thresholds);
    let effective = bevy_als::als::motion::movement_input_for_action(*override_command.map_or(command, |v|&v.0), input_blocked);
    let used_yaw = player_view.map_or(view.0, |v|v.0);
    let expected_direction = bevy_als::als::motion::view_relative_move_direction(effective.move_dir, used_yaw)*effective.move_dir.length().min(1.0);
    let raw_direction = bevy_als::als::motion::view_relative_move_direction(effective.move_dir, view.0)*effective.move_dir.length().min(1.0);
    let camera = views.iter().find(|(_,cam,t,owner)|owner.character==actor && cam.is_active && !t.is_added()).map(|(id,_,t,_)|serde_json::json!({"entity":format!("{id:?}"),"yaw":t.rotation().to_euler(EulerRot::YXZ).0,"rotation":t.rotation().to_array()}));
    let row = serde_json::json!({
        "ordinal": probe.frame, "player_view_yaw":player_view.map(|v|v.0), "consumed_yaw":used_yaw, "input_move":effective.move_dir.to_array(), "input_blocked":input_blocked, "camera_at_update":camera, "expected_direction":expected_direction.to_array(), "raw_control_direction":raw_direction.to_array(), "delta": time.delta_secs(), "simulation_delta": cmc.simulation_delta_seconds,
        "simulation_steps": cmc.simulation_substeps, "locomotion": format!("{locomotion:?}"), "stance": format!("{stance:?}"), "rotation_mode": format!("{mode:?}"),
        "view_yaw": view.0, "position": position.0.to_array(), "physical_velocity": physical.0.to_array(), "world_velocity_at_dynamics": base.world_velocity.to_array(),
        "support_velocity": base.support_velocity.to_array(), "has_base": base.entity.is_some(), "relative_velocity_consumed": base.relative_velocity.to_array(),
        "desired_motion": controller.basis.desired_motion.to_array(), "gait_amount": gait.0, "public_gait":public_gait, "horizontal_gait":horizontal_gait, "allowed_gait":format!("{:?}",allowed.0), "thresholds": [thresholds.walk,thresholds.run,thresholds.sprint],
        "dynamics": [dynamics.0.acceleration,dynamics.0.braking_deceleration,dynamics.0.ground_friction],
        "public_sample": [expected.acceleration,expected.braking_deceleration,expected.ground_friction], "max_speed": maximum,
        "planned_velocity": base.pre_collision_velocity.map(|v|v.to_array()), "planned_delta": cmc.grounded_movement_delta.map(|v|v.to_array()),"grounded_move_start":cmc.grounded_move_start.map(|v|v.to_array()),
        "braking_friction_factor": braking.factor, "has_animation_root_velocity": cmc.has_animation_root_velocity,
    });
    probe.rows.push(row);
    probe.frame += 1;
    if probe.frame % 30 == 0 { info!("[C450B] captured_parameters={}",probe.frame); c450b_progress(serde_json::json!({"captured":probe.frame})); }
}
fn c450b_late(
    mut probe: ResMut<C450BProbe>,
    bodies: Query<(&Position,&LinearVelocity,&avian3d::prelude::ComputedCenterOfMass),With<bevy_als::als::character::AlsCharacter>>,
    frame_time: Res<Time<Virtual>>,
    physics: Res<Time<avian3d::prelude::Physics>>,
    substep_time: Res<Time<avian3d::prelude::Substeps>>,
    substeps: Res<avian3d::prelude::SubstepCount>,
    gravity: Res<avian3d::prelude::Gravity>,
    views: Query<(&Camera,&GlobalTransform,&bevy_als::als_camera::AlsCharacterCamera),(With<Camera3d>,With<AlsCameraViewSource>)>,
    mut exit: MessageWriter<AppExit>,
) {
    let Some(actor)=probe.actor else{return;};
    if probe.frame==0{return;}
    let camera=views.iter().find(|(cam,_,owner)|owner.character==actor && cam.is_active).map(|(_,t,_)|serde_json::json!({"yaw":t.rotation().to_euler(EulerRot::YXZ).0,"rotation":t.rotation().to_array()}));
    let ordinal=probe.frame-1;
    let Ok((position,velocity,com))=bodies.get(actor) else{panic!("Observed body is missing");};
    probe.camera_rows.push(serde_json::json!({"ordinal":ordinal,"camera":camera,"position":position.0.to_array(),"velocity":velocity.0.to_array(),"physics_delta":physics.delta_secs(),"physics_delta_ns":physics.delta().as_nanos() as u64,"frame_delta_ns":frame_time.delta().as_nanos() as u64,"substep_delta_ns":substep_time.delta().as_nanos() as u64,"center_of_mass":com.0.to_array(),"substep_delta":substep_time.delta_secs(),"substeps":substeps.0,"gravity":gravity.0.to_array()}));
    if probe.frame==240 {
        let output=std::env::var("ALS_C450B_OUTPUT").expect("input direction evidence output is required");
        std::fs::write(output,serde_json::to_vec_pretty(&serde_json::json!({"gate":"CAPTURED_REAL_INPUT_CAMERA_AND_DESIRED_DIRECTION240","rows":probe.rows,"after_motors":probe.motor_rows,"animation_before":probe.animation_before,"animation_after":probe.animation_after,"camera_after_post_update":probe.camera_rows,"screenshots":0,"performance_acceptance":"paused"})).unwrap()).unwrap();
        exit.write(AppExit::Success);
    }
}

fn c450b_motor(
    mut probe: ResMut<C450BProbe>,
    bodies: Query<(&Position,&LinearVelocity),With<bevy_als::als::character::AlsCharacter>>,
) {
    let Some(actor)=probe.actor else{return;};
    if probe.frame==0{return;}
    let Ok((position,velocity))=bodies.get(actor) else{panic!("Observed motor body is missing");};
    let ordinal=probe.frame-1;
    probe.motor_rows.push(serde_json::json!({"ordinal":ordinal,"position":position.0.to_array(),"velocity":velocity.0.to_array()}));
}
fn c450b_animation(
    In(before): In<bool>,
    app_frame:Res<bevy::diagnostic::FrameCount>,
    root_contexts:Query<&bevy_als::animation_context_projection::AlsRootAnimationContext>,
    transforms:bevy::transform::helper::TransformHelper,
    history:Query<(&Transform,Has<ChildOf>,&bevy_als::als::state::LocomotionState,&bevy_als::als::state::Gait,&bevy_als::als::state::Stance,&bevy_als::als::state::RotationMode,
        &bevy_als::als::motion::AlsViewYaw,&bevy_als::als::gait::AlsDesiredGait,&bevy_als::als::transition::AlsDesiredStance,
        (&bevy_als::als::input::AlsInputCommand,Option<&bevy_als::als::motion::AlsInputOverride>,Option<&bevy_als::als::view::AlsPlayerViewYaw>,Has<bevy_als::als::motion::AlsMovementInputBlocked>,&bevy_tnua::TnuaController<bevy_als::als::control_scheme::AlsControlScheme>))>,
    sources:Query<&bevy_als::extensions::SourceAnimationInputs>,
    ready:Res<ManualPlaygroundStartupReady>,
    factor:Res<C450BDataFactor>,
    skeletons:Res<Assets<bevy_animation_graph::core::skeleton::Skeleton>>,
    graphs:Res<Assets<bevy_animation_graph::core::animation_graph::AnimationGraph>>,
    assets:Res<AssetServer>,
    mut probe: ResMut<C450BProbe>,
    time: Res<Time>,
    virtual_time: Res<Time<Virtual>>,
    avatars: Query<(Entity,&ChildOf,&bevy_animation_graph::core::animated_scene::AnimatedSceneInstance),With<bevy_als::character_assembly::AlsCharacterAvatar>>,
    characters: Query<(&bevy_als::actual_motion::AlsActualMotion,&Position,&LinearVelocity)>,
    players: Query<(&bevy_animation_graph::core::animation_graph_player::AnimationGraphPlayer,Option<&bevy_als::extensions::post_physics_animation::AnimationPoseUpdateGate>)>,
) {
    let Some((avatar,parent,instance))=avatars.iter().next() else{return;};
    let Ok((motion,position,velocity))=characters.get(parent.parent()) else{return;};
    let Ok((player,gate))=players.get(instance.player_entity()) else{return;};
    let scalar=|key|player.get_global_input_data(key).and_then(|v|v.as_f32().ok());
    let vector=|key|player.get_global_input_data(key).and_then(|v|v.as_vec3().ok()).map(|v|v.to_array());
    let boolean=|key|player.get_global_input_data(key).and_then(|v|v.as_bool().ok());
    let actor=parent.parent();
    let (local,parented,locomotion,gait,stance,rotation,view,desired_gait,desired_stance,(command,input_override,player_view,blocked,controller))=history.get(actor).expect("Initial state producer components are required");
    let actor_world=transforms.compute_global_transform(actor).unwrap().to_scale_rotation_translation();
    let avatar_world=transforms.compute_global_transform(avatar).unwrap().to_scale_rotation_translation();
    let source=sources.get(instance.player_entity()).unwrap();
    let tags=["locomotion_mode","gait","stance","rotation_mode","locomotion_action"].into_iter().map(|key|(key.to_owned(),serde_json::json!(source.gameplay_tag_input(key).map(|v|v.as_str().to_owned())))).collect::<serde_json::Map<String,serde_json::Value>>();
    let effective=bevy_als::als::motion::movement_input_for_action(*input_override.map_or(command,|v|&v.0),blocked);
    let history_input=serde_json::json!({"startup_ready":ready.passed,"root_tags":tags,"locomotion":locomotion,"gait":gait,"stance":stance,"rotation_mode":rotation,
        "desired_gait":desired_gait.0,"desired_stance":desired_stance.0,"raw_view_yaw":view.0,"player_view_yaw":player_view.map(|v|v.0),
        "input_blocked":blocked,"input_override_present":input_override.is_some(),"command_move":command.move_dir.to_array(),"effective_move":effective.move_dir.to_array(),"desired_world_motion":controller.basis.desired_motion.to_array(),
        "actor_parented":parented,"local_translation":local.translation.to_array(),"local_rotation":local.rotation.to_array(),"local_scale":local.scale.to_array(),
        "actor_world_translation":actor_world.2.to_array(),"actor_world_rotation":actor_world.1.to_array(),"actor_world_scale":actor_world.0.to_array(),
        "avatar_world_translation":avatar_world.2.to_array(),"avatar_world_rotation":avatar_world.1.to_array(),"avatar_world_scale":avatar_world.0.to_array()});
    let root_context=root_contexts.get(instance.player_entity()).ok();
    let curve_frame_input=serde_json::json!({"app_frame":app_frame.0,"virtual_elapsed_ns":virtual_time.elapsed().as_nanos() as u64,
        "projected_context_present":root_context.is_some(),"projected_curve_count":root_context.map(|v|v.curves.len()),
        "projected_standing":root_context.and_then(|v|v.f32("PoseStanding")),"projected_grounded":root_context.and_then(|v|v.f32("PoseGrounded")),
        "global_standing":scalar("pose_state.standing_amount"),"global_grounded":scalar("pose_state.grounded_amount"),
        "source_property_standing":bevy_als::extensions::source_instance::source_player_property(player,"pose_state.standing_amount").and_then(|v|v.as_f32().ok()),
        "source_property_grounded":bevy_als::extensions::source_instance::source_player_property(player,"pose_state.grounded_amount").and_then(|v|v.as_f32().ok())});
    let row=serde_json::json!({
        "ordinal":probe.actor.map(|_|probe.frame.saturating_sub(1)),
        "curve_frame_input":curve_frame_input,
        "head_values":c450b_head_values(player,&graphs),
        "forward_difference":if before{Vec::new()}else{c450b_difference(player,&graphs,&skeletons,&assets)},
        "data_factor":serde_json::json!({"applications":factor.applications,"info":factor.info}),
        "pelvis_markers":if before{Vec::new()}else{c450b_markers(player,&graphs,&skeletons)},
        "history_input":history_input,
        "leaf_nodes":if before{Vec::new()}else{c450b_leaf_rows(player,&graphs,&assets)},
        "frame_delta":time.delta_secs(),"virtual_delta_ns":virtual_time.delta().as_nanos() as u64,
        "position":position.0.to_array(),"velocity":velocity.0.to_array(),
        "motion_world_velocity":motion.world_velocity.to_array(),"motion_acceleration":motion.world_acceleration.to_array(),
        "motion_speed":motion.horizontal_speed,"motion_has_input":motion.has_input,
        "motion_local_velocity":motion.local_velocity.to_array(),
        "provider_velocity":vector("locomotion_state.velocity_world_space"),
        "provider_acceleration":vector("locomotion_state.acceleration_world_space"),
        "provider_speed":scalar("locomotion_state.speed"),"provider_has_input":boolean("locomotion_state.b_has_input"),
        "provider_max_acceleration":scalar("locomotion_state.max_acceleration"),
        "provider_max_braking":scalar("locomotion_state.max_braking_deceleration"),
        "animation_delta":scalar("animation.delta_seconds"),
        "animation_pending":boolean("animation.pending_update"),
        "arena_frame":player.get_context_arena().map(|a|a.frame_index()),
        "root_time_update":player.get_context_arena().map(|a|format!("{:?}",a.get_toplevel().query_output_time)),
        "elapsed":player.elapsed(),"pose_timestamp":player.get_default_output_pose().map(|p|p.timestamp),
        "paused":player.is_paused(),"graph_error":player.get_error().map(|e|format!("{e:?}")),
        "gate":gate.map(|g|serde_json::json!({"update_graph":g.update_graph,"refresh_bones":g.refresh_bones,"pending_update":g.pending_update}))
    });
    if before {probe.animation_before.push(row);}else{probe.animation_after.push(row);}
}
fn c450b_updated_keys(caches:&bevy_animation_graph::core::context::node_caches::NodeCaches)->Vec<(bevy_animation_graph::core::animation_graph::NodeId,bevy_animation_graph::core::context::node_states::StateKey)>{
    use bevy::reflect::{PartialReflect,ReflectRef};
    use bevy_animation_graph::core::animation_graph::NodeId;
    use bevy_animation_graph::core::context::node_caches::NodeCache;
    let ReflectRef::Struct(fields)=caches.reflect_ref()else{panic!("Node caches are not a reflected struct");};
    let ReflectRef::Map(map)=fields.field("caches").unwrap().reflect_ref()else{panic!("Node cache entries are not a reflected map");};
    let mut output=Vec::new();
    for (node,value) in map.iter(){
        let node=*node.try_downcast_ref::<NodeId>().unwrap();
        let cache=value.try_downcast_ref::<std::sync::Arc<NodeCache>>().unwrap();
        output.extend(cache.updated.iter().map(|key|(node,*key)));
    }
    output.sort_by_key(|(node,key)|format!("{node:?}:{key:?}"));
    output
}
fn c450b_leaf_rows(
    player:&bevy_animation_graph::core::animation_graph_player::AnimationGraphPlayer,
    graphs:&Assets<bevy_animation_graph::core::animation_graph::AnimationGraph>,
    assets:&AssetServer,
)->Vec<serde_json::Value>{
    use bevy_animation_graph::core::context::node_states::StateKey;
    let Some(arena)=player.get_context_arena()else{return Vec::new();};
    let mut rows=Vec::new();
    for context_id in arena.iter_context_ids(){
        let context=arena.get_context(context_id).unwrap();
        let Some(graph)=graphs.get(context.get_graph_id())else{continue;};
        let updated=c450b_updated_keys(&context.node_caches);
        for node in graph.nodes.values(){
            let sequence=node.try_inner_downcast_ref::<bevy_als::extensions::SourceSequencePlayerNode>();
            let evaluator=node.try_inner_downcast_ref::<bevy_als::extensions::SourceSequenceEvaluatorNode>();
            let blend=node.try_inner_downcast_ref::<bevy_als::extensions::NormalizedBlendSpaceNode>();
            let builtin=node.try_inner_downcast_ref::<bevy_animation_graph::builtin_nodes::clip_node::ClipNode>();
            let kind=if sequence.is_some(){"sequence_player"}else if evaluator.is_some(){"sequence_evaluator"}else if blend.is_some(){"blendspace"}else if builtin.is_some(){"clip"}else{continue;};
            let builtin_clip=builtin.and_then(|v|{
                use bevy::reflect::PartialReflect;
                let bevy::reflect::ReflectRef::Struct(fields)=v.reflect_ref()else{return None;};
                fields.field("clip").and_then(|v|v.try_downcast_ref::<Handle<bevy_animation_graph::core::animation_clip::GraphClip>>()).cloned()
            });
            let clip=sequence.map(|v|&v.clip).or_else(||evaluator.map(|v|&v.clip)).or(builtin_clip.as_ref());
            let mut clocks=Vec::new();
            for (_,key) in updated.iter().filter(|(id,_)|*id==node.id){
                let state=bevy_als::extensions::animation_context::node_context_state(&context.node_states,*key,node.id);
                let relevant=state.and_then(|v|v.relevant_asset.as_ref()).map(|v|serde_json::json!({"position_seconds":v.position,"previous_seconds":v.previous_position,"duration_seconds":v.duration,"delta_seconds":v.delta_seconds,"playback_delta_seconds":v.playback_delta_seconds,"looping":v.looping}));
                let sync=state.map(|v|v.sync_players.iter().map(|t|serde_json::json!({"phase":t.phase,"previous_phase":t.previous_phase,"phase_duration":t.phase_duration,"duration_seconds":t.duration,"group":t.key,"weight":t.weight,"delta_seconds":t.delta_seconds,"play_rate":t.play_rate})).collect::<Vec<_>>()).unwrap_or_default();
                let samples=state.map(|v|v.asset_samples.iter().map(|s|serde_json::json!({"clip":assets.get_path(s.clip.id()).map(|p|p.to_string()),"position_seconds":s.clock.position,"duration_seconds":s.clock.duration,"weight":s.clock.weight})).collect::<Vec<_>>()).unwrap_or_default();
                let pose=context.node_caches.get_output_data(node.id,*key,if blend.is_some()||builtin.is_some(){"pose"}else{"Pose"}.into()).and_then(|v|v.as_pose().ok().map(|p|p.timestamp));
                let point_times=blend.map(|v|v.points.iter().map(|p|serde_json::json!({"id":p.id,"requested_time":context.node_caches.get_input_time_update(node.id,*key,bevy_als::extensions::NormalizedBlendSpaceNode::time_pin(&p.id)).ok().map(|t|format!("{t:?}"))})).collect::<Vec<_>>());
                clocks.push(serde_json::json!({"key":format!("{key:?}"),"temporary":matches!(key,StateKey::Temporary(_)),"node_time":context.node_states.get_time(node.id,*key),"sample_time":context.node_caches.get_output_data(node.id,*key,"sample_time".into()).and_then(|v|v.as_f32().ok()),"pose_timestamp":pose,"relevant":relevant,"sync":sync,"samples":samples,"point_times":point_times}));
            }
            rows.push(serde_json::json!({"context":format!("{context_id:?}"),"graph":assets.get_path(context.get_graph_id()).map(|p|p.to_string()),"node_guid":node.id,"name":node.name,"kind":kind,"clip":clip.and_then(|v|assets.get_path(v.id())).map(|p|p.to_string()),"teleport":blend.map(|v|v.teleport_to_normalized_time),"points":blend.map(|v|v.points.iter().map(|p|serde_json::json!({"id":p.id,"duration":p.duration})).collect::<Vec<_>>()),"clocks":clocks}));
        }
    }
    rows.sort_by_key(|v|(v["graph"].to_string(),v["node_guid"].to_string(),v["context"].to_string()));
    rows
}
fn c450b_head_values(
    player:&bevy_animation_graph::core::animation_graph_player::AnimationGraphPlayer,
    graphs:&Assets<bevy_animation_graph::core::animation_graph::AnimationGraph>,
)->serde_json::Value{
    let mut floats=serde_json::Map::new();
    for key in ["animation.delta_seconds","animation.delta_seconds_without_time_dilation","locomotion_state.input_yaw_angle_world_space","locomotion_state.target_yaw_angle_world_space","locomotion_state.rotation_yaw","locomotion_state.yaw_velocity_world_space","view_state.yaw_angle","view_state.pitch_angle","movement_base.delta_yaw","head_state.pitch_angle","head_state.yaw_angle","head_state.yaw_velocity","head_state.yaw_amount"]{
        let global=player.get_global_input_data(key).and_then(|v|v.as_f32().ok());
        let property=bevy_als::extensions::source_instance::source_player_property(player,key).and_then(|v|v.as_f32().ok());
        let frame_input=player.get_global_input_data(format!("source_frame_input.{key}")).and_then(|v|v.as_f32().ok());
        floats.insert(key.to_owned(),serde_json::json!({"global":global,"property":property,"frame_input":frame_input,"resolved":property.or(global)}));
    }
    let mut booleans=serde_json::Map::new();
    for key in ["view_mode.first_person","rotation_mode.velocity_direction","locomotion_state.b_has_input","movement_base.b_has_relative_rotation","head_state.b_initialization_required","head_state.b_switching_look_sides"]{
        let global=player.get_global_input_data(key).and_then(|v|v.as_bool().ok());
        let property=bevy_als::extensions::source_instance::source_player_property(player,key).and_then(|v|v.as_bool().ok());
        let frame_input=player.get_global_input_data(format!("source_frame_input.{key}")).and_then(|v|v.as_bool().ok());
        booleans.insert(key.to_owned(),serde_json::json!({"global":global,"property":property,"frame_input":frame_input,"resolved":property.or(global)}));
    }
    let mut pins=Vec::new();
    if let Some(arena)=player.get_context_arena(){
        for context_id in arena.iter_context_ids(){
            let context=arena.get_context(context_id).unwrap();
            let Some(graph)=graphs.get(context.get_graph_id())else{continue;};
            for node in graph.nodes.values(){
                let Some(input)=node.try_inner_downcast_ref::<bevy_als::extensions::GlobalF32InputNode>()else{continue;};
                if input.key!="head_state.yaw_amount"&&input.key!="head_state.pitch_angle"{continue;}
                let key=bevy_animation_graph::core::context::node_states::StateKey::Default;
                if !context.node_caches.is_updated(node.id,key){continue;}
                let value=context.node_caches.get_output_data(node.id,key,input.key.clone()).and_then(|v|v.as_f32().ok());
                pins.push(serde_json::json!({"key":input.key,"node_guid":node.id,"value":value}));
            }
        }
    }
    pins.sort_by_key(|v|(v["key"].to_string(),v["node_guid"].to_string()));
    serde_json::json!({"floats":floats,"booleans":booleans,"actual_look_pins":pins})
}
fn c450b_pose_value(
    pose:&bevy_animation_graph::core::pose::Pose,
    skeletons:&Assets<bevy_animation_graph::core::skeleton::Skeleton>,
    assets:&AssetServer,
)->serde_json::Value{
    let skeleton=skeletons.get(&pose.skeleton);
    let mut bones=pose.paths.iter().map(|(id,index)|{
        let path=skeleton.and_then(|s|s.id_to_path(*id)).map(|p|p.parts.iter().map(|v|v.as_str().to_owned()).collect::<Vec<_>>());
        let bone=&pose.bones[*index];
        serde_json::json!({"name":path.as_ref().and_then(|p|p.last()),"path":path,"translation":bone.translation.map(|v|v.to_array()),"rotation":bone.rotation.map(|v|v.to_array()),"scale":bone.scale.map(|v|v.to_array()),"weights":bone.weights})
    }).collect::<Vec<_>>();
    bones.sort_by_key(|v|v["name"].to_string());
    serde_json::json!({"timestamp":pose.timestamp,"skeleton_asset":assets.get_path(pose.skeleton.id()).map(|p|p.to_string()),"bones":bones,"root_motion":pose.root_motion})
}
fn c450b_difference(
    player:&bevy_animation_graph::core::animation_graph_player::AnimationGraphPlayer,
    graphs:&Assets<bevy_animation_graph::core::animation_graph::AnimationGraph>,
    skeletons:&Assets<bevy_animation_graph::core::skeleton::Skeleton>,
    assets:&AssetServer,
)->Vec<serde_json::Value>{
    use bevy_animation_graph::core::animation_graph::{SourcePin,TargetPin};
    let mut rows=Vec::new();
    if let Some(arena)=player.get_context_arena(){for context_id in arena.iter_context_ids(){
        let context=arena.get_context(context_id).unwrap();let Some(graph)=graphs.get(context.get_graph_id())else{continue;};
        for node in graph.nodes.values(){
            if node.id.uuid().to_string()!="73bc3f9e-083f-5fb4-a4a1-590ad157f515"{continue;}
            for(_,key)in c450b_updated_keys(&context.node_caches).into_iter().filter(|(id,_)|*id==node.id){
                let output=context.node_caches.get_output_data(node.id,key,"pose".into()).unwrap().into_pose().unwrap();
                let mut inputs=Vec::new();let mut poses=Vec::new();
                for pin in ["reference_pose","target_pose"]{
                    let source=graph.edges_inverted.get(&TargetPin::NodeData(node.id,pin.into())).unwrap();
                    let SourcePin::NodeData(id,name)=source else{panic!("A current difference input is not a node output");};
                    assert!(context.node_caches.is_updated(*id,key));
                    let pose=context.node_caches.get_output_data(*id,key,name.clone()).unwrap().into_pose().unwrap();
                    inputs.push(serde_json::json!({"pin":pin,"source_node":id,"source_pin":name,"pose":c450b_pose_value(&pose,skeletons,assets)}));poses.push(pose);
                }
                let skeleton=skeletons.get(&poses[0].skeleton).unwrap();
                let mut expected=bevy_als::extensions::mesh_space_difference::make_mesh_space_difference(&poses[0],&poses[1],skeleton);expected.timestamp=poses[1].timestamp;
                let data:serde_json::Value=serde_json::from_slice(&std::fs::read("docs/audit/bf144/450b/key-data.json").unwrap()).unwrap();
                let mut raw_reference=poses[0].clone();let pelvis=raw_reference.paths.keys().find(|id|skeleton.id_to_path(**id).and_then(|p|p.parts.last().map(|n|n.as_str().to_owned()))==Some("pelvis".to_owned())).copied().unwrap();
                let i=*raw_reference.paths.get(&pelvis).unwrap();raw_reference.bones[i].translation.as_mut().unwrap().x=data["raw_reference_x"].as_f64().unwrap()as f32;
                let raw_ref_control=bevy_als::extensions::mesh_space_difference::make_mesh_space_difference(&raw_reference,&poses[1],skeleton).get_bone(pelvis).unwrap().translation.unwrap().x;
                rows.push(serde_json::json!({"node_guid":node.id,"context":format!("{context_id:?}"),"state_key":format!("{key:?}"),"graph":assets.get_path(context.get_graph_id()).map(|p|p.to_string()),"inputs":inputs,"actual":c450b_pose_value(&output,skeletons,assets),"public":c450b_pose_value(&expected,skeletons,assets),"pose_exact_public":output==expected,"raw_reference_x_only_delta":raw_ref_control}));
            }
        }
    }}rows.sort_by_key(|v|(v["context"].to_string(),v["state_key"].to_string()));rows
}
use bevy::animation::animated_field;
#[derive(Resource,Default)]
struct C450BDataFactor { applications:u32, info:serde_json::Value }
fn c450b_data_factor(
    mut state:ResMut<C450BDataFactor>,
    mut graphs:ResMut<Assets<bevy_animation_graph::core::animation_graph::AnimationGraph>>,
    mut clips:ResMut<Assets<bevy_animation_graph::core::animation_clip::GraphClip>>,
    skeletons:Res<Assets<bevy_animation_graph::core::skeleton::Skeleton>>,
){
    if state.applications>0{return;}
    for(_,graph)in graphs.iter_mut(){for node in graph.nodes.values_mut(){
        if node.id.uuid().to_string()!="5ced9e76-5148-58a8-a591-ffda3ad6d593"{continue;}
        let mut builtin=node.try_inner_downcast_ref::<bevy_animation_graph::builtin_nodes::clip_node::ClipNode>().unwrap().clone();
        let original_handle={let bevy::reflect::ReflectRef::Struct(fields)=builtin.reflect_ref()else{panic!("The Forward clip is not a reflected struct");};fields.field("clip").unwrap().try_downcast_ref::<Handle<bevy_animation_graph::core::animation_clip::GraphClip>>().unwrap().clone()};
        let Some(original)=clips.get(&original_handle).cloned()else{return;};let Some(skeleton)=skeletons.get(&original.skeleton)else{return;};
        let pelvis=skeleton.iter_bones().find(|id|skeleton.id_to_path(*id).and_then(|p|p.parts.last().map(|n|n.as_str().to_owned()))==Some("pelvis".to_owned())).unwrap();
        let data:serde_json::Value=serde_json::from_slice(&std::fs::read("docs/audit/bf144/450b/key-data.json").unwrap()).unwrap();
        let mut keys=data["times"].as_array().unwrap().iter().zip(data["values"].as_array().unwrap()).map(|(t,v)|(t.as_f64().unwrap()as f32,Vec3::new(v[0].as_f64().unwrap()as f32,v[1].as_f64().unwrap()as f32,v[2].as_f64().unwrap()as f32))).collect::<Vec<_>>();
        let index=data["half_index"].as_u64().unwrap()as usize;assert_eq!(keys[index].0,0.5);
        for(time,value)in &keys{assert_eq!(bevy_als::extensions::source_sequence_player::sample_pose(&original,*time).get_bone(pelvis).unwrap().translation,Some(*value));}
        let before=keys[index].1;keys[index].1.x=data["current_reference_x"].as_f64().unwrap()as f32+data["compressed_delta_x"].as_f64().unwrap()as f32;let after=keys[index].1;
        let replacement=VariableCurve::new(AnimatableCurve::new(animated_field!(Transform::translation),UnevenSampleAutoCurve::new(keys.clone()).unwrap()));
        let mut cloned=original.clone();let curves=cloned.curves.get_mut(&pelvis.animation_target_id()).unwrap();let curve_index=curves.iter().position(|v|v.0.evaluator_id()==replacement.0.evaluator_id()).unwrap();curves[curve_index]=replacement;
        for(time,_)in &keys{
            let mut expected=bevy_als::extensions::source_sequence_player::sample_pose(&original,*time);
            if *time==0.5{let i=*expected.paths.get(&pelvis).unwrap();expected.bones[i].translation=Some(after);}
            assert_eq!(bevy_als::extensions::source_sequence_player::sample_pose(&cloned,*time),expected);
        }
        let handle=clips.add(cloned);let cloned_id=handle.id();
        {let bevy::reflect::ReflectMut::Struct(fields)=builtin.reflect_mut()else{panic!("The Forward clip is not mutable reflection data");};*fields.field_mut("clip").unwrap().try_downcast_mut::<Handle<bevy_animation_graph::core::animation_clip::GraphClip>>().unwrap()=handle;}
        let routed={let bevy::reflect::ReflectRef::Struct(fields)=builtin.reflect_ref()else{panic!("The Forward clip reflection was lost");};fields.field("clip").unwrap().try_downcast_ref::<Handle<bevy_animation_graph::core::animation_clip::GraphClip>>().unwrap().id()==cloned_id};
        state.info=serde_json::json!({"before":before.to_array(),"after":after.to_array(),"routed_to_clone":routed,"translation_key_count":keys.len(),"changed_key":index,"only_one_key_pelvis_x_changed":true});
        node.inner=bevy_animation_graph::core::animation_node::dyn_node_like::DynNodeLike::new(builtin);state.applications+=1;return;
    }}
}
fn c450b_markers(
    player:&bevy_animation_graph::core::animation_graph_player::AnimationGraphPlayer,
    graphs:&Assets<bevy_animation_graph::core::animation_graph::AnimationGraph>,
    skeletons:&Assets<bevy_animation_graph::core::skeleton::Skeleton>,
)->Vec<serde_json::Value>{
    let mut rows=Vec::new();
    if let Some(arena)=player.get_context_arena(){for context_id in arena.iter_context_ids(){
        let context=arena.get_context(context_id).unwrap();let Some(graph)=graphs.get(context.get_graph_id())else{continue;};
        for node in graph.nodes.values(){
            let guid=node.id.uuid().to_string();let label=match guid.as_str(){"ccaab6a2-424d-b00d-6794-f3a705d6f8c2"=>"base_provider_input","e4f33e44-4019-9bac-8e0a-dcbc0e3ae178"=>"rig_source","8191c2ea-49e5-bd21-ee33-86bd87b2f93c"=>"root_result",_=>continue};
            let key=bevy_animation_graph::core::context::node_states::StateKey::Default;if !context.node_caches.is_updated(node.id,key){continue;}
            let Some(value)=context.node_caches.get_output_data(node.id,key,"Pose".into())else{continue;};let Ok(pose)=value.as_pose()else{continue;};let Some(skeleton)=skeletons.get(&pose.skeleton)else{continue;};
            let id=pose.paths.keys().find(|id|skeleton.id_to_path(**id).and_then(|p|p.parts.last().map(|n|n.as_str().to_owned()))==Some("pelvis".to_owned())).unwrap();let b=pose.get_bone(*id).unwrap();
            rows.push(serde_json::json!({"boundary":label,"node_guid":node.id,"translation":b.translation.map(|v|v.to_array()),"rotation":b.rotation.map(|v|v.to_array()),"scale":b.scale.map(|v|v.to_array())}));
        }
    }}rows.sort_by_key(|v|v["boundary"].to_string());rows
}
