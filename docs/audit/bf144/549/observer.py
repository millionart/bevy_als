from pathlib import Path

root = Path(__file__).resolve().parent
previous = root.parent / '545' / 'observer.py'
script = previous.read_text().replace('tools/tmp/bf144/545', 'tools/tmp/bf144/549')
script = script.replace("(root.parent/'537'/'target_fin.inc').read_text()+", "")
script = script.replace('target_overhang.inc', 'target_height.inc')
exec(compile(script, previous.name, 'exec'))
path = Path('tools/tmp/bf144/549/playground_observed.rs')
driver = path.read_text()
for before, after in (
    ('near_floor_guard: bool, events:', 'omit_center_height_guard: bool, events:'),
    ('if skip {"with-near-floor-guard"} else {"actual"}', 'if skip {"without-center-height"} else {"actual"}'),
    ('''            let normal = if (hit.normal1.y > 0.0 && hit.normal1.y < c529_deg_to_rad(C529_MAX_SLOPE_DEGREES).cos())
                || (near_floor_guard && hit.normal1.y < -1e-4 && initial_floor_gap < 0.019) {''',
     '''            let normal = if hit.normal1.y > 0.0 && hit.normal1.y < c529_deg_to_rad(C529_MAX_SLOPE_DEGREES).cos() {'''),
    ('''    if (next_y - center.y).abs() > C529_MAX_STEP_HEIGHT
        || initial_floor_y.is_some_and(|initial| floor.point1.y - initial > C529_MAX_STEP_HEIGHT) {
        return Some(result);
    }''',
     '''    let center_delta = next_y-center.y;
    let impact_delta = initial_floor_y.map(|initial| floor.point1.y-initial);
    let center_exceeds = center_delta.abs() > C529_MAX_STEP_HEIGHT;
    let impact_exceeds = impact_delta.is_some_and(|delta| delta > C529_MAX_STEP_HEIGHT);
    let rejected = (center_exceeds && !omit_center_height_guard) || impact_exceeds;
    events.push(serde_json::json!({"kind":"height_guard","center_delta":center_delta,
        "impact_delta":impact_delta,"center_exceeds":center_exceeds,"impact_exceeds":impact_exceeds,
        "rejected":rejected}));
    if rejected { return Some(result); }'''),
):
    assert driver.count(before) == 1, before
    driver = driver.replace(before, after)
path.write_text(driver)
print('Prepared actual height limits and isolated center height control', flush=True)
