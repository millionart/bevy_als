import gzip
import hashlib
import json
import math
import struct
from pathlib import Path

root = Path(__file__).resolve().parent
labels = ('limited', 'clear')


def raw(path):
    if not path.exists():
        return gzip.decompress(path.with_suffix('.json.gz').read_bytes())
    return path.read_bytes()


def read(path):
    return json.loads(raw(path))


def load(label, name):
    return read(root / f'runtime-{label}' / f'{name}.json')


def xyz(value):
    return [value[k] for k in 'xyz']


def world(value):
    x, y, z = xyz(value)
    return [x / 100, 2 + z / 100, y / 100]


def f32(value):
    return struct.unpack('<f', struct.pack('<f', value))[0]


def event(case, kind):
    match, = [item for item in case['events'] if item['kind'] == kind]
    return match


def digest(data):
    return dict(bytes=len(data), sha256=hashlib.sha256(data).hexdigest())


for name in ('build-roof.report', 'roof.native.json.reader.report', 'compile-roof.report'):
    assert read(root / f'{name}.json')['success']

request = read(root / 'roof.request.json')
source_file = root / 'roof.native.json'
frames = {case['name']: case['frames'][40] for case in read(source_file)['character_animation_probe']['cases']}
assert set(frames) == set(labels)
native = {}
target = {}
for label, case in zip(labels, request['cases'], strict=True):
    assert case['name'] == label
    limited = label == 'limited'
    frame = frames[label]
    assert frame['frame'] == 40 and frame['world_delta_seconds'] == f32(.05)
    before, after = (frame[f'movement_{phase}_tick'] for phase in ('before', 'after'))
    assert before['movement_mode_name'] == after['movement_mode_name'] == 'MOVE_Walking'
    assert before['velocity_cm_s'] == {'x': 650, 'y': 0, 'z': 0}
    assert before['floor_blocking'] and before['walkable_floor'] and frame['mode_events'] == []
    assert after['floor_blocking'] and after['walkable_floor']
    first, down, *later = frame['blocking_hits']
    assert first['component'] == down['component'] == 'AuditStepLimit'
    assert not first['penetrating'] and not down['penetrating']
    assert xyz(first['impact_normal']) == [-1, 0, 0] and not first['walkable_hit']
    assert xyz(down['impact_normal']) == [0, 0, 1] and down['walkable_hit']
    assert abs(down['impact_point_cm']['z'] - 44) < .0001
    assert abs(down['trace_start_cm']['z'] - 135) < .0001
    assert abs(down['trace_start_cm']['x'] - first['trace_end_cm']['x']) < 1e-6
    down_stop = down['move_stop_cm']
    final = frame['actor_transform']['translation']
    assert final['x'] == down_stop['x'] and final['y'] == down_stop['y']
    actual_adjust = final['z'] - down_stop['z']
    assert actual_adjust > 0
    if limited:
        roof, = later
        assert roof['component'] == 'AuditStepRoof'
        assert not roof['penetrating'] and not roof['walkable_hit']
        assert xyz(roof['impact_normal']) == [0, 0, -1]
        assert roof['trace_start_cm'] == down_stop and roof['move_stop_cm'] == final
        assert abs(roof['impact_point_cm']['z'] - case['step_roof_bottom_cm']) < .0001
        assert 0 < roof['time'] < 1
        requested_adjust = roof['trace_end_cm']['z'] - roof['trace_start_cm']['z']
        assert .4 < actual_adjust < .6 and 1 < requested_adjust < 1.2
    else:
        assert later == []
        requested_adjust = actual_adjust
        assert 1 < actual_adjust < 1.2
        assert abs(after['floor_distance_cm'] - 2.15) < 1e-5
    native[label] = dict(hits=frame['blocking_hits'], requested_adjust_cm=requested_adjust,
                         actual_adjust_cm=actual_adjust, floor_after_cm=after['floor_distance_cm'],
                         position=world(final))

    functional = read(root / f'{label}-functional.json')
    assert functional['pass'] and not functional['issues'] and functional['images'] == []
    assert load(label, 'process.report')['success']
    stage = load(label, 'stage-before')
    assert stage['mode'] == 'Grounded' and stage['substeps'] == 1 and stage['delta'] == frame['world_delta_seconds']
    assert stage['cached_world_velocity'] == [6.5, 0, 0]
    assert math.dist(stage['position'], world(first['trace_start_cm'])) < 1e-6
    assert math.dist(stage['planned_move'], [.3125, 0, 0]) < 1e-7
    fixture = load(label, 'step-roof')
    assert math.dist(fixture['step'], [-13.4, 2.22, -15]) < 1e-6
    assert abs(fixture['roof_bottom'] - (2 + case['step_roof_bottom_cm'] / 100)) < 1e-6
    replay = load(label, 'step-down-replay')['variants']
    actual, control = (replay[k] for k in ('actual', 'with-swept-stop'))
    for kind in ('forward', 'lift', 'down_request'):
        assert event(actual, kind) == event(control, kind)
    assert event(actual, 'forward')['hit']['entity'] == 'AuditStepLimit'
    assert event(actual, 'lift')['hit'] is None
    assert not any(e['kind'] == 'step_forward' for e in actual['events'])
    observed_down = event(actual, 'down_request')['start']
    assert math.dist(observed_down, world(down['trace_start_cm'])) < .0003
    landing, landing_control = (event(row, 'landing_check') for row in (actual, control))
    assert landing['valid'] and landing_control['valid'] and landing['down_hit'] == landing_control['down_hit']
    assert landing['down_hit']['entity'] == 'AuditStepLimit'
    overlap, stopped = (event(row, 'placement_overlap') for row in (actual, control))
    assert overlap['target_y'] == stopped['target_y'] and overlap['swept_y'] == stopped['swept_y']
    assert overlap['target_y'] - stage['position'][1] < .45
    assert overlap['selected_y'] == landing['position'][1] == overlap['target_y']
    assert stopped['selected_y'] == landing_control['position'][1] == stopped['swept_y']
    assert math.dist(landing_control['position'], world(down_stop)) < .0001
    assert stopped['blockers'] == [] and control['result']['next_y'] == stopped['swept_y']
    assert overlap['blockers'] == (['AuditStepRoof'] if limited else [])
    assert (overlap['target_y'] + .9 > fixture['roof_bottom']) == limited
    assert stopped['swept_y'] + .9 < fixture['roof_bottom']
    result = actual['result']
    txn = load(label, 'walking-transaction')
    expected = result['advanced'].copy()
    if limited:
        assert result['next_y'] is None
    else:
        assert result['next_y'] == overlap['target_y']
        expected[1] = result['next_y']
    assert txn['position'] == expected
    after_target = load(label, 'contact-after-40')
    assert after_target['mode'] == txn['mode'] == 'Grounded' and after_target['transitions'] == []
    if limited:
        assert native[label]['position'][0] - after_target['position'][0] > .29
    else:
        assert math.dist(native[label]['position'], after_target['position']) < .00001
    target[label] = dict(variants=replay, pre_physics=txn['position'], after=after_target)

for a, b in zip(native['limited']['hits'][:2], native['clear']['hits'][:2], strict=True):
    for key in ('component', 'time', 'trace_start_cm', 'trace_end_cm', 'impact_point_cm', 'move_stop_cm'):
        assert a[key] == b[key]
assert abs(native['limited']['requested_adjust_cm'] - native['clear']['requested_adjust_cm']) < 1e-6
assert load('limited', 'stage-before') == load('clear', 'stage-before')
print('Verified accepted native step, truncated height correction and target roof-overlap rollback', flush=True)

paths = [source_file]
paths.extend(root / f'runtime-{label}' / f'{name}.json' for label in labels for name in (
    'parameters', 'report', 'walking-step', 'walking-transaction', 'step-down-replay'))
decompressed = {}
for path in paths:
    data = raw(path)
    archive = path.with_suffix('.json.gz')
    if path.exists():
        archive.write_bytes(gzip.compress(data, compresslevel=6, mtime=0))
    assert gzip.decompress(archive.read_bytes()) == data
    decompressed[str(archive.relative_to(root))] = digest(data)
    path.unlink(missing_ok=True)

external_paths = [root.parent / name for name in (
    '549/height.request.json', '545/observer.py', '537/observer.py',
    '536/prepare_reader.py', '536/step_down_floor.inc', '536/observer.py', '536/step_probe.rs')]
external = [dict(path=str(p.relative_to(Path.cwd())), **digest(p.read_bytes())) for p in external_paths]
summary = dict(gate='PASS_STEPUP_POST_COMMIT_HEIGHT_SWEEP_OBSTRUCTED', supplements=['BF144-D298'],
               new_differences=0, total_differences=301, ordinal=40, native=native, target=target,
               decompressed_files=decompressed, external_source_files=external)
(root / 'summary.json').write_text(json.dumps(summary, indent=2) + '\n')
print('Verified compressed evidence identities', flush=True)
