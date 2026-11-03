import gzip
import hashlib
import json
from pathlib import Path

root = Path(__file__).resolve().parent
parent = root.parent
def read(name):
    return json.loads((root/name).read_bytes())

def identity(path):
    raw = path.read_bytes()
    return dict(path=path.relative_to(Path.cwd()).as_posix(),bytes=len(raw),sha256=hashlib.sha256(raw).hexdigest())

restored = read('replay-restored.json')
negative = read('replay-no-scale-toggle.json')
assert (root/'replay-restored.json').read_bytes()==(root/'replay-baseline.json').read_bytes()
for label in ['baseline','restored','no-scale-toggle']:
    assert read(f'replay-compile-{label}.report.json')['success']
    assert read(f'replay-{label}.report.json')['success']==(label!='no-scale-toggle')
assert not read('prepare-line-escape.report.json')['success']
assert read('prepare.report.json')['success']
assert negative['native_translation_changes']==negative['target_rotation_changes']==0
assert restored['native_translation_changes']==192 and restored['target_rotation_changes']==190
for cohort,length in [('native',634),('target',636)]:
    assert len(restored[cohort])==len(negative[cohort])==length
    for positive, ablated in zip(restored[cohort],negative[cohort],strict=True):
        assert ablated['scale']==ablated['control_scale']==positive['scale']
        key = 'frame' if cohort=='native' else 'query'
        assert (positive[key],positive['side'])==(ablated[key],ablated['side'])
        if cohort=='native':
            assert ablated['actual']==ablated['uniform']==positive['actual']
            assert positive['actual']['rotation']==positive['uniform']['rotation']
            assert (positive['scale']!=positive['control_scale'])==(positive['actual']['translation']!=positive['uniform']['translation'])
        else:
            assert all(positive[k]==ablated[k] for k in ['actual_q','actual_t','actual_norm_sq','actual_shear'])
            assert ablated['actual_q']==ablated['uniform_q'] and ablated['actual_t']==ablated['uniform_t']
            assert (positive['scale']!=positive['control_scale'])==(positive['actual_q']!=positive['uniform_q'])
    print(f'Qualified all positive and ablated {cohort} outputs',flush=True)

changed = [row for row in restored['target'] if row['actual_q']!=row['uniform_q']]
assert len(changed)==190 and len({row['query'] for row in changed})==95
assert max(row['uniform_shear'] for row in changed)<1e-6
assert max(abs(row['uniform_norm_sq']-1.0) for row in changed)<1e-6
peak_angle = max(changed,key=lambda row:row['normalized_orientation_delta_deg'])
peak_shear = max(changed,key=lambda row:row['actual_shear'])
min_norm = min(changed,key=lambda row:row['actual_norm_sq'])
assert (peak_angle['query'],peak_angle['side'])==(105,0)
assert (peak_shear['query'],peak_shear['side'])==(105,0)
assert (min_norm['query'],min_norm['side'])==(203,0)
assert peak_angle['normalized_orientation_delta_deg']>6.9 and peak_shear['actual_shear']>0.25
assert min_norm['actual_norm_sq']<0.95
decoded = {}
for label in ['baseline','restored','no-scale-toggle']:
    path = root/f'replay-{label}.json'
    raw = path.read_bytes()
    packed = gzip.compress(raw,mtime=0)
    assert gzip.decompress(packed)==raw
    path.with_suffix('.json.gz').write_bytes(packed)
    decoded[path.name] = dict(bytes=len(raw),sha256=hashlib.sha256(raw).hexdigest())

references = ['484/inputs.json.gz','485/native-input.json.gz','485/native.rs','486/replay.rs',
              '487/summary.json','479/replay.rs','478/replay.rs','471/replay.rs','481a/reader-migration.json',
              '441c/library-identities.json']
summary = dict(gate='PASS_POSITIVE_NONUNIFORM_FOOT_SOCKET_ROTATION_ISOLATION',new_differences=1,difference='D278',
               priority='P2',known_total=275,source_exact_socket_rows=634,source_rotation_changes=0,
               source_translation_changes=192,target_exact_samples=636,target_rotation_changes=190,
               target_changed_queries=95,peak_angle=peak_angle,minimum_actual_norm=min_norm,
               maximum_control_shear=max(row['uniform_shear'] for row in changed),
               lock_and_vm_propagation_claimed=False,decoded=decoded,
               external_source_files=[identity(parent/path) for path in references],
               screenshots=0,performance_acceptance='paused')
(root/'final-source.json').write_text(json.dumps(identity(root/'replay.rs'),indent=2)+'\n')
(root/'summary.json').write_text(json.dumps(summary,indent=2)+'\n')
print('Verified scoped D278 and raw/gzip evidence identities',flush=True)
