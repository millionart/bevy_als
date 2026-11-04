import re
from pathlib import Path

root = Path(__file__).resolve().parent
previous = root.parent / '526' / 'observer.py'
script = previous.read_text().replace('tools/tmp/bf144/526','tools/tmp/bf144/529')
exec(compile(script, previous.name, 'exec'))
path = Path('tools/tmp/bf144/529/playground_observed.rs')
driver = path.read_text()
source = Path('src/als/character/movement/motion.rs').read_text()

def change(body, before, after):
    assert body.count(before) == 1, before
    return body.replace(before, after)

sweep = source[source.index('fn floor_sweep_hit('):source.index('pub fn find_ground_floor(')]
sweep = change(sweep, 'fn floor_sweep_hit(', 'fn c529_sweep(')
sweep = change(sweep, 'accepts: &dyn Fn(Entity) -> bool)',
               'accepts: &dyn Fn(Entity) -> bool, skip: &str, events: &mut Vec<serde_json::Value>, names: &Query<&Name>)')
sweep = change(sweep, '''    let mut hit = spatial.cast_shape_predicate(capsule, origin, Quat::IDENTITY, Dir3::NEG_Y,
        &ShapeCastConfig::from_max_distance(distance), filter, accepts)?;''', '''    let raw = spatial.cast_shape_predicate(capsule, origin, Quat::IDENTITY, Dir3::NEG_Y,
        &ShapeCastConfig::from_max_distance(distance), filter, accepts);
    events.push(serde_json::json!({"kind":"capsule","origin":origin,"radius_m":radius,
        "cast_distance_m":distance,"hit":raw.as_ref().map(|hit| c529_hit(hit,names))}));
    let mut hit = raw?;''')
sweep = change(sweep, '''    if Vec2::new(hit.point1.x - origin.x, hit.point1.z - origin.z).length_squared() >= reduced_radius * reduced_radius {''',
'''    let radial_sq = Vec2::new(hit.point1.x - origin.x, hit.point1.z - origin.z).length_squared();
    events.push(serde_json::json!({"kind":"edge","radial_sq":radial_sq,"limit_sq":reduced_radius*reduced_radius,
        "rejected":radial_sq >= reduced_radius*reduced_radius}));
    if radial_sq >= reduced_radius * reduced_radius {''')
sweep = change(sweep, 'if normal.y < deg_to_rad', 'if skip != "short" && normal.y < deg_to_rad')
sweep = change(sweep, '''        if let Some(surface) = spatial.cast_ray_predicate(ray_origin, Dir3::NEG_Y, EDGE_REJECT_DISTANCE * 2.0, false, filter, accepts)
            .filter(|surface| surface.entity == hit.entity) {''',
'''        let surface = spatial.cast_ray_predicate(ray_origin, Dir3::NEG_Y, EDGE_REJECT_DISTANCE * 2.0, false, filter, accepts);
        events.push(serde_json::json!({"kind":"short_ray","origin":ray_origin,
            "hit":surface.as_ref().map(|ray|serde_json::json!({"entity":c529_name(ray.entity,names),
                "normal":ray.normal,"distance_m":ray.distance,"same_entity":ray.entity==hit.entity}))}));
        if let Some(surface) = surface.filter(|surface| surface.entity == hit.entity) {''')

floor = source[source.index('fn find_ground_floor_with_penetration('):source.index('fn resolved_walkable_contact(')]
floor = change(floor, 'fn find_ground_floor_with_penetration(', 'fn c529_floor(')
floor = change(floor, 'candidates: &Query<(Option<&CollisionLayers>, Has<Sensor>, Option<&ColliderOf>, Option<&AlsCollisionLayerOwnership>)>)',
'''candidates: &Query<(Option<&CollisionLayers>, Has<Sensor>, Option<&ColliderOf>, Option<&AlsCollisionLayerOwnership>)>,
    skip: &str, events: &mut Vec<serde_json::Value>, names: &Query<&Name>)''')
floor = change(floor, 'character_blocks_collider(character, candidate, candidates)',
               'bevy_als::als::motion::character_blocks_collider(character, candidate, candidates)')
floor = change(floor, 'sweep_distance: f32|', 'sweep_distance: f32, events: &mut Vec<serde_json::Value>|')
floor = change(floor, 'let capsule = als_character_capsule',
               'let capsule = bevy_als::als::character::als_character_capsule')
floor = change(floor, 'floor_sweep_hit(spatial, &capsule, sweep_radius, at,\n                sweep_distance + shrink_height, filter, &accepts)',
               'c529_sweep(spatial, &capsule, sweep_radius, at,\n                sweep_distance + shrink_height, filter, &accepts, skip, events, names)')
floor = change(floor, '                if valid { return result; }',
'''                events.push(serde_json::json!({"kind":"capsule_qualification","hit":c529_hit(&hit,names),
                    "floor_distance_m":floor_distance,"valid":valid,"shrink_m":shrink_height}));
                if valid { return result; }''')
floor = change(floor, '''        if result.is_some() && line_distance > 0.0
            && let Some(ray) = spatial.cast_ray_predicate(at, Dir3::NEG_Y,
                line_distance + half, false, filter, &accepts)
            && ray.distance > 0.0 && ray.normal.y >= deg_to_rad(MAX_SLOPE_DEGREES).cos() {''',
'''        let center_ray = if skip != "center" && result.is_some() && line_distance > 0.0 {
            let ray = spatial.cast_ray_predicate(at, Dir3::NEG_Y, line_distance + half, false, filter, &accepts);
            events.push(serde_json::json!({"kind":"center_ray","origin":at,"hit":ray.as_ref().map(|ray|
                serde_json::json!({"entity":c529_name(ray.entity,names),"normal":ray.normal,"distance_m":ray.distance}))}));
            ray
        } else { None };
        if let Some(ray) = center_ray
            && ray.distance > 0.0 && ray.normal.y >= deg_to_rad(MAX_SLOPE_DEGREES).cos() {''')
floor = change(floor, '    let (mut hit, mut floor_distance, valid, line, retained_penetration) = compute(origin, radius, max_floor, max_floor)?;',
'''    let (mut hit, mut floor_distance, valid, line, retained_penetration) = compute(origin, radius, max_floor, max_floor, events)?;''')
floor = change(floor, 'if !line && threshold > EDGE_REJECT_DISTANCE',
               'if skip != "perch" && !line && threshold > EDGE_REJECT_DISTANCE')
floor = change(floor, '        let (perch, perch_distance, perch_valid, _, _) = compute(at, perch_radius,',
'''        events.push(serde_json::json!({"kind":"perch","origin":at,"radius_m":perch_radius}));
        let (perch, perch_distance, perch_valid, _, _) = compute(at, perch_radius,''')
floor = change(floor, '(max_perch - above_base).max(0.0), max_perch + radius)?;',
               '(max_perch - above_base).max(0.0), max_perch + radius, events)?;')
floor = change(floor, '    Some(((origin.y - floor_distance, hit.point1.y, hit.entity), retained_penetration))',
'''    events.push(serde_json::json!({"kind":"selected","line":line,"floor_distance_m":floor_distance,
        "hit":c529_hit(&hit,names)}));
    Some(((origin.y - floor_distance, hit.point1.y, hit.entity), retained_penetration))''')
floor = floor.replace('crate::als::movement_constants::', '')
floor = floor.replace('crate::als::params::CmcAdapterParams', 'bevy_als::als::params::CmcAdapterParams')
constants = Path('src/als/settings/movement_constants.rs').read_text()
functions = sweep + floor + source[source.index('fn deg_to_rad('):source.index('fn deg_to_rad(')+source[source.index('fn deg_to_rad('):].index('\n}')+2]
functions = re.sub(r'\bdeg_to_rad\b', 'c529_deg_to_rad', functions)
declarations = []
for key in sorted(set(re.findall(r'\b[A-Z][A-Z_]+\b', sweep + floor)) - {'IDENTITY','NEG_Y'}):
    matches = re.findall(r'pub\(crate\) const '+key+r': f32 = [^;]+;', constants)
    assert len(matches) == 1, key
    declarations.append(matches[0].replace('pub(crate) const '+key, 'const C529_'+key))
    functions = re.sub(r'\b'+key+r'\b', 'C529_'+key, functions)

driver = change(driver, '    let manifolds:Vec<_>=collisions.collisions_with(actor).flat_map(|pair| {',
'''    if frame==control.frame {
        c529_trace(&spatial,position.0,half,radius,&params.cmc,*mode,actor,&candidates,&names);
    }
    let manifolds:Vec<_>=collisions.collisions_with(actor).flat_map(|pair| {''')
path.write_text(driver+'\n'+ '\n'.join(declarations)+'\n'+functions+'\n'+(root/'floor_trace.rs').read_text())
print('Prepared instrumented copies of the live floor query with scoped ablations', flush=True)
