import gzip
import hashlib
import json
import math
import struct
from pathlib import Path

root = Path(__file__).resolve().parent
native_labels = ('ceiling-split', 'ceiling-unsplit', 'clear')


def raw(path):
    if not path.exists():
        return gzip.decompress(path.with_suffix('.json.gz').read_bytes())
    return path.read_bytes()


def read(path):
    return json.loads(raw(path))


def load(label, name):
    return read(root / f'runtime-{label}' / f'{name}.json')


def f32(value):
    return struct.unpack('<f', struct.pack('<f', value))[0]


def digest(data):
    return dict(bytes=len(data), sha256=hashlib.sha256(data).hexdigest())


for name in ('build-apex.report', 'build-observed.report', 'apex.native.json.reader.report',
             'apex-observed.native.json.reader.report', 'compile-apex.report'):
    assert read(root / f'{name}.json')['success']
assert not read(root / 'compile-unqualified-type.report.json')['success']
assert 'E0425' in (root / 'compile-unqualified-type.txt').read_text()

request = read(root / 'apex.request.json')
original_file = root / 'apex.native.json'
observed_file = root / 'apex-observed.native.json'
original = {c['name']: c['frames'] for c in read(original_file)['character_animation_probe']['cases']}
observed = {c['name']: c['frames'] for c in read(observed_file)['character_animation_probe']['cases']}
assert set(original) == set(observed) == set(native_labels)
source = {}
for label, case in zip(native_labels, request['cases'], strict=True):
    assert case['name'] == label
    assert case['force_apex_split'] == (label != 'ceiling-unsplit')
    assert case['apex_roof'] == (label != 'clear')
    for left, right in zip(original[label], observed[label], strict=True):
        for key in ('actor_transform', 'actor_velocity_cm_s', 'world_delta_seconds'):
            assert left[key] == right[key]
        for key in ('movement_before_tick', 'movement_after_tick'):
            assert all(right[key][k] == v for k, v in left[key].items())
    row = observed[label][49]
    assert row['frame'] == 49 and row['world_delta_seconds'] == f32(.05)
    assert row['gravity_cm_s2'] == -981
    before, after = (row[f'movement_{phase}_tick'] for phase in ('before', 'after'))
    assert before['movement_mode_name'] == after['movement_mode_name'] == 'MOVE_Falling'
    assert before['velocity_cm_s'] == {'x': 0, 'y': 0, 'z': 20}
    end = row['actor_transform']['translation']
    assert end['x'] == -1300 and end['y'] == -1500
    assert abs(after['velocity_cm_s']['z'] + 29.05) < .00001
    hits = row['blocking_hits']
    apex_time = f32(20 / 981)
    apex_height = f32(.5 * apex_time) * 20
    if label == 'ceiling-split':
        hit, = hits
        assert hit['component'] == 'AuditApexRoof'
        assert not hit['penetrating'] and 0 < hit['time'] < 1
        assert hit['trace_start_cm'] == {'x': -1300, 'y': -1500, 'z': 200}
        assert abs(hit['trace_end_cm']['z'] - 200 - apex_height) < 1e-6
        assert abs(hit['impact_point_cm']['z'] - 290.15) < .0001
        assert hit['impact_normal'] == {'x': 0, 'y': 0, 'z': -1}
        remaining = f32(row['world_delta_seconds'] - apex_time)
        predicted = hit['move_stop_cm']['z'] - 981 * remaining**2 * .5
        assert abs(predicted - end['z']) < .0001
    else:
        assert hits == []
        predicted = 200 + (20 - 29.05) * .5 * row['world_delta_seconds']
        assert abs(predicted - end['z']) < .0001
    source[label] = dict(hits=hits, end_z_cm=end['z'], end_velocity_cm_s=after['velocity_cm_s'])
assert .15 < source['ceiling-unsplit']['end_z_cm'] - source['ceiling-split']['end_z_cm'] < .16
assert abs(source['ceiling-unsplit']['end_z_cm'] - source['clear']['end_z_cm']) < 1e-6

target = {}
for label in ('ceiling', 'clear'):
    functional = read(root / f'{label}-functional.json')
    assert functional['pass'] and not functional['issues'] and functional['images'] == []
    assert load(label, 'process.report')['success']
    trace = load(label, 'apex-queries')
    assert trace['mode'] == 'Air'
    assert trace['position'] == [-13, 4, -15]
    assert trace['desired'] == [0, 0, 0] and trace['raw_velocity'] == trace['initial'] == [0, f32(.2), 0]
    assert trace['step'] == f32(.05) and trace['gravity'] == [0, f32(-9.81), 0]
    assert trace['full_hit'] is None and -.00227 < trace['full'][1] < -.00225
    assert .0203 < trace['apex_time'] < .0205 and .00203 < trace['ascent'][1] < .00205
    if label == 'ceiling':
        hit = trace['ascent_hit']
        assert hit['name'] == 'AuditApexRoof' and hit['normal'][1] < -.999
        assert .00145 < hit['distance'] < .00155
        assert abs(hit['point'][1] - 4.9015) < 1e-6
    else:
        assert trace['ascent_hit'] is None
    result = load(label, 'contact-after-49')
    assert result['mode'] == 'Air' and result['transitions'] == []
    assert abs(result['position'][1] - f32(trace['position'][1] + trace['full'][1])) < 1e-7
    assert abs(result['published'][1] + .2905) < 1e-6
    assert abs(result['position'][1] - (2 + source['ceiling-unsplit']['end_z_cm'] / 100)) < 1e-6
    target[label] = dict(queries=trace, actual=result)

assert target['ceiling']['actual'] == target['clear']['actual']
print('Verified native apex switch, actual upward hit and target midpoint sweep', flush=True)

paths = [original_file, observed_file]
paths.extend(root / f'runtime-{label}' / f'{name}.json' for label in target for name in ('parameters', 'report'))
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
    '523/wall.request.json', '523/native_initial.inc', '523/deck.rs', '523/observer.py',
    '519/prepare_reader.py', '553/prepare_reader.py')]
external = [dict(path=str(p.relative_to(Path.cwd())), **digest(p.read_bytes())) for p in external_paths]
summary = dict(gate='PASS_NATIVE_APEX_SWITCH_AND_REAL_CEILING_SWEEP', supplements=['BF144-D28'],
               new_differences=0, total_differences=303, ordinal=49, native=source, target=target,
               decompressed_files=decompressed, external_source_files=external)
(root / 'summary.json').write_text(json.dumps(summary, indent=2) + '\n')
print('Verified compressed evidence identities', flush=True)
