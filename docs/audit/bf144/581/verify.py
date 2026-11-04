import gzip
import hashlib
import json
from pathlib import Path

root = Path(__file__).resolve().parent
labels = ('near-normal', 'outside-angle', 'single')


def raw(path):
    if path.exists():
        return path.read_bytes()
    return gzip.decompress(path.with_suffix('.json.gz').read_bytes())


def read(path):
    return json.loads(raw(path))


def digest(data):
    return dict(bytes=len(data), sha256=hashlib.sha256(data).hexdigest())


def dot(a, b):
    return sum(x*y for x,y in zip(a,b))


def xyz(value):
    return [value[key] for key in 'xyz']


for name in ('build-nudge.report', 'nudge.native.json.reader.report', 'compile-nudge.report'):
    assert read(root / f'{name}.json')['success']
assert all(result == 'PASS' for result in read(root / 'watchdog-selftest.json').values())
source_path = root / 'nudge.native.json'
cases = {case['name']: case['frames'] for case in read(source_path)['character_animation_probe']['cases']}
assert set(cases) == set(labels)
native, target = {}, {}
for label in labels:
    row = cases[label][49]
    before, after = (row[f'movement_{phase}_tick'] for phase in ('before','after'))
    assert row['frame'] == 49 and abs(row['world_delta_seconds']-.05) < 2e-8
    assert before['movement_mode_name'] == after['movement_mode_name'] == 'MOVE_Falling'
    assert before['velocity_cm_s'] == dict(x=200,y=400,z=-200)
    assert not row['mode_events'] and not row['landed_events']
    assert after['last_input_world'] == dict(x=0,y=0,z=0)
    hits = row['blocking_hits']
    first = hits[0]
    assert first['component'] == 'AuditNudgeFirst' and first['time'] == 0
    assert first['normal'] == dict(x=-1,y=0,z=0)
    assert not first['penetrating'] and not first['walkable_hit']
    velocity = xyz(after['velocity_cm_s'])
    if label == 'single':
        assert len(hits) == 1 and abs(velocity[0]) < 1e-8
    else:
        assert len(hits) == 2 and hits[1]['component'] == 'AuditNudgeSecond'
        second = hits[1]
        assert .45 < second['time'] < .75
        assert not second['penetrating'] and not second['walkable_hit']
        normal = xyz(second['normal'])
        assert abs(normal[2]) < 1e-8
        cosine = dot(xyz(first['normal']), normal)
        remaining = row['world_delta_seconds'] * (1-first['time']) * (1-second['time'])
        end = xyz(row['actor_transform']['translation'])
        delta = [a-b for a,b in zip(end,xyz(second['move_stop_cm']))]
        displacement = dot(delta,normal)
        projected = dot(velocity,normal)
        if label == 'near-normal':
            assert 0 < 1-cosine < 1e-4
            assert abs(displacement-.01) < 2e-6
            assert abs(projected*remaining-.01) < 2e-6
        else:
            assert 1-cosine > 1e-4
            assert abs(displacement) < 2e-6 and abs(projected*remaining) < 2e-6
        native[label] = dict(normal_dot=cosine, remaining_seconds=remaining,
                             displacement_cm=displacement, velocity_cm_s=projected)

    path = root / f'runtime-{label}'
    functional = read(root / f'{label}-functional.json')
    assert functional['pass'] and functional['images'] == [] and not functional['issues']
    assert read(path / 'process.report.json')['success']
    initial = read(path / 'initial-observed.json')
    assert initial['initial_with_motor'] == [2,-2,4] and initial['desired'] == [0,0,0]
    assert initial['wall'] == 'AuditNudgeFirst'
    result = read(path / 'contact-after-49.json')
    assert result['mode'] == 'Air' and not result['has_base'] and not result['transitions']
    assert read(path / 'contact-before-49.json')['floor'] is None
    if label == 'single':
        assert initial['second_hit'] is None and abs(result['published'][0]) < .001
    else:
        assert initial['second_hit'][0] == 'AuditNudgeSecond'
        assert .45 < initial['second_fraction'] < .75
        normal = initial['second_normal']
        cosine = dot(initial['normal'],normal)
        assert (1-cosine < 1e-4) == (label == 'near-normal')
        projected = dot(result['published'],normal)
        assert abs(projected) < 2e-6
        target[label] = dict(normal_dot=cosine,velocity_m_s=projected,position=result['position'])
    target.setdefault(label,dict(position=result['position']))
    native.setdefault(label,dict(velocity_cm_s=velocity))

print('Verified the native same-normal nudge and the target normal projection',flush=True)
paths = [source_path]
paths.extend(root / f'runtime-{label}' / f'{name}.json'
             for label in labels for name in ('parameters','report'))
decompressed = {}
for path in paths:
    data = raw(path)
    archive = path.with_suffix('.json.gz')
    if path.exists():
        archive.write_bytes(gzip.compress(data,compresslevel=6,mtime=0))
    assert gzip.decompress(archive.read_bytes()) == data
    decompressed[str(archive.relative_to(root))] = digest(data)
    path.unlink(missing_ok=True)

external = []
for name in ('580/ditch.request.json','580/observer.py','553/prepare_reader.py',
             '523/native_initial.inc','523/deck.rs','522/contact.rs'):
    path = root.parent / name
    external.append(dict(path=str(path.relative_to(Path.cwd())), **digest(path.read_bytes())))
summary = dict(gate='PASS_FALLING_REPEATED_NORMAL_NUDGE', findings=['BF144-D316'],
               new_differences=1, total_differences=312, native=native, target=target,
               decompressed_files=decompressed, external_source_files=external)
(root / 'summary.json').write_text(json.dumps(summary,indent=2)+'\n')
print('Verified compressed evidence identities',flush=True)
