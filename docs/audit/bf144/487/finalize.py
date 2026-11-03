import gzip
import hashlib
import json
import struct
from collections import Counter
from pathlib import Path

root = Path(__file__).resolve().parent
parent = root.parent
temporary = Path('tools/tmp/bf144/487')
def read(name):
    return json.loads((root/name).read_bytes())

def identity(path):
    raw = path.read_bytes()
    return dict(path=path.relative_to(Path.cwd()).as_posix(),bytes=len(raw),sha256=hashlib.sha256(raw).hexdigest())

for label in ['orientation-assumption','no-component-effect','baseline','restored']:
    assert read(f'replay-compile-{label}.report.json')['success']
    assert read(f'replay-{label}.report.json')['success'] == (label in ['baseline','restored'])
for name in ['foot','vm-off','vm-on']:
    if name=='foot':
        baseline, restored = [f'foot-{mode}.json' for mode in ['baseline','restored']]
    else:
        baseline, restored = [f'vm-{mode}-{name[3:]}.json' for mode in ['baseline','restored']]
    assert (root/baseline).read_bytes()==(root/restored).read_bytes()

foot = read('foot-restored.json')
negative = read('foot-no-component-effect.json')
assert foot['rig_rotation_differences']==154 and negative['rig_rotation_differences']==0
assert len(foot['rows'])==len(negative['rows'])==318
counts = Counter()
locks = Counter()
for row, control in zip(foot['rows'],negative['rows'],strict=True):
    assert row['query']==control['query'] and row['baseline']==control['baseline']
    assert row['off']==control['off']==control['on']
    off,on = row['off'],row['on']
    assert off['rig']['location']==on['rig']['location']==row['baseline']['rig']['location']
    for side,(a,b) in enumerate(zip(off['states'],on['states'],strict=True)):
        counts.update(field for field in a if a[field]!=b[field])
        if off['rig']['rotation'][side]!=on['rig']['rotation'][side]:
            amount = a['lock_amount']
            locks['zero' if amount==0 else 'full' if amount==1 else 'partial']+=1
assert counts=={'target_rotation':398,'final_rotation':165}
assert locks=={'zero':132,'partial':22}
print('Verified all actual foot inputs and isolated paired target and RigInput rotations',flush=True)

modes = [read(f'vm-restored-{mode}.json') for mode in ['off','on']]
for mode in modes:
    assert (mode['queries'],mode['calls'],mode['traces'])==(319,13378,636)
modes = [{row['query']:row for row in mode['outputs']} for mode in modes]
def f32(value):
    return struct.unpack('<f',struct.pack('<f',value))[0]
vm_changes = []
for query in sorted(modes[0].keys()|modes[1].keys()):
    off,on = [mode.get(query,{}) for mode in modes]
    if off and on:
        assert off['traces']==on['traces']
    a,b = [{bone['bone']:bone for bone in mode.get('local',[])} for mode in [off,on]]
    for bone in sorted(a.keys()|b.keys()):
        if bone in a and bone in b:
            assert a[bone]['before']==b[bone]['before']
        x = a[bone]['after'] if bone in a else b[bone]['before']
        y = b[bone]['after'] if bone in b else a[bone]['before']
        if x==y:
            continue
        assert x['translation']==y['translation'] and x['scale']==y['scale']
        assert list(map(f32,x['rotation']))!=list(map(f32,y['rotation']))
        vm_changes.append(dict(query=query,bone=bone,off=x,on=y))
assert len(vm_changes)==154 and len({row['query'] for row in vm_changes})==104
assert Counter(row['bone'] for row in vm_changes)=={'foot_l':78,'foot_r':76}
(root/'paired-vm.json').write_text(json.dumps(vm_changes,indent=2)+'\n')
print('Verified 154 F32 local rows across 104 paired VM queries',flush=True)

decoded = {}
for path in [*root.glob('foot-*.json'),*root.glob('vm-*.json')]:
    raw = path.read_bytes()
    packed = gzip.compress(raw,mtime=0)
    assert gzip.decompress(packed)==raw
    path.with_suffix('.json.gz').write_bytes(packed)
    decoded[path.name] = dict(bytes=len(raw),sha256=hashlib.sha256(raw).hexdigest())
references = ['484/inputs.json.gz','484/replay.rs','484/replay-unit-restored.json.gz','485/replay.rs',
              '459/inputs.json','486/replay-restored.json.gz','481a/reader-migration.json','441c/library-identities.json']
generated = {}
for name in ['production.rs','foot_types.rs','vm.rs']:
    raw = (temporary/name).read_bytes()
    generated[name]=dict(bytes=len(raw),sha256=hashlib.sha256(raw).hexdigest())
summary = dict(gate='PASS_D277_PAIRED_FOOT_LOCK_AND_VM',new_differences=0,existing_difference='D277',
               actual_foot_frames=318,actual_vm_frames=319,actual_vm_calls=13378,actual_vm_traces=636,
               foot_changes=counts,rig_rotation_changes=154,rig_lock_cohorts=locks,
               vm_local_f32_changes=154,vm_changed_queries=104,vm_trace_changes=0,
               actual_baseline_and_paired_tqs_are_separate=True,continuous_counterfactual=False,
               generated_sources=generated,decoded=decoded,
               external_source_files=[identity(parent/name) for name in references],
               screenshots=0,performance_acceptance='paused')
(root/'final-source.json').write_text(json.dumps(identity(root/'replay.rs'),indent=2)+'\n')
(root/'summary.json').write_text(json.dumps(summary,indent=2)+'\n')
print('Verified lossless replay reports and generated source identities',flush=True)
