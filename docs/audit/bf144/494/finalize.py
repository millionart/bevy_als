import gzip
import hashlib
import json
import math
from collections import Counter
from pathlib import Path

root = Path(__file__).resolve().parent
parent = root.parent
temporary = Path('tools/tmp/bf144/494')
def read(name):
    return json.loads((root/name).read_bytes())

def identity(path):
    raw = path.read_bytes()
    return dict(path=path.resolve().relative_to(Path.cwd()).as_posix(),bytes=len(raw),sha256=hashlib.sha256(raw).hexdigest())

def angle(a,b):
    an,bn = math.hypot(*a),math.hypot(*b)
    a,b = [v/an for v in a],[v/bn for v in b]
    sign = 1 if sum(x*y for x,y in zip(a,b))>=0 else -1
    short = math.hypot(*(x-sign*y for x,y in zip(a,b)))
    long = math.hypot(*(x+sign*y for x,y in zip(a,b)))
    return math.degrees(4*math.atan2(short,long))

for name in ['replay','consumers']:
    assert (root/f'{name}-baseline.json').read_bytes()==(root/f'{name}-restored.json').read_bytes()
for label in ['baseline','restored','no-scale-toggle','no-rotation-toggle']:
    assert read(f'replay-compile-{label}.report.json')['success']
    assert read(f'replay-{label}.report.json')['success']==(label in ['baseline','restored'])
assert not read('replay-compile-excerpt-module.report.json')['success']

samples = read('replay-restored.json')
no_scale = read('replay-no-scale-toggle.json')
assert samples==read('replay-no-rotation-toggle.json')
assert samples['native_translation_changes']==samples['target_rotation_changes']==104
assert no_scale['native_translation_changes']==no_scale['target_rotation_changes']==0
assert len(samples['native'])==len(no_scale['native'])==314
assert len(samples['target'])==len(no_scale['target'])==316
for a,b in zip(samples['native'],no_scale['native'],strict=True):
    assert (a['frame'],a['side'])==(b['frame'],b['side'])
    assert a['actual']==b['actual']==b['uniform']
    assert a['actual']['rotation']==a['uniform']['rotation']
for a,b in zip(samples['target'],no_scale['target'],strict=True):
    assert (a['query'],a['side'])==(b['query'],b['side'])
    assert a['actual_q']==b['actual_q']==b['uniform_q']
    assert a['actual_t']==b['actual_t']==b['uniform_t']
changed = [row for row in samples['target'] if row['actual_q']!=row['uniform_q']]
assert len(changed)==104 and len({row['query'] for row in changed})==52
peak_sample = max(changed,key=lambda row:row['normalized_orientation_delta_deg'])
assert (peak_sample['query'],peak_sample['side'])==(105,0)
assert 6.98<peak_sample['normalized_orientation_delta_deg']<6.99
assert max(row['uniform_shear'] for row in changed)<1e-6
print('Qualified native and target virtual-foot scale controls',flush=True)

consumers = read('consumers-restored.json')
no_rotation = read('consumers-no-rotation-toggle.json')
assert len(consumers['rows'])==len(no_rotation['rows'])==158
assert consumers['changed_inputs']==96 and no_rotation['changed_inputs']==0
for row,negative in zip(consumers['rows'],no_rotation['rows'],strict=True):
    assert row['query']==negative['query']
    assert row['baseline']==negative['baseline']==negative['unit']==negative['uniform']
stats = {}
for mode in ['unit','uniform']:
    counts = Counter()
    locks = Counter()
    peak = (0,0,0)
    for row in consumers['rows']:
        before,after = row['baseline'],row[mode]
        assert before['rig']['location']==after['rig']['location']
        for side,(x,y) in enumerate(zip(before['states'],after['states'],strict=True)):
            counts.update(key for key in x if x[key]!=y[key])
            a,b = [item['rig']['rotation'][side] for item in [before,after]]
            if a!=b:
                counts['rig_rotation']+=1
                amount = x['lock_amount']
                locks['zero' if amount==0 else 'full' if amount==1 else 'partial']+=1
                peak = max(peak,(angle(a,b),row['query'],side))
    stats[mode]=dict(changes=counts,locks=locks,maximum_rig_direction_delta=peak)
assert stats['unit']['changes']=={'target_rotation':100,'final_rotation':90,'rig_rotation':88}
assert stats['uniform']['changes']=={'target_rotation':104,'final_rotation':96,'rig_rotation':96}
assert stats['unit']['locks']=={'zero':82,'partial':6}
assert stats['uniform']['locks']=={'zero':90,'partial':6}
assert 0.071<stats['unit']['maximum_rig_direction_delta'][0]<0.073
assert 6.98<stats['uniform']['maximum_rig_direction_delta'][0]<6.99
print('Qualified fixed-history virtual foot lock and RigInput rotation propagation',flush=True)

decoded = {}
for label in ['baseline','restored','no-scale-toggle','no-rotation-toggle']:
    for prefix in ['replay','consumers']:
        path = root/f'{prefix}-{label}.json'
        if not path.exists():
            assert (prefix,label)==('consumers','no-scale-toggle')
            continue
        raw = path.read_bytes()
        packed = gzip.compress(raw,mtime=0)
        assert gzip.decompress(packed)==raw
        path.with_suffix('.json.gz').write_bytes(packed)
        decoded[path.name] = dict(bytes=len(raw),sha256=hashlib.sha256(raw).hexdigest())
references = ['493/virtual-motion.native.json.gz','493/runtime-virtual/parameters.json.gz',
              '493/summary.json','488/replay.rs','486/replay.rs','485/native.rs','479/replay.rs',
              '478/replay.rs','471/replay.rs','487/prepare.py','487/replay.rs','484/replay.rs',
              '491/reader-migration.json','441c/library-identities.json']
summary = dict(gate='PASS_D278_VIRTUAL_FOOT_SCALE_AND_LOCK_CONSUMER',new_differences=0,
               existing_difference='D278',known_total=276,native_exact_samples=314,
               native_rotation_changes=0,native_translation_changes=104,target_exact_samples=316,
               target_rotation_changes=104,target_changed_frames=52,peak_world_orientation=peak_sample,
               target_consumer_frames=158,controls=stats,vm_replayed=False,
               screenshots=0,performance_acceptance='paused',decoded=decoded,
               generated_sources=[identity(temporary/name) for name in ['scale.rs','foot.rs','production.rs','foot_types.rs']],
               external_source_files=[identity(parent/path) for path in references])
(root/'final-source.json').write_text(json.dumps(identity(root/'replay.rs'),indent=2)+'\n')
(root/'summary.json').write_text(json.dumps(summary,indent=2)+'\n')
