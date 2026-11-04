import gzip
import hashlib
import json
from pathlib import Path

root = Path(__file__).resolve().parent
source = root.parent / '572'
labels = ('baseline', 'cached')


def raw(path):
    if path.exists():
        return path.read_bytes()
    return gzip.decompress(path.with_suffix('.json.gz').read_bytes())


def read(path):
    return json.loads(raw(path))


def digest(data):
    return dict(bytes=len(data), sha256=hashlib.sha256(data).hexdigest())


def projected(row):
    t = row['actor_transform']['translation']
    v = row['movement_after_tick']['velocity_cm_s']
    return dict(frame=row['frame'], x=t['x']/100, y=2+t['z']/100, vx=v['x']/100, vy=v['z']/100)


assert read(root / 'compile-cached.report.json')['success']
assert all(value == 'PASS' for value in read(root / 'watchdog-selftest.json').values())
native = {case['name']: case['frames']
          for case in read(source / 'pelvis.native.json')['character_animation_probe']['cases']}
coarse = native['coarse']
start = coarse[54]['movement_before_tick']
after = coarse[54]['movement_after_tick']
event, = coarse[54]['mode_events']
assert coarse[54]['world_delta_seconds'] == .125
assert start['movement_mode_name'] == 'MOVE_Walking' and start['walkable_floor']
assert after['movement_mode_name'] == 'MOVE_Falling'
assert start['max_acceleration_cm_s2'] == after['acceleration_cm_s2']['x'] == 2000
assert not coarse[54]['blocking_hits'] and not coarse[54]['landed_events']
assert event['previous'] == 'MOVE_Walking' and event['current'] == 'MOVE_Falling'
step_velocity = event['velocity_cm_s']['x'] / 100
assert abs(step_velocity - 6) < 2e-7
assert abs(event['location_cm']['x']/100 - projected(coarse[53])['x'] - .3) < 2e-7
source_result = projected(coarse[54])
source_air = source_result['vx'] - step_velocity
assert abs(source_air - .225) < 2e-7
assert abs(source_result['vy'] / -9.81 - .075) < 2e-7

old = read(source / 'runtime-coarse/parameters.json')
old_rows = {row['ordinal']: row for row in old['after']}
runtime, samples = {}, {}
for label in labels:
    path = root / f'runtime-{label}'
    functional = read(root / f'{label}-functional.json')
    assert not functional['pass'] and functional['images'] == []
    assert set(functional['issues']) == {'faults is non-empty', 'temporal_anomalies is non-empty'}
    assert read(path / 'process.report.json')['success']
    report = read(path / 'report.json')
    assert not report['graph_errors'] and report['primary_failure'] is None and report['screenshots'] == []
    warning, = report['temporal_anomalies']
    assert warning['kind'] == 'local_translation_spike' and warning['subject'] == 'pelvis'
    before = read(path / 'mixed-before.json')
    after = read(path / 'mixed-after.json')
    assert before['mode'] == 'Grounded' and before['velocity'] == [5, 0, 0]
    assert before['base_support'] == [0, 0, 0]
    assert before['delta'] == .125 and before['planned_substeps'] == 3
    assert before['cached_acceleration'] == 20
    assert 9.673 < before['fresh_acceleration'] < 9.675
    assert after['restored_acceleration'] == before['fresh_acceleration']
    selected = before['cached_acceleration'] if label == 'cached' else before['fresh_acceleration']
    assert before['walking_acceleration'] == selected
    rows = {row['ordinal']: row for row in read(path / 'parameters.json')['after']}
    assert rows[53]['gameplay'] == 'Grounded'
    assert rows[54]['gameplay'] == after['mode'] == 'Air'
    assert rows[54]['position'] == after['position'] and rows[54]['velocity'] == after['velocity']
    assert abs(after['velocity'][1] - source_result['vy']) < 2e-6
    assert abs(after['velocity'][0] - (5 + selected * .05 + source_air)) < 5e-6
    assert rows[57]['gameplay'] == 'Air' and rows[58]['gameplay'] == 'Grounded'
    if label == 'baseline':
        assert abs(rows[54]['position'][0] - old_rows[54]['position'][0]) < 1e-5
        assert abs(rows[58]['position'][0] - old_rows[58]['position'][0]) < 1e-5
    runtime[label] = dict(before=before, after=after, functional_pass=functional['pass'])
    samples[label] = []
    for index in range(53, 59):
        row = rows[index]
        native_row = projected(coarse[index])
        samples[label].append(dict(frame=index, position=row['position'], velocity=row['velocity'],
                                   mode=row['gameplay'], source=native_row, x_gap=native_row['x']-row['position'][0]))

before_a, before_b = [runtime[label]['before'].copy() for label in labels]
before_a.pop('walking_acceleration')
before_b.pop('walking_acceleration')
assert before_a == before_b
assert .0645 < samples['baseline'][1]['x_gap'] < .0646
assert .1568 < samples['baseline'][-1]['x_gap'] < .1569
assert all(abs(row['x_gap']) < 5e-6 for row in samples['cached'])
assert abs(runtime['cached']['after']['velocity'][0] - source_result['vx']) < 5e-6

fixed_target = read(source / 'runtime-fixed/parameters.json')
frame = native['fixed'][59]
assert frame['movement_before_tick']['movement_mode_name'] == frame['movement_after_tick']['movement_mode_name'] == 'MOVE_Walking'
assert not frame['mode_events'] and not frame['blocking_hits']
row = next(row for row in fixed_target['dynamics'] if row['ordinal'] == 59)
assert row['world_velocity_at_dynamics'][0] == 4.375
assert frame['movement_before_tick']['velocity_cm_s']['x'] == 437.5
assert frame['movement_after_tick']['acceleration_cm_s2']['x'] == 2000
assert abs(frame['movement_after_tick']['max_acceleration_cm_s2']/100 - row['dynamics'][0]) < 1e-6
assert abs(row['planned_velocity'][0] - (4.375 + row['dynamics'][0]/32)) < 3e-7
assert projected(frame)['vx'] == 5
fixed = dict(native=projected(frame), target=row)

print('Verified restored horizontal trajectory using only the retained Walking acceleration', flush=True)
paths = [root / f'runtime-{label}' / f'{name}.json'
         for label in labels for name in ('parameters', 'report')]
decompressed = {}
for path in paths:
    data = raw(path)
    archive = path.with_suffix('.json.gz')
    if path.exists():
        archive.write_bytes(gzip.compress(data, compresslevel=6, mtime=0))
    assert gzip.decompress(archive.read_bytes()) == data
    decompressed[str(archive.relative_to(root))] = digest(data)
    path.unlink(missing_ok=True)

external = []
for name in ('pelvis.native.json.gz', 'pelvis.request.json', 'runtime-coarse/parameters.json.gz',
             'runtime-fixed/parameters.json.gz', 'observer.py', 'evidence-manifest.json'):
    path = source / name
    external.append(dict(path=str(path.relative_to(Path.cwd())), **digest(path.read_bytes())))
summary = dict(gate='PASS_CACHED_WALKING_INPUT_RESTORES_RUNOFF_TRAJECTORY', findings=['BF144-D07'],
               new_differences=0, total_differences=310, runtime=runtime, horizontal=samples,
               fixed_first_difference=fixed, air_seconds=.075, source_air_velocity_gain=source_air,
               decompressed_files=decompressed, external_source_files=external)
(root / 'summary.json').write_text(json.dumps(summary, indent=2) + '\n')
print('Verified compressed evidence identities', flush=True)
