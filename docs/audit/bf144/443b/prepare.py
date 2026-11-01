import hashlib
import json
from pathlib import Path

r=Path(__file__).resolve().parent
s=Path('docs/audit/bf144/443/observer.rs').read_text().replace('C443','C443B').replace('c443_','c443b_')
needle='    assets: Res<Assets<AlsMovementParams>>,'
assert s.count(needle)==1
s=s.replace(needle,needle+'''
    spatial: SpatialQuery,
    shapes: Query<(&Collider,&GlobalTransform)>,
    candidates: Query<(Option<&CollisionLayers>,Has<Sensor>,Option<&ColliderOf>,Option<&bevy_als::als::character::AlsCollisionLayerOwnership>)>,''')
needle='    let row = serde_json::json!({'
assert s.count(needle)==1
s=s.replace(needle,'''    let (capsule,body_world)=shapes.get(actor).expect("Observed capsule is required");
    let radius=params.crouch.standing_capsule_radius*body_world.to_scale_rotation_translation().0.x.min(body_world.to_scale_rotation_translation().0.z);
    let delta=cmc.grounded_movement_delta.unwrap_or(Vec3::ZERO).with_y(0.0);
    let filter=SpatialQueryFilter::from_excluded_entities([actor]);
    let accepts=|candidate|bevy_als::als::motion::character_blocks_collider(actor,candidate,&candidates);
    let capture=|h:ShapeHitData|serde_json::json!({"entity":format!("{:?}",h.entity),"distance":h.distance,"point1":h.point1.to_array(),"point2":h.point2.to_array(),"normal1":h.normal1.to_array(),"normal2":h.normal2.to_array()});
    let mut floor_probe=serde_json::json!({"eligible":false,"delta":delta.to_array(),"scale":body_world.to_scale_rotation_translation().0.to_array(),"radius":radius});
    if let Ok((direction,distance))=Dir3::new_and_length(delta) {
        let forward=spatial.cast_shape_predicate(capsule,position.0,Quat::IDENTITY,direction,
            &ShapeCastConfig {ignore_origin_penetration:true,..ShapeCastConfig::from_max_distance(distance)},&filter,&accepts);
        floor_probe["forward"]=forward.map_or(serde_json::Value::Null,capture);
        if forward.is_none() {
            let destination=position.0+delta;
            let hit=spatial.cast_shape_predicate(capsule,destination,Quat::IDENTITY,Dir3::NEG_Y,
                &ShapeCastConfig::from_max_distance(0.45+0.024*2.0),&filter,&accepts);
            floor_probe["eligible"]=serde_json::json!(true);
            floor_probe["destination"]=serde_json::json!(destination.to_array());
            floor_probe["floor"]=hit.map_or(serde_json::Value::Null,capture);
            if let Some(hit)=hit {
                let reduced=(radius-0.0015).max(0.001501);
                floor_probe["edge_valid"]=serde_json::json!(Vec2::new(hit.point1.x-destination.x,hit.point1.z-destination.z).length_squared()<reduced*reduced);
                floor_probe["predicted_y"]=serde_json::json!(destination.y-hit.distance+(bevy_als::als::character::ALS_CHARACTER_FLOOR_CLEARANCE+bevy_als::als::character::ALS_CHARACTER_TNUA_FLOAT_EPSILON));
            }
        }
    }
''' +needle)
s=s.replace('"ordinal": probe.frame,','"ordinal": probe.frame,"floor_probe":floor_probe,')
s=s.replace('motor_rows: Vec<serde_json::Value>,','motor_rows: Vec<serde_json::Value>,\n    raw_physics_rows: Vec<serde_json::Value>,')
s=s.replace('motor_rows:Vec::new()','motor_rows:Vec::new(),raw_physics_rows:Vec::new()')
s=s.replace('raw_physics_rows: Vec<serde_json::Value>,','raw_physics_rows: Vec<serde_json::Value>,\n    coefficient_rows: Vec<serde_json::Value>,')
s=s.replace('raw_physics_rows:Vec::new()','raw_physics_rows:Vec::new(),coefficient_rows:Vec::new()')
s=s.replace('"after_motors":probe.motor_rows,','"after_motors":probe.motor_rows,"before_floor_sync":probe.raw_physics_rows,')
s=s.replace('"before_floor_sync":probe.raw_physics_rows,','"before_floor_sync":probe.raw_physics_rows,"integration_coefficients":probe.coefficient_rows,')
s=s.replace('bodies: Query<(&Position,&LinearVelocity),With<bevy_als::als::character::AlsCharacter>>,','bodies: Query<(&Position,&LinearVelocity,&bevy_tnua::TnuaMotor,Option<&bevy_tnua::TnuaGravity>,Option<&GravityScale>),With<bevy_als::als::character::AlsCharacter>>,')
s=s.replace('let Ok((position,velocity))=bodies.get(actor) else{panic!("Observed motor body is missing");};','let Ok((position,velocity,motor,gravity,gravity_scale))=bodies.get(actor) else{panic!("Observed motor body is missing");};')
s=s.replace('probe.motor_rows.push(serde_json::json!({"ordinal":ordinal,','probe.motor_rows.push(serde_json::json!({"ordinal":ordinal,"motor_acceleration":motor.lin.acceleration.to_array(),"tnua_gravity":gravity.map_or(Vec3::ZERO,|g|g.0).to_array(),"gravity_scale":gravity_scale.map_or(1.0,|g|g.0),"custom_gravity_present":gravity.is_some(),"gravity_scale_present":gravity_scale.is_some(),')
s+='''
fn c443b_coefficients(
    mut probe: ResMut<C443BProbe>,
    bodies: Query<(&avian3d::dynamics::integrator::VelocityIntegrationData,&avian3d::dynamics::solver::solver_body::SolverBody,Ref<Collider>)>,
) {
    let Some(actor)=probe.actor else{return;};
    if probe.frame==0{return;}
    let (integration,body,collider)=bodies.get(actor).expect("Observed solver body is required");
    let ordinal=probe.frame-1;
    probe.coefficient_rows.push(serde_json::json!({"ordinal":ordinal,"linear_increment":integration.linear_increment.to_array(),
        "linear_damping_rhs":integration.linear_damping_rhs,"initial_velocity":body.linear_velocity.to_array(),"initial_delta":body.delta_position.to_array(),"collider_changed":collider.is_changed()}));
}
'''
s+='''
fn c443b_raw_physics(
    mut probe: ResMut<C443BProbe>,
    spatial: SpatialQuery,
    transforms: bevy::transform::helper::TransformHelper,
    assets: Res<Assets<AlsMovementParams>>,
    bodies: Query<(&Position,&LinearVelocity,&Collider,&GlobalTransform,&bevy_als::als::character::AlsMovementParamsHandle)>,
    candidates: Query<(Option<&CollisionLayers>,Has<Sensor>,Option<&ColliderOf>,Option<&bevy_als::als::character::AlsCollisionLayerOwnership>)>,
) {
    let Some(actor)=probe.actor else{return;};
    if probe.frame==0{return;}
    let (position,velocity,capsule,world,handle)=bodies.get(actor).expect("Observed raw physics body is required");
    let params=assets.get(handle.id()).expect("Observed movement parameters are required");
    let cached_scale=world.to_scale_rotation_translation().0;
    let current_world=transforms.compute_global_transform(actor).expect("Current body transform is required");
    let scale=current_world.to_scale_rotation_translation().0;
    let half=params.crouch.standing_capsule_half_height*scale.y;
    let radius=params.crouch.standing_capsule_radius*scale.x.min(scale.z);
    let filter=SpatialQueryFilter::from_excluded_entities([actor]);
    let accepts=|candidate|bevy_als::als::motion::character_blocks_collider(actor,candidate,&candidates);
    let floor=bevy_als::als::motion::find_ground_floor(&spatial,position.0,half,radius,&params.cmc,
        bevy_als::als::state::LocomotionState::Grounded,&filter,actor,&candidates);
    let mut correction=serde_json::Value::Null;
    if let Some((center,impact,entity))=floor {
        let distance=position.y-center;
        let next=if (0.019..=0.024).contains(&distance){position.y}else{center+(0.019+0.024)*0.5};
        let displacement=Vec3::Y*(next-position.y);
        let (direction,length)=Dir3::new_and_length(displacement).unwrap_or((Dir3::NEG_Y,0.0));
        let hit=spatial.cast_shape_predicate(capsule,position.0,current_world.rotation(),direction,
            &ShapeCastConfig{ignore_origin_penetration:true,..ShapeCastConfig::from_max_distance(length)},&filter,&accepts);
        correction=serde_json::json!({"center":center,"impact_y":impact,"entity":format!("{entity:?}"),"distance":distance,"target_y":next,"scale":scale.to_array(),"cached_scale":cached_scale.to_array(),"half":half,
            "adjustment_hit":hit.map(|h|serde_json::json!({"distance":h.distance,"normal":h.normal1.to_array(),"point":h.point1.to_array()}))});
    }
    let ordinal=probe.frame-1;
    probe.raw_physics_rows.push(serde_json::json!({"ordinal":ordinal,"position":position.0.to_array(),"velocity":velocity.0.to_array(),"floor":correction}));
}
'''
(r/'observer.rs').write_text(s)
source=Path('src/als_extras/levels/manual_playground.rs').read_text();needle='    if let AppExit::Error(code) = app.run() {';assert source.count(needle)==1
injection='''    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(1.0/30.0)));
    app.world_mut().resource_mut::<RenderEffectsProbe>().directory=None;
    app.init_resource::<C443BProbe>();
    app.add_systems(First,c443b_drive);
    app.add_systems(Update,c443b_sample.after(bevy_als::als::dynamics::grounded_dynamics_motor_system).before(bevy_tnua::TnuaPipelineSystems::Motors));
    app.add_systems(Update,c443b_motor.after(bevy_tnua::TnuaPipelineSystems::Motors));
    app.add_systems(PostUpdate,c443b_raw_physics.after(avian3d::prelude::PhysicsSystems::Last).before(bevy_als::als::motion::AlsActualVelocitySet));
    app.add_systems(avian3d::prelude::PhysicsSchedule,c443b_coefficients.after(avian3d::dynamics::solver::schedule::SolverSystems::PreSubstep).before(avian3d::dynamics::solver::schedule::SolverSystems::Substep));
    app.add_systems(Last,c443b_late);
'''
(r/'playground_observed.rs').write_text(source.replace(needle,injection+needle)+s)
args=json.loads(Path('docs/audit/bf144/443/rustc-args.json').read_text());(r/'rustc-args.json').write_text(json.dumps([v.replace('/443/','/443b/').replace('phase443','phase443b')for v in args],indent=2)+'\n')
for name in ['compile.ps1','run.ps1','run_python.ps1','functional.ps1']:
    (r/name).write_text(Path('docs/audit/bf144/443',name).read_text().replace('/443','/443b').replace('ALS_C443_OUTPUT','ALS_C443B_OUTPUT'))
(r/'provenance.json').write_text(json.dumps(dict(original_manual_playground_sha256=hashlib.sha256(source.encode()).hexdigest(),base_observer='docs/audit/bf144/443/observer.rs',added='Readonly actual forward and floor shape queries at observed input stage',source_or_target_production_changed=False,render_gpu_pass_acceptance='deferred by user peripheral exclusion after unchanged public entry reproduced TAA timeout',render_components_and_scene_preserved=True,screenshots=0,performance_acceptance='paused'),indent=2)+'\n')
print('Prepared readonly current floor queries and complete old phase records',flush=True)
