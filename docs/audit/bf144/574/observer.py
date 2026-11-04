from pathlib import Path

root = Path(__file__).resolve().parent
previous = root.parent / '573' / 'observer.py'
script = previous.read_text().replace('tools/tmp/bf144/573', 'tools/tmp/bf144/574')
script = script.replace('573/initial.json', '574/initial.json')
script = script.replace("(root / 'cached_acceleration.inc')", "(root.parent / '573' / 'cached_acceleration.inc')")
exec(compile(script, previous.name, 'exec'))

driver = path.read_text()
anchor = '    c573_install(&mut app);'
assert driver.count(anchor) == 1
driver = driver.replace(anchor, anchor + '\n    c574_install(&mut app);')
path.write_text(driver + (root / 'height_probe.inc').read_text())
print('Prepared same-floor pre-physics height isolation', flush=True)
