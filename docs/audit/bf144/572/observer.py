from pathlib import Path

root = Path(__file__).resolve().parent
previous = root.parent / '570' / 'observer.py'
script = previous.read_text().replace('tools/tmp/bf144/570', 'tools/tmp/bf144/572')
script = script.replace('570/initial.json', '572/initial.json')
script = script.replace('c570_install(&mut app);', 'app.add_systems(Last, c572_clock.after(c523_delta));')
script = script.replace("driver + (root / 'body_probe.inc').read_text()", "driver")
exec(compile(script, previous.name, 'exec'))

driver = path.read_text()
for before, after in (
    ('gait.0 = if (40..62).contains(&probe.frame)', 'gait.0 = if (40..c572_release()).contains(&probe.frame)'),
    ('let moving = (43..62).contains(&probe.frame) || (120..160).contains(&probe.frame);',
     'let moving = probe.frame == 44 || (52..c572_release()).contains(&probe.frame);'),
    ('"rig_poses":if row["sprint"]["mixed"].as_array().is_some_and(|values|!values.is_empty())\n                && row["ordinal"].as_u64().is_some_and(|ordinal|ordinal < 50) {row["rig_snapshot"]["poses"].clone()} else {serde_json::Value::Null}',
     '"rig_poses":c572_pelvis(&row["rig_snapshot"]["poses"])'),
):
    assert driver.count(before) == 1, before
    driver = driver.replace(before, after)
assert driver.count('probe.frame >= 80') == 2
assert driver.count('probe.frame==80') == 1
driver = driver.replace('probe.frame >= 80', 'probe.frame >= 105').replace('probe.frame==80', 'probe.frame==105')
path.write_text(driver + (root / 'pose_probe.inc').read_text())
print('Prepared same-duration run-off and pose boundary recording', flush=True)
