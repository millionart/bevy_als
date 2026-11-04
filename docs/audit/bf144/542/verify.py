import gzip
import hashlib
import json
import math
import struct
from pathlib import Path

root = Path(__file__).resolve().parent
prior = root.parent / '541'
labels = ('baseline', 'clip', 'restored')


def read(path):
    if not path.exists():
        path = path.with_suffix('.json.gz')
    data = path.read_bytes()
    return json.loads(gzip.decompress(data) if path.suffix == '.gz' else data)


def save(name, value):
    (root / name).write_text(json.dumps(value, indent=2) + '\n')


def load(label, name):
    return read(root / f'runtime-{label}' / f'{name}.json')


def vector(value):
    return [value[axis] for axis in 'xyz']


def delta(start, end):
    return [b - a for a, b in zip(start, end, strict=True)]


def dot(a, b):
    return sum(x * y for x, y in zip(a, b, strict=True))


def scale(v, factor):
    return [x * factor for x in v]


def unit(v):
    return scale(v, 1 / math.hypot(*v))


def remaining_time(hit):
    return struct.unpack('<f', struct.pack('<f', 1 - hit['time']))[0]


def sweep_request(hit):
    return delta(vector(hit['trace_start_cm']), vector(hit['trace_end_cm']))


def horizontal_runtime(v):
    return [v[0] / 100, 0.0, v[1] / 100]


def impulses(contacts, entity):
    pair, = [x for x in contacts if x['entity'] == entity]
    assert pair['constraints']
    return sum(value for m in pair['manifolds'] for value in m['impulses'])


source_record = prior / 'wall.native.json.gz'
source = read(source_record)['character_animation_probe']['cases']
frame = next(case['frames'][40] for case in source if case['name'] == 'high')
assert frame['frame'] == 40 and frame['world_delta_seconds'] == 0.05000000074505806
before, after = (frame[f'movement_{phase}_tick'] for phase in ('before', 'after'))
assert before['movement_mode_name'] == after['movement_mode_name'] == 'MOVE_Walking'
assert after['walkable_floor'] and frame['mode_events'] == []
candidate = frame['candidate_down_replay']
assert candidate['above_old'] and candidate['direction_dot'] >= 0
first, slide, corner = frame['blocking_hits']
assert [hit['component'] for hit in (first, slide, corner)] == ['AuditStepFin', 'AuditStepSlope', 'AuditStepSlope']
assert all(not hit['penetrating'] and not hit['walkable_hit'] for hit in (first, slide, corner))
assert first['move_stop_cm'] == slide['trace_start_cm']
assert slide['move_stop_cm'] == corner['trace_start_cm']
flat_normal = unit(vector(first['normal'])[:2] + [0])
full_delta = sweep_request(first)
projected = delta(scale(flat_normal, dot(full_delta, flat_normal)), full_delta)
slide_request = scale(projected, remaining_time(first))
assert math.dist(slide_request, sweep_request(slide)) < 1e-9

normal = vector(slide['normal'])
assert dot(flat_normal, normal) < 0
crease = unit([normal[(i + 1) % 3] * flat_normal[(i + 2) % 3]
               - normal[(i + 2) % 3] * flat_normal[(i + 1) % 3] for i in range(3)])
adjusted = scale(crease, dot(slide_request, crease) * remaining_time(slide))
assert dot(adjusted, full_delta) > 0 and adjusted[2] > 0
assert math.dist(adjusted[:2] + [0], sweep_request(corner)) < 1e-9

source_slide = horizontal_runtime(slide_request)
source_travel = horizontal_runtime(delta(vector(slide['trace_start_cm']), vector(slide['move_stop_cm'])))
corner_travel = horizontal_runtime(delta(vector(corner['trace_start_cm']), vector(corner['move_stop_cm'])))
source_pos = vector(frame['actor_transform']['translation'])
source_pos = [source_pos[0] / 100, 2 + source_pos[2] / 100, source_pos[1] / 100]
assert math.hypot(*corner_travel) < 0.0002

assert read(root / 'compile-slide.report.json')['success']
failure = load('sample-order-failure', 'process.report')
assert not failure['success'] and failure['process_exit_code'] != 0
target = {}
old = read(prior / 'summary.json')['target']['high']
same_inputs = ('fixture', 'step-fin', 'step-slope', 'stage-before', 'walking-step', 'step-down-replay',
               'walking-transaction', 'stage-after', 'solver-control')
for name in same_inputs:
    baseline = load('baseline', name)
    assert baseline == load('clip', name) == load('restored', name), name
    if name != 'solver-control':
        assert baseline == read(prior / 'runtime-high' / f'{name}.json'), name

for label in labels:
    functional = read(root / f'{label}-functional.json')
    assert functional['pass'] and not functional['issues'] and functional['images'] == []
    assert load(label, 'process.report')['success']
    trace = load(label, 'horizontal-slide')
    txn = load(label, 'walking-transaction')
    control = load(label, 'solver-control')
    entry = load(label, 'solver-entry')
    exit_solver = load(label, 'solver-exit')
    support = load(label, 'contact-before-40')
    result = load(label, 'contact-after-40')
    assert trace['enabled'] == (label == 'clip')
    assert trace['start'] == control['position'] == txn['position']
    assert trace['original_boost'] == control['boost'] and trace['velocity'] == control['velocity']
    assert trace['selected_boost'][1] == trace['original_boost'][1] and not control['masked_fin']
    assert trace['acceleration'] == control['acceleration']
    assert trace['hit']['entity'] == 'AuditStepSlope'
    assert trace['dt'] == frame['world_delta_seconds']
    old_result = old['trace']['variants']['actual']['result']
    assert old_result['next_y'] is None and trace['start'] == old_result['advanced']
    assert math.dist(trace['remaining'], old_result['remaining']) < 1e-7
    assert math.dist(trace['remaining'], source_slide) < 0.00011
    assert math.dist(trace['clipped'], source_travel) < 0.00003
    selected_delta = trace['clipped'] if trace['enabled'] else trace['remaining']
    motor_xz = [entry['velocity'][0] * trace['dt'], 0.0, entry['velocity'][2] * trace['dt']]
    assert math.dist(motor_xz, selected_delta) < 1e-7
    assert entry['velocity'][1] == trace['original_boost'][1]
    assert len(exit_solver['steps']) == 6 and support['position'] == exit_solver['position']
    slope = impulses(exit_solver['contacts'], 'AuditStepSlope')
    fin = impulses(exit_solver['contacts'], 'AuditStepFin')
    if label == 'clip':
        assert slope == fin == 0
        assert exit_solver['velocity'][1] < 0.051
        assert support['floor']['name'] == 'AuditStepFin'
        assert result['mode'] == 'Grounded' and result['transitions'] == []
        assert math.hypot(result['position'][0] - source_pos[0], result['position'][2] - source_pos[2]) < 0.00013
        assert math.dist(result['position'], source_pos) < 0.0007
    else:
        assert slope > 1 and fin > 0.8
        assert exit_solver['steps'][0]['velocity'][1] > 1.9
        assert exit_solver['velocity'][1] > 2.7 and support['floor'] is None
        assert result == old['after']
    target[label] = dict(sweep=trace, solver_entry=entry['velocity'],
                         first_solver_substep=exit_solver['steps'][0], solver_exit=exit_solver['velocity'],
                         slope_impulse=slope, fin_impulse=fin, post_solver_floor=support['floor'], result=result)
for name in ('horizontal-slide', 'solver-entry', 'solver-exit', 'contact-before-40', 'contact-after-40',
             'contact-before-41', 'contact-after-41'):
    assert load('baseline', name) == load('restored', name), name

external_paths = [
    source_record, prior / 'summary.json', prior / 'contact_control.inc', prior / 'target_slope.inc',
    prior / 'observer.py', root.parent / '538' / 'solver_probe.rs',
    *(prior / 'runtime-high' / f'{n}.json' for n in same_inputs if n != 'solver-control'),
]
external_files = {}
for path in external_paths:
    if not path.exists():
        path = path.with_suffix('.json.gz')
    data = path.read_bytes()
    external_files[str(path.relative_to(Path.cwd()))] = dict(bytes=len(data), sha256=hashlib.sha256(data).hexdigest())

compressed = {}
for label in labels:
    for name in ('parameters', 'report', 'walking-step', 'walking-transaction', 'step-down-replay'):
        path = root / f'runtime-{label}' / f'{name}.json'
        archive = path.with_suffix('.json.gz')
        raw = path.read_bytes() if path.exists() else gzip.decompress(archive.read_bytes())
        if path.exists():
            archive.write_bytes(gzip.compress(raw, compresslevel=6, mtime=0))
        assert gzip.decompress(archive.read_bytes()) == raw
        path.unlink(missing_ok=True)
        compressed[str(archive.relative_to(root))] = dict(bytes=len(raw), sha256=hashlib.sha256(raw).hexdigest())

save('summary.json', dict(
    gate='PASS_REJECTED_STEP_FIRST_SLIDE_SWEEP_RECOVERS_SINGLE_STEP_SUPPORT',
    new_differences=0, total_differences=296, supplements=['BF144-D15'],
    ordinal=40, native=dict(position=source_pos, walkable_floor=after['walkable_floor'],
        floor_distance_cm=after['floor_distance_cm'], hits=[first, slide, corner],
        slide_request_m=source_slide, first_slide_travel_m=source_travel, corner_travel_m=corner_travel),
    target=target, external_source_files=[dict(path=p, **v) for p, v in sorted(external_files.items())],
    decompressed_files=compressed,
))
print('Verified native slide and TwoWall requests, live single-step floor recovery and full removal control', flush=True)
