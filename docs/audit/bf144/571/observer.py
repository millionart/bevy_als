from pathlib import Path

root = Path(__file__).resolve().parent
previous = root.parent / '570' / 'observer.py'
script = previous.read_text().replace('tools/tmp/bf144/570', 'tools/tmp/bf144/571')
script = script.replace('570/initial.json', '571/initial.json')
script = script.replace('c570_install(&mut app);', 'c571_install(&mut app);')
script = script.replace("driver + (root / 'body_probe.inc').read_text()", "driver")
exec(compile(script, previous.name, 'exec'))

driver = path.read_text()
before = 'let moving = (43..62).contains(&probe.frame) ||'
after = 'let moving = probe.frame == 56 || (76..79).contains(&probe.frame) ||'
assert driver.count(before) == 1
driver = driver.replace(before, after)
anchor = 'move_dir: if moving { Vec2::Y } else { Vec2::ZERO }'
assert driver.count(anchor) == 1
driver = driver.replace(anchor, 'move_dir: if probe.frame == 56 { Vec2::Y * 0.05 } else if moving { Vec2::Y } else { Vec2::ZERO }')

a = driver.index('fn c523_begin(')
b = driver.index('fn c523_delta(', a)
anchor = 'if probe.frame != control.frame {'
assert driver[a:b].count(anchor) == 1
start = '''let relanding = probe.frame == 57 && std::env::var("ALS_C571_CASE").unwrap() == "double";
    if probe.frame != control.frame && !relanding {'''
block = driver[a:b].replace(anchor, start)
assert block.count('position.0 = air.position;') == 1
block = block.replace('position.0 = air.position;', 'position.0 = air.position - Vec3::Y * if relanding { 0.02 } else { 0.0 };')
driver = driver[:a] + block + driver[b:]
path.write_text(driver + (root / 'timer_probe.inc').read_text())
print('Prepared repeat landing and actual movement and timer phase observations', flush=True)
