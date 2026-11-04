from pathlib import Path

root = Path(__file__).resolve().parent
previous = root.parent / '545' / 'observer.py'
script = previous.read_text().replace('tools/tmp/bf144/545', 'tools/tmp/bf144/547')
script = script.replace('target_overhang.inc', 'target_edge.inc')
exec(compile(script, previous.name, 'exec'))
path = Path('tools/tmp/bf144/547/playground_observed.rs')
driver = path.read_text()
for before, after in (
    ('near_floor_guard: bool, events:', 'use_edge_face: bool, events:'),
    ('if skip {"with-near-floor-guard"} else {"actual"}', 'if skip {"with-edge-face"} else {"actual"}'),
    ('''            let normal = if (hit.normal1.y > 0.0 && hit.normal1.y < c529_deg_to_rad(C529_MAX_SLOPE_DEGREES).cos())
                || (near_floor_guard && hit.normal1.y < -1e-4 && initial_floor_gap < 0.019) {''',
     '''            let edge_face_is_wall = use_edge_face && c529_name(hit.entity,names)==Some("AuditStepEdge");
            let normal = if hit.normal1.y > 0.0
                && (hit.normal1.y < c529_deg_to_rad(C529_MAX_SLOPE_DEGREES).cos() || edge_face_is_wall) {'''),
):
    assert driver.count(before) == 1, before
    driver = driver.replace(before, after)
path.write_text(driver)
print('Prepared production contact qualification and the edge face counterfactual', flush=True)
