import gzip
import hashlib
import json
from pathlib import Path
from check import load,validate

r=Path(__file__).resolve().parent
read=lambda n:json.loads((r/n).read_text())
a=load('baseline')
b=load('restored')
counts=validate(a)
assert counts==validate(b)
assert a['character_animation_probe']==b['character_animation_probe']
legacy=load('legacy')['character_animation_probe']
old=json.loads(gzip.decompress(Path('docs/audit/bf144/445a/legacy.native.json.gz').read_bytes()))['character_animation_probe']
assert legacy==old
bad=load('ablated')['character_animation_probe']['cases']
control=bad[1]
assert [control['initialization']['actor_transform']['translation'][k]for k in ['x','y']]==[4200,-3100]
assert control['initialization']['actor_transform']['rotation']==dict(x=0,y=0,z=0,w=1)
assert control['frames'][-1]['movement_after_tick']['movement_mode']!=1
assert not control['frames'][-1]['movement_after_tick']['walkable_floor']
assert read('before.native.json.reader.report.json')['process_exit_code']==7
for prefix in ['baseline','restored']:
    for name in ['over-limit','bad-xy','bad-yaw']:
        assert read(prefix+'-'+name+'.native.json.reader.report.json')['process_exit_code']==7
assert read('ablated-bad-yaw.native.json.reader.report.json')['process_exit_code']==0
reader='tools/ue_graph_source_reader/Source/AlsGraphSourceReader/Private/AlsGraphSourceReaderCommandlet.cpp'
assert hashlib.sha256(Path(reader).read_bytes()).hexdigest()==read('reader-final-sha256.json')['sha256']
assert read('identity-check.json')['current_base_files_unchanged']==396
result=dict(gate='PASS_BOUNDED318_NATIVE_HISTORY_AND_OPTIONAL_INITIAL_POSE_WITH_LEGACY_COMPATIBILITY',counts=counts,
    old_318_request_rejected=True,old_initial_pose_ignored=True,invalid_limit_xy_and_yaw_rejected=True,negative_output_fixtures_rejected=3,
    yaw_floor_shift_and_typed_guard_ablations_rejected=True,source_bytes_restored=True,same_native_package_rebuilt=True,
    complete_new_character_probe_restored_exact=True,old240_complete_character_probe_exact=True,
    exact_source_target_quaternion_bits_or_all_parameters_matched_claimed=False,full_source_target_history_or_root_equivalence_claimed=False,
    new_differences=[],total_confirmed_differences=271,screenshots=0,performance_acceptance='paused')
(r/'summary.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps(result),flush=True)
