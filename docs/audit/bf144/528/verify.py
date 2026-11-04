import gzip
import hashlib
import json
import math
import os
import re
from pathlib import Path

root = Path(__file__).resolve().parent
prior = root.parent / '527'
labels = ('top-215','top-215-raised')

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

assert read(root / 'target_identity-before.report.json')['success']
assert read(root / 'target_identity-after.report.json')['success']
target, traces = {}, {}
for label in labels:
    functional = read(root / f'{label}-functional.json')
    assert functional['pass'] and not functional['issues'] and functional['images'] == []
    assert load(label,'process.report')['success']
    assert math.isclose(load(label,'barrier')['top_m'], 2.215, abs_tol=2e-7)
    assert math.isclose(load(label,'deck')['top'], 1.978 if label.endswith('raised') else 1.973, abs_tol=2e-7)
    traces[label] = trace = load(label,'initial-observed')
    assert trace['wall'] == 'AuditWalkOffBarrier' and trace['second_hit'] is None
    before = load(label,'contact-before-49')
    after = load(label,'contact-after-49')
    assert before['mode'] == 'Air' and not before['has_basis_displacement'] and before['manifolds'] == []
    assert before['generated_contact'] and before['consumed_contact'] and before['requested_mode'] is None
    assert before['position'] == trace['single_hit_position'] == after['position']
    assert before['planned_velocity'] == trace['single_hit_velocity'] == after['velocity']
    assert before['floor']['name'] == 'AuditLandingDeck'
    assert before['floor']['impact'] < before['floor']['lower_hemisphere']
    assert after['mode'] == ('Grounded' if label.endswith('raised') else 'Air')
    assert after['has_base'] == label.endswith('raised')
    rows = {row['ordinal']: row for row in load(label,'parameters')['after']}
    assert rows[49]['gameplay'] == after['mode'] and rows[49]['graph_error'] is None
    target[label] = dict(before=before, after=after, modes=[rows[frame]['gameplay'] for frame in (49,50,51)])
assert traces[labels[0]] == traces[labels[1]]
assert raw(file(labels[0],'stage-after')) == raw(file(labels[1],'stage-after'))
assert raw(file(labels[0],'parameters')) == raw(prior / 'runtime-top-215/parameters.json')
base, raised = [target[label]['before'] for label in labels]
assert all(base[key] == raised[key] for key in base if key != 'floor')
assert 0 < raised['floor']['gap'] < .024 < base['floor']['gap']
assert abs(raised['floor']['impact'] - base['floor']['impact'] - .005) < 2e-7

assert read(root / 'build-floor-hit.report.json')['success']
assert read(root / 'wall.native.json.reader.report.json')['success']
native_path = root / 'wall.native.json'
cases = {case['name']:case['frames'] for case in read(native_path)['character_animation_probe']['cases']}
obs, unobs = [cases[name] for name in ('top-215-raised','top-215-raised-unobserved')]

def normalized_state(row):
    value = json.dumps(row['source_state_after_tick'], sort_keys=True)
    return re.sub(r'/Temp/Untitled_\d+\.Untitled:', '/Temp/Untitled.Untitled:', value)

for observed, control in zip(obs, unobs, strict=True):
    for key in ('movement_before_tick','movement_after_tick','actor_transform','actor_velocity_cm_s',
                'getter_after_tick','mesh_local_pose'):
        assert observed[key] == control[key]
    assert normalized_state(observed) == normalized_state(control)
    assert ('landed_events' in observed) == (observed['frame'] in (49,50))
    assert 'landed_events' not in control and 'blocking_hits' not in control

native = {}
for label in ('top-205','top-215','top-225','top-215-raised'):
    frame = cases[label][49]
    assert frame['movement_before_tick']['movement_mode_name'] == 'MOVE_Falling'
    first = frame['blocking_hits'][0]
    assert first['component'] == 'AuditWall' and not first['walkable_hit']
    assert not first['normals_equal'] and first['edge_within_at_move_stop']
    floor = first['floor_at_move_stop']
    assert floor['walkable'] and not floor['line_trace']
    native[label] = dict(
        modes=[cases[label][i]['movement_after_tick']['movement_mode_name'] for i in (49,50,51)],
        first_hit=first, actual_landed_events=frame['landed_events'])

def f32(value):
    import struct
    return struct.unpack('<f', struct.pack('<f', value))[0]

rejected = native['top-215']
first = rejected['first_hit']
floor = first['floor_at_move_stop']
hit = floor['floor_hit']
assert rejected['modes'] == ['MOVE_Falling','MOVE_Falling','MOVE_Walking']
assert rejected['actual_landed_events'] == [] and cases['top-215'][50]['landed_events'] == []
assert floor['component'] == 'AuditWall' and floor['query_movement_mode'] == 'MOVE_Falling'
assert hit['blocking'] and not hit['penetrating'] and hit['walkable'] and hit['edge_within']
assert not hit['below_lower_hemisphere'] and not floor['valid_landing_spot']
rejection_height_cm = hit['impact_height_cm'] - hit['lower_hemisphere_cm']
assert .45 < rejection_height_cm < .47

for label, component in (('top-205','AuditWall'), ('top-225','AuditLandingDeck'), ('top-215-raised','AuditLandingDeck')):
    record = native[label]
    landed, = record['actual_landed_events']
    floor = record['first_hit']['floor_at_move_stop']
    assert record['modes'] == ['MOVE_Walking']*3
    assert floor['valid_landing_spot'] and floor['component'] == landed['component'] == component
    assert landed['blocking'] and not landed['penetrating']
    assert landed['walkable'] and landed['below_lower_hemisphere'] and landed['edge_within']
    assert landed['impact_point_cm'] == floor['floor_hit']['impact_point_cm']
    assert all(abs(landed['location_cm'][axis] - floor['floor_hit']['location_cm'][axis]) < 1e-5
               for axis in ('x','y','z'))
    row = cases[label][49]
    walking = row['blocking_hits'][1]
    remaining = f32(f32(row['world_delta_seconds']) * f32(1-record['first_hit']['time']))
    sweep = walking['trace_end_cm']['x'] - walking['trace_start_cm']['x']
    assert walking['trace_start_cm']['z'] == walking['trace_end_cm']['z']
    assert abs(sweep - remaining * row['movement_after_tick']['velocity_cm_s']['x']) < 1e-6
    record.update(walking_remaining_s=remaining, walking_sweep_cm=sweep)

raised_first = native['top-215-raised']['first_hit']
assert all(first[key] == raised_first[key] for key in first if key not in ('floor_at_hit','floor_at_move_stop'))
assert target['top-215']['modes'] == ['Air','Air','Grounded']
assert target['top-215-raised']['modes'][0] == 'Grounded'

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
actor = engine + '/Character.cpp'
motion = 'src/als/character/movement/motion.rs'
facts = []
for key, path, ranges in (
    (cmc, ue / cmc, [(5061,5076), (6373,6383), (7049,7056), (7111,7143), (7313,7322), (7374,7424)]),
    (actor, ue / actor, [(300,306)]),
    (motion, Path(motion), [(3875,3914), (4074,4120)]),
):
    data = path.read_bytes()
    lines = data.decode().splitlines()
    facts.append(dict(key=key, **identity(data), excerpts=[
        dict(start=a,end=b,text='\n'.join(lines[a-1:b])+'\n') for a,b in ranges]))
save('source-facts.json', facts)

inputs = {row['path'] for row in read(prior / 'summary.json')['external_source_files']}
inputs.update(str(path.relative_to(Path.cwd())) for path in (
    prior / 'wall.request.json', prior / 'summary.json', prior / 'target-binary.json',
    prior / 'runtime-top-215/parameters.json.gz', root.parent / '526' / 'reader-source.json',
))
save('summary.json', dict(
    gate='PASS_SECONDARY_FLOOR_LOWER_HEMISPHERE_AND_ACTUAL_LANDED_EVENT',
    new_differences=0,total_differences=289,related_difference='BF144-D292',
    ordinals=[49,50,51], native=native, target=target,
    rejected_floor_above_lower_hemisphere_cm=rejection_height_cm,
    native_observer_disabled_motion_exact_frames=len(obs),
    baseline_previous_target_parameter_report_byte_exact=True, decoded=decoded,
    screenshots=0,performance_acceptance='paused',
    external_source_files=[dict(path=name, **identity(Path(name).read_bytes())) for name in sorted(inputs)]))
print('Verified the secondary hemisphere rejection and actual accepted floor event', flush=True)
