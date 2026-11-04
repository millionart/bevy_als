import gzip
import hashlib
import json
import math
import os
import struct
from pathlib import Path

root = Path(__file__).resolve().parent
prior = root.parent / '526'
heights = (190, 205, 215, 225, 235, 245)
matrix = tuple(f'top-{height}' for height in heights)
controls = ('top-205-lower','top-190-raise','top-205-restored')
threshold = .024

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
assert read(root / 'wall.package.json') == read(prior / 'wall.package.json')
assert read(root / 'wall.native.json.reader.report.json')['success']
assert read(root / 'binary_identity-before.report.json')['success']
assert read(root / 'binary_identity-after.report.json')['success']
assert read(root / 'compile-gap-control.report.json')['success']
native_path = root / 'wall.native.json'
native_cases = read(native_path)['character_animation_probe']['cases']
native = {}
for case, label in zip(native_cases, matrix, strict=True):
    assert case['name'] == label
    frame = case['frames'][49]
    assert frame['movement_before_tick']['movement_mode_name'] == 'MOVE_Falling'
    assert frame['world_delta_seconds'] == initial['delta_s']
    first = frame['blocking_hits'][0]
    assert first['component'] == 'AuditWall' and not first['penetrating']
    assert not first['walkable_hit'] and not first['normals_equal'] and first['normal']['z'] > 0
    floor = first['floor_at_move_stop']
    assert floor['walkable'] and not floor['line_trace']
    native[label] = dict(
        modes=[case['frames'][i]['movement_after_tick']['movement_mode_name'] for i in (49,50,51)],
        first_hit=first,
        q49_hit_count=len(frame['blocking_hits']),
    )
    if native[label]['modes'][0] == 'MOVE_Walking':
        assert first['edge_within_at_move_stop'] and len(frame['blocking_hits']) == 3
        second = frame['blocking_hits'][1]
        assert second['trace_start_cm']['z'] == second['trace_end_cm']['z']
        remaining = f32(f32(initial['delta_s']) * f32(1 - first['time']))
        sweep = second['trace_end_cm']['x'] - second['trace_start_cm']['x']
        assert abs(sweep - remaining * frame['movement_after_tick']['velocity_cm_s']['x']) < 1e-6
        native[label].update(walking_remaining_s=remaining, walking_sweep_cm=sweep)

assert [native[label]['modes'][0] for label in matrix] == [
    'MOVE_Walking','MOVE_Walking','MOVE_Falling','MOVE_Walking','MOVE_Falling','MOVE_Falling']
assert [native[label]['first_hit']['edge_within_at_move_stop'] for label in matrix] == [True]*4 + [False]*2
assert all(native[label]['first_hit']['floor_at_move_stop']['component'] == 'AuditWall' for label in matrix[:3])
assert all(native[label]['first_hit']['floor_at_move_stop']['component'] == 'AuditLandingDeck' for label in matrix[3:])
assert native['top-190']['first_hit']['floor_at_move_stop']['floor_dist_cm'] / 100 < threshold
assert native['top-205']['first_hit']['floor_at_move_stop']['floor_dist_cm'] / 100 > threshold

target, traces, parameters = {}, {}, {}
for label in matrix + controls:
    functional = read(root / f'{label}-functional.json')
    assert functional['pass'] and not functional['issues'] and functional['images'] == []
    assert load(label,'process.report')['success']
    height = int(label.split('-')[1]) / 10
    assert math.isclose(load(label,'barrier')['top_m'], 2+height/100, abs_tol=2e-7)
    assert math.isclose(load(label,'deck')['top'], 1.973, abs_tol=2e-7)
    traces[label] = trace = load(label,'initial-observed')
    assert trace['position'] == initial['position_m'] and trace['step'] == initial['delta_s']
    assert trace['velocity'] == trace['initial_with_motor'] == initial['velocity_mps']
    assert trace['wall'] == 'AuditWalkOffBarrier' and trace['second_hit'] is None
    before = load(label,'contact-before-49')
    after = load(label,'contact-after-49')
    assert before['mode'] == 'Air' and not before['has_basis_displacement']
    assert before['manifolds'] == [] and before['requested_mode'] is None
    assert before['generated_contact'] and before['consumed_contact']
    assert before['planned_velocity'] == trace['single_hit_velocity']
    assert before['floor']['impact'] < before['floor']['lower_hemisphere']
    assert after['position'] == before['position']
    assert after['mode'] == ('Grounded' if before['floor']['gap'] <= threshold else 'Air')
    assert after['has_base'] == (after['mode'] == 'Grounded')
    if label in matrix or label.endswith('restored'):
        assert before['position'] == trace['single_hit_position']
    parameters[label] = read(file(label, 'parameters'))
    after_rows = {row['ordinal']: row for row in parameters[label]['after']}
    modes = [after_rows[frame]['gameplay'] for frame in (49,50,51)]
    assert modes[0] == after['mode']
    for frame in (49,50,51):
        row = after_rows[frame]
        assert row['graph_tag'] == 'Als.LocomotionMode.' + ('InAir' if row['gameplay'] == 'Air' else 'Grounded')
        assert row['graph_error'] is None
    target[label] = dict(before=before, after=after, modes=modes)

assert [target[label]['after']['mode'] for label in matrix] == ['Grounded'] + ['Air']*5
assert target['top-205']['modes'] == target['top-225']['modes'] == ['Air','Air','Grounded']
assert all(target[label]['before']['floor']['name'] == 'AuditWalkOffBarrier' for label in matrix[:2])
assert all(target[label]['before']['floor']['name'] == 'AuditLandingDeck' for label in matrix[2:])
control_results = {}
for normal, adjusted, expected, shift in (
    ('top-205','top-205-lower','Grounded',-.003), ('top-190','top-190-raise','Air',.0045)):
    assert traces[normal] == traces[adjusted]
    assert raw(file(normal,'stage-after')) == raw(file(adjusted,'stage-after'))
    before, changed = [target[label]['before'] for label in (normal,adjusted)]
    record = load(adjusted,'gap-control')
    assert record['ordinal'] == 49 and record['shift_m'] == f32(shift)
    assert record['original_position'] == before['position'] and record['shifted_position'] == changed['position']
    assert record['planned_velocity'] == before['planned_velocity'] and record['generated_contact']
    assert changed['position'][1] == f32(before['position'][1] + f32(shift))
    assert before['position'][::2] == changed['position'][::2]
    assert all(before[key] == changed[key] for key in before if key not in ('position','floor'))
    assert before['floor']['name'] == changed['floor']['name'] == 'AuditWalkOffBarrier'
    assert before['floor']['impact'] == changed['floor']['impact']
    assert target[normal]['after']['mode'] != target[adjusted]['after']['mode'] == expected
    control_results[adjusted] = dict(shift_m=shift, before_gap_m=before['floor']['gap'],
                                     shifted_gap_m=changed['floor']['gap'], resulting_mode=expected)
assert 'gap-control.json' not in [path.name for path in (root/'runtime-top-205-restored').iterdir()]
assert target['top-205-restored'] == target['top-205'] and traces['top-205-restored'] == traces['top-205']
assert raw(file('top-205-restored','parameters')) == raw(file('top-205','parameters'))

decoded = {}
for path in [native_path] + [file(label, name) for label in matrix+controls for name in ('parameters','report')]:
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
constants = 'src/als/settings/movement_constants.rs'
facts = []
for key, path, ranges in (
    (cmc, ue / cmc, [(5036,5078), (6373,6410), (7374,7443)]),
    (motion, Path(motion), [(2723,2747), (4074,4120)]),
    (constants, Path(constants), [(1,5)]),
):
    data = path.read_bytes()
    lines = data.decode().splitlines()
    facts.append(dict(key=key, **identity(data), excerpts=[
        dict(start=a, end=b, text='\n'.join(lines[a-1:b]) + '\n') for a,b in ranges]))
save('source-facts.json', facts)

inputs = {row['path'] for row in read(prior / 'summary.json')['external_source_files']}
inputs.update(str(path.relative_to(Path.cwd())) for path in (
    prior / 'prepare_reader.py', prior / 'wall.request.json', prior / 'wall.package.json',
    prior / 'observer.py', prior / 'summary.json', prior / 'identity-check.json', prior / 'final-source.json',
))
save('summary.json', dict(
    gate='PASS_VALID_EDGE_POST_PHYSICS_DISTANCE_THRESHOLD_BIDIRECTIONAL', new_differences=0,
    total_differences=289, related_difference='BF144-D292', ordinals=[49,50,51],
    target_gap_threshold_m=threshold, native=native, target=target, controls=control_results,
    restored_parameter_report_byte_exact=True, decoded=decoded, screenshots=0, performance_acceptance='paused',
    unresolved_native_qualifier='top-215 has edge tolerance and an apparently walkable capsule floor but remains Falling',
    external_source_files=[dict(path=name, **identity(Path(name).read_bytes())) for name in sorted(inputs)]))
print('Verified both gap crossings and exact restoration against the native wall-height matrix', flush=True)
