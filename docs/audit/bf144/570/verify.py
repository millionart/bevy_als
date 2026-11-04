import gzip
import hashlib
import json
from pathlib import Path

root = Path(__file__).resolve().parent
labels = ('dynamic', 'disabled', 'static')
deck = 'AuditLandingDeck'
zero = dict(x=0, y=0, z=0)


def raw(path):
    if path.exists():
        return path.read_bytes()
    return gzip.decompress(path.with_suffix('.json.gz').read_bytes())


def read(path):
    return json.loads(raw(path))


def digest(data):
    return dict(bytes=len(data), sha256=hashlib.sha256(data).hexdigest())


for name in ('build-impact.report', 'impact.native.json.reader.report', 'compile-impact.report'):
    assert read(root / f'{name}.json')['success']
assert all(result == 'PASS' for result in read(root / 'watchdog-selftest.json').values())
source_path = root / 'impact.native.json'
unarmed_path = root / 'unarmed.native.json'
for case in read(unarmed_path)['character_animation_probe']['cases']:
    row = case['frames'][40]
    assert row['movement_before_tick']['movement_mode_name'] == 'MOVE_Walking'
    assert row['landed_events'] == []
cases = {case['name']: case['frames'] for case in read(source_path)['character_animation_probe']['cases']}
assert set(cases) == set(labels)
native, target = {}, {}
for label in labels:
    row = cases[label][40]
    next_row = cases[label][41]
    assert row['frame'] == 40 and abs(row['world_delta_seconds'] - .05) < 2e-8
    before, after = (row[f'movement_{phase}_tick'] for phase in ('before', 'after'))
    assert before['movement_mode_name'] == 'MOVE_Falling'
    assert before['velocity_cm_s'] == dict(x=0, y=0, z=-500)
    assert after['movement_mode_name'] == 'MOVE_Walking'
    landed, = row['landed_events']
    assert landed['component'] == deck and not landed['penetrating']
    assert landed['impact_normal']['z'] > .99999
    sample = row['landing_body']
    assert sample['standing_force_scale'] == 0
    assert sample['interaction'] == (label != 'disabled')
    assert sample['simulates'] == (label != 'static')
    assert sample['velocity_before_cm_s'] == zero
    if sample['simulates']:
        assert abs(sample['mass_kg'] - 100) < .01
    velocity = sample['velocity_after_cm_s']
    after_next = next_row['landing_body']
    if label == 'dynamic':
        assert -5.01 < velocity['z'] < -4.99 and velocity['x'] == velocity['y'] == 0
        assert after_next['velocity_before_cm_s'] == velocity
        assert sample['position_after_cm']['z'] < sample['position_before_cm']['z']
    else:
        assert velocity == after_next['velocity_after_cm_s'] == zero
        assert sample['position_before_cm'] == sample['position_after_cm']
    native[label] = dict(body=sample, next_velocity_cm_s=after_next['velocity_after_cm_s'],
                         landed=landed, actor=after['velocity_cm_s'])

assert native['dynamic']['landed'] == native['disabled']['landed'] == native['static']['landed']

for label in ('dynamic', 'static'):
    run = root / f'runtime-{label}'
    functional = read(root / f'{label}-functional.json')
    assert functional['pass'] and functional['images'] == [] and not functional['issues']
    assert read(run / 'process.report.json')['success']
    initial = read(run / 'initial-observed.json')
    before = read(run / 'deck-before.json')
    after = read(run / 'deck-after-40.json')
    following = read(run / 'deck-after-41.json')
    pre = read(run / 'contact-before-40.json')
    contact = read(run / 'contact-after-40.json')
    assert initial['mode'] == 'Air' and initial['initial_with_motor'] == [0, -5, 0]
    assert initial['desired'] == [0, 0, 0]
    assert initial['wall'] == pre['floor']['name'] == after['actor_base'] == following['actor_base'] == deck
    assert abs(initial['hit_distance'] - .17) < .001
    assert pre['generated_contact'] and pre['manifolds'] == []
    assert contact['mode'] == 'Grounded' and contact['has_base']
    assert contact['transitions'] == [['LocomotionState(Air)', 'LocomotionState(Grounded)']]
    assert before['body'] == label.title()
    if label == 'dynamic':
        assert before['mass'] == after['mass'] == following['mass'] == 100
    assert before['velocity'] == after['velocity'] == following['velocity'] == [0, 0, 0]
    assert before['position'] == after['position'] == following['position']
    assert after['contact_impulses'] == following['contact_impulses'] == []
    target[label] = dict(before=before, after_landing=after, following=following)

print('Verified native landing-only force and unchanged target dynamic body', flush=True)
paths = [source_path, unarmed_path]
paths.extend(root / f'runtime-{label}' / f'{name}.json'
             for label in ('dynamic', 'static') for name in ('parameters', 'report'))
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
for name in ('566/bases.request.json', '553/prepare_reader.py', '523/observer.py',
             '523/deck.rs', '523/native_initial.inc', '522/contact.rs'):
    path = root.parent / name
    external.append(dict(path=str(path.relative_to(Path.cwd())), **digest(path.read_bytes())))
summary = dict(gate='PASS_SIMULATED_DECK_LANDING_IMPACT', findings=['BF144-D38'],
               new_differences=0, total_differences=310, ordinal=40, native=native, target=target,
               decompressed_files=decompressed, external_source_files=external)
(root / 'summary.json').write_text(json.dumps(summary, indent=2) + '\n')
print('Verified compressed evidence identities', flush=True)
