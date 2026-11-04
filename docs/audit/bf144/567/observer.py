from pathlib import Path

root = Path(__file__).resolve().parent
previous = root.parent / '566' / 'observer.py'
script = previous.read_text().replace('tools/tmp/bf144/566', 'tools/tmp/bf144/567')
script = script.replace('app.add_systems(First,c566_enable);', 'c567_install(&mut app);')
script = script.replace("driver + (root / 'base_probe.inc').read_text()", "driver")
exec(compile(script, previous.name, 'exec'))

driver = path.read_text()
old = 'Name::new("AuditWalkOffPlatform"),\n        Transform::from_xyz(origin.x,1.9,origin.z),\n        RigidBody::Static,'
assert driver.count(old) == 1
driver = driver.replace(old, old.replace('RigidBody::Static,', 'RigidBody::Kinematic,LinearVelocity::ZERO,'))
path.write_text(driver + (root / 'late_base.inc').read_text())
print('Prepared observations bracketing the real post-physics base follow', flush=True)
