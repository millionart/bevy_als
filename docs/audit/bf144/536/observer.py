import re
from pathlib import Path

root=Path(__file__).resolve().parent
previous=root.parent/'534'/'observer.py'
script=previous.read_text().replace('tools/tmp/bf144/534','tools/tmp/bf144/536')
exec(compile(script,previous.name,'exec'))
path=Path('tools/tmp/bf144/536/playground_observed.rs')
driver=path.read_text()
def change(body,anchor,replacement):
    assert body.count(anchor)==1,anchor
    return body.replace(anchor,replacement)

start=driver.index('fn c516_delta(')
stop=driver.index('fn c516_cache(',start)
driver=driver[:start]+driver[start:stop].replace('from_secs_f32(0.06)','from_secs_f32(0.03)')+driver[stop:]
driver=change(driver,'    assert_eq!(runtime.simulation_substeps, 2);','    assert_eq!(runtime.simulation_substeps, 1);')
driver=change(driver,'''    assert!(
        delta > params.cmc.max_simulation_time_step && params.cmc.max_simulation_iterations > 1
    );
    let step = params.cmc.max_simulation_time_step.min(delta * 0.5);''','    let step = delta;')
driver=change(driver,'        schedule.add_systems(c533_first_step.after(c516_before).before(transaction));',
'''        schedule.add_systems(c533_first_step.after(c516_before).before(transaction));
        schedule.add_systems(c536_trace.after(c533_first_step).before(transaction));''')
source=Path('src/als/character/movement/motion.rs').read_text()
step=source[source.index('fn grounded_movement_transaction('):source.index('fn adjust_walking_floor_height(')]
step=change(step,'fn grounded_movement_transaction(','fn c536_step(')
step=change(step,'step_permissions: &Query<&AlsMantleTargetPermission>,',
'''step_permissions: &Query<&bevy_als::als::mantle::AlsMantleTargetPermission>,
    skip_floor_guard: bool, events: &mut Vec<serde_json::Value>, names: &Query<&Name>,''')
step=step.replace('crate::als::params::','bevy_als::als::params::')
for name in ('character_blocks_collider','movement_sweep_travel','find_ground_floor'):
    step=re.sub(r'\b'+name+r'\b','bevy_als::als::motion::'+name,step)
for name in ('MAX_STEP_HEIGHT','MAX_FLOOR_DISTANCE','MAX_SLOPE_DEGREES','deg_to_rad'):
    prefix='c529_' if name=='deg_to_rad' else 'C529_'
    step=re.sub(r'\b'+name+r'\b',prefix+name,step)
for name in ('ALS_CHARACTER_FLOOR_CLEARANCE','ALS_CHARACTER_TNUA_FLOAT_EPSILON'):
    step=re.sub(r'\b'+name+r'\b','bevy_als::als::character::'+name,step)
step=change(step,'        let advance = *direction * bevy_als::als::motion::movement_sweep_travel(distance, hit.distance);',
'''        events.push(serde_json::json!({"kind":"forward","hit":c529_hit(&hit,names)}));
        let advance = *direction * bevy_als::als::motion::movement_sweep_travel(distance, hit.distance);''')
step=change(step,'''let Some(floor) = floor_sweep_hit(&spatial_query, capsule, radius, destination,
        C529_MAX_STEP_HEIGHT + C529_MAX_FLOOR_DISTANCE * 2.0, &filter, &accepts)''',
'''let Some(floor) = c529_sweep(&spatial_query, capsule, radius, destination,
        C529_MAX_STEP_HEIGHT + C529_MAX_FLOOR_DISTANCE * 2.0, &filter, &accepts,"none",events,names)''')
step=change(step,'''    if initial_floor_y.is_some() && bevy_als::als::motion::find_ground_floor(&spatial_query, destination.with_y(next_y), half, radius,
        cmc, mode, &filter, entity, &candidates).is_none() { return Some(result); }''',
'''    if initial_floor_y.is_some() {
        let at=destination.with_y(next_y);
        let landing_floor=bevy_als::als::motion::find_ground_floor(&spatial_query,at,half,radius,cmc,mode,&filter,entity,&candidates);
        let mut floor_events=Vec::new();
        let reproduced=c529_floor(spatial_query,at,half,radius,cmc,mode,&filter,entity,candidates,"none",&mut floor_events,names).map(|(floor,_)|floor);
        assert_eq!(landing_floor,reproduced);
        events.push(serde_json::json!({"kind":"landing_check","position":at,"down_hit":c529_hit(&floor,names),
            "valid":landing_floor.is_some(),"floor_events":floor_events}));
        if !skip_floor_guard && landing_floor.is_none() { return Some(result); }
    }''')
path.write_text(driver+'\n'+step+'\n'+(root/'step_probe.rs').read_text())
print('Prepared a real step and an isolated destination floor guard replay',flush=True)
