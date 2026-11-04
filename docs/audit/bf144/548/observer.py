from pathlib import Path

root = Path(__file__).resolve().parent
previous = root.parent / '545' / 'observer.py'
script = previous.read_text().replace('tools/tmp/bf144/545', 'tools/tmp/bf144/548')
script = script.replace('target_overhang.inc', 'target_stop.inc')
exec(compile(script, previous.name, 'exec'))
path = Path('tools/tmp/bf144/548/playground_observed.rs')
driver = path.read_text()
for before, after in (
    ('near_floor_guard: bool, events:', 'use_axis_progress: bool, events:'),
    ('if skip {"with-near-floor-guard"} else {"actual"}', 'if skip {"with-axis-progress"} else {"actual"}'),
    ('''            let normal = if (hit.normal1.y > 0.0 && hit.normal1.y < c529_deg_to_rad(C529_MAX_SLOPE_DEGREES).cos())
                || (near_floor_guard && hit.normal1.y < -1e-4 && initial_floor_gap < 0.019) {''',
     '''            let normal = if hit.normal1.y > 0.0 && hit.normal1.y < c529_deg_to_rad(C529_MAX_SLOPE_DEGREES).cos() {'''),
    ('''            if progressed.length_squared() <= f32::EPSILON { return Some(result); }''',
     '''            let axis_control = use_axis_progress && c529_name(hit.entity,names)==Some("AuditStepStop");
            let rejected = if axis_control { advance.x <= 0.0 } else { progressed.length_squared() <= f32::EPSILON };
            events.push(serde_json::json!({"kind":"step_progress","advance":advance,"total":progressed,
                "length_sq":progressed.length_squared(),"rejected":rejected}));
            if rejected { return Some(result); }'''),
):
    assert driver.count(before) == 1, before
    driver = driver.replace(before, after)
path.write_text(driver)
print('Prepared the original progress comparison and fixture-specific axial zero comparison', flush=True)
