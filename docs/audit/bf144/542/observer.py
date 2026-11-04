from pathlib import Path

root=Path(__file__).resolve().parent
previous=root.parent/'537'/'observer.py'
script=previous.read_text().replace('tools/tmp/bf144/537','tools/tmp/bf144/542')
script=script.replace("(root/'target_fin.inc')","(root.parent/'537'/'target_fin.inc')")
exec(compile(script,previous.name,'exec'))
path=Path('tools/tmp/bf144/542/playground_observed.rs')
driver=path.read_text()
for anchor,replacement in (
    ('    c516_write("fixture.json"',(root.parent/'541'/'target_slope.inc').read_text()+'    c516_write("fixture.json"'),
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
anchor='    c533_install(&mut app);'
assert driver.count(anchor)==1
driver=driver.replace(anchor,anchor+chr(10)+'    c538_install(&mut app);\n    app.add_systems(Update, c542_clip.after(c538_control).before(c458_sample).before(TnuaPipelineSystems::Motors));')
solver=(root.parent/'538'/'solver_probe.rs').read_text()
anchor='            .after(c533_after_transaction)'
assert solver.count(anchor)==1
driver+=solver.replace(anchor,anchor+chr(10)+'            .before(c458_sample)')
driver+=(root/'clip_probe.rs').read_text()
path.write_text(driver)
print('Prepared the steep StepDown replay and live horizontal sweep control',flush=True)
