import gzip
import hashlib
import json
import struct
from pathlib import Path

root=Path(__file__).resolve().parent
prior=root.parent/'539'
labels=('grazing-verified','source-gap')

def raw(path):
    return path.read_bytes() if path.exists() else gzip.decompress(path.with_suffix('.json.gz').read_bytes())

def read(path):
    return json.loads(raw(path))

def identity(data):
    return dict(bytes=len(data),sha256=hashlib.sha256(data).hexdigest())

def save(name,result):
    (root/name).write_text(json.dumps(result,indent=2)+'\n')

def f32(value):
    return struct.unpack('<f',struct.pack('<f',value))[0]

def event(trace,kind):
    value,=[row for row in trace['events'] if row['kind']==kind]
    return value

def mapped(vector):
    return [vector[axis]*100 for axis in (0,2,1)]

assert read(root/'compile-floor.report.json')['success']
assert read(root/'geometry.native.json.reader.report.json')['success']
source=read(prior/'summary.json')
origin_source=source['source_position_m']
request=read(root/'geometry.request.json')['samples']
geom_path=root/'geometry.native.json'
response=read(geom_path)['geometry_query_probe']['samples']
assert [row['input'] for row in response]==request
queries={row['input']['name']:row for row in response}

min_gap,max_gap=f32(.019),f32(.024)
avg=f32(f32(min_gap+max_gap)*.5)

def height(origin,distance,shrink):
    floor_distance=f32(distance-shrink)
    center=f32(origin-floor_distance)
    gap=f32(origin-center)
    return dict(center=center, gap=gap, adjusted=not(min_gap<=gap<=max_gap),
                output_y=origin if min_gap<=gap<=max_gap else f32(center+avg))

results={}
for label in labels:
    runtime=root/f'runtime-{label}'
    old=prior/runtime.name
    functional=read(root/f'{label}-functional.json')
    assert functional['pass'] and not functional['issues'] and functional['images']==[]
    assert read(runtime/'process.report.json')['success']
    for name in ('gap-control','solver-exit','contact-before-40','contact-after-40'):
        assert read(runtime/f'{name}.json')==read(old/f'{name}.json')
    trace=read(runtime/'fresh-floor.json')
    before=read(runtime/'contact-before-40.json')
    after=read(runtime/'contact-after-40.json')
    assert trace['origin']==before['position']
    assert trace['mode']==before['mode']==after['mode']=='Grounded'
    assert trace['floor']==dict(center=before['floor']['center'],impact=before['floor']['impact'],
                               entity=before['floor']['name'])
    assert [row['kind'] for row in trace['events']]==['capsule','edge','capsule_qualification','selected']
    cast=event(trace,'capsule')
    qualify=event(trace,'capsule_qualification')
    selected=event(trace,'selected')
    assert qualify['valid'] and not selected['line'] and cast['origin']==trace['origin']
    assert selected['hit']==qualify['hit']==cast['hit']
    assert cast['hit']['entity']=='AuditWalkOffPlatform'
    assert abs(trace['box_center'][1]+trace['box_half_extents'][1]-2)<1e-7
    actual=height(trace['origin'][1],cast['hit']['distance_m'],qualify['shrink_m'])
    assert actual['center']==trace['floor']['center'] and actual['gap']==before['floor']['gap']
    assert actual['output_y']==after['position'][1]
    assert actual['adjusted']==(label=='source-gap')

    world,relative=[queries[f'{label}-{kind}'] for kind in ('world','relative')]
    for variant in (world,relative):
        info=variant['input']['capsule_sweep']
        assert info['box_half_extents_cm']==mapped(trace['box_half_extents'])
        assert info['radius_cm']==cast['radius_m']*100
        assert info['half_height_cm']==f32(trace['half_m']-qualify['shrink_m'])*100
        assert info['length_cm']==cast['cast_distance_m']*100 and info['direction']==[0,0,-1]
        assert variant['capsule_sweep']['hit']
        assert variant['capsule_sweep']['normal']=={'x':0,'y':0,'z':1}
    o=trace['origin']
    b=trace['box_center']
    assert world['input']['capsule_sweep']['start_cm']==mapped([o[0],o[1]-2,o[2]])
    assert world['input']['capsule_sweep']['box_translation_cm']==mapped([b[0],b[1]-2,b[2]])
    assert relative['input']['capsule_sweep']['start_cm']==mapped([a-z for a,z in zip(o,b,strict=True)])
    assert relative['input']['capsule_sweep']['box_translation_cm']==[0,0,0]
    chaos=world['capsule_sweep']['distance_cm']/100
    assert chaos==relative['capsule_sweep']['distance_cm']/100
    parry=cast['hit']['distance_m']
    difference=chaos-parry
    assert .00022<difference<.00026
    witness=cast['hit']['point2'][1]-cast['hit']['point'][1]
    assert abs(witness-difference)<1e-6
    corrected=height(o[1],f32(chaos),qualify['shrink_m'])
    assert corrected['adjusted']
    assert abs(corrected['output_y']-origin_source[1])<1e-6
    if label=='grazing-verified':
        assert actual['gap']<max_gap<corrected['gap']
    else:
        assert actual['gap']<corrected['gap']<min_gap
        assert abs(actual['output_y']-corrected['output_y']-difference)<1e-6
    results[label]=dict(raw_parry_distance_m=parry,raw_chaos_distance_m=chaos,
                        raw_distance_delta_m=difference,parry_witness_separation_m=witness,
                        actual=actual,with_chaos_distance=corrected,
                        actual_residual_m=actual['output_y']-origin_source[1])

decoded={}
for path in [geom_path]+[root/f'runtime-{label}'/f'{name}.json' for label in labels for name in (
    'parameters','report','walking-step','walking-transaction','step-down-replay')]:
    data=raw(path)
    archive=path.with_suffix('.json.gz')
    archive.write_bytes(gzip.compress(data,compresslevel=6,mtime=0))
    assert gzip.decompress(archive.read_bytes())==data
    decoded[str(path.relative_to(root))]=identity(data)
    if path.exists():
        path.unlink()
    print('Verified gzip '+str(path.relative_to(root)),flush=True)

motion='src/als/character/movement/motion.rs'
constants='src/als/settings/movement_constants.rs'
facts=[]
for key,ranges in ((motion,[(3846,3864),(3884,3902),(3927,3933),(4012,4027),(4065,4077)]),
                   (constants,[(1,12)])):
    data=Path(key).read_bytes()
    lines=data.decode().splitlines()
    facts.append(dict(key=key,**identity(data),excerpts=[
        dict(start=a,end=b,text='\n'.join(lines[a-1:b])+'\n') for a,b in ranges]))
save('source-facts.json',facts)

inputs={row['path'] for row in source['external_source_files']}
inputs.update(str(path.relative_to(Path.cwd())) for path in (
    prior/'summary.json',prior/'wall.native.json.gz',prior/'observer.py',prior/'gap_probe.rs',prior/'source-gap.json',
))
for label in labels:
    inputs.update(str((prior/f'runtime-{label}'/f'{name}.json').relative_to(Path.cwd())) for name in (
        'gap-control','solver-exit','contact-before-40','contact-after-40'))
save('summary.json',dict(
    gate='PASS_FRESH_FLOOR_COMMON_SWEEP_AND_HEIGHT_RECOMPUTATION',
    new_differences=0,total_differences=295,related_differences=['BF144-D125','BF144-D298'],
    ordinal=40,source_position_m=origin_source,results=results,
    decoded=decoded,screenshots=0,performance_acceptance='paused',
    external_source_files=[dict(path=name,**identity(Path(name).read_bytes())) for name in sorted(inputs)]))
print('Verified both live floor heights by replacing only the raw same-input sweep distance',flush=True)
