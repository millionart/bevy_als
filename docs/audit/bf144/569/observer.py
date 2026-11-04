from pathlib import Path

root = Path(__file__).resolve().parent
previous = root.parent / '568' / 'observer.py'
script = previous.read_text().replace('tools/tmp/bf144/568', 'tools/tmp/bf144/569')
exec(compile(script, previous.name, 'exec'))
driver = path.read_text()
a = driver.index('fn c516_delta(')
b = driver.index('fn c516_cache(', a)
assert driver[a:b].count('from_secs_f32(0.1)') == 1
driver = driver[:a] + driver[a:b].replace('from_secs_f32(0.1)', 'from_secs_f32(0.05)') + driver[b:]
a = driver.index('fn c533_first_step(')
b = driver.index('fn c533_after_transaction(', a)
probe = driver[a:b].replace('runtime.simulation_substeps, 2','runtime.simulation_substeps, 1').replace(
    'delta > params.cmc.max_simulation_time_step && params.cmc.max_simulation_iterations > 1',
    'delta <= params.cmc.max_simulation_time_step').replace(
    'params.cmc.max_simulation_time_step.min(delta * 0.5)', 'delta')
driver = driver[:a] + probe + driver[b:]
anchor = '    c568_install(&mut app);'
assert driver.count(anchor) == 1
driver = driver.replace(anchor, anchor + '''
    app.add_systems(PostUpdate, c569_direction
        .after(bevy_als::als::motion::AlsRotationResponseSet)
        .after(bevy_als::actual_motion::AlsActualMotionSet));''')
path.write_text(driver + (root / 'direction.inc').read_text())
print('Prepared one-step Walking and actual locomotion direction observations', flush=True)
