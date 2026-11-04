import gzip
import hashlib
import json
import math
import os
import re
from pathlib import Path

root = Path(__file__).resolve().parent
prior = root.parent / '524'
labels = ('plane', 'cleared', 'restored')
frames = (49, 50)

def raw(path):
    return path.read_bytes() if path.exists() else gzip.decompress(path.with_suffix('.json.gz').read_bytes())

def read(path):
    return json.loads(raw(path))

def identity(data):
    return dict(bytes=len(data), sha256=hashlib.sha256(data).hexdigest())

def save(name, result):
    (root / name).write_text(json.dumps(result, indent=2) + '\n')

def file(label, name):
    return root / f'runtime-{label}' / f'{name}.json'

def load(label, name):
    return read(file(label, name))

initial = read(root.parent / '523' / 'initial.json')
assert read(root / 'build-blocking-hits.report.json')['success']
assert read(root / 'wall.native.json.reader.report.json')['success']
native_path = root / 'wall.native.json'
cases = {case['name']: case['frames'] for case in read(native_path)['character_animation_probe']['cases']}
observed, disabled = (cases[name] for name in ('plane','plane-unobserved'))
motion_fields = ('movement_before_tick','movement_after_tick','actor_transform','actor_velocity_cm_s',
                 'getter_after_tick','mesh_local_pose')
def normalized_state(row):
    value = json.dumps(row['source_state_after_tick'], sort_keys=True)
    return re.sub(r'/Temp/Untitled_\d+\.Untitled:', '/Temp/Untitled.Untitled:', value)

for active, control in zip(observed, disabled, strict=True):
    assert all(active[key] == control[key] for key in motion_fields)
    assert normalized_state(active) == normalized_state(control)
    assert ('blocking_hits' in active) == (active['frame'] in frames)
    assert 'blocking_hits' not in control and 'floor_after_tick_query' not in control
native = [dict(
    mode=observed[frame]['movement_after_tick']['movement_mode_name'],
    walkable_floor=observed[frame]['movement_after_tick']['walkable_floor'],
    hits=observed[frame]['blocking_hits'],
    floor_at_end=observed[frame]['floor_after_tick_query'],
) for frame in frames]
assert native[0]['mode'] == 'MOVE_Falling' and not native[0]['walkable_floor']
assert native[1]['mode'] == 'MOVE_Walking' and native[1]['walkable_floor']
wall, = native[0]['hits']
assert wall['component'] == 'AuditWall' and not wall['penetrating'] and wall['time'] == 0
assert wall['location_cm'] != wall['trace_start_cm']
assert .0001 < wall['normal']['z'] < .001
assert .0001 < wall['impact_normal']['z'] < .001 and wall['normals_equal'] and not wall['walkable_hit']
assert abs(wall['normal']['z'] - math.sin(math.radians(.03))) < 1e-8
assert not wall['edge_within_at_hit']
assert math.hypot(*(wall['trace_start_cm'][axis] - wall['impact_point_cm'][axis]
                    for axis in ('x','y'))) > 30.0 - .15
for floor in (wall['floor_at_hit'], native[0]['floor_at_end']):
    assert floor['walkable'] and not floor['line_trace'] and floor['component'] == 'AuditLandingDeck'
assert .013 < native[0]['floor_at_end']['floor_dist_cm'] / 100 < .014
assert any(hit['component'] == 'AuditLandingDeck' for hit in native[1]['hits'])
assert .013 < observed[49]['actor_transform']['translation']['z'] / 100 + 2 - .9 - initial['deck_top_m'] < .014
assert observed[49]['world_delta_seconds'] == initial['delta_s']

rows, first, parameters = {}, {}, {}
for label in labels:
    functional = read(root / f'{label}-functional.json')
    assert functional['pass'] and not functional['issues'] and functional['images'] == []
    assert load(label,'process.report')['success']
    assert math.isclose(load(label,'barrier')['pitch_source_degrees'], -.03, abs_tol=1e-8)
    rows[label] = {f'{stage}-{frame}': load(label,f'contact-{stage}-{frame}')
                   for frame in frames for stage in ('before','after')}
    first[label] = trace = load(label,'initial-observed')
    assert trace['position'] == initial['position_m'] and trace['step'] == initial['delta_s']
    assert trace['velocity'] == trace['initial_with_motor'] == initial['velocity_mps']
    assert trace['wall'] == 'AuditWalkOffBarrier' and trace['second_hit'] is None
    assert .0001 < trace['normal'][1] < .001 and abs(trace['normal'][1] - wall['normal']['z']) < 1e-6
    before = rows[label]['before-49']
    assert before['mode'] == 'Air' and not before['has_basis_displacement']
    assert before['position'] == trace['single_hit_position'] and before['planned_velocity'] == trace['single_hit_velocity']
    assert before['manifolds'] == [] and before['requested_mode'] is None and before['generated_contact']
    assert before['floor']['name'] == 'AuditLandingDeck' and .013 < before['floor']['gap'] < .014
    assert before['consumed_contact'] == (label != 'cleared')
    parameters[label] = read(file(label, 'parameters'))

plane, cleared, restored = [rows[label] for label in labels]
assert restored == plane and first['restored'] == first['cleared'] == first['plane']
assert plane['before-49'] == (cleared['before-49'] | {'consumed_contact': True})
assert plane['after-49']['position'] == cleared['after-49']['position']
event = [['LocomotionState(Air)', 'LocomotionState(Grounded)']]
assert plane['after-49']['mode'] == 'Grounded' and plane['after-49']['transitions'] == event
assert plane['after-49']['velocity'][1] == 0 and plane['after-49']['has_base']
assert cleared['after-49']['mode'] == 'Air' and cleared['after-49']['transitions'] == []
assert cleared['after-49']['velocity'] == plane['before-49']['planned_velocity']
assert cleared['before-50']['mode'] == 'Grounded' and cleared['after-50']['transitions'] == event

restoration = {}
for key in parameters['plane']:
    old, new = [{row['ordinal']:row for row in parameters[label][key]} for label in ('plane','restored')]
    assert old.keys() == new.keys()
    restoration[key] = next((ordinal for ordinal in sorted(old) if old[ordinal] != new[ordinal]), None)
    assert restoration[key] is None or restoration[key] > max(frames)
for label, records in parameters.items():
    after = {row['ordinal']:row for row in records['after']}
    for frame in frames:
        row = after[frame]
        assert row['gameplay'] == rows[label][f'after-{frame}']['mode']
        assert row['graph_tag'] == 'Als.LocomotionMode.' + ('InAir' if row['gameplay'] == 'Air' else 'Grounded')
        assert row['graph_error'] is None

decoded = {}
for path in [native_path] + [file(label, name) for label in labels for name in ('parameters','report')]:
    data = raw(path)
    archive = path.with_suffix('.json.gz')
    archive.write_bytes(gzip.compress(data, compresslevel=6, mtime=0))
    assert gzip.decompress(archive.read_bytes()) == data
    decoded[str(path.relative_to(root))] = identity(data)
    if path.exists():
        path.unlink()
    print('Verified gzip ' + str(path.relative_to(root)), flush=True)

ue = Path(os.environ['ALS_UE_SOURCE_ROOT'])
engine = 'Engine/Source/Runtime/Engine/Private'
cmc = engine + '/Components/CharacterMovementComponent.cpp'
vector = 'Engine/Source/Runtime/Core/Public/Math/Vector.h'
motion = 'src/als/character/movement/motion.rs'
facts = []
for key, path, ranges in (
    (cmc, ue / cmc, [(101,101), (5047,5083), (7050,7057), (7313,7322), (7428,7443)]),
    (vector, ue / vector, [(371,371), (1598,1603)]),
    (engine + '/Components/PrimitiveComponent.cpp', ue / engine / 'Components/PrimitiveComponent.cpp', [(3095,3099), (3440,3452), (3536,3553), (3588,3605)]),
    (engine + '/Actor.cpp', ue / engine / 'Actor.cpp', [(3525,3555)]),
    (engine + '/Collision/CollisionConversions.cpp', ue / engine / 'Collision/CollisionConversions.cpp', [(417,445)]),
    (motion, Path(motion), [(2723,2746), (4074,4100)]),
):
    data = path.read_bytes()
    lines = data.decode().splitlines()
    facts.append(dict(key=key, **identity(data), excerpts=[
        dict(start=a, end=b, text='\n'.join(lines[a-1:b]) + '\n') for a,b in ranges]))
save('source-facts.json', facts)

inputs = {row['path'] for row in read(prior / 'summary.json')['external_source_files']}
inputs.update(str(path.relative_to(Path.cwd())) for path in (
    prior / 'prepare_reader.py', prior / 'wall.request.json', prior / 'observer.py', prior / 'summary.json',
))
save('summary.json', dict(
    gate='PASS_ACTUAL_PLANAR_WALL_HIT_REJECTS_NATIVE_LANDING_FALLBACK', new_differences=0,
    total_differences=288, related_difference='BF144-D291', ordinals=list(frames),
    native=native, native_observer_disabled_motion_exact_frames=len(observed),
    target=rows, target_normal=first['plane']['normal'], focused_restoration_exact=True,
    restored_first_late_parameter_difference=restoration, decoded=decoded, screenshots=0,
    performance_acceptance='paused',
    external_source_files=[dict(path=name, **identity(Path(name).read_bytes())) for name in sorted(inputs)]))
print('Verified actual wall hit normals, nonintrusive native observation and live landing control', flush=True)
