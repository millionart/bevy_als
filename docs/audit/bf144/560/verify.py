import gzip
import hashlib
import json
import math
from pathlib import Path

root = Path(__file__).resolve().parent
prior = root.parent / '559'
labels = ('baseline', 'corrected', 'restored')


def raw(path):
    if not path.exists():
        return gzip.decompress(path.with_suffix('.json.gz').read_bytes())
    return gzip.decompress(path.read_bytes()) if path.suffix == '.gz' else path.read_bytes()


def read(path):
    return json.loads(raw(path))


def load(label, name):
    return read(root / f'runtime-{label}' / f'{name}.json')


def digest(data):
    return dict(bytes=len(data), sha256=hashlib.sha256(data).hexdigest())


def world(value):
    return [value['x'] / 100, 2 + value['z'] / 100, value['y'] / 100]


assert read(root / 'compile-handoff.report.json')['success']
assert not read(root / 'compile-unqualified-type.report.json')['success']
source_file = prior / 'third.native.json.gz'
case = next(c for c in read(source_file)['character_animation_probe']['cases'] if c['name'] == 'positive')
row = case['frames'][49]
assert row['frame'] == 49
one, two, ground = row['blocking_hits']
assert [h['component'] for h in (one, two, ground)] == [
    'AuditAirCornerWall', 'AuditAirCornerSecond', 'AuditAirCornerFloor']
assert ground['time'] > 0 and not ground['penetrating'] and ground['walkable_hit']
assert row['movement_before_tick']['movement_mode_name'] == 'MOVE_Falling'
assert row['movement_after_tick']['movement_mode_name'] == 'MOVE_Walking'
event, = row['mode_events']
assert event['current'] == 'MOVE_Walking' and event['previous'] == 'MOVE_Falling'
assert event['velocity_cm_s'] == row['actor_velocity_cm_s'] == {'x': 0, 'y': 0, 'z': 0}
source_end = world(row['actor_transform']['translation'])
assert source_end[::2] == world(ground['move_stop_cm'])[::2]

old = prior / 'runtime-positive'
target = {}
for name in ('air-corner', 'third-contact', 'stage-before', 'stage-after'):
    assert load('baseline', name) == load('corrected', name) == load('restored', name)
    assert load('baseline', name) == read(old / f'{name}.json')
for label in labels:
    corrected = label == 'corrected'
    functional = read(root / f'{label}-functional.json')
    assert functional['pass'] and not functional['issues'] and functional['images'] == []
    assert load(label, 'process.report')['success']
    trace = load(label, 'third-contact')
    replay = load(label, 'walking-handoff-replay')
    assert trace['landing_candidate'] and trace['hits'][2]['travel'] > 0
    assert replay['remaining'] == trace['hits'][2]['time_left'] and replay['remaining'] > .037
    assert replay['landing'][::2] == trace['hits'][2]['stop'][::2]
    incoming, deflected = (replay['variants'][key] for key in ('incoming', 'deflected'))
    assert incoming['walking_input'] == [4, 0, 4]
    v = trace['third_velocity']
    assert deflected['walking_input'] == [v[0], 0, v[2]]
    assert math.hypot(*deflected['walking_input']) < .001
    assert deflected['requested_velocity'] == deflected['feedback'] == [0, 0, 0]
    assert deflected['end'] == replay['landing']
    assert .117 < incoming['end'][2] - replay['landing'][2] < .119
    assert incoming['feedback'][2] > 3.1
    production = load(label, 'stage-after')
    handoff = load(label, 'handoff-control')
    assert handoff['enabled'] == corrected
    assert production['substeps'] == 2 and production['mode'] == 'Grounded'
    assert incoming['end'] == handoff['original_position'] == production['position']
    assert incoming['feedback'] == handoff['original_feedback'] == production['collision_velocity']
    selected = deflected if corrected else incoming
    assert handoff['selected_position'] == selected['end']
    assert handoff['selected_feedback'] == selected['feedback']
    for key in ('motor_boost', 'motor_acceleration'):
        assert handoff[key] == load('baseline', 'handoff-control')[key]
    actual = load(label, 'contact-after-49')
    assert actual['mode'] == 'Grounded' and actual['has_base']
    assert actual['transitions'] == [['LocomotionState(Air)', 'LocomotionState(Grounded)']]
    assert actual['published'] == selected['feedback']
    if corrected:
        assert actual['position'] == deflected['end']
        assert math.dist(actual['position'], source_end) < .0003
    else:
        assert actual == read(old / 'contact-after-49.json')
    target[label] = dict(replay=replay, handoff=handoff, actual=actual)
assert target['baseline'] == target['restored']
print('Verified real Falling deflection velocity handoff independent of the Walking time budget', flush=True)

decompressed = {}
for label in labels:
    for name in ('parameters', 'report'):
        path = root / f'runtime-{label}' / f'{name}.json'
        data = raw(path)
        archive = path.with_suffix('.json.gz')
        if path.exists():
            archive.write_bytes(gzip.compress(data, compresslevel=6, mtime=0))
        assert gzip.decompress(archive.read_bytes()) == data
        decompressed[str(archive.relative_to(root))] = digest(data)
        path.unlink(missing_ok=True)

external_paths = [source_file, prior / 'observer.py', prior / 'corner_fixture.inc', prior / 'corner_probe.inc',
                  *(old / f'{n}.json' for n in ('air-corner', 'third-contact', 'stage-before', 'stage-after', 'contact-after-49'))]
external = [dict(path=str(p.relative_to(Path.cwd())), **digest(p.read_bytes())) for p in external_paths]
summary = dict(gate='PASS_LANDING_WALKING_RECEIVES_DEFLECTED_HORIZONTAL_VELOCITY', findings=['BF144-D309'],
               new_differences=1, total_differences=306, ordinal=49, native=dict(end=source_end,
                   velocity_cm_s=event['velocity_cm_s']), target=target, decompressed_files=decompressed,
               external_source_files=external)
(root / 'summary.json').write_text(json.dumps(summary, indent=2) + '\n')
print('Verified compressed evidence identities', flush=True)
