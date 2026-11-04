import gzip
import hashlib
import json
import struct
from pathlib import Path

root = Path(__file__).resolve().parent
parent = root.parent
temporary = Path('tools/tmp/bf144/501')
def read(name):
    return json.loads((root/name).read_bytes())

def identity(path):
    raw = path.read_bytes()
    return dict(path=path.resolve().relative_to(Path.cwd()).as_posix(),bytes=len(raw),sha256=hashlib.sha256(raw).hexdigest())

assert read('runtime-observed/process.report.json')['success']
gate = read('observed-functional.json')
assert gate['pass'] and gate['validation_scope']=='functional' and not gate['issues'] and not gate['images']
assert not read('compile-key-lifetime.report.json')['success']
assert not read('prepare-static-base.report.json')['success']
assert not read('replay-compile-unprepared-extract.report.json')['success']
for label in ['baseline','restored','no-plan','no-direct-l1']:
    assert read(f'replay-compile-{label}.report.json')['success']
    assert read(f'replay-{label}.report.json')['success']==(label in ['baseline','restored'])
assert (root/'replay-baseline.json').read_bytes()==(root/'replay-restored.json').read_bytes()
result=read('replay-restored.json')
evidence=read('evidence-input.json')
assert evidence['source']['previous']==dict(amounts=[0,0,0,0],init=False)
assert evidence['target']['previous']==[0,0,0,0]
assert result['native_modes']==96 and len(result['native_direct_l1'])==96
native=result['native_seed']
assert native==evidence['source']['actual']['amounts']
assert result['target_seed']==evidence['target']['actual']
controls=result['controls']
assert controls[0]['seed']==result['target_seed']
assert all(value==[0.10747473686933517,0,0,native[3]] for value in result['native_direct_l1'])

def bits(value):
    return struct.unpack('<I',struct.pack('<f',value))[0]
def gap(value):
    return [bits(value[i])-bits(native[i]) for i in [0,3]]
assert gap(controls[0]['seed'])==[132,-133]
assert gap(controls[1]['seed'])==[2,-1]
assert gap(controls[3]['with_euclidean_reciprocal'])==[0,-1]
assert all(bits(controls[0]['reciprocal_only'][i])-bits(controls[0]['seed'][i])==-1 for i in [0,3])
no_plan=read('replay-no-plan.json')
assert no_plan['native_direct_l1']==result['native_direct_l1']
for i,control in enumerate(no_plan['controls']):
    assert {key:value for key,value in control.items() if key!='use_plan'}=={
        key:value for key,value in controls[i&2].items() if key!='use_plan'}
no_direct=read('replay-no-direct-l1.json')
assert no_direct['controls']==controls and all(value==native for value in no_direct['native_direct_l1'])
print('Verified first-motion seed, D254 correction and independent D281 division control',flush=True)

decoded = {}
for name in ['runtime-observed/parameters.json','runtime-observed/report.json']:
    path = root/name
    raw = path.read_bytes()
    packed = gzip.compress(raw,mtime=0)
    assert gzip.decompress(packed)==raw
    path.with_suffix('.json.gz').write_bytes(packed)
    decoded[name]=dict(bytes=len(raw),sha256=hashlib.sha256(raw).hexdigest())
references=[parent/name for name in ['484/playground_observed.rs','484/foot_observer.rs',
    '484/runtime-observed/parameters.json.gz','493/virtual-motion.native.json.gz',
    '499/replay-restored.json.gz','499/replay.rs','500/replay.rs',
    '491/reader-migration.json','441c/library-identities.json']]
summary=dict(gate='PASS_FIRST_VELOCITY_BLEND_SEED_AND_INDEPENDENT_L1_DIVISION',new_differences=1,
             difference='D281',priority='P3',known_total=278,existing_primary_input_difference='D254',
             witness_query=99,actual_prior_zero=True,initialization_required=False,
             original_seed_gap_ulps=gap(controls[0]['seed']),planned_velocity_seed_gap_ulps=gap(controls[1]['seed']),
             all_controlled_seed_gap_ulps=gap(controls[3]['with_euclidean_reciprocal']),
             native_direct_l1_gap_ulps=gap(result['native_direct_l1'][0]),
             exact_native_models=96,screenshots=0,performance_acceptance='paused',decoded=decoded,
             generated_sources=[identity(temporary/name) for name in ['playground_observed.rs','math.rs','basis.rs']],
             external_source_files=[identity(path) for path in references])
(root/'final-source.json').write_text(json.dumps(identity(root/'replay.rs'),indent=2)+'\n')
(root/'summary.json').write_text(json.dumps(summary,indent=2)+'\n')
