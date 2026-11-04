import gzip
import hashlib
import json
import math
from pathlib import Path

root = Path(__file__).resolve().parent
stages = ('rig_source', 'rig_output', 'root_result')
leg = ('thigh_r', 'calf_r', 'foot_r')


def raw(path):
    if path.exists():
        return path.read_bytes()
    return gzip.decompress(path.with_suffix('.json.gz').read_bytes())


def read(path):
    return json.loads(raw(path))


def digest(data):
    return dict(bytes=len(data), sha256=hashlib.sha256(data).hexdigest())


def mul(a, b):
    return [x * y for x, y in zip(a, b)]


def dot(a, b):
    return sum(mul(a, b))


def sub(a, b):
    return [x - y for x, y in zip(a, b)]


def cross(a, b):
    return [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]


def qmul(a, b):
    vector = [a[3] * y + b[3] * x + z for x, y, z in zip(a[:3], b[:3], cross(a, b))]
    return vector + [a[3] * b[3] - dot(a[:3], b[:3])]


def native_components(frame, stage):
    bones = frame['pose_boundaries'][stage]['pose']['bones']
    result = []
    for bone in bones:
        transform = bone['local_transform']
        pos, rot, scale = [list(transform[key].values()) for key in ('translation', 'rotation', 'scale')]
        parent = bone['parent_index']
        if parent >= 0:
            parent_pos, parent_rot, parent_scale = result[parent]
            inverse = [-x for x in parent_rot[:3]] + [parent_rot[3]]
            relative = qmul(qmul(parent_rot, mul(parent_scale, pos) + [0]), inverse)[:3]
            pos = [x + y for x, y in zip(parent_pos, relative)]
            rot = qmul(parent_rot, rot)
            scale = mul(parent_scale, scale)
        result.append((pos, rot, scale))
    return {bone['name']: transform for bone, transform in zip(bones, result)}


def native_bone(frame, stage, name):
    return next(bone['local_transform'] for bone in frame['pose_boundaries'][stage]['pose']['bones']
                if bone['name'] == name)


def target_pelvis(frame, stage):
    pose = frame['rig_poses'][stage]
    assert pose['updated']
    return next(bone['translation'] for bone in pose['bones'] if bone['name'] == 'pelvis')


def plane(points):
    first, second, third = points
    upper, lower = sub(second, first), sub(third, second)
    normal = cross(upper, lower)
    length = math.hypot(*normal)
    bend = length / math.hypot(*upper) / math.hypot(*lower)
    assert bend > .08
    return [x / length for x in normal], bend


def flip(prev, curr):
    a, bend_a = plane(prev)
    b, bend_b = plane(curr)
    return dict(dot=dot(a, b), bend=[bend_a, bend_b])


for name in ('build-pelvis.report', 'pelvis.native.json.reader.report', 'compile-pelvis.report'):
    assert read(root / f'{name}.json')['success']
assert all(result == 'PASS' for result in read(root / 'watchdog-selftest.json').values())
source_path = root / 'pelvis.native.json'
cases = {case['name']: case['frames'] for case in read(source_path)['character_animation_probe']['cases']}
assert set(cases) == {'coarse', 'fixed'}
native, target, external = {}, {}, []
for label, landing, step, release in (('coarse', 58, .125, 62), ('fixed', 79, .03125, 92)):
    frames = cases[label]
    source_landing, = [row for row in frames[52:] if row['landed_events']]
    assert source_landing['frame'] == landing
    assert source_landing['landed_events'][0]['component'] == 'AuditLowerFloor'
    assert frames[landing - 1]['movement_after_tick']['movement_mode_name'] == 'MOVE_Falling'
    assert frames[landing]['movement_after_tick']['movement_mode_name'] == 'MOVE_Walking'
    assert all(row['world_delta_seconds'] == step for row in frames[52:])
    request = read(root / 'pelvis.request.json')
    case = next(case for case in request['cases'] if case['name'] == label)
    assert sum(row['delta_seconds'] for row in case['frames'][52:]
               if row['movement_input_world']['x']) == 1.25

    path = root / f'runtime-{label}'
    functional = read(root / f'{label}-functional.json')
    assert not functional['pass'] and functional['images'] == []
    assert set(functional['issues']) == {'faults is non-empty', 'temporal_anomalies is non-empty'}
    assert read(path / 'process.report.json')['success']
    report = read(path / 'report.json')
    assert report['primary_failure'] is None and not report['graph_errors']
    assert report['screenshots'] == []
    parameters = read(path / 'parameters.json')
    tail = [row for row in parameters['dynamics'] if row['ordinal'] >= 52]
    assert all(row['delta'] == step for row in tail)
    assert [row['ordinal'] for row in tail if row['input'] != [0, 0]] == list(range(52, release))
    assert sum(row['delta'] for row in tail if row['input'] != [0, 0]) == 1.25
    actual = {row['ordinal']: row for row in parameters['after']}
    assert actual[landing - 1]['gameplay'] == 'Air' and actual[landing]['gameplay'] == 'Grounded'
    assert abs(actual[landing]['position'][1] - .9215) < .003
    source_delta, target_delta = {}, {}
    for stage in stages:
        for frame in frames[landing-1:landing+3]:
            pose = frame['pose_boundaries'][stage]
            assert pose['required_bones_asset'] == '/ALS/ALS/Character/SKM_Als.SKM_Als'
            assert pose['delta_seconds'] == step
        source_delta[stage] = [
            math.dist(list(native_bone(frames[frame - 1], stage, 'pelvis')['translation'].values()),
                      list(native_bone(frames[frame], stage, 'pelvis')['translation'].values())) / 100
            for frame in range(landing, landing + 3)]
        target_delta[stage] = [
            math.dist(target_pelvis(actual[frame - 1], stage), target_pelvis(actual[frame], stage))
            for frame in range(landing, landing + 3)]
    anomaly, = report['temporal_anomalies']
    assert anomaly['stage'] == 'pre_rig' and anomaly['animation_delta_seconds'] == step
    index, = [i for i, row in enumerate(report['pose_samples']) if row['elapsed_secs'] == anomaly['elapsed_secs']]
    observed = report['pose_samples'][index-1:index+1]
    ordinal = landing if label == 'coarse' else landing + 2
    for frame, sample in zip((ordinal - 1, ordinal), observed):
        x = sample['pre_rig_bones']['pelvis']['translation'][0]
        nearest = min(actual, key=lambda i: abs(actual[i]['position'][0] - x))
        assert nearest == frame and abs(actual[frame]['position'][0] - x) < .1
    native[label] = dict(landing=landing, pelvis_delta=source_delta)
    target[label] = dict(landing=landing, pelvis_delta=target_delta, report_ordinal=ordinal, anomaly=anomaly)

    if label == 'coarse':
        assert anomaly['kind'] == 'local_translation_spike' and anomaly['subject'] == 'pelvis'
        assert abs(anomaly['current'] - target_delta['rig_source'][0]) < 2e-5
        assert .398 < source_delta['rig_source'][0] < .4
        assert all(source_delta[stage][0] > .35 and target_delta[stage][0] > .35 for stage in stages)
    else:
        assert anomaly['kind'] == 'limb_plane_flip' and anomaly['subject'] == 'right_leg'
        assert all(max(source_delta[stage]) < .25 and max(target_delta[stage]) < .25 for stage in stages)
        old_mesh, new_mesh = (frames[frame]['mesh_to_world'] for frame in (80, 81))
        assert old_mesh['rotation'] == new_mesh['rotation']
        assert old_mesh['scale'] == new_mesh['scale'] == dict(x=1, y=1, z=1)
        for key, native_stage in (('pre_rig_bones', 'rig_source'), ('bones', 'root_result')):
            src = [native_components(frames[frame], native_stage) for frame in (80, 81)]
            src_result = flip(*([row[bone][0] for bone in leg] for row in src))
            tgt_result = flip(*([row[key][bone]['translation'] for bone in leg] for row in observed))
            native[label][key] = src_result
            target[label][key] = tgt_result
        src_dot = native[label]['pre_rig_bones']['dot']
        tgt_dot = target[label]['pre_rig_bones']['dot']
        assert -.96 < src_dot < -.94 and abs(tgt_dot - anomaly['current']) < 3e-5
        assert math.acos(src_dot) / step > 30 and math.acos(tgt_dot) / step > 30
        knee_a, knee_b = [list(native_bone(frames[frame], 'rig_source', 'calf_r')['rotation'].values())
                          for frame in (80, 81)]
        native[label]['calf_local_angular_rate'] = 2 * math.acos(abs(dot(knee_a, knee_b))) / step
        assert native[label]['calf_local_angular_rate'] > 30
        assert native[label]['bones']['dot'] > .95 and target[label]['bones']['dot'] > .95

print('Verified source pelvis jump and pre-rig right-leg flip at the same landing-relative ordinals', flush=True)
prior_paths = []
for name, ordinal in (('full-input', 58), ('low-input', 59)):
    path = root / f'prior-{name}.json'
    prior_paths.append(path)
    previous = read(root.parent / '571' / f'{name}-visual.json')
    anomaly, = previous['temporal_anomalies']
    sample, = [row for row in previous['pose_samples'] if row['elapsed_secs'] == anomaly['elapsed_secs']]
    x = sample['pre_rig_bones']['pelvis']['translation'][0]
    actual = {row['ordinal']: row for row in read(path)['after']}
    nearest = min(actual, key=lambda i: abs(actual[i]['position'][0] - x))
    assert nearest == ordinal and abs(actual[ordinal]['position'][0] - x) < .1
    assert actual[ordinal-1]['gameplay'] == 'Air' and actual[ordinal]['gameplay'] == 'Grounded'
    assert anomaly['animation_delta_seconds'] == .125
paths = [source_path, *prior_paths]
paths.extend(root / f'runtime-{label}' / f'{name}.json'
             for label in ('coarse', 'fixed') for name in ('parameters', 'report'))
decompressed = {}
for path in paths:
    data = raw(path)
    archive = path.with_suffix('.json.gz')
    if path.exists():
        archive.write_bytes(gzip.compress(data, compresslevel=6, mtime=0))
    assert gzip.decompress(archive.read_bytes()) == data
    decompressed[str(archive.relative_to(root))] = digest(data)
    path.unlink(missing_ok=True)

for name in ('571/full-input.request.json', '571/full-input-visual.json.gz', '571/low-input-visual.json.gz',
             '571/prepare_reader.py', '570/observer.py', '523/observer.py', '523/deck.rs'):
    path = root.parent / name
    external.append(dict(path=str(path.relative_to(Path.cwd())), **digest(path.read_bytes())))
summary = dict(gate='PASS_LANDING_POSE_GUARD_MATCHES_NATIVE_BOUNDARIES', findings=[],
               new_differences=0, total_differences=310, native=native, target=target,
               public_functional_pass=False, decompressed_files=decompressed, external_source_files=external)
(root / 'summary.json').write_text(json.dumps(summary, indent=2) + '\n')
print('Verified compressed evidence identities', flush=True)
