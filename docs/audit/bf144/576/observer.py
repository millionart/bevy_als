from pathlib import Path

root = Path(__file__).resolve().parent
previous = root.parent / '575' / 'observer.py'
script = previous.read_text().replace('tools/tmp/bf144/575', 'tools/tmp/bf144/576')
script = script.replace('575/initial.json', '576/initial.json')
script = script.replace("(root / 'direction_probe.inc')", "(root.parent / '575' / 'direction_probe.inc')")
exec(compile(script, previous.name, 'exec'))

driver = path.read_text()
anchor = '    c575_install(&mut app);'
assert driver.count(anchor) == 1
driver = driver.replace(anchor, anchor + '\n    c576_install(&mut app);')
path.write_text(driver + (root / 'landing_probe.inc').read_text())
print('Prepared actual landing sweep replay and publish-boundary recording', flush=True)
