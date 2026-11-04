from pathlib import Path

root = Path(__file__).resolve().parent
previous = root.parent / '533' / 'observer.py'
script = previous.read_text().replace('tools/tmp/bf144/533', 'tools/tmp/bf144/566')
script = script.replace("(root/'walking_probe.rs')", "(root.parent/'533'/'walking_probe.rs')")
exec(compile(script, previous.name, 'exec'))
path = Path('tools/tmp/bf144/566/playground_observed.rs')
driver = path.read_text()
for before, after in (
    ('    c516_write("fixture.json"', (root / 'second_base.inc').read_text() + '    c516_write("fixture.json"'),
    ('    c533_install(&mut app);', '    c533_install(&mut app);\n    app.add_systems(First,c566_enable);'),
    ('position.0 = Vec3::new(-14.01, 2.9215, -15.0);', 'position.0 = Vec3::new(-13.98, 2.9215, -15.0);'),
    ('velocity.0 = Vec3::new(6.0, 0.0, 0.0);', 'velocity.0 = Vec3::new(6.5, 0.0, 0.0);'),
):
    assert driver.count(before) == 1, before
    driver = driver.replace(before, after)
a = driver.index('fn c516_delta(')
b = driver.index('fn c516_cache(', a)
driver = driver[:a] + driver[a:b].replace('from_secs_f32(0.06)', 'from_secs_f32(0.1)') + driver[b:]
path.write_text(driver + (root / 'base_probe.inc').read_text())
print('Prepared exact first-step floor and full-frame support velocity observations', flush=True)
