import gzip
import hashlib
import json
import struct
from pathlib import Path

root = Path(__file__).resolve().parent
parent = root.parent
def read(name):
    return json.loads((root/name).read_bytes())

def identity(path):
    raw = path.read_bytes()
    return dict(path=path.relative_to(Path.cwd()).as_posix(),bytes=len(raw),sha256=hashlib.sha256(raw).hexdigest())

assert not read('replay-compile-native-array-size.report.json')['success']
for label in ['baseline','restored','no-euclidean']:
    assert read(f'replay-compile-{label}.report.json')['success']
    assert read(f'replay-{label}.report.json')['success']==(label!='no-euclidean')
assert (root/'replay-baseline.json').read_bytes()==(root/'replay-restored.json').read_bytes()
baseline = read('replay-restored.json')
negative = read('replay-no-euclidean.json')
modes = baseline['best_source_models']
assert baseline['minimum_source_field_mismatches']==0 and len(modes)==96
assert negative['minimum_source_field_mismatches']==3 and len(negative['best_source_models'])==192
rows = baseline['rows']
assert [r['query'] for r in rows]==list(range(100,111))
inputs = read('evidence-input.json')
changed_native = []
changed_target = []
explicit_wrong = []
for row,control,input_row in zip(rows,negative['rows'],inputs,strict=True):
    query=row['query']
    assert query==control['query']==input_row['query']
    assert row['target']==control['target']
    actual = input_row['source']['actual']['amounts']
    for i in modes:
        model=row['native'][i]
        assert model['amounts']==actual==model['target_lerp_amounts']
        assert model['alpha']==row['target']['alpha']
        assert model['no_euclidean_amounts']==row['native'][modes[0]]['no_euclidean_amounts']
        assert model['no_euclidean_amounts']==control['native'][i]['amounts']
    n=row['native'][modes[0]]
    if n['amounts']!=n['no_euclidean_amounts']:
        assert sum(a!=b for a,b in zip(n['amounts'],n['no_euclidean_amounts']))==1
        changed_native.append(query)
    t=row['target']
    assert t['delta_lerp']==t['actual']==input_row['target']['actual']
    if t['with_euclidean_amounts']!=t['actual']:
        assert sum(a!=b for a,b in zip(t['with_euclidean_amounts'],t['actual']))==1
        changed_target.append(query)
    if t['explicit_two_terms']!=t['actual']:
        explicit_wrong.append(query)
assert changed_native==[102,105,110]
assert changed_target==[101,105,109]
assert explicit_wrong==[101,107,108]

def f32(x):
    return struct.unpack('<f',struct.pack('<f',x))[0]

def gait(amounts,child):
    total=f32(amounts[0]+amounts[3])
    weights=[f32(amounts[0]/total),f32(amounts[3]/total)]
    return f32(f32(child[0]*weights[0])+f32(child[1]*weights[1]))

for row in json.loads((parent/'498/replay-restored.json').read_bytes())['rows']:
    model=next(r['native'][modes[0]] for r in rows if r['query']==row['query'])
    children=[sample['native_gait'] for sample in row['samples']]
    assert gait(model['amounts'],children)==gait(model['no_euclidean_amounts'],children)==row['source']
meter_counts = [sum(r['native'][i]['goal']!=r['native'][i]['meter_after_local_goal'] for r in rows) for i in modes]
assert min(meter_counts)==7 and max(meter_counts)==9
print('Verified independent Euclidean omission, exact actual interpolation and bounded unit controls',flush=True)

decoded = {}
for label in ['baseline','restored','no-euclidean']:
    path = root/f'replay-{label}.json'
    raw = path.read_bytes()
    packed = gzip.compress(raw,mtime=0)
    assert gzip.decompress(packed)==raw
    path.with_suffix('.json.gz').write_bytes(packed)
    decoded[path.name] = dict(bytes=len(raw),sha256=hashlib.sha256(raw).hexdigest())
references = ['493/virtual-motion.native.json.gz','498/runtime-observed/parameters.json.gz',
              '498/replay-restored.json','491/reader-migration.json','441c/library-identities.json']
witness = rows[2]['native'][modes[0]]
summary = dict(gate='PASS_INDEPENDENT_VELOCITY_BLEND_EUCLIDEAN_STAGE',new_differences=1,difference='D280',
               priority='P3',known_total=277,actual_native_frames=11,actual_target_frames=11,
               native_qualifying_models=len(modes),native_changed_queries=changed_native,
               target_changed_queries=changed_target,actual_lerp_mismatches=0,pose_gait_consumer_changes=0,
               ablation_native_field_mismatches=3,isolated_meter_goal_changes_range=[7,9],
               witness=dict(query=102,observed_right=witness['amounts'][3],without_euclidean=witness['no_euclidean_amounts'][3]),
               upstream_actual_direction_drift_closed=False,
               screenshots=0,performance_acceptance='paused',decoded=decoded,
               external_source_files=[identity(parent/path) for path in references])
(root/'final-source.json').write_text(json.dumps(identity(root/'replay.rs'),indent=2)+'\n')
(root/'summary.json').write_text(json.dumps(summary,indent=2)+'\n')
