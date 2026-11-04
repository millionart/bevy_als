from pathlib import Path

root = Path(__file__).resolve().parent
previous = root.parent / '533' / 'observer.py'
script = previous.read_text().replace('tools/tmp/bf144/533', 'tools/tmp/bf144/551')
script = script.replace("(root/'walking_probe.rs')", "(root.parent/'533'/'walking_probe.rs')")
exec(compile(script, previous.name, 'exec'))
path = Path('tools/tmp/bf144/551/playground_observed.rs')
driver = path.read_text()
for before, after in (
    ('    c516_write("fixture.json"', (root / 'target_wall.inc').read_text() + '    c516_write("fixture.json"'),
    ('position.0 = Vec3::new(-14.01, 2.9215, -15.0);', 'position.0 = Vec3::new(-13.98, 2.9215, -15.0);'),
    ('velocity.0 = Vec3::new(6.0, 0.0, 0.0);', 'velocity.0 = Vec3::new(6.5, 0.0, 0.0);'),
    ('''    c516_write(
        "walking-step.json"''', (root / 'substeps.inc').read_text() + '''    c516_write(
        "walking-step.json"'''),
    ('''fn c533_first_step(
    probe: Res<C458Probe>,''',
     '''fn c533_first_step(
    capsules:Query<&Collider>,
    permissions:Query<&bevy_als::als::mantle::AlsMantleTargetPermission>,
    probe: Res<C458Probe>,'''),
):
    assert driver.count(before) == 1, before
    driver = driver.replace(before, after)
path.write_text(driver)
print('Prepared the production two-step observer and exact no-step wall replay', flush=True)
