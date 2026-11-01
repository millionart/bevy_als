import hashlib
import json
from pathlib import Path

r=Path(__file__).resolve().parent
s=Path('docs/audit/bf144/443b/observer.rs').read_text().replace('C443B','C443C').replace('c443b_','c443c_')
old='bodies: Query<(&Position,&LinearVelocity,&Collider,&GlobalTransform,&bevy_als::als::character::AlsMovementParamsHandle)>, '
old=old.rstrip();new='bodies: Query<(&Position,&LinearVelocity,&Collider,&GlobalTransform,&bevy_als::als::character::AlsMovementParamsHandle,&Transform,Has<ChildOf>)>,'
assert s.count(old)==1;s=s.replace(old,new)
s=s.replace('let (position,velocity,capsule,world,handle)=bodies.get(actor)', 'let (position,velocity,capsule,world,handle,local,parented)=bodies.get(actor)')
needle='    let ordinal=probe.frame-1;\n    probe.raw_physics_rows.push('
assert s.count(needle)==1
extra='''    let source_half=params.crouch.standing_capsule_half_height*local.scale.y;
    let source_radius=params.crouch.standing_capsule_radius*local.scale.x.min(local.scale.z);
    let source_floor=bevy_als::als::motion::find_ground_floor(&spatial,position.0,source_half,source_radius,&params.cmc,
        bevy_als::als::state::LocomotionState::Grounded,&filter,actor,&candidates);
    let source_floor=source_floor.map(|(center,impact,entity)| {
        let distance=position.y-center;
        let target=if (0.019..=0.024).contains(&distance){position.y}else{center+(0.019+0.024)*0.5};
        let (direction,length)=Dir3::new_and_length(Vec3::Y*(target-position.y)).unwrap_or((Dir3::NEG_Y,0.0));
        let hit=spatial.cast_shape_predicate(capsule,position.0,current_world.rotation(),direction,
            &ShapeCastConfig{ignore_origin_penetration:true,..ShapeCastConfig::from_max_distance(length)},&filter,&accepts);
        serde_json::json!({"center":center,"impact_y":impact,"distance":distance,"entity":format!("{entity:?}"),"target_y":target,"unblocked_final_y":(position.0+*direction*length).y,"adjustment_blocked":hit.is_some()})
    });
    let scale_inputs=serde_json::json!({"parented":parented,"local_scale":local.scale.to_array(),"local_rotation":local.rotation.to_array(),"local_translation":local.translation.to_array(),
        "actual_scale":scale.to_array(),"cached_scale":cached_scale.to_array(),"collider_scale":capsule.scale().to_array(),
        "base_half":params.crouch.standing_capsule_half_height,"base_radius":params.crouch.standing_capsule_radius,
        "actual_half":half,"actual_radius":radius,"source_scale_half":source_half,"source_scale_radius":source_radius,"source_scale_floor":source_floor});
'''
s=s.replace(needle,extra+needle)
s=s.replace('"velocity":velocity.0.to_array(),"floor":correction}', '"velocity":velocity.0.to_array(),"floor":correction,"scale_inputs":scale_inputs}')
(r/'observer.rs').write_text(s)
original=Path('src/als_extras/levels/manual_playground.rs').read_text();needle='    if let AppExit::Error(code) = app.run() {';assert original.count(needle)==1
old=Path('docs/audit/bf144/443b/playground_observed.rs').read_text();b=old.index(needle);a=old.rindex('    app.insert_resource(TimeUpdateStrategy::ManualDuration',0,b);injection=old[a:b].replace('C443B','C443C').replace('c443b_','c443c_')
(r/'playground_observed.rs').write_text(original.replace(needle,injection+needle)+s)
args=json.loads(Path('docs/audit/bf144/443b/rustc-args.json').read_text());(r/'rustc-args.json').write_text(json.dumps([v.replace('/443b/','/443c/').replace('phase443b','phase443c')for v in args],indent=2)+'\n')
for n in ['compile.ps1','run.ps1','run_python.ps1','functional.ps1']:(r/n).write_text(Path('docs/audit/bf144/443b',n).read_text().replace('/443b','/443c').replace('ALS_C443B_OUTPUT','ALS_C443C_OUTPUT'))
(r/'provenance.json').write_text(json.dumps(dict(original_manual_playground_sha256=hashlib.sha256(original.encode()).hexdigest(),base_observer='docs/audit/bf144/443b/observer.rs',added='Actual local TQS, parent qualification, capsule scale and source stored-scale-only floor control',production_changed=False,render_gpu_pass_acceptance='deferred under existing user peripheral exclusion',screenshots=0,performance_acceptance='paused'),indent=2)+'\n')
print('Prepared stored TQS scale and same-input floor consumer observation',flush=True)
