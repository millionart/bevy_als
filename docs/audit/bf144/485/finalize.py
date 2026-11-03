import gzip
import hashlib
import json
from collections import Counter
from pathlib import Path

root = Path(__file__).resolve().parent
parent = root.parent
def load(name):
    return json.loads((root/name).read_bytes())

def identity(path):
    raw = path.read_bytes()
    return dict(path=path.relative_to(Path.cwd()).as_posix(), bytes=len(raw), sha256=hashlib.sha256(raw).hexdigest())

positive = load('replay-restored.json')
negative = load('replay-ablated.json')
assert (root/'replay-restored.json').read_bytes() == (root/'replay-baseline.json').read_bytes()
assert (positive['queries'], positive['calls'], positive['traces']) == (319,13378,636)
assert (negative['queries'], negative['calls'], negative['traces']) == (319,13378,636)
assert negative['changed_sides'] == 103 and negative['changed_local_rows'] == negative['changed_f32_local_rows'] == 0
assert all(not row['local'] and not row['calls'] for row in negative['outputs'])
assert not load('replay-ablated.report.json')['success']
assert load('replay-compile-ablated.report.json')['success']
assert load('replay-restored.report.json')['success']
assert load('replay-native.report.json')['success']
assert positive['changed_sides'] == positive['changed_local_rows'] == positive['changed_f32_local_rows'] == 103
assert len(positive['outputs']) == 84
bones = Counter(entry['bone'] for row in positive['outputs'] for entry in row['local'])
assert bones == {'foot_l':49,'foot_r':54}
calls = Counter(entry['pc'] for row in positive['outputs'] for entry in row['calls'])
assert calls == {91:49,98:49,100:49,121:54,128:54,130:54}
native = load('native.json')
assert native['exact_t_q'] == [[634,634],[341,343],[636,342],[632,1]]
assert len(native['rows']) == 318
compression = {}
for name in ['native-input.json','vm-input.json','native.json','replay-baseline.json','replay-ablated.json','replay-restored.json']:
    raw = (root/name).read_bytes()
    packed = gzip.compress(raw,mtime=0)
    assert gzip.decompress(packed)==raw
    (root/(name+'.gz')).write_bytes(packed)
    compression[name]=dict(bytes=len(raw),sha256=hashlib.sha256(raw).hexdigest())
    print('Verified roundtrip '+name,flush=True)
source = identity(root/'replay.rs')
(root/'final-source.json').write_text(json.dumps(source,indent=2)+'\n')
references = ['457/restored-final.native.json.gz','459/inputs.json','459/replay.rs','484/replay-unit-restored.json.gz',
              '482/current_vm.rs','479/replay.rs','478/replay.rs','471/replay.rs','481a/reader-migration.json',
              '441c/library-identities.json']
summary=dict(gate='PASS_NATIVE_SOCKET_AND_D276_FIXED_PRESTATE_VM', new_differences=0,
             native_valid_frames=317,native_initial_current_pose_frames=1,native_exact_t_q=native['exact_t_q'],
             target_actual_vm_queries=319,target_actual_calls=13378,target_actual_traces=636,
             changed_frames=84,changed_sides=103,changed_f32_local_rows=103,changed_bones=bones,
             changed_call_arguments=calls,changed_trace_results=0,ablation_f32_changes=0,
             source_fixed_prestate=True,continuous_counterfactual=False,screenshots=0,performance_acceptance='paused',
             decoded=compression,external_source_files=[identity(parent/path) for path in references])
(root/'summary.json').write_text(json.dumps(summary,indent=2)+'\n')
