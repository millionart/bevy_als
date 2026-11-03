import gzip
import hashlib
import json
from pathlib import Path

root = Path(__file__).resolve().parent
parent = root.parent
temporary = Path('tools/tmp/bf144/496')
def read(name):
    return json.loads((root/name).read_bytes())

def identity(path):
    raw = path.read_bytes()
    return dict(path=path.resolve().relative_to(Path.cwd()).as_posix(),bytes=len(raw),sha256=hashlib.sha256(raw).hexdigest())

for label in ['lean','forward','right']:
    assert read(f'{label}.native.json.reader.report.json')['success']
assert read('runtime-observed/process.report.json')['success']
gate = read('observed-functional.json')
assert gate['pass'] and not gate['issues'] and not gate['images'] and gate['validation_scope']=='functional'
assert not read('prepare-inactive-leg-input.report.json')['success']
for label in ['baseline','restored','no-raw-lean','no-neutral-stand']:
    assert read(f'replay-compile-{label}.report.json')['success']
    assert read(f'replay-{label}.report.json')['success']==(label in ['baseline','restored'])
assert (root/'replay-baseline.json').read_bytes()==(root/'replay-restored.json').read_bytes()
rows = read('replay-restored.json')
assert [row['query'] for row in rows]==[105,158]
source = read('evidence-input.json')
assert source['rows'][0]['movement_before_run_start']==dict(state_machine=[1,1,1],
    lean_delta=[0,1.1920928955078125e-07,0],
    lean_output=[1,1.0000001192092896,1],cached_movement=[1,1.0000001192092896,1])
assert source['rows'][0]['legs_alpha']==0.664577305316925 and source['rows'][1]['legs_alpha']==1.0
ulp = 1.1920928955078125e-07
for row in rows:
    for control in row['controls']:
        double = control['fixed_input_f64']
        assert all(double['f32_'+key]==control[key] for key in ['overlay','legs','head'])
        assert double['source_complement_legs']==double['target_weights_legs']
    assert row['controls'][0]['fixed_input_f64']['f32_dynamic']!=row['controls'][0]['dynamic']
run,tail = [row['controls'] for row in rows]
assert [item['run_delta'][1] for item in run]==[ulp,0,ulp,0]
assert [item['locomotion'][1] for item in run]==[1+2*ulp,1+ulp,1+2*ulp,1+ulp]
assert [item['legs'][1] for item in run]==[1+3*ulp,1+2*ulp,1+3*ulp,1+2*ulp]
assert [item['dynamic'][1] for item in tail]==[ulp,ulp,0,0]
assert [item['legs'][1] for item in tail]==[1+2*ulp,1+2*ulp,1+ulp,1+ulp]
assert all(item['run_delta'] is None for item in tail)
for negative,flag in [('no-raw-lean','raw_lean'),('no-neutral-stand','unit_stand')]:
    for row,control in zip(rows,read(f'replay-{negative}.json'),strict=True):
        assert row['query']==control['query']
        for index,item in enumerate(control['controls']):
            unchanged_index = index & (2 if flag=='raw_lean' else 1)
            assert {k:v for k,v in item.items() if k!=flag} == {
                k:v for k,v in row['controls'][unchanged_index].items() if k!=flag}
print('Verified scoped reference and dynamic scale controls and restoration',flush=True)

decoded = {}
for name in ['lean.native.json','forward.native.json','right.native.json',
             'runtime-observed/parameters.json','runtime-observed/report.json']:
    path = root/name
    raw = path.read_bytes()
    packed = gzip.compress(raw,mtime=0)
    assert gzip.decompress(packed)==raw
    path.with_suffix('.json.gz').write_bytes(packed)
    decoded[name]=dict(bytes=len(raw),sha256=hashlib.sha256(raw).hexdigest())
references = [parent/name for name in ['495/observer.py','495/runtime-moving/parameters.json.gz','495/summary.json',
    '484/playground_observed.rs','484/foot_observer.rs','491/reader-migration.json','441c/library-identities.json']]
references += [Path(name) for name in source['graphs']]
references += [Path('assets/ALS/Animations')/(name+suffix) for name in source['assets'].values() for suffix in ['.gltf','.bin']]
references.append(Path('tools/import/audit_animation_transform_conversion.py'))
summary = dict(gate='PASS_RUN_START_AND_LAYERING_VIRTUAL_FOOT_SCALE_FACTORS',new_differences=0,known_total=276,
               existing_difference='D235',native_leaf_samples=3,actual_motion_queries=[105,158],
               run_start_raw_lean_removes_one_f32_step=True,full_weight_dynamic_ordinary_stand_isolated=True,
               fixed_input_f64_delta_changes=2,fixed_input_f64_projected_output_changes=0,
               preceding_lean_application_traced=True,preceding_lean_application_isolated=False,
               screenshots=0,performance_acceptance='paused',
               decoded=decoded,generated_observer=identity(temporary/'playground_observed.rs'),
               external_source_files=[identity(path) for path in references])
(root/'final-source.json').write_text(json.dumps(identity(root/'replay.rs'),indent=2)+'\n')
(root/'summary.json').write_text(json.dumps(summary,indent=2)+'\n')
