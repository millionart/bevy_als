from pathlib import Path

root = Path(__file__).resolve().parent
previous = root.parent / '572' / 'observer.py'
script = previous.read_text().replace('tools/tmp/bf144/572', 'tools/tmp/bf144/573')
script = script.replace('572/initial.json', '573/initial.json')
exec(compile(script, previous.name, 'exec'))

driver = path.read_text()
anchor = 'app.add_systems(Last, c572_clock.after(c523_delta));'
assert driver.count(anchor) == 1
driver = driver.replace(anchor, anchor + '\n    c573_install(&mut app);')
path.write_text(driver + (root / 'cached_acceleration.inc').read_text())
print('Prepared before-refresh and actual Walking transaction acceleration observations', flush=True)
