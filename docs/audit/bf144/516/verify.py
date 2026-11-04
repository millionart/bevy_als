import gzip
import hashlib
import json
import os
import struct
from pathlib import Path

root = Path(__file__).resolve().parent
parent = root.parent

def raw(name):
    path = root/name
    return path.read_bytes() if path.exists() else gzip.decompress(path.with_suffix('.json.gz').read_bytes())

def load(name):
    return json.loads(raw(name))

def save(name,value):
    (root/name).write_text(json.dumps(value,indent=2)+'\n')

def f32(value):
    return struct.unpack('<f',struct.pack('<f',value))[0]

def table(rows):
    return {row['ordinal']:row for row in rows}

assert load('build-ledge.report.json')['success']
assert not load('compile-duplicate-import.report.json')['success']
assert load('compile-initial-observer.report.json')['success']
assert load('compile-observed.report.json')['success']
assert load('ledge-wrong-reader.package.json')['package'].endswith('_510')
assert load('ledge.package.json')['package'].endswith('_516')
assert load('late.package.json')['sha256']==load('ledge.package.json')['sha256']
for label in ('ledge','late','ledge-wrong-reader'):
    assert load(f'{label}.native.json.reader.report.json')['success']
wrong = load('ledge-wrong-reader.native.json')['character_animation_probe']['cases']
assert all(row['movement_after_tick']['movement_mode_name']=='MOVE_Walking' for case in wrong for row in case['frames'])
del wrong

air_control = f32(.15)
native = {}
for label,frame,edge in (('ledge',46,35),('late',48,70)):
    report = load(f'{label}.native.json')
    settings = [p['numeric_value'] for obj in report['blueprint_cdo_subobjects']
                for p in obj['properties'] if p['name']=='AirControl']
    assert settings==[air_control]
    case = next(c for c in report['character_animation_probe']['cases'] if c['name'].endswith(f'-{edge}'))
    data = case['frames'][frame]
    before,after = data['movement_before_tick'],data['movement_after_tick']
    assert before['movement_mode_name']=='MOVE_Walking' and after['movement_mode_name']=='MOVE_Falling'
    assert data['world_delta_seconds']==f32(.06)
    assert before['velocity_cm_s']['z']==0
    cached = before['max_acceleration_cm_s2']/100
    assert cached==after['acceleration_cm_s2']['x']/100
    gravity = -data['gravity_cm_s2']/100
    end = after['velocity_cm_s']['x']/100
    air_time = -after['velocity_cm_s']['z']/100/gravity
    assert abs(air_time-.03)<1e-7
    walking = before['velocity_cm_s']['x']/100 + cached*(data['world_delta_seconds']-air_time)
    air_increment = end-walking
    assert abs(air_increment-cached*air_control*air_time)<1e-6
    native[label] = dict(frame=frame,width_cm=edge,cached_acceleration=cached,air_control=air_control,
        air_time=air_time,walking_speed=walking,air_increment=air_increment,end_speed=end)
assert native['ledge']['cached_acceleration']==20
assert 11<native['late']['cached_acceleration']<12
save('native-boundary.json',native)
print('Verified two native mode changes and cached lateral acceleration',flush=True)

labels=('ledge-35','ledge-40','ledge-100','early-baseline','early-walking-cached',
        'late-baseline','late-air-cached','late-restored')
reports={}
stages={}
for label in labels:
    for name in (f'{label}-functional-process.json',f'runtime-{label}/process.report.json'):
        assert load(name)['success']
    gate = load(f'{label}-functional.json')
    assert gate['pass'] and gate['images']==[] and gate['validation_scope']=='functional'
    fixture = load(f'runtime-{label}/fixture.json')
    profile = load(f'runtime-{label}/profile.json')
    width = (int(label.split('-')[-1]) if label.startswith('ledge-') else
             35 if label.startswith('early-') else 70)
    assert fixture['half_width_cm']==width and fixture['floor_y']==2
    assert profile['effective']['responsive']==profile['original']['normal']
    reports[label] = {key:table(rows) for key,rows in load(f'runtime-{label}/parameters.json').items()}
    before = load(f'runtime-{label}/stage-before.json')
    after = load(f'runtime-{label}/stage-after.json')
    frame = before['frame']
    assert before['mode']=='Grounded' and before['substeps']==after['substeps']==2
    assert before['delta']==after['delta']==f32(.06)
    assert before['support']==[0,0,0]
    row = reports[label]['after'][frame]
    assert row['gameplay']==after['mode'] and row['graph_error'] is None
    if label=='ledge-100':
        assert after['mode']=='Grounded'
    else:
        assert after['mode']=='Air'
        assert row['velocity']==after['collision_velocity']
        assert abs(-row['velocity'][1]/f32(9.81)-.03)<1e-6
        assert reports[label]['after'][frame-1]['gameplay']=='Grounded'
    stages[label] = dict(before=before,after=after,output=row)

early_a,early_b = (stages[label] for label in ('early-baseline','early-walking-cached'))
assert early_a['before']['cached_acceleration']==early_b['before']['walking_acceleration']==20
assert early_a['before']['fresh_acceleration']==early_b['before']['fresh_acceleration']
assert early_a['before']['relative']==early_b['before']['relative']
assert early_a['after']['collision_velocity'][0]<early_b['after']['collision_velocity'][0]
assert abs(early_b['after']['collision_velocity'][0]-native['ledge']['end_speed'])<0.0001
assert early_a['after']['published_grounded_acceleration']==early_b['after']['published_grounded_acceleration']

late_a,late_b,late_r = (stages[label] for label in ('late-baseline','late-air-cached','late-restored'))
assert late_a==late_r
for key in reports['late-baseline']:
    assert [row for i,row in reports['late-baseline'][key].items() if i<48]==[
        row for i,row in reports['late-air-cached'][key].items() if i<48]
    assert reports['late-baseline'][key][48]==reports['late-restored'][key][48]
assert {k:v for k,v in late_a['before'].items() if k!='air_maximum'}=={
    k:v for k,v in late_b['before'].items() if k!='air_maximum'}
assert late_a['after']['walking_last_input']==late_b['after']['walking_last_input']
assert late_a['after']['collision_velocity'][1]==late_b['after']['collision_velocity'][1]
assert late_a['after']['published_grounded_acceleration']==late_b['after']['published_grounded_acceleration']
assert late_a['before']['air_maximum']==20
assert late_b['before']['air_maximum']==late_b['before']['cached_acceleration']<20
target = {}
for label in ('late-baseline','late-air-cached'):
    stage = stages[label]
    v = stage['after']['collision_velocity']
    walking = stage['after']['walking_last_input'][0]
    air_time = -v[1]/f32(9.81)
    air_increment = v[0]-walking
    assert abs(air_increment-stage['before']['air_maximum']*air_control*air_time)<1e-6
    next_v = reports[label]['after'][49]['velocity']
    assert abs((next_v[0]-v[0])-0.1)<1e-6
    target[label] = dict(cached=stage['before']['cached_acceleration'],used_maximum=stage['before']['air_maximum'],
        walking_speed=walking,air_time=air_time,air_increment=air_increment,end_speed=v[0],
        lean_goal=stage['output']['lean']['goal'],lean_amounts=stage['output']['lean']['amounts'])
assert target['late-baseline']['end_speed']-target['late-air-cached']['end_speed']>.03
assert target['late-baseline']['lean_amounts']!=target['late-air-cached']['lean_amounts']
restored_diff = {key:next((i for i,row in rows.items() if row!=reports['late-restored'][key][i]),None)
                 for key,rows in reports['late-baseline'].items()}
assert all(i is None or i>48 for i in restored_diff.values())
save('actual-comparison.json',dict(native=native,late=target,early_baseline=early_a,early_cached=early_b,
    restored_first_difference=restored_diff))
print('Verified distinct walking and air cache consumers through actual transitions',flush=True)

decoded = {}
for name in ['ledge.native.json','late.native.json','ledge-wrong-reader.native.json'] + [
    f'runtime-{label}/{file}.json' for label in labels for file in ('report','parameters')]:
    data = raw(name)
    path = root/name
    archive = path.with_suffix('.json.gz')
    if path.exists():
        archive.write_bytes(gzip.compress(data,compresslevel=6,mtime=0))
    assert gzip.decompress(archive.read_bytes())==data
    decoded[name] = dict(bytes=len(data),sha256=hashlib.sha256(data).hexdigest())
    print('Verified gzip '+name,flush=True)
env = json.loads(Path('tools/env.local.json').read_bytes())
als = Path(env['ue_project']).parent/'Plugins/ALS-Refactored/Source/ALS'
ue = Path(os.environ['ALS_UE_SOURCE_ROOT'])
facts = []
for key,path,ranges in (
    ('source:Private/AlsCharacterMovementComponent.cpp',als/'Private/AlsCharacterMovementComponent.cpp',[(292,301),(333,341)]),
    ('Engine/Source/Runtime/Engine/Private/Components/CharacterMovementComponent.cpp',
     ue/'Engine/Source/Runtime/Engine/Private/Components/CharacterMovementComponent.cpp',[(4827,4858),(5427,5457),(6442,6457)]),
    ('src/als/character/movement/dynamics.rs',Path('src/als/character/movement/dynamics.rs'),[(303,327)]),
    ('src/als/character/movement/motion.rs',Path('src/als/character/movement/motion.rs'),[(118,129),(2752,2779),(3770,3807)]),
):
    data = path.read_bytes()
    lines = data.decode().splitlines()
    facts.append(dict(key=key,sha256=hashlib.sha256(data).hexdigest(),
        excerpts=[dict(start=a,end=b,text='\n'.join(lines[a-1:b])+'\n') for a,b in ranges]))
save('source-facts.json',facts)
save('final-source.json',dict(sha256=hashlib.sha256((root/'ledge_observer.rs').read_bytes()).hexdigest()))
paths = [parent/name for name in (
    '484/playground_observed.rs','484/foot_observer.rs','505/observer.py','505/velocity_observer.rs',
    '509/observer.py','509/lean_observer.rs','510/observer.py','510/acceleration_observer.rs',
    '512/observer.py','512/sprint_observer.rs','515/sprint.request.json',
    '513/observer.py','513/profile.rs','515/summary.json','491/reader-migration.json','441c/library-identities.json')]
external = [dict(path=str(path.resolve().relative_to(Path.cwd())),bytes=path.stat().st_size,
                 sha256=hashlib.sha256(path.read_bytes()).hexdigest()) for path in paths]
save('summary.json',dict(
    gate='PASS_SAME_FRAME_WALK_OFF_ACCELERATION_HANDOFF',new_differences=1,total_differences=284,
    difference='BF144-D287',related='BF144-D07',native=native,target=target,
    restored_witness_exact=True,restored_first_difference=restored_diff,screenshots=0,
    performance_acceptance='paused',decoded=decoded,external_source_files=external))
