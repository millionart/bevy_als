import gzip
import hashlib
import json
import os
import struct
from pathlib import Path

root = Path(__file__).resolve().parent
prior = root.parent / '528'
labels = ('top-205','top-215','top-215-raised','top-225')
phase_names = ('post','target-stop','native-stop')
sources = {'top-205':'527','top-215':'528','top-215-raised':'528','top-225':'527'}

def raw(path):
    return path.read_bytes() if path.exists() else gzip.decompress(path.with_suffix('.json.gz').read_bytes())

def read(path):
    return json.loads(raw(path))

def identity(data):
    return dict(bytes=len(data), sha256=hashlib.sha256(data).hexdigest())

def save(name, result):
    (root / name).write_text(json.dumps(result, indent=2) + '\n')

def f32(value):
    return struct.unpack('<f', struct.pack('<f', value))[0]

def selected(variant):
    final, = [row for row in variant['events'] if row['kind']=='selected']
    return final

native = read(prior/'summary.json')['native']
assert read(root/'compile-provenance.report.json')['success']
origins = read(root/'native-origins.json')
summary, restoration = {}, {}
for label in labels:
    runtime = root/f'runtime-{label}'
    previous = root.parent/sources[label]/runtime.name
    functional = read(root/f'{label}-functional.json')
    assert functional['pass'] and not functional['issues'] and functional['images']==[]
    assert read(runtime/'process.report.json')['success']
    for name in ('initial-observed','stage-after','contact-before-49','contact-after-49',
                 'contact-before-50','contact-after-50'):
        assert read(runtime/f'{name}.json') == read(previous/f'{name}.json')
    trace = read(runtime/'floor-provenance.json')
    before = read(runtime/'contact-before-49.json')
    assert trace['mode']==before['mode']=='Air'
    point = native[label]['first_hit']['move_stop_cm']
    assert origins[label] == [point['x']/100, 2+point['z']/100, point['y']/100]
    assert trace['phases']['native-stop']['origin'] == [f32(x) for x in origins[label]]
    assert trace['phases']['post']['origin'] == before['position']
    post_floor = trace['phases']['post']['public']
    assert post_floor == dict(entity=before['floor']['name'], center=before['floor']['center'], impact=before['floor']['impact'])
    assert set(trace['phases']) == set(phase_names)
    phases = {}
    for name in phase_names:
        stage = trace['phases'][name]
        v = stage['variants']
        hit = selected(v['none'])
        assert not hit['line'] and hit['hit']['entity']==stage['public']['entity']
        assert v['none']['result']==stage['public']==v['center']['result']==v['perch']['result']
        events = v['none']['events']
        kinds = [event['kind'] for event in events]
        assert 'center_ray' not in kinds
        wall = label=='top-205' or (label=='top-215' and name!='post')
        if wall:
            assert hit['hit']['entity']=='AuditWalkOffBarrier'
            first = events[0]
            assert first['kind']=='capsule' and first['hit']['entity']=='AuditWalkOffBarrier'
            assert first['hit']['normal'][1] < .71
            short, = [event['hit'] for event in events if event['kind']=='short_ray']
            assert short['same_entity'] and short['normal']==[0,1,0]
            assert 'perch' in kinds
            assert v['short']['result']['entity']=='AuditLandingDeck'
        else:
            assert hit['hit']['entity']=='AuditLandingDeck'
            assert kinds==['capsule','edge','capsule_qualification','selected']
            assert events[0]['hit']['entity']=='AuditLandingDeck'
            assert v['short']['result']==stage['public']
        phases[name] = dict(origin=stage['origin'], actual_floor=stage['public'],
                            line=hit['line'], raw_first_normal=events[0]['hit']['normal'],
                            short_ray='short_ray' in kinds, perch='perch' in kinds,
                            ablated_short_floor=v['short']['result'], ablated_short_line=selected(v['short'])['line'])
    native_floor = native[label]['first_hit']['floor_at_move_stop']
    assert not native_floor['line_trace']
    native_entity = {'AuditWall':'AuditWalkOffBarrier','AuditLandingDeck':'AuditLandingDeck'}[native_floor['component']]
    assert phases['native-stop']['actual_floor']['entity']==native_entity
    if label=='top-205':
        assert phases['post']['ablated_short_line']
    summary[label] = dict(phases=phases,native_at_stop=native_floor['component'],
                          native_gap_cm=native_floor['floor_dist_cm'],
                          actual_target_mode=read(runtime/'contact-after-49.json')['mode'])

    data = raw(runtime/'parameters.json')
    expected = raw(previous/'parameters.json')
    difference = {}
    for group, rows in json.loads(data).items():
        baseline = json.loads(expected)[group]
        assert [row['ordinal'] for row in rows]==[row['ordinal'] for row in baseline]
        first = next((row['ordinal'] for row, other in zip(rows,baseline,strict=True) if row!=other), None)
        assert first is None or first>50
        difference[group] = first
    restoration[label] = dict(byte_exact=data==expected, first_different_ordinal=difference)

assert [summary[label]['actual_target_mode'] for label in labels]==['Air','Air','Grounded','Air']
assert not native['top-215']['first_hit']['floor_at_move_stop']['valid_landing_spot']
assert native['top-215-raised']['actual_landed_events'][0]['component']=='AuditLandingDeck'

decoded = {}
for label in labels:
    for name in ('parameters','report','floor-provenance'):
        path=root/f'runtime-{label}'/f'{name}.json'
        data=raw(path)
        archive=path.with_suffix('.json.gz')
        archive.write_bytes(gzip.compress(data,compresslevel=6,mtime=0))
        assert gzip.decompress(archive.read_bytes())==data
        decoded[str(path.relative_to(root))]=identity(data)
        if path.exists():
            path.unlink()
        print('Verified gzip '+str(path.relative_to(root)),flush=True)

ue = Path(os.environ['ALS_UE_SOURCE_ROOT'])
cmc = 'Engine/Source/Runtime/Engine/Private/Components/CharacterMovementComponent.cpp'
motion = 'src/als/character/movement/motion.rs'
constants = 'src/als/settings/movement_constants.rs'
facts = []
for key, path, ranges in (
    (cmc,ue/cmc,[(5061,5076),(7374,7424)]),
    (motion,Path(motion),[(3846,3933),(4074,4120)]),
    (constants,Path(constants),[(1,12),(26,27)]),
):
    data=path.read_bytes()
    lines=data.decode().splitlines()
    facts.append(dict(key=key,**identity(data),excerpts=[
        dict(start=a,end=b,text='\n'.join(lines[a-1:b])+'\n') for a,b in ranges]))
save('source-facts.json',facts)

inputs = {row['path'] for row in read(prior/'summary.json')['external_source_files']}
inputs.update(str(path.relative_to(Path.cwd())) for path in (
    prior/'summary.json', root.parent/'526'/'observer.py', root.parent/'526'/'identity-check.json',
))
for label in labels:
    previous=root.parent/sources[label]/f'runtime-{label}'
    inputs.update(str((previous/f'{name}.json').relative_to(Path.cwd())) for name in (
        'initial-observed','stage-after','contact-before-49','contact-after-49','contact-before-50','contact-after-50'))
    inputs.add(str((previous/'parameters.json.gz').relative_to(Path.cwd())))
save('summary.json',dict(
    gate='PASS_ACTUAL_FLOOR_PROVENANCE_CAPSULE_AND_QUERY_POSITION', new_differences=0,
    total_differences=289, related_difference='BF144-D292', ordinal=49,
    phases=summary, prior_parameter_report_comparison=restoration, decoded=decoded,
    screenshots=0,performance_acceptance='paused',
    external_source_files=[dict(path=name,**identity(Path(name).read_bytes())) for name in sorted(inputs)]))
print('Verified live floor provenance, all three diagnostic ablations and prior behavior',flush=True)
