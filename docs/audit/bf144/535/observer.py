from pathlib import Path

root=Path(__file__).resolve().parent
previous=root.parent/'534'/'observer.py'
script=previous.read_text().replace('tools/tmp/bf144/534','tools/tmp/bf144/535')
exec(compile(script,previous.name,'exec'))
path=Path('tools/tmp/bf144/535/playground_observed.rs')
driver=path.read_text()
for anchor,replacement in (
    ('    c533_install(&mut app);','    app.add_observer(c535_scale_spawn);\n    c533_install(&mut app);'),
    ('ground_probe.expected_center_y = Some(point.transform.translation.y);',
     'ground_probe.expected_center_y = Some(point.transform.translation.y - 0.63);'),
    ('let capsule_bottom = character.translation.y - capsule_half_height;',
     'let capsule_bottom = character.translation.y - capsule_half_height * character.scale.y;'),
    ('let moving = (40..62).contains(&probe.frame)',
     'let moving = (43..62).contains(&probe.frame)'),
    ('if average - floor_distance + perch_distance >= max_perch { floor_distance = average; }',
     'if skip != "clamp" && average - floor_distance + perch_distance >= max_perch { floor_distance = average; }'),
    ('for skip in ["none", "perch"]', 'for skip in ["none", "perch", "clamp"]'),
):
    assert driver.count(anchor)==1,anchor
    driver=driver.replace(anchor,replacement)
a=driver.index('fn c533_start(')
b=driver.index('fn c533_floor_read(',a)
driver=driver[:a]+(root/'scaled_start.inc').read_text()+'\n'+driver[b:]
a=driver.index('fn c533_first_step(')
probe=driver[a:].replace('params.crouch.standing_capsule_half_height','params.crouch.standing_capsule_half_height * 0.3')
probe=probe.replace('params.crouch.standing_capsule_radius','params.crouch.standing_capsule_radius * 0.3')
probe=probe.replace('    assert_eq!(runtime.simulation_substeps, 2);\n    assert!(runtime.grounded_movement_delta.is_some());\n','')
path.write_text(driver[:a]+probe+'\n'+(root/'scaled_spawn.inc').read_text())
print('Prepared real scaled Walking fixture and clamp-only query control',flush=True)
