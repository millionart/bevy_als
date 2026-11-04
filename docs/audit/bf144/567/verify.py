import gzip
import hashlib
import json
from pathlib import Path

root = Path(__file__).resolve().parent
labels = ('late', 'still')
old_base = 'AuditWalkOffPlatform'
new_base = 'AuditWalkOffSecondBase'


def raw(path):
    if path.exists():
        return path.read_bytes()
    return gzip.decompress(path.with_suffix('.json.gz').read_bytes())


def read(path):
    return json.loads(raw(path))


def digest(data):
    return dict(bytes=len(data), sha256=hashlib.sha256(data).hexdigest())


def world(vector):
    return [vector['x'] / 100, 2 + vector['z'] / 100, vector['y'] / 100]


def nearby(a, b, tolerance=2e-6):
    assert len(a) == len(b)
    assert all(abs(x - y) < tolerance for x, y in zip(a, b)), (a, b)


for name in ('build-late.report', 'late.native.json.reader.report', 'compile-late.report'):
    assert read(root / f'{name}.json')['success']
assert all(result == 'PASS' for result in read(root / 'watchdog-selftest.json').values())
source_path = root / 'late.native.json'
cases = {case['name']: case['frames'][40] for case in read(source_path)['character_animation_probe']['cases']}
assert set(cases) == set(labels)
native, target = {}, {}
for label, offset in (('late', .2), ('still', 0)):
    row = cases[label]
    before, after = (row[f'movement_{phase}_tick'] for phase in ('before', 'after'))
    event = row['late_old_base']
    assert row['frame'] == 40 and abs(row['world_delta_seconds'] - .1) < 2e-8
    assert before['movement_mode_name'] == after['movement_mode_name'] == 'MOVE_Walking'
    assert before['base_component'] == 'BoxComponent_0'
    assert event['actor_base'] == after['base_component'] == new_base
    assert before['velocity_cm_s'] == dict(x=650, y=0, z=0)
    assert not row['mode_events'] and not row['blocking_hits']
    assert event['actor_cm'] == row['actor_transform']['translation']
    assert event['old_after_cm']['y'] - event['old_before_cm']['y'] == offset * 100
    source_position = world(event['actor_cm'])
    native[label] = dict(base=event['actor_base'], actor=source_position, old_move_m=offset)

    run = root / f'runtime-{label}'
    functional = read(root / f'{label}-functional.json')
    assert functional['pass'] and functional['images'] == [] and not functional['issues']
    assert read(run / 'process.report.json')['success']
    txn = read(run / 'walking-transaction.json')
    assert txn['mode'] == 'Grounded' and txn['end_floor']['variants']['none']['result']['entity'] == new_base
    stage = read(run / 'stage-after.json')
    assert stage['substeps'] == 2 and stage['frame'] == 40
    injected = read(run / 'late-base-before.json')
    pre = read(run / 'contact-before-40.json')
    post = read(run / 'late-base-after.json')
    contact = read(run / 'contact-after-40.json')
    assert injected['actor'] == pre['position'] == txn['position']
    assert injected['base'] == old_base and injected['old_velocity'] == [0, 0, 0]
    assert pre['floor']['name'] == post['base'] == new_base
    assert injected['mode'] == pre['mode'] == post['mode'] == contact['mode'] == 'Grounded'
    assert not pre['manifolds'] and not contact['transitions']
    assert contact['has_base'] and contact['position'] == post['actor']
    nearby(injected['actor'], source_position, tolerance=1e-4)
    nearby([end - start for start, end in zip(injected['old_before'], injected['old_after'])], [0, 0, offset])
    nearby([end - start for start, end in zip(injected['actor'], post['actor'])], [0, 0, offset])
    nearby([post['actor'][2] - source_position[2]], [offset])
    target[label] = dict(before_follow_base=injected['base'], end_floor=pre['floor']['name'],
                         published_base=post['base'], before_follow_actor=injected['actor'],
                         final_actor=post['actor'], old_move_m=offset)

assert native['late']['actor'] == native['still']['actor']
assert target['late']['before_follow_actor'] == target['still']['before_follow_actor']
print('Verified a late old-base displacement with a fresh end-floor and unchanged Walking mode', flush=True)

paths = [source_path]
paths.extend(root / f'runtime-{label}' / f'{name}.json' for label in labels
             for name in ('parameters', 'report', 'walking-step', 'walking-transaction'))
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
for name in ('566/bases.request.json', '566/prepare_reader.py', '566/native_base.inc', '566/native_velocity.inc',
             '566/observer.py', '533/observer.py', '533/walking_probe.rs', '522/contact.rs'):
    path = root.parent / name
    external.append(dict(path=str(path.relative_to(Path.cwd())), **digest(path.read_bytes())))
summary = dict(gate='PASS_WALKING_SWITCH_BEFORE_OLD_BASE_POST_PHYSICS', findings=['BF144-D313'],
               new_differences=0, total_differences=309, ordinal=40, native=native, target=target,
               decompressed_files=decompressed, external_source_files=external)
(root / 'summary.json').write_text(json.dumps(summary, indent=2) + '\n')
print('Verified compressed evidence identities', flush=True)
