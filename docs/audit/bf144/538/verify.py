import gzip
import hashlib
import json
import math
import os
from pathlib import Path

root=Path(__file__).resolve().parent
prior=root.parent/'537'
labels=('baseline','masked','restored')

def raw(path):
    return path.read_bytes() if path.exists() else gzip.decompress(path.with_suffix('.json.gz').read_bytes())

def read(path):
    return json.loads(raw(path))

def identity(data):
    return dict(bytes=len(data),sha256=hashlib.sha256(data).hexdigest())

def save(name,result):
    (root/name).write_text(json.dumps(result,indent=2)+'\n')

def run_label(label):
    return 'baseline-verified' if label=='baseline' else label

def file(label,name):
    return root/f'runtime-{run_label(label)}'/f'{name}.json'

def load(label,name):
    return read(file(label,name))

assert read(root/'compile-solver.report.json')['success']
assert not read(root/'compile-immutable-layer.report.json')['success']
assert not load('baseline-constraints-failure','process.report')['success']
assert read(root/'runtime-baseline/process.report.json')['exit_reason']=='no_progress_timeout'

previous=read(prior/'summary.json')
source=previous['native']['supported']
assert len(source['hits'])==2 and source['hits'][1]['reused_down_hit_floor']['walkable']
assert source['velocity_cm_s']['y']==0 and source['position_m'][2]==-15
assert source['mode_events']==[]

target={}
for label in labels:
    functional=read(root/f'{run_label(label)}-functional.json')
    assert functional['pass'] and not functional['issues'] and functional['images']==[]
    assert load(label,'process.report')['success']
    replay=load(label,'step-down-replay')
    transaction=load(label,'walking-transaction')
    assert replay==previous['target']['supported']['replay']
    assert transaction==previous['target']['supported']['transaction']
    control=load(label,'solver-control')
    entry=load(label,'solver-entry')
    exit=load(label,'solver-exit')
    post=load(label,'contact-after-40')
    assert control['masked_fin']==(label=='masked')
    assert control['position']==transaction['position']
    assert entry['position_delta']==[0,0,0] and len(exit['steps'])==6
    assert entry['velocity'][1]==0 and abs(entry['velocity'][2])<1e-6
    assert post['mode']=='Grounded' and post['has_base'] and post['transitions']==[]
    remaining=replay['variants']['actual']['result']['remaining']
    assert abs(entry['velocity'][0]-remaining[0]/.05)<1e-6
    assert abs(entry['velocity'][2]-remaining[2]/.05)<1e-8
    entry_fin=[row for row in entry['contacts'] if row['entity']=='AuditStepFin']
    exit_fin=[row for row in exit['contacts'] if row['entity']=='AuditStepFin']
    if label=='masked':
        assert entry_fin==exit_fin==[]
        assert all(abs(row['velocity'][2])<1e-6 for row in exit['steps'])
        assert abs(post['position'][2]-source['position_m'][2])<1e-6
        assert abs(post['position'][0]-source['position_m'][0])<1e-6
        assert .0024<post['position'][1]-source['position_m'][1]<.0026
    else:
        assert len(entry_fin)==len(exit_fin)==1
        assert entry_fin[0]['constraints'] and exit_fin[0]['constraints']
        assert exit['steps'][0]['velocity'][2]<-.6
        assert .028<abs(post['position'][2]-source['position_m'][2])<.03
        assert abs(post['position'][2]-control['position'][2]-exit['steps'][-1]['position_delta'][2])<2e-6
    target[label]=dict(control=control,entry=entry,exit=exit,post=post)

assert target['baseline']==target['restored']
base,masked=[target[label] for label in ('baseline','masked')]
assert base['control']==(masked['control']|{'masked_fin':False})
assert base['entry']['velocity']==masked['entry']['velocity']
assert base['post']==previous['target']['supported']['post']

assert raw(file("baseline","parameters"))==raw(file("restored","parameters"))
restoration={}
for key in load('baseline','parameters'):
    old,new=[{row['ordinal']:row for row in load(label,'parameters')[key]} for label in ('baseline','restored')]
    assert old.keys()==new.keys()
    restoration[key]=next((i for i in sorted(old) if old[i]!=new[i]),None)
    assert restoration[key] is None or restoration[key]>40

decoded={}
for path in [file(label,name) for label in labels for name in (
    'parameters','report','walking-step','walking-transaction','step-down-replay')]:
    data=raw(path)
    archive=path.with_suffix('.json.gz')
    archive.write_bytes(gzip.compress(data,compresslevel=6,mtime=0))
    assert gzip.decompress(archive.read_bytes())==data
    decoded[str(path.relative_to(root))]=identity(data)
    if path.exists():
        path.unlink()
    print('Verified gzip '+str(path.relative_to(root)),flush=True)

ue=Path(os.environ['ALS_UE_SOURCE_ROOT'])
cmc='Engine/Source/Runtime/Engine/Private/Components/CharacterMovementComponent.cpp'
motion='src/als/character/movement/motion.rs'
avian_path='avian3d-0.7.0/src/dynamics/solver/schedule.rs'
avian=next((Path.home()/'.cargo/registry/src').glob('*/'+avian_path))
facts=[]
for key,path,ranges in (
    (cmc,ue/cmc,[(7632,7656),(7683,7697)]),
    (motion,Path(motion),[(3560,3597),(3814,3831)]),
    ('dependency:'+avian_path,avian,[(39,53),(62,75),(125,145)]),
):
    data=path.read_bytes()
    lines=data.decode().splitlines()
    facts.append(dict(key=key,**identity(data),excerpts=[
        dict(start=a,end=b,text='\n'.join(lines[a-1:b])+'\n') for a,b in ranges]))
save('source-facts.json',facts)

inputs={row['path'] for row in previous['external_source_files']}
inputs.update(str(path.relative_to(Path.cwd())) for path in (
    prior/'summary.json',prior/'wall.native.json.gz',prior/'target_fin.inc',
    prior/'observer.py',prior/'runtime-supported'/'step-down-replay.json.gz',
))
save('summary.json',dict(
    gate='PASS_STEPUP_RECONTACT_SOLVER_PHASE_CAUSAL_MASK',findings=['BF144-D297'],
    new_differences=1,total_differences=294,ordinal=40,native_supported_position_m=source['position_m'],
    target=target,restored_first_late_parameter_difference=restoration,restored_parameters_byte_exact=True,
    masked_height_residual_m=masked['post']['position'][1]-source['position_m'][1],
    decoded=decoded,screenshots=0,performance_acceptance='paused',
    external_source_files=[dict(path=name,**identity(Path(name).read_bytes())) for name in sorted(inputs)]))
print('Verified the accepted StepUp and isolated fin recontact in the real solver',flush=True)
