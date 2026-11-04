import gzip
import hashlib
import json
import math
import struct
from pathlib import Path

root = Path(__file__).resolve().parent
labels = ('within', 'beyond', 'beyond-adjusted', 'beyond-restored')


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


def digest(data):
    return dict(bytes=len(data), sha256=hashlib.sha256(data).hexdigest())


for name in ('build-lower.report', 'lower.native.json.reader.report', 'compile-lower.report'):
    assert read(root / f'{name}.json')['success']
assert not load('query-conflict', 'process.report')['success']
assert 'B0001' in (root / 'runtime-query-conflict' / 'diagnostic.txt').read_text()

request = read(root / 'lower.request.json')
source_file = root / 'lower.native.json'
frames = {case['name']: case['frames'][40] for case in read(source_file)['character_animation_probe']['cases']}
assert set(frames) == {'within', 'beyond'}
native = {}
for case in request['cases']:
    label = case['name']
    row = frames[label]
    assert row['frame'] == 40 and row['world_delta_seconds'] == f32(.05)
    before, after = (row[f'movement_{phase}_tick'] for phase in ('before', 'after'))
    assert before['movement_mode_name'] == after['movement_mode_name'] == 'MOVE_Walking'
    assert before['velocity_cm_s'] == {'x': 650, 'y': 0, 'z': 0}
    assert before['walkable_floor'] and before['floor_blocking']
    assert before['floor_component'] != 'AuditLandingDeck'
    assert after['floor_component'] == 'AuditLandingDeck'
    assert row['mode_events'] == row['blocking_hits'] == []
    floor = row['actual_current_floor']
    hit = floor['hit']
    assert floor['blocking'] and floor['walkable'] and not floor['line_trace']
    assert hit['component'] == 'AuditLandingDeck' and hit['walkable'] and not hit['penetrating']
    assert xyz(hit['impact_normal']) == [0, 0, 1]
    assert abs(hit['impact_height_cm'] - case['landing_deck_top_cm']) < .0001
    assert abs(floor['distance_cm'] - 2.15) < 1e-5
    start = world(floor['trace_start_cm'])
    end = world(row['actor_transform']['translation'])
    assert start[::2] == end[::2]
    assert abs((start[1] - end[1]) * 100 + case['landing_deck_top_cm']) < .0001
    assert (start[1] - end[1] > .45) == (label == 'beyond')
    native[label] = dict(start=start, end=end, floor=floor, world_delta_seconds=row['world_delta_seconds'])

target = {}
for label in labels:
    enabled = label == 'beyond-adjusted'
    within = label == 'within'
    source = native['within' if within else 'beyond']
    functional = read(root / f'{label}-functional.json')
    assert functional['pass'] and not functional['issues'] and functional['images'] == []
    assert load(label, 'process.report')['success']
    stage = load(label, 'stage-before')
    assert stage['mode'] == 'Grounded' and stage['substeps'] == 1 and stage['delta'] == source['world_delta_seconds']
    assert stage['cached_world_velocity'] == [6.5, 0, 0]
    assert math.dist(stage['planned_move'], [.3125, 0, 0]) < 1e-7
    replay = load(label, 'step-down-replay')['variants']['actual']
    assert not any(e['kind'] in ('forward', 'step_forward', 'lift') for e in replay['events'])
    before = load(label, 'contact-before-40')
    after = load(label, 'contact-after-40')
    assert before['mode'] == after['mode'] == 'Grounded' and after['transitions'] == []
    assert before['has_basis_displacement']
    record = load(label, 'floor-adjustment')
    assert record['enabled'] == enabled and record['start'] == before['position']
    assert math.dist(record['start'], source['start']) < .00001
    assert record['floor']['name'] == before['floor']['name'] == 'AuditLandingDeck'
    assert record['floor']['center'] == before['floor']['center']
    assert abs(record['floor']['impact'] - (2 + source['floor']['hit']['impact_height_cm'] / 100)) < 1e-6
    gap = record['start'][1] - record['floor']['center']
    assert gap == before['floor']['gap']
    assert .46 < gap < .474
    assert abs(record['request_y'] - (.0215 - gap)) < 1e-6
    assert (record['request_y'] < -.45) == (not within)
    assert record['hit'] is None
    assert math.dist(record['end'], source['end']) < .0003
    assert record['selected'] == (record['end'] if enabled else record['start'])
    assert after['position'] == (record['end'] if enabled or within else record['start'])
    target[label] = dict(adjustment=record, before=before, after=after)

for name in ('stage-before', 'stage-after', 'contact-before-40', 'walking-transaction', 'step-down-replay'):
    assert load('beyond', name) == load('beyond-adjusted', name) == load('beyond-restored', name)
assert target['beyond'] == target['beyond-restored']
assert load('beyond', 'contact-after-41') == load('beyond-restored', 'contact-after-41')
print('Verified valid low-floor queries, observed cap rejection and isolated adjustment recovery', flush=True)

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
    '550/roof.request.json', '537/observer.py', '536/prepare_reader.py',
    '536/step_down_floor.inc', '536/observer.py', '536/step_probe.rs')]
external = [dict(path=str(p.relative_to(Path.cwd())), **digest(p.read_bytes())) for p in external_paths]
summary = dict(gate='PASS_WALKING_POST_FLOOR_DOWN_ADJUSTMENT_CAP', findings=['BF144-D306'],
               new_differences=1, total_differences=303, ordinal=40, native=native, target=target,
               decompressed_files=decompressed, external_source_files=external)
(root / 'summary.json').write_text(json.dumps(summary, indent=2) + '\n')
print('Verified compressed evidence identities', flush=True)
