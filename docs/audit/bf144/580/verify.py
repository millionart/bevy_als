import gzip
import hashlib
import json
import math
from pathlib import Path

root = Path(__file__).resolve().parent
labels = ('tight', 'wide', 'without-wall')


def raw(path):
    if path.exists():
        return path.read_bytes()
    return gzip.decompress(path.with_suffix('.json.gz').read_bytes())


def read(path):
    return json.loads(raw(path))


def digest(data):
    return dict(bytes=len(data), sha256=hashlib.sha256(data).hexdigest())


for name in ('build-ditch.report', 'ditch.native.json.reader.report', 'repeat.native.json.reader.report',
             'compile-ditch.report'):
    assert read(root / f'{name}.json')['success']
assert all(result == 'PASS' for result in read(root / 'watchdog-selftest.json').values())
native, target = {}, {}
for sample in ('ditch', 'repeat'):
    report = read(root / f'{sample}.native.json')
    cases = {case['name']: case['frames'] for case in report['character_animation_probe']['cases']}
    assert set(cases) == set(labels)
    native[sample] = {}
    for label in labels:
        row = cases[label][49]
        later = cases[label][50]
        assert row['frame'] == 49 and abs(row['world_delta_seconds'] - .05) < 2e-8
        before, after = (row[f'movement_{phase}_tick'] for phase in ('before','after'))
        assert before['movement_mode_name'] == after['movement_mode_name'] == 'MOVE_Falling'
        assert not row['mode_events'] and not row['landed_events']
        assert before['velocity_cm_s'] == dict(x=-50,y=2,z=-200)
        assert after['last_input_world'] == dict(x=0,y=0,z=0)
        hits = row['blocking_hits']
        first = hits[0]
        assert first['component'] == 'AuditDitchLedge'
        assert first['walkable_hit'] and first['impact_normal']['z'] > .71
        assert first['time'] == 0 and not first['penetrating']
        floor = first['floor_at_move_stop']
        assert floor['perch_requested'] and not floor['perch_valid'] and floor['perch_radius_cm'] == 15
        assert not floor['valid_landing_spot']
        velocity = after['velocity_cm_s']
        end = row['actor_transform']['translation']
        initial = first['trace_start_cm']
        if label == 'tight':
            assert len(hits) in (2,3)
            second = hits[1]
            assert second['component'] == 'AuditDitchWall'
            assert second['time'] == 0 and second['impact_normal']['z'] == 0
            assert velocity['z'] == 105
            if len(hits) == 3:
                kick = hits[2]
                kick_start = kick['trace_start_cm']
                assert kick['component'] == 'AuditDitchWall'
                assert abs(kick['trace_end_cm']['z'] - kick_start['z'] - 5.25) < 1e-5
            else:
                kick_start = {axis: end[axis] - velocity[axis]*row['world_delta_seconds'] for axis in 'xyz'}
            dx,dy,dz = (kick_start[axis]-initial[axis] for axis in 'xyz')
            assert 0 < math.hypot(dx,dy) <= 4*row['world_delta_seconds']
            assert abs(dz) <= .2*row['world_delta_seconds']
            assert later['actor_transform']['translation']['z'] - end['z'] > 1
            assert later['movement_after_tick']['velocity_cm_s']['z'] > 0
        elif label == 'wide':
            assert len(hits) == 2 and hits[1]['component'] == 'AuditDitchWall'
            assert .18 < hits[1]['time'] < .22
            assert abs(end['z'] - initial['z']) > .2*row['world_delta_seconds']
            assert velocity['z'] == 0
        else:
            assert len(hits) == 1 and -.80 < velocity['z']/100 < -.76
        native[sample][label] = dict(position_cm=end, velocity_cm_s=velocity,
                                     next_position_cm=later['actor_transform']['translation'],
                                     hits=len(hits))

for label in labels:
    run = root / f'runtime-{label}'
    assert read(run / 'process.report.json')['success']
    functional = read(root / f'{label}-functional.json')
    assert functional['pass'] and functional['images'] == [] and not functional['issues']
    initial = read(run / 'initial-observed.json')
    physical = read(run / 'contact-before-49.json')
    after = read(run / 'contact-after-49.json')
    later = read(run / 'contact-after-50.json')
    assert initial['wall'] == 'AuditDitchLedge'
    assert .765 < initial['normal'][1] < .767
    assert initial['initial_with_motor'][:2] == [-.5,-2]
    assert abs(initial['initial_with_motor'][2]-.02) < 1e-9
    assert initial['desired'] == [0,0,0]
    assert physical['floor'] is None and physical['generated_contact']
    assert after['mode'] == later['mode'] == 'Air'
    assert not after['has_base'] and not after['transitions']
    second = initial['second_hit']
    if label == 'tight':
        assert second[0] == 'AuditDitchWall' and 0 < second[1] < .002
        assert abs(after['velocity'][1]) < 1e-5
        assert abs(after['position'][1] - initial['position'][1]) < .00001
        assert abs(after['position'][2] - initial['position'][2]) < .002
        assert later['position'][1] - after['position'][1] < .00001
    elif label == 'wide':
        assert second[0] == 'AuditDitchWall' and .015 < second[1] < .022
        assert abs(after['velocity'][1]) < 1e-5 and after['position'][1] < initial['position'][1] - .007
    else:
        assert second is None and -.80 < after['velocity'][1] < -.76
    target[label] = dict(initial=initial, before_publish=physical, result=after, next=later)

print('Verified the source virtual-ditch impulse, ordinary controls, and missing target consumer',flush=True)
paths = [root / f'{name}.native.json' for name in ('ditch','repeat')]
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
for name in ('562/side.request.json','553/prepare_reader.py','523/native_initial.inc',
             '523/observer.py','523/deck.rs','522/contact.rs'):
    path = root.parent / name
    external.append(dict(path=str(path.relative_to(Path.cwd())), **digest(path.read_bytes())))
summary = dict(gate='PASS_FALLING_VIRTUAL_DITCH_KICK', findings=['BF144-D315'],
               new_differences=1, total_differences=311, native=native, target=target,
               decompressed_files=decompressed, external_source_files=external)
(root / 'summary.json').write_text(json.dumps(summary,indent=2)+'\n')
print('Verified compressed evidence identities',flush=True)
