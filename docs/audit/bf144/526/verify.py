import gzip
import hashlib
import json
import math
import os
import re
import struct
from pathlib import Path

root = Path(__file__).resolve().parent
prior = root.parent / '525'
labels = ('edge-in', 'edge-out', 'edge-gap', 'cleared', 'restored')
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

def f32(value):
    return struct.unpack('<f', struct.pack('<f', value))[0]

initial = read(root.parent / '523' / 'initial.json')
assert read(root / 'build-edges.report.json')['success']
assert read(root / 'wall.native.json.reader.report.json')['success']
native_path = root / 'wall.native.json'
cases = {case['name']: case['frames'] for case in read(native_path)['character_animation_probe']['cases']}
inside, outside, deep, disabled = [cases[name] for name in ('edge-in','edge-out','edge-gap','edge-in-unobserved')]
motion_fields = ('movement_before_tick','movement_after_tick','actor_transform','actor_velocity_cm_s',
                 'getter_after_tick','mesh_local_pose')

def normalized_state(row):
    value = json.dumps(row['source_state_after_tick'], sort_keys=True)
    return re.sub(r'/Temp/Untitled_\d+\.Untitled:', '/Temp/Untitled.Untitled:', value)

for active, control in zip(inside, disabled, strict=True):
    assert all(active[key] == control[key] for key in motion_fields)
    assert normalized_state(active) == normalized_state(control)
    assert ('blocking_hits' in active) == (active['frame'] in frames)
    assert 'blocking_hits' not in control and 'floor_after_tick_query' not in control

def native_mode(row):
    return row['movement_after_tick']['movement_mode_name']

for case in (inside, outside, deep):
    assert case[49]['movement_before_tick']['movement_mode_name'] == 'MOVE_Falling'
    assert case[49]['world_delta_seconds'] == initial['delta_s']
assert [native_mode(inside[frame]) for frame in (49, 50, 51)] == ['MOVE_Walking'] * 3
assert [native_mode(outside[frame]) for frame in (49, 50, 51)] == ['MOVE_Falling','MOVE_Falling','MOVE_Walking']
assert outside[50]['blocking_hits'] == [] and outside[51]['movement_after_tick']['walkable_floor']
deep_pose_changed = []
for frame in (49, 50):
    assert all(inside[frame][key] == deep[frame][key] for key in motion_fields if key != 'mesh_local_pose')
    if inside[frame]['mesh_local_pose'] != deep[frame]['mesh_local_pose']:
        deep_pose_changed.append(frame)
    assert inside[frame]['blocking_hits'] == deep[frame]['blocking_hits']
    assert inside[frame]['floor_after_tick_query'] == deep[frame]['floor_after_tick_query']

first, walking, step_down = inside[49]['blocking_hits']
outer, = outside[49]['blocking_hits']
for hit in (first, outer):
    assert hit['component'] == 'AuditWall' and not hit['penetrating'] and 0 < hit['time'] < 1
    assert not hit['normals_equal'] and not hit['walkable_hit']
    assert hit['normal']['z'] > .001 and hit['impact_normal']['z'] == 0
    floor = hit['floor_at_move_stop']
    assert floor['walkable'] and not floor['line_trace']
assert first['edge_within_at_move_stop'] and first['floor_at_move_stop']['component'] == 'AuditWall'
assert not outer['edge_within_at_move_stop'] and outer['floor_at_move_stop']['component'] == 'AuditLandingDeck'

remaining = f32(f32(inside[49]['world_delta_seconds']) * f32(1.0 - first['time']))
ground_velocity = inside[49]['movement_after_tick']['velocity_cm_s']
walking_cm = walking['trace_end_cm']['x'] - walking['trace_start_cm']['x']
assert .0308 < remaining < .0309 and 649.99 < ground_velocity['x'] < 650.01
assert abs(walking_cm - remaining * ground_velocity['x']) < 1e-6
assert walking['trace_start_cm']['z'] == walking['trace_end_cm']['z']
for axis in ('x','y'):
    assert abs(walking['trace_start_cm'][axis] - first['move_stop_cm'][axis]) < 1e-7
assert walking['trace_start_cm']['z'] > first['move_stop_cm']['z']
assert step_down['walkable_hit'] and step_down['component'] == 'AuditWall'
assert step_down['trace_start_cm']['z'] - step_down['trace_end_cm']['z'] > 40
for axis in ('x','y'):
    assert step_down['trace_start_cm'][axis] == step_down['trace_end_cm'][axis]
    assert abs(step_down['trace_end_cm'][axis] - inside[49]['actor_transform']['translation'][axis]) < 1e-7
native_position = inside[49]['actor_transform']['translation']
native_position_m = [native_position['x']/100, native_position['z']/100 + 2, native_position['y']/100]

rows, traces, parameters = {}, {}, {}
for label in labels:
    functional = read(root / f'{label}-functional.json')
    assert functional['pass'] and not functional['issues'] and functional['images'] == []
    assert load(label,'process.report')['success']
    assert math.isclose(load(label,'barrier')['top_m'], 2.29 if label == 'edge-out' else 2.16, abs_tol=2e-7)
    assert math.isclose(load(label,'deck')['top'], 1.935 if label == 'edge-gap' else 1.973, abs_tol=2e-7)
    rows[label] = {f'{stage}-{frame}': load(label,f'contact-{stage}-{frame}')
                   for frame in frames for stage in ('before','after')}
    traces[label] = trace = load(label,'initial-observed')
    assert trace['position'] == initial['position_m'] and trace['step'] == initial['delta_s']
    assert trace['velocity'] == trace['initial_with_motor'] == initial['velocity_mps']
    assert trace['wall'] == 'AuditWalkOffBarrier' and trace['second_hit'] is None
    assert trace['request'] == traces['edge-in']['request']
    assert .001 < trace['normal'][1] < .707
    before = rows[label]['before-49']
    assert before['mode'] == 'Air' and not before['has_basis_displacement'] and before['manifolds'] == []
    assert before['position'] == trace['single_hit_position'] and before['planned_velocity'] == trace['single_hit_velocity']
    assert before['requested_mode'] is None and before['generated_contact']
    assert before['floor']['name'] == ('AuditLandingDeck' if label == 'edge-out' else 'AuditWalkOffBarrier')
    assert 0 < before['floor']['gap'] <= .024
    assert before['consumed_contact'] == (label != 'cleared')
    assert rows[label]['after-49']['position'] == before['position']
    parameters[label] = read(file(label, 'parameters'))

target_inside, target_outside, target_deep, cleared, restored = [rows[label] for label in labels]
assert restored == target_deep == target_inside
assert traces['restored'] == traces['cleared'] == traces['edge-gap'] == traces['edge-in']
assert target_inside['before-49'] == (cleared['before-49'] | {'consumed_contact': True})
event = [['LocomotionState(Air)', 'LocomotionState(Grounded)']]
for case in (target_inside, target_outside):
    assert case['after-49']['mode'] == 'Grounded' and case['after-49']['transitions'] == event
assert cleared['after-49']['mode'] == 'Air' and cleared['after-49']['transitions'] == []
assert cleared['after-49']['velocity'] == target_inside['after-49']['velocity']
assert cleared['after-49']['position'] == target_inside['after-49']['position']
assert cleared['after-50']['mode'] == 'Air'
target_position_m = target_inside['after-49']['position']
assert .15 < native_position_m[1] - target_position_m[1] < .153

restoration = {}
for key in parameters['edge-in']:
    old, new = [{row['ordinal']:row for row in parameters[label][key]} for label in ('edge-in','restored')]
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
cmc = 'Engine/Source/Runtime/Engine/Private/Components/CharacterMovementComponent.cpp'
motion = 'src/als/character/movement/motion.rs'
facts = []
for key, path, ranges in (
    (cmc, ue / cmc, [(5036,5078), (5653,5668), (5713,5748), (6373,6410), (7050,7057), (7313,7322), (7428,7443)]),
    (motion, Path(motion), [(2723,2747), (2775,2805), (3724,3737), (4074,4120)]),
):
    data = path.read_bytes()
    lines = data.decode().splitlines()
    facts.append(dict(key=key, **identity(data), excerpts=[
        dict(start=a, end=b, text='\n'.join(lines[a-1:b]) + '\n') for a,b in ranges]))
save('source-facts.json', facts)

inputs = {row['path'] for row in read(prior / 'summary.json')['external_source_files']}
inputs.update(str(path.relative_to(Path.cwd())) for path in (
    prior / 'prepare_reader.py', prior / 'wall.request.json', prior / 'observer.py', prior / 'summary.json',
    prior / 'AlsAuditBlockingHitObserver.h', prior / 'capture_hits.inc',
))
save('summary.json', dict(
    gate='PASS_VALID_FALLING_EDGE_WALKING_REMAINDER_AND_OUTSIDE_CONTROL', findings=['BF144-D292'],
    related_difference='BF144-D291', new_differences=1, total_differences=289, ordinals=list(frames),
    native={name: [dict(mode=native_mode(cases[name][frame]), hits=cases[name][frame].get('blocking_hits'),
                        floor=cases[name][frame].get('floor_after_tick_query'))
                   for frame in (49,50,51)] for name in ('edge-in','edge-out','edge-gap')},
    native_observer_disabled_motion_exact_frames=len(inside), source_walking_remaining_s=remaining,
    source_walking_sweep_cm=walking_cm, source_walking_expected_sweep_cm=remaining*ground_velocity['x'],
    native_inside_position_m=native_position_m, target_inside_position_m=target_position_m,
    target=rows, target_normals={label: trace['normal'] for label, trace in traces.items()},
    deep_deck_keeps_same_actual_wall_support=True, deep_deck_native_pose_changed_frames=deep_pose_changed,
    focused_restoration_exact=True,
    restored_first_late_parameter_difference=restoration, decoded=decoded, screenshots=0,
    performance_acceptance='paused',
    external_source_files=[dict(path=name, **identity(Path(name).read_bytes())) for name in sorted(inputs)]))
print('Verified native accepted edge handoff, outside-edge rejection and target post-update timing', flush=True)
