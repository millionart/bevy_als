use super::*;
use bevy::diagnostic::FrameCount;
use bevy_animation_graph::core::{
    animated_scene::AnimatedSceneInstance, animation_graph::AnimationGraph,
    animation_graph_player::AnimationGraphPlayer, context::node_states::StateKey,
    pose::Pose, skeleton::Skeleton,
};
use bevy_als::{
    als::{gait::AlsDesiredGait, input::AlsInputCommand, motion::{AlsInputOverride, AlsViewYaw},
        state::{Gait, LocomotionState, RotationMode, Stance}, transition::AlsDesiredStance},
    actual_motion::AlsActualMotion,
    animation_context_projection::{AlsRootAnimationContext, AlsRootAnimationContextSet},
    character_assembly::{AlsCharacterAvatar, AlsCharacterAssembly, AlsCharacterBlueprint, spawn_als_character_with_materials},
    extensions::{control_rig_hierarchy::RigSpace, post_physics_animation::PostPhysicsAnimationSet,
        root_graph_boundaries::{current_control_rig, ControlRigBoundaryNode}, source_instance::source_player_property},
    foot_ik_apply::AlsFootIkTelemetryEnabled,
};
use serde_json::{json, Value};
use std::collections::BTreeMap;

#[derive(Resource)]
struct Probe {
    output: PathBuf,
    actor: Option<Entity>,
    start_frame: Option<u32>,
    active: usize,
    previous_frame: Option<u64>,
    inputs: Option<Value>,
    motion_plan: Option<Value>,
    physics_stages: Vec<Value>,
    rows: Vec<Value>,
}

pub(super) fn install(app: &mut App, output: PathBuf) {
    app.insert_resource(Probe { output, actor: None, start_frame: None, active: 0, previous_frame: None,
        inputs: None, motion_plan: None, physics_stages: Vec::new(), rows: Vec::new() });
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(1.0 / 30.0)));
    app.insert_resource(AlsFootIkTelemetryEnabled(true));
    app.add_systems(First, drive);
    app.add_systems(Update, (|probe: ResMut<Probe>, actors: Query<PhysicsSample>| sample_physics(probe, actors, "before_motor"))
        .after(bevy_als::als::dynamics::grounded_dynamics_motor_system).before(bevy_tnua::TnuaPipelineSystems::Motors));
    app.add_systems(Update, (|probe: ResMut<Probe>, actors: Query<PhysicsSample>| sample_physics(probe, actors, "after_motor"))
        .after(bevy_tnua::TnuaPipelineSystems::Motors));
    app.add_systems(PostUpdate, (|probe: ResMut<Probe>, actors: Query<PhysicsSample>| sample_physics(probe, actors, "before_physics"))
        .before(PhysicsSystems::First));
    app.add_systems(PostUpdate, (|probe: ResMut<Probe>, actors: Query<PhysicsSample>| sample_physics(probe, actors, "after_physics"))
        .after(PhysicsSystems::Last).before(bevy_als::als::motion::AlsActualVelocitySet));
    app.add_systems(PostUpdate, inputs.after(PostPhysicsAnimationSet::Inputs).before(PostPhysicsAnimationSet::Evaluate));
    app.add_systems(PostUpdate, motion_plan.after(bevy_als::als::motion::AlsActualVelocitySet)
        .before(bevy_als::als::motion::AlsRotationResponseSet));
    app.add_systems(PostUpdate, outputs.after(AlsRootAnimationContextSet));
}

fn drive(mut commands: Commands, mut probe: ResMut<Probe>, ready: Res<ManualPlaygroundStartupReady>, frame: Res<FrameCount>,
    admissions: Query<&AlsPlayerAdmissionComplete>, assets: Res<AssetServer>, mut materials: ResMut<Assets<StandardMaterial>>,
    mut params: ResMut<Assets<AlsMovementParams>>, mut configs: ResMut<Assets<bevy_als::als::control_scheme::AlsControlSchemeConfig>>,
    mut actors: Query<(&mut AlsDesiredGait, &mut AlsDesiredStance, &mut AlsViewYaw, &mut RotationMode)>,
    assemblies: Query<&AlsCharacterAssembly>,
    mut exit: MessageWriter<AppExit>,
    spatial: SpatialQuery,
) {
    if !ready.passed { return; }
    if probe.active >= 89 {
        exit.write(AppExit::Success);
        return;
    }
    let Some(admission) = admissions.iter().next() else { return; };
    if probe.actor.is_none() {
        let fit = admission.selection.fit;
        let start = Vec3::new(fit.transform.translation.x, fit.ground_y + 1.5, fit.transform.translation.z);
        let mut blueprint = AlsCharacterBlueprint::new(Transform::from_translation(start)
            .with_rotation(Quat::from_rotation_y(-std::f32::consts::FRAC_PI_2)));
        blueprint.spawn_input = false;
        let Ok(assembly) = assemblies.get(admission.character) else { return; };
        commands.entity(assembly.avatar).remove::<AlsCharacterAvatar>();
        commands.entity(admission.character).remove::<AlsCharacter>();
        commands.entity(admission.character).insert((CollisionLayers::NONE, TnuaToggle::Disabled, GravityScale(0.0)));
        let ray_origin = start + Vec3::Y;
        let hit = spatial.cast_ray(ray_origin, Dir3::NEG_Y, 3.0, true,
            &SpatialQueryFilter::from_excluded_entities([admission.character]).with_mask(AlsCollisionLayer::Default));
        probe.physics_stages.push(json!({"stage":"spawn_ray", "fit_height":fit.ground_y,
            "ray_height":hit.map(|hit| ray_origin.y - hit.distance), "ray_normal":hit.map(|hit|hit.normal.to_array())}));
        probe.actor = Some(spawn_als_character_with_materials(&mut commands, &assets, &mut materials,
            &mut params, &mut configs, blueprint));
        probe.start_frame = Some(frame.0);
        return;
    }
    let actor = probe.actor.unwrap();
    let Ok((mut gait, mut stance, mut view, mut mode)) = actors.get_mut(actor) else { return; };
    gait.0 = if probe.active >= 12 { Gait::Walking } else { Gait::Running };
    stance.0 = if probe.active >= 12 { Stance::Crouching } else { Stance::Standing };
    view.0 = -std::f32::consts::FRAC_PI_2;
    *mode = RotationMode::ViewDirection;
    commands.entity(actor).insert(AlsInputOverride(AlsInputCommand {
        move_dir: if (12..72).contains(&probe.active) { Vec2::Y * 0.8 } else { Vec2::ZERO }, ..default()
    }));
}

fn inputs(mut probe: ResMut<Probe>, frame: Res<FrameCount>, time: Res<Time<Virtual>>,
    assemblies: Query<&AlsCharacterAssembly>, avatars: Query<&AnimatedSceneInstance, With<AlsCharacterAvatar>>,
    players: Query<(&AnimationGraphPlayer, Option<&AlsRootAnimationContext>)>,
) {
    if probe.active >= 89 { return; }
    let Some(actor) = probe.actor else { return; };
    let Ok(assembly) = assemblies.get(actor) else { return; };
    let Ok(avatar) = avatars.get(assembly.avatar) else { return; };
    let Ok((player, previous)) = players.get(avatar.player_entity()) else { return; };
    let fields = ["animation.delta_seconds", "pose_state.standing_amount", "pose_state.grounded_amount"];
    let properties = fields.map(|field| source_player_property(player, field).or_else(|| player.get_global_input_data(field)).and_then(|value| value.as_f32().ok()));
    let curves = previous.map(|context| context.curves.iter().map(|(key, &value)| (key.to_string(), value)).collect::<BTreeMap<_, _>>());
    probe.inputs = Some(json!({"frame": frame.0, "delta_ns":time.delta().as_nanos() as u64,
        "properties":properties,"previous_curves":curves}));
}

fn pose_value(pose: &Pose, skeletons: &Assets<Skeleton>) -> Value {
    let skeleton = skeletons.get(&pose.skeleton).expect("Observed pose skeleton is required");
    let bones = pose.paths.iter().map(|(id, index)| {
        let name = skeleton.id_to_path(*id).and_then(|path| path.parts.last().map(ToString::to_string)).expect("Observed bone path is required");
        let bone = &pose.bones[*index];
        (name, json!([bone.translation.map(|v| v.to_array()), bone.rotation.map(|v| v.to_array()),
            bone.scale.map(|v| v.to_array())]))
    }).collect::<BTreeMap<_,_>>();
    json!({"timestamp":pose.timestamp,"bones":bones,"root_motion":pose.root_motion})
}

fn outputs(mut probe: ResMut<Probe>, frame: Res<FrameCount>, ready: Res<ManualPlaygroundStartupReady>,
    graphs: Res<Assets<AnimationGraph>>, skeletons: Res<Assets<Skeleton>>,
    avatars: Query<(&ChildOf, &AnimatedSceneInstance), With<AlsCharacterAvatar>>,
    players: Query<(&AnimationGraphPlayer, Option<&AlsRootAnimationContext>)>,
    actors: Query<(&Position, &LinearVelocity, &LocomotionState, &AlsActualMotion, &AlsViewYaw,
        &bevy_tnua::TnuaController<bevy_als::als::control_scheme::AlsControlScheme>)>,
) {
    if probe.active >= 89 { return; }
    let Some(actor) = probe.actor else { return; };
    let Some((parent, avatar)) = avatars.iter().find(|(parent, _)| parent.parent() == actor) else { return; };
    let Ok((player, output)) = players.get(avatar.player_entity()) else { return; };
    let Some(arena) = player.get_context_arena() else { return; };
    if probe.previous_frame == Some(arena.frame_index()) { return; }
    let Some(root) = player.get_default_output_pose() else { return; };
    let Some((context, node, state)) = current_control_rig(player, &graphs) else { return; };
    let Ok((position, velocity, mode, motion, view, controller)) = actors.get(parent.parent()) else { return; };
    let graph = graphs.get(context.get_graph_id()).unwrap();
    let definition = node.try_inner_downcast_ref::<ControlRigBoundaryNode>().unwrap().definition.as_ref().unwrap();
    let mut hierarchy = state.snapshot_hierarchy().expect("Observed Rig hierarchy is required");
    let rig_local = definition.bones.iter().enumerate().map(|(index, bone)|
        (&bone.name, hierarchy.transform(index, RigSpace::Local, false).unwrap())).collect::<BTreeMap<_,_>>();
    let poses = [("rig_source","e4f33e44-4019-9bac-8e0a-dcbc0e3ae178"),
        ("rig_output","5514524f-4eea-c42c-19cd-349ea6f99e36")].into_iter().map(|(name, guid)| {
        let id = graph.nodes.values().find(|node| node.id.uuid().to_string() == guid).unwrap().id;
        let pose = context.node_caches.get_output_data(id, StateKey::Default, "Pose".into())
            .and_then(|value| value.as_pose().ok().map(|pose| pose_value(pose, &skeletons)));
        (name, pose)
    }).collect::<BTreeMap<_,_>>();
    let curves = output.map(|context| context.curves.iter().map(|(key,&value)| (key.to_string(),value)).collect::<BTreeMap<_,_>>());
    let raw_rig = (probe.rows.len() < 8).then(|| {
        let externals = definition.externals.iter().map(|(name, reference)| (name, state.value(reference).unwrap())).collect::<BTreeMap<_,_>>();
        let traces = state.traces.iter().map(|trace| json!({"instruction":trace.instruction,"start":trace.start,"end":trace.end,"hit":trace.hit})).collect::<Vec<_>>();
        json!({"externals":externals,"traces":traces})
    });
    let source_machines = (probe.active <= 16).then(|| arena.iter_context_ids().filter_map(|id| {
        let current = arena.get_context(id)?;
        let graph = graphs.get(current.get_graph_id())?;
        let states = graph.nodes.values().filter_map(|node| {
            let state = current.node_states.get::<bevy_als::extensions::interruptible_fsm::InterruptibleFsmState>(node.id, StateKey::Default).ok()?;
            let reflected = bevy::reflect::PartialReflect::reflect_ref(state).as_struct().ok()?;
            let field = |name| reflected.field(name).and_then(|v| v.try_downcast_ref::<f32>()).copied();
            Some(json!({"name":node.name,"state":format!("{:?}",state.state),"elapsed":field("transition_elapsed_time"),
                "alpha":field("blend_alpha"),"remaining":field("blend_remaining"),"state_time":state.current_state_elapsed_time()}))
        }).collect::<Vec<_>>();
        (!states.is_empty()).then_some(states)
    }).collect::<Vec<_>>());
    let gait_chain = (probe.active <= 17).then(|| arena.iter_context_ids().filter_map(|id| {
        let graph_context = arena.get_context(id)?;
        let graph = graphs.get(graph_context.get_graph_id())?;
        let curve = bevy_als::extensions::SourceCurveName::from("PoseGait");
        let nodes = graph.nodes.values().filter_map(|node| {
            let key = StateKey::Default;
            if !graph_context.node_caches.is_updated(node.id, key) { return None; }
            let context = bevy_als::extensions::animation_context::node_context_state(&graph_context.node_states, key, node.id)?;
            let gait = context.curves.get(&curve).copied()?;
            Some(json!({"node":node.name,"id":node.id.uuid().to_string(),"gait":gait,
                "asset":context.relevant_asset.as_ref().map(|clock| clock.position)}))
        }).collect::<Vec<_>>();
        (!nodes.is_empty()).then(|| json!({"context":format!("{id:?}"),"nodes":nodes}))
    }).collect::<Vec<_>>());
    let physics_stages = std::mem::take(&mut probe.physics_stages);
    let motion_plan = probe.motion_plan.take();
    let before = probe.inputs.take();
    let active = Some(probe.active);
    let row = json!({"frame":frame.0,"arena":arena.frame_index(),"active":active,"startup_ready":ready.passed,"spawn_frame":probe.start_frame,
        "inputs":before,"position":position.0.to_array(),"velocity":velocity.0.to_array(),"mode":mode,
        "view_yaw":view.0,"actual_speed":motion.horizontal_speed,"movement_input":controller.basis.desired_motion.to_array(),
        "motion_plan":motion_plan,"gait_chain":gait_chain,"physics_stages":physics_stages,"source_machines":source_machines,"raw_rig":raw_rig,"root":pose_value(root,&skeletons),"poses":poses,"curves":curves,"rig_local":rig_local,
        "rig_units_to_meters":state.meters_per_unit(),"rig_to_world":state.to_world,"rig_updates":state.executed_steps});
    if let Some(error) = player.get_error() {
        error!("The observed character graph failed: {error:?}");
    }
    probe.rows.push(row);
    probe.previous_frame = Some(arena.frame_index());
    if active.is_some() { probe.active += 1; }
    if probe.rows.len() % 30 == 0 { info!("[ROOT-HISTORY] observed={} active={}", probe.rows.len(), probe.active); }
    if probe.active == 89 {
        std::fs::write(&probe.output, serde_json::to_vec(&probe.rows).unwrap()).unwrap();
        info!("[ROOT-HISTORY] complete rows={} active={}", probe.rows.len(), probe.active);
    }
}

fn motion_plan(mut probe: ResMut<Probe>, physics: Res<Time<Physics>>, virtual_time: Res<Time<Virtual>>,
    actors: Query<(&bevy_als::als::character::AlsCmcAdapterRuntime,
        Option<&bevy_als::als::dynamics::AlsMovementBaseDynamicsState>,
        Option<&bevy_als::als_camera::AlsCameraCustomTimeDilation>), With<bevy_als::als::character::AlsCharacter>>,
) {
    if probe.active >= 89 { return; }
    let Some(actor) = probe.actor else { return; };
    let Ok((cmc, base, dilation)) = actors.get(actor) else { return; };
    probe.motion_plan = Some(json!({"delta":cmc.simulation_delta_seconds,"substeps":cmc.simulation_substeps,
        "move_start":cmc.grounded_move_start,"planned_delta":cmc.grounded_movement_delta,"physics_delta_ns":physics.delta().as_nanos() as u64,
        "physics_delta":physics.delta_secs(),"virtual_delta_ns":virtual_time.delta().as_nanos() as u64,
        "planned_velocity":base.and_then(|base|base.pre_collision_velocity),
        "support_velocity":base.map(|base|base.support_velocity),"dilation":dilation.map(|dilation|dilation.0)}));
}


type PhysicsSample<'a> = (
    &'a Position, &'a Transform, &'a LinearVelocity, &'a LocomotionState,
    &'a TnuaMotor, &'a bevy_als::als::character::AlsEffectiveGravity,
    &'a bevy_als::als::character::AlsCmcAdapterRuntime, Option<&'a TnuaToggle>,
    Has<CustomPositionIntegration>,
);

fn sample_physics(mut probe: ResMut<Probe>, actors: Query<PhysicsSample>, stage: &str) {
    if probe.active > 8 { return; }
    let Some(actor) = probe.actor else { return; };
    let Ok((position, transform, velocity, mode, motor, gravity, runtime, toggle, custom)) = actors.get(actor) else { return; };
    probe.physics_stages.push(json!({"stage":stage,"position":position.0.to_array(),
        "transform":transform.translation.to_array(),"velocity":velocity.0.to_array(),"mode":mode,
        "boost":motor.lin.boost.to_array(),"acceleration":motor.lin.acceleration.to_array(),
        "gravity":gravity.value.to_array(),"collision_velocity":runtime.grounded_collision_velocity,
        "toggle":format!("{toggle:?}"),"custom":custom}));
}
