from pathlib import Path

root = Path(__file__).resolve().parent
previous = root.parent / '576' / 'observer.py'
script = previous.read_text().replace('tools/tmp/bf144/576', 'tools/tmp/bf144/577')
script = script.replace('576/initial.json', '577/initial.json')
script = script.replace("(root / 'landing_probe.inc')", "(root.parent / '576' / 'landing_probe.inc')")
exec(compile(script, previous.name, 'exec'))

driver = path.read_text()
a = driver.index('fn c576_before(')
b = driver.index('fn c576_after(', a)
probe = driver[a:b]
for before, after in (
    ('    names: Query<&Name>,', '    names: Query<&Name>,\n    geometry: Query<(&Collider, &Position, &Rotation)>,'),
    ('        let hit = hit.unwrap();', '''        let hit = hit.unwrap();
        c577_query(&spatial, &geometry, capsule, position, transform.rotation,
            direction, distance, hit, &filter, step);'''),
):
    assert probe.count(before) == 1, before
    probe = probe.replace(before, after)
path.write_text(driver[:a] + probe + driver[b:] + (root / 'geometry_probe.inc').read_text())
print('Prepared actual floor and capsule geometry with independent TOI controls', flush=True)
