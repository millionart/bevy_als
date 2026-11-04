from pathlib import Path

root=Path(__file__).resolve().parent
previous=root.parent/'537'/'observer.py'
script=previous.read_text().replace('tools/tmp/bf144/537','tools/tmp/bf144/541')
script=script.replace("(root/'target_fin.inc')","(root.parent/'537'/'target_fin.inc')")
exec(compile(script,previous.name,'exec'))
path=Path('tools/tmp/bf144/541/playground_observed.rs')
driver=path.read_text()
for anchor,replacement in (
    ('    c516_write("fixture.json"',(root/'target_slope.inc').read_text()+'    c516_write("fixture.json"'),
    ('skip_floor_guard: bool, events:', 'skip_floor_guard: bool, skip_angle_guard: bool, events:'),
    ('''    let Some(floor) = c529_sweep(&spatial_query, capsule, radius, destination,
        C529_MAX_STEP_HEIGHT + C529_MAX_FLOOR_DISTANCE * 2.0, &filter, &accepts,"none",events,names)
        .filter(|hit| hit.normal1.y >= c529_deg_to_rad(C529_MAX_SLOPE_DEGREES).cos()) else {''',
     '''    let floor_probe = c529_sweep(&spatial_query, capsule, radius, destination,
        C529_MAX_STEP_HEIGHT + C529_MAX_FLOOR_DISTANCE * 2.0, &filter, &accepts,"none",events,names);
    events.push(serde_json::json!({"kind":"down_qualification","hit":floor_probe.as_ref().map(|hit|c529_hit(hit,names))}));
    let Some(floor) = floor_probe.filter(|hit| skip_angle_guard || hit.normal1.y >= c529_deg_to_rad(C529_MAX_SLOPE_DEGREES).cos()) else {'''),
    ('    for skip in [false,true] {',
     '''    for (label,skip,skip_angle) in [("actual",false,false),("no-floor-guard",true,false),
        ("no-angle-guard",false,true),("no-angle-or-floor-guard",true,true)] {'''),
    ('&candidates,&permissions,skip,&mut events,&names);','&candidates,&permissions,skip,skip_angle,&mut events,&names);'),
    ('if skip {"no-floor-guard"} else {"actual"}.to_string()','label.to_string()'),
):
    assert driver.count(anchor)==1,anchor
    driver=driver.replace(anchor,replacement)
path.write_text(driver)
print('Prepared actual steep StepDown cases and ordered guard ablations',flush=True)
