import gzip
import hashlib
import json
from pathlib import Path

root = Path(__file__).resolve().parent
parent = root.parent
temporary = Path('tools/tmp/bf144/498')
def read(name):
    return json.loads((root/name).read_bytes())

def identity(path):
    raw = path.read_bytes()
    return dict(path=path.resolve().relative_to(Path.cwd()).as_posix(),bytes=len(raw),sha256=hashlib.sha256(raw).hexdigest())

assert read('runtime-observed/process.report.json')['success']
gate = read('observed-functional.json')
assert gate['pass'] and gate['validation_scope']=='functional' and not gate['issues'] and not gate['images']
assert not read('prepare-cache-save-boundary.report.json')['success']
for label in ['baseline','restored','no-direction-replacement']:
    assert read(f'replay-compile-{label}.report.json')['success']
    assert read(f'replay-{label}.report.json')['success']==(label!='no-direction-replacement')
assert (root/'replay-baseline.json').read_bytes()==(root/'replay-restored.json').read_bytes()
result = read('replay-restored.json')
negative = read('replay-no-direction-replacement.json')
assert result['control_mismatches']==0 and negative['control_mismatches']==2
rows = result['rows']
assert [row['query'] for row in rows]==[104,105,106]
assert [row['query'] for row in rows if row['target']!=row['source']]==[104,106]
for row,control in zip(rows,negative['rows'],strict=True):
    assert all(s['native_gait']==s['target_gait'] for s in row['samples'])
    assert row['native_children_only']==row['target']
    assert row['native_weights_only']==row['source']
    assert row['native_weights']!=row['target_weights']
    assert row['query']==control['query'] and control['native_weights_only']==row['target']
    assert {k:v for k,v in row.items() if k!='native_weights_only'}=={
        k:v for k,v in control.items() if k!='native_weights_only'}
input = read('evidence-input.json')
assert input['rows'][0]['target_desired']==[0.49046874046325684,0,0,0.2596219480037689]
assert input['rows'][0]['native_desired']==[0.4904688596725464,0,0,0.259621798992157]
print('Verified same-child direction-weight restoration and rejected replacement ablation',flush=True)

decoded = {}
for name in ['runtime-observed/parameters.json','runtime-observed/report.json']:
    path = root/name
    raw = path.read_bytes()
    packed = gzip.compress(raw,mtime=0)
    assert gzip.decompress(packed)==raw
    path.with_suffix('.json.gz').write_bytes(packed)
    decoded[name]=dict(bytes=len(raw),sha256=hashlib.sha256(raw).hexdigest())
references = [parent/name for name in ['495/observer.py','497/runtime-observed/parameters.json.gz',
    '497/replay.rs','493/virtual-motion.native.json.gz','484/playground_observed.rs','484/foot_observer.rs',
    '491/reader-migration.json','441c/library-identities.json']]
references.append(Path(input['graph']))
summary = dict(gate='PASS_DIRECTION_WEIGHT_PROVENANCE_FOR_LEAN_GAIT_CURVE',new_differences=0,known_total=276,
               actual_producer_queries=[104,105,106],directional_child_curves_exact=6,
               source_direction_weights_restore_queries=[104,106],source_child_replacement_changes=0,
               witness=rows[0],native_and_target_grounded_factor=1,
               velocity_blend_upstream_root_cause_closed=False,cmc_gait_mapping_compared=False,
               screenshots=0,performance_acceptance='paused',decoded=decoded,
               generated_sources=[identity(temporary/name) for name in ['playground_observed.rs','weights.rs']],
               external_source_files=[identity(path) for path in references])
(root/'final-source.json').write_text(json.dumps(identity(root/'replay.rs'),indent=2)+'\n')
(root/'summary.json').write_text(json.dumps(summary,indent=2)+'\n')
