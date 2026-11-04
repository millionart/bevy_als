import gzip
import hashlib
import json
import struct
from pathlib import Path

root = Path(__file__).resolve().parent
parent = root.parent
temporary = Path('tools/tmp/bf144/503')
def read(name):
    return json.loads((root/name).read_bytes())

def identity(path):
    raw = path.read_bytes()
    return dict(path=path.resolve().relative_to(Path.cwd()).as_posix(),bytes=len(raw),sha256=hashlib.sha256(raw).hexdigest())

def bits(value):
    return struct.unpack('<I',struct.pack('<f',value))[0]

assert not read('compile-graph-error-equality.report.json')['success']
assert read('compile-observed.report.json')['success']
for label in ['baseline','widened']:
    assert read(f'runtime-{label}/process.report.json')['success']
    gate=read(f'{label}-functional.json')
    assert gate['pass'] and gate['validation_scope']=='functional' and not gate['issues'] and not gate['images']
for label in ['baseline','restored','no-width','no-history']:
    assert read(f'replay-compile-{label}.report.json')['success']
    assert read(f'replay-{label}.report.json')['success']==(label in ['baseline','restored'])
for prefix in ['replay','damper']:
    assert (root/f'{prefix}-baseline.json').read_bytes()==(root/f'{prefix}-restored.json').read_bytes()
velocity=read('replay-restored.json')
assert [row['query'] for row in velocity if row['baseline']!=row['widened']]==[101]
assert all(row['baseline']==row['widened'] for row in read('replay-no-width.json'))
assert read('replay-no-history.json')==velocity
assert read('damper-restored.json')['mismatches']==0 and len(read('damper-restored.json')['rows'])==16
assert read('damper-no-history.json')['mismatches']==16

comparison=read('consumer-comparison.json')
assert [row['query'] for row in comparison]==list(range(95,111))
changed_direct=[row['query'] for row in comparison if any(d['baseline_right']!=d['widened_right'] for d in row['direct'].values())]
assert changed_direct==[101]
assert all(row['query']==101 or all(not d['changed_bones'] for d in row['direct'].values()) for row in comparison)
by_query={row['query']:row for row in comparison}
first=by_query[101]
assert len(first['direct']['41fd7093']['changed_bones'])==79
assert len(first['direct']['69a33590']['changed_bones'])==70
assert len(first['changed_pose_bones']['root_result'])==73
assert bits(first['direct']['41fd7093']['gait'][1])-bits(first['direct']['41fd7093']['gait'][0])==-1
assert [row['query'] for row in comparison if row['root_gait'][0]!=row['root_gait'][1]]==[102]
assert bits(by_query[102]['root_gait'][1])-bits(by_query[102]['root_gait'][0])==1
assert [r['query'] for r in comparison if r['changed_rig_input']]==[102]
assert [r['index'] for r in by_query[102]['changed_rig_input']]==[10]
assert [r['query'] for r in comparison if r['lean'][0]!=r['lean'][1]]==[103]
assert by_query[103]['lean'][0]['effective']==by_query[103]['lean'][1]['effective']==0.5
assert all(not by_query[q]['changed_pose_bones']['rig_source'] for q in range(103,111))
assert [len(by_query[q]['changed_pose_bones']['root_result']) for q in range(103,111)]==[6]*7+[5]
print('Verified exact downstream pose, PoseGait, next-frame Lean and persistent pole damper effects',flush=True)

decoded={}
for name in [f'runtime-{label}/{file}.json' for label in ['baseline','widened'] for file in ['parameters','report']]:
    path=root/name
    raw=path.read_bytes()
    packed=gzip.compress(raw,mtime=0)
    assert gzip.decompress(packed)==raw
    path.with_suffix('.json.gz').write_bytes(packed)
    decoded[name]=dict(bytes=len(raw),sha256=hashlib.sha256(raw).hexdigest())
references=['498/observer.py','498/runtime-observed/parameters.json.gz','502/replay-restored.json.gz',
            '501/evidence-input.json','499/evidence-input.json','484/playground_observed.rs',
            '484/foot_observer.rs','491/reader-migration.json','441c/library-identities.json']
summary=dict(gate='PASS_D282_ACTUAL_DIRECTION_POSE_AND_POLE_HISTORY',new_differences=0,known_total=279,
             existing_difference='D282',overridden_query=101,direction_blend_bones=79,run_start_blend_bones=70,
             initial_root_changed_bones=73,root_gait_changed_query=102,rig_input_changed_query=102,
             lean_raw_changed_query=103,lean_effective_changed=False,
             persistent_pole_state_pairs=16,persistent_pose_last_observed_query=110,
             trace_values_unchanged=True,screenshots=0,performance_acceptance='paused',decoded=decoded,
             generated_observer=identity(temporary/'playground_observed.rs'),
             external_source_files=[identity(parent/path) for path in references])
(root/'final-source.json').write_text(json.dumps(identity(root/'replay.rs'),indent=2)+'\n')
(root/'summary.json').write_text(json.dumps(summary,indent=2)+'\n')
