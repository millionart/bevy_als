from pathlib import Path

root = Path(__file__).resolve().parent
previous = root.parent / '566' / 'observer.py'
script = previous.read_text().replace('tools/tmp/bf144/566', 'tools/tmp/bf144/568')
script = script.replace("'second_base.inc'", "'walls.inc'")
script = script.replace('app.add_systems(First,c566_enable);', 'c568_install(&mut app);')
script = script.replace("driver + (root / 'base_probe.inc').read_text()", "driver")
script = script.replace('-13.98, 2.9215', '-13.48, 2.9215')
exec(compile(script, previous.name, 'exec'))
path.write_text(path.read_text() + (root / 'forward_probe.inc').read_text())
print('Prepared raw forward overlap and capsule cast observation', flush=True)
