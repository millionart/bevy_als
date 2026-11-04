from pathlib import Path

root = Path(__file__).resolve().parent
previous = root.parent / '523' / 'observer.py'
script = previous.read_text().replace('tools/tmp/bf144/523', 'tools/tmp/bf144/565')
script = script.replace('deck_fixture.inc', 'new_base.inc').replace("root/'deck.rs'", "root/'jump_probe.inc'")
exec(compile(script, previous.name, 'exec'))
path = Path('tools/tmp/bf144/565/playground_observed.rs')
driver = path.read_text()
for before, after in (
    ('let moving = (40..62).contains(&probe.frame) || (120..160).contains(&probe.frame);', 'let moving = false;'),
    ('sprint_held: moving, ..default()', 'sprint_held: false, jump_pressed: probe.frame==49, ..default()'),
    ('''        Name::new("AuditWalkOffPlatform"),
        Transform::from_xyz(origin.x,1.9,origin.z),
        RigidBody::Static,''',
     '''        Name::new("AuditWalkOffPlatform"),
        Transform::from_xyz(origin.x,1.9,origin.z),
        RigidBody::Kinematic,LinearVelocity::ZERO,'''),
):
    assert driver.count(before) == 1, before
    driver = driver.replace(before, after)
a = driver.index('fn c516_delta(')
b = driver.index('fn c516_cache(',a)
driver = driver[:a] + driver[a:b].replace('from_secs_f32(0.06)', 'from_secs_f32(0.05)') + driver[b:]
path.write_text(driver)
print('Prepared real Jump support selection and motor observations', flush=True)
