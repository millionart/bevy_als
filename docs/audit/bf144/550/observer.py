from pathlib import Path

root = Path(__file__).resolve().parent
previous = root.parent / '545' / 'observer.py'
script = previous.read_text().replace('tools/tmp/bf144/545', 'tools/tmp/bf144/550')
script = script.replace("(root.parent/'537'/'target_fin.inc').read_text()+", "")
script = script.replace('target_overhang.inc', 'target_roof.inc')
exec(compile(script, previous.name, 'exec'))
path = Path('tools/tmp/bf144/550/playground_observed.rs')
driver = path.read_text()
for before, after in (
    ('near_floor_guard: bool, events:', 'use_swept_stop: bool, events:'),
    ('if skip {"with-near-floor-guard"} else {"actual"}', 'if skip {"with-swept-stop"} else {"actual"}'),
    ('''            let normal = if (hit.normal1.y > 0.0 && hit.normal1.y < c529_deg_to_rad(C529_MAX_SLOPE_DEGREES).cos())
                || (near_floor_guard && hit.normal1.y < -1e-4 && initial_floor_gap < 0.019) {''',
     '''            let normal = if hit.normal1.y > 0.0 && hit.normal1.y < c529_deg_to_rad(C529_MAX_SLOPE_DEGREES).cos() {'''),
    ('''    let next_y = destination.y - floor.distance + clearance;''',
     '''    let target_y = destination.y - floor.distance + clearance;
    let down_distance = C529_MAX_STEP_HEIGHT + C529_MAX_FLOOR_DISTANCE * 2.0;
    let swept_y = destination.y - bevy_als::als::motion::movement_sweep_travel(down_distance, floor.distance);
    let next_y = if use_swept_stop { swept_y } else { target_y };'''),
    ('''    if initial_floor_y.is_some() && spatial_query.shape_intersections(capsule, destination.with_y(next_y),
        rotation, &filter).into_iter().any(accepts) { return Some(result); }''',
     '''    if initial_floor_y.is_some() {
        let blockers:Vec<_> = spatial_query.shape_intersections(capsule, destination.with_y(next_y),
            rotation, &filter).into_iter().filter(|entity| accepts(*entity))
            .map(|entity| c529_name(entity,names)).collect();
        events.push(serde_json::json!({"kind":"placement_overlap","selected_y":next_y,
            "target_y":target_y,"swept_y":swept_y,"blockers":blockers}));
        if !blockers.is_empty() { return Some(result); }
    }'''),
):
    assert driver.count(before) == 1, before
    driver = driver.replace(before, after)
path.write_text(driver)
print('Prepared production placement and the actual down-sweep stop counterfactual', flush=True)
