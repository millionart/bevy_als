#[derive(Resource)]
struct C445Probe {
    actor: Option<Entity>,
    frame: usize,
    rows: Vec<serde_json::Value>,
    progress: Option<(bool, usize, usize)>,
    camera_rows: Vec<serde_json::Value>,
    motor_rows: Vec<serde_json::Value>,
    animation_before: Vec<serde_json::Value>,
    animation_after: Vec<serde_json::Value>,
}
impl Default for C445Probe {
    fn default() -> Self { Self { actor: None, frame: 0, rows: Vec::new(), progress: None, camera_rows:Vec::new(),motor_rows:Vec::new(),animation_before:Vec::new(),animation_after:Vec::new() } }
}
fn c445_progress(value: serde_json::Value) {
    let output = std::env::var("ALS_C445_OUTPUT").expect("parameter evidence output is required");
    std::fs::write(std::path::Path::new(&output).with_extension("progress.json"), value.to_string()).unwrap();
}
fn c445_drive(
    mut commands: Commands,
    mut probe: ResMut<C445Probe>,
    ready: Res<ManualPlaygroundStartupReady>,
    admissions: Query<&AlsPlayerAdmissionComplete>,
    graphs: Res<Assets<bevy_animation_graph::core::animation_graph::AnimationGraph>>,
    mut actors: Query<(&mut bevy_als::als::gait::AlsDesiredGait, &mut bevy_als::als::transition::AlsDesiredStance, &mut bevy_als::als::motion::AlsViewYaw, &mut bevy_als::als::state::RotationMode)>,
) {
    let progress = (ready.passed, admissions.iter().count(), graphs.len());
    if probe.progress != Some(progress) { info!("[C445] startup_progress={progress:?}"); probe.progress = Some(progress); c445_progress(serde_json::json!({"startup":progress,"captured":probe.frame})); }
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
fn c445_sample(
    mut probe: ResMut<C445Probe>,
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
    if probe.frame % 30 == 0 { info!("[C445] captured_parameters={}",probe.frame); c445_progress(serde_json::json!({"captured":probe.frame})); }
}
fn c445_late(
    mut probe: ResMut<C445Probe>,
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
        let output=std::env::var("ALS_C445_OUTPUT").expect("input direction evidence output is required");
        std::fs::write(output,serde_json::to_vec_pretty(&serde_json::json!({"gate":"CAPTURED_REAL_INPUT_CAMERA_AND_DESIRED_DIRECTION240","rows":probe.rows,"after_motors":probe.motor_rows,"animation_before":probe.animation_before,"animation_after":probe.animation_after,"camera_after_post_update":probe.camera_rows,"screenshots":0,"performance_acceptance":"paused"})).unwrap()).unwrap();
        exit.write(AppExit::Success);
    }
}

fn c445_motor(
    mut probe: ResMut<C445Probe>,
    bodies: Query<(&Position,&LinearVelocity),With<bevy_als::als::character::AlsCharacter>>,
) {
    let Some(actor)=probe.actor else{return;};
    if probe.frame==0{return;}
    let Ok((position,velocity))=bodies.get(actor) else{panic!("Observed motor body is missing");};
    let ordinal=probe.frame-1;
    probe.motor_rows.push(serde_json::json!({"ordinal":ordinal,"position":position.0.to_array(),"velocity":velocity.0.to_array()}));
}
fn c445_animation(
    In(before): In<bool>,
    graphs:Res<Assets<bevy_animation_graph::core::animation_graph::AnimationGraph>>,
    assets:Res<AssetServer>,
    mut probe: ResMut<C445Probe>,
    time: Res<Time>,
    virtual_time: Res<Time<Virtual>>,
    avatars: Query<(&ChildOf,&bevy_animation_graph::core::animated_scene::AnimatedSceneInstance),With<bevy_als::character_assembly::AlsCharacterAvatar>>,
    characters: Query<(&bevy_als::actual_motion::AlsActualMotion,&Position,&LinearVelocity)>,
    players: Query<(&bevy_animation_graph::core::animation_graph_player::AnimationGraphPlayer,Option<&bevy_als::extensions::post_physics_animation::AnimationPoseUpdateGate>)>,
) {
    let Some((parent,instance))=avatars.iter().next() else{return;};
    let Ok((motion,position,velocity))=characters.get(parent.parent()) else{return;};
    let Ok((player,gate))=players.get(instance.player_entity()) else{return;};
    let scalar=|key|player.get_global_input_data(key).and_then(|v|v.as_f32().ok());
    let vector=|key|player.get_global_input_data(key).and_then(|v|v.as_vec3().ok()).map(|v|v.to_array());
    let boolean=|key|player.get_global_input_data(key).and_then(|v|v.as_bool().ok());
    let row=serde_json::json!({
        "ordinal":probe.actor.map(|_|probe.frame.saturating_sub(1)),
        "leaf_nodes":if before{Vec::new()}else{c445_leaf_rows(player,&graphs,&assets)},
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
fn c445_updated_keys(caches:&bevy_animation_graph::core::context::node_caches::NodeCaches)->Vec<(bevy_animation_graph::core::animation_graph::NodeId,bevy_animation_graph::core::context::node_states::StateKey)>{
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
fn c445_leaf_rows(
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
        let updated=c445_updated_keys(&context.node_caches);
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
