import re
from pathlib import Path

root = Path(__file__).resolve().parent
previous = root.parent / '559' / 'observer.py'
script = previous.read_text().replace('tools/tmp/bf144/559', 'tools/tmp/bf144/561')
script = script.replace("'corner_fixture.inc'", "'corner_fixture.inc'")
script = script.replace("'corner_probe.inc'", "'corner_probe.inc'")
exec(compile(script, previous.name, 'exec'))
path = Path('tools/tmp/bf144/561/playground_observed.rs')
driver = path.read_text()
anchor = '        schedule.add_systems(c523_before.after(c516_before).before(transaction));'
assert driver.count(anchor) == 1
driver = driver.replace(anchor, anchor + '''
        schedule.add_systems(c560_replay.after(c523_before).before(transaction));
        schedule.add_systems(c560_handoff.after(c516_after).before(c458_sample).before(TnuaPipelineSystems::Motors));''')
source = Path('src/als/character/movement/motion.rs').read_text()
helpers = source[source.index('fn grounded_movement_transaction('):source.index('fn walking_movement_transaction(')]
helpers += source[source.index('fn floor_sweep_hit('):source.index('pub fn find_ground_floor(')]
start = source.index('fn deg_to_rad(')
helpers += source[start:source.index('\n}', start) + 2]
helpers = helpers.replace('crate::als::params::', 'bevy_als::als::params::')
for name in ('find_ground_floor', 'character_blocks_collider', 'movement_sweep_travel'):
    helpers = re.sub(r'\b' + name + r'\b', 'bevy_als::als::motion::' + name, helpers)
for name in ('ALS_CHARACTER_FLOOR_CLEARANCE', 'ALS_CHARACTER_TNUA_FLOAT_EPSILON'):
    helpers = re.sub(r'\b' + name + r'\b', 'bevy_als::als::character::' + name, helpers)
for name in ('grounded_movement_transaction', 'adjust_walking_floor_height', 'floor_sweep_hit', 'deg_to_rad'):
    helpers = re.sub(r'\b' + name + r'\b', 'c560_' + name, helpers)
constants = Path('src/als/settings/movement_constants.rs').read_text()
for name in ('MAX_STEP_HEIGHT', 'MAX_FLOOR_DISTANCE', 'MIN_FLOOR_DISTANCE', 'MAX_SLOPE_DEGREES',
             'EDGE_REJECT_DISTANCE', 'SWEEP_EDGE_MIN_RADIUS'):
    constant, = re.findall(r'const ' + name + r':[^\n]+', constants)
    helpers = constant + '\n' + helpers
path.write_text(driver + '\n' + helpers + '\n' + (root / 'handoff_probe.inc').read_text())
print('Prepared the actual Walking queries and handoff-only live counterfactual', flush=True)
