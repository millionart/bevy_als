import gzip
import hashlib
import json
import math
from collections import Counter
from pathlib import Path

root = Path(__file__).resolve().parent
parent = root.parent
temporary = Path('tools/tmp/bf144/489')
def read(name):
    return json.loads((root/name).read_bytes())

def identity(path):
    raw = path.read_bytes()
    return dict(path=path.relative_to(Path.cwd()).as_posix(),bytes=len(raw),sha256=hashlib.sha256(raw).hexdigest())

def angle(a,b):
    norm_a,norm_b = math.hypot(*a),math.hypot(*b)
    a,b = [x/norm_a for x in a],[x/norm_b for x in b]
    sign = 1 if sum(x*y for x,y in zip(a,b))>=0 else -1
    short = math.hypot(*(x-sign*y for x,y in zip(a,b)))
    long = math.hypot(*(x+sign*y for x,y in zip(a,b)))
    return math.degrees(4*math.atan2(short,long))

for label in ['baseline','restored','no-rotation-toggle']:
    assert read(f'replay-compile-{label}.report.json')['success']
    assert read(f'replay-{label}.report.json')['success']==(label!='no-rotation-toggle')
for pattern in ['foot-{}.json','vm-{}-unit.json','vm-{}-uniform.json']:
    assert (root/pattern.format('restored')).read_bytes()==(root/pattern.format('baseline')).read_bytes()
foot = read('foot-restored.json')
negative = read('foot-no-rotation-toggle.json')
assert (foot['changed_targets'],foot['changed_inputs'])==(190,176)
assert (negative['changed_targets'],negative['changed_inputs'])==(0,0)
assert len(foot['rows'])==len(negative['rows'])==318
for row,control in zip(foot['rows'],negative['rows'],strict=True):
    assert row['query']==control['query']
    assert row['baseline']==control['baseline']==control['unit']==control['uniform']

statistics = {}
for mode in ['unit','uniform']:
    counts = Counter()
    locks = Counter()
    rig_changes = set()
    peak_rig = (0,0,'')
    peak_final = (0,0,'')
    for row in foot['rows']:
        baseline,control = row['baseline'],row[mode]
        assert baseline['rig']['location']==control['rig']['location']
        for side,(before,after) in enumerate(zip(baseline['states'],control['states'],strict=True)):
            bone = ['foot_l','foot_r'][side]
            counts.update(key for key in before if before[key]!=after[key])
            before_q,after_q = [data['rig']['rotation'][side] for data in [baseline,control]]
            if before_q!=after_q:
                rig_changes.add((row['query'],bone))
                amount = before['lock_amount']
                locks['zero' if amount==0 else 'full' if amount==1 else 'partial']+=1
            peak_rig=max(peak_rig,(angle(before_q,after_q),row['query'],bone))
            peak_final=max(peak_final,(angle(before['final_rotation'],after['final_rotation']),row['query'],bone))
    vm = read(f'vm-restored-{mode}.json')
    assert (vm['queries'],vm['calls'],vm['traces'])==(319,13378,636)
    vm_changes = set()
    peak_vm = (0,0,'')
    for row in vm['outputs']:
        for change in row['local']:
            assert change['f32_changed']
            before,after = change['before'],change['after']
            assert before['translation']==after['translation'] and before['scale']==after['scale']
            vm_changes.add((row['query'],change['bone']))
            peak_vm=max(peak_vm,(angle(before['rotation'],after['rotation']),row['query'],change['bone']))
    assert vm_changes==rig_changes
    assert len(vm_changes)==vm['changed_local_rows']==vm['changed_f32_local_rows']==vm['changed_sides']
    statistics[mode]=dict(foot_fields=counts,rig_rotations=len(rig_changes),lock_cohorts=locks,
                          vm_local_f32_rows=len(vm_changes),vm_changed_frames=len(vm['outputs']),
                          max_final_angle=peak_final,max_rig_angle=peak_rig,max_vm_angle=peak_vm)
    print(f'Qualified {mode} controlled propagation into the corresponding VM foot rows',flush=True)
unit,uniform = [statistics[m] for m in ['unit','uniform']]
assert unit['foot_fields']=={'target_rotation':185,'final_rotation':164}
assert uniform['foot_fields']=={'target_rotation':190,'final_rotation':176}
assert (unit['rig_rotations'],uniform['rig_rotations'])==(161,176)
assert unit['lock_cohorts']=={'zero':149,'partial':12}
assert uniform['lock_cohorts']=={'zero':164,'partial':12}
assert 0.089<unit['max_vm_angle'][0]<0.09
assert 6.98<uniform['max_vm_angle'][0]<6.99
assert unit['max_vm_angle'][1:]==(203,'foot_l')
assert uniform['max_vm_angle'][1:]==(105,'foot_l')

decoded = {}
for path in [*root.glob('foot-*.json'),*root.glob('vm-*.json')]:
    raw = path.read_bytes()
    packed = gzip.compress(raw,mtime=0)
    assert gzip.decompress(packed)==raw
    path.with_suffix('.json.gz').write_bytes(packed)
    decoded[path.name] = dict(bytes=len(raw),sha256=hashlib.sha256(raw).hexdigest())
references = ['484/inputs.json.gz','484/replay.rs','485/replay.rs','459/inputs.json',
              '487/prepare.py','487/replay.rs','488/replay-restored.json.gz',
              '481a/reader-migration.json','441c/library-identities.json']
generated = {}
for name in ['production.rs','foot_types.rs','foot.rs','vm.rs']:
    raw = (temporary/name).read_bytes()
    generated[name]=dict(bytes=len(raw),sha256=hashlib.sha256(raw).hexdigest())
summary = dict(gate='PASS_D278_FOOT_LOCK_AND_VM_ROTATION_PROPAGATION',new_differences=0,existing_difference='D278',
               actual_foot_frames=318,actual_vm_frames=319,actual_vm_calls=13378,actual_vm_traces=636,
               controlled_world_rotation_sides=190,controls=statistics,vm_trace_changes=0,
               actual_target_positions_frozen=True,continuous_counterfactual=False,
               generated_sources=generated,decoded=decoded,
               external_source_files=[identity(parent/name) for name in references],
               screenshots=0,performance_acceptance='paused')
(root/'final-source.json').write_text(json.dumps(identity(root/'replay.rs'),indent=2)+'\n')
(root/'summary.json').write_text(json.dumps(summary,indent=2)+'\n')
print('Verified lossless replay reports and generated source identities',flush=True)
