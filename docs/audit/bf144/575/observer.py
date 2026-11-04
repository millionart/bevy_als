from pathlib import Path

root = Path(__file__).resolve().parent
previous = root.parent / '574' / 'observer.py'
script = previous.read_text().replace('tools/tmp/bf144/574', 'tools/tmp/bf144/575')
script = script.replace('574/initial.json', '575/initial.json')
script = script.replace("(root / 'height_probe.inc')", "(root.parent / '574' / 'height_probe.inc')")
exec(compile(script, previous.name, 'exec'))

driver = path.read_text()
anchor = '    c574_install(&mut app);'
assert driver.count(anchor) == 1
driver = driver.replace(anchor, anchor + '\n    c575_install(&mut app);')
path.write_text(driver + (root / 'direction_probe.inc').read_text())
print('Prepared common world input controls after the production input producer', flush=True)
