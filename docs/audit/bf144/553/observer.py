from pathlib import Path

root = Path(__file__).resolve().parent
previous = root.parent / '537' / 'observer.py'
script = previous.read_text().replace('tools/tmp/bf144/537', 'tools/tmp/bf144/553')
script = script.replace("(root/'target_fin.inc').read_text()", "''")
exec(compile(script, previous.name, 'exec'))
path = Path('tools/tmp/bf144/553/playground_observed.rs')
driver = path.read_text()
anchor = '            c522_after.after(AlsActualVelocitySet),'
assert driver.count(anchor) == 1
driver = driver.replace(anchor, '''            c553_adjust.after(c522_before).before(AlsActualVelocitySet),
''' + anchor)
path.write_text(driver + (root / 'adjust_probe.inc').read_text())
print('Prepared real post-physics ground adjustment and isolated cap control', flush=True)
