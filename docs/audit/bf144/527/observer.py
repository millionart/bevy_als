from pathlib import Path

root = Path(__file__).resolve().parent
previous = root.parent / '526' / 'observer.py'
script = previous.read_text().replace('tools/tmp/bf144/526','tools/tmp/bf144/527')
exec(compile(script, previous.name, 'exec'))
path = Path('tools/tmp/bf144/527/playground_observed.rs')
driver = path.read_text()
anchor = '    c522_install(&mut app);'
assert driver.count(anchor) == 1
driver = driver.replace(anchor, anchor + '\n    app.add_systems(PostUpdate,c527_nudge.after(PhysicsSystems::Last).before(c522_before));')
path.write_text(driver + '\n' + (root / 'gap_control.rs').read_text())
print('Prepared a post-physics vertical position control before locomotion sync', flush=True)
