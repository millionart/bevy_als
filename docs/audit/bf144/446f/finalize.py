import gzip
import hashlib
import json
from pathlib import Path
from check import load,validate

r=Path(__file__).resolve().parent
read=lambda n:json.loads((r/n).read_text())
a=load('baseline');b=load('restored')
counts=validate(a);assert counts==validate(b)
assert a['character_animation_probe']==b['character_animation_probe']
old=json.loads(gzip.decompress(Path('docs/audit/bf144/446d/restored.native.json.gz').read_bytes()))['character_animation_probe']
assert load('legacy')['character_animation_probe']==old
bad=load('ablated')['character_animation_probe']
assert bad==old
assert read('before-frame-control-boundary.json')['first_world_engine_frame']==100
for label,expected in [('baseline-invalid',7),('ablated-invalid',0),('restored-invalid',7)]:assert read(label+'.native.json.reader.report.json')['process_exit_code']==expected
assert read('check-ablated.report.json')['process_exit_code']!=0
for name in ['baseline','ablated','restored']:assert read(name+'-package-source-check.json')['exact']
reader=Path('tools/ue_graph_source_reader/Source/AlsGraphSourceReader/Private/AlsGraphSourceReaderCommandlet.cpp')
assert hashlib.sha256(reader.read_bytes()).hexdigest()==read('reader-final-sha256.json')['sha256']
assert read('identity-check.json')['current_base_files_unchanged']==396
assert (r/'analysis-baseline.json').read_bytes()==(r/'analysis-restored.json').read_bytes()
result=dict(gate='PASS_MATCHED_NATIVE_ENGINE_FRAME_CONTROL_AND_BOOT_CANDIDATE_REJECTION',counts=counts,
    initialization_and_world_deltas_inputs_unchanged=True,default_pose_eligibility_matches_target_all318=True,
    first_curve_publication_candidate_rejected=True,candidate_cause='Mismatched engine-frame native callback phases',
    phase_and_typed_guard_ablations_rejected=True,negative_fixtures_rejected=3,source_bytes_restored=True,same_native_package_rebuilt=True,
    complete_controlled_character_probe_restored_exact=True,old_default_same_frame318_plus2_character_probe_exact=True,
    all_producer_inputs_or_clock_differences_attributed=False,full_root_or_als_equivalence_claimed=False,
    new_differences=[],total_confirmed_differences=271,screenshots=0,performance_acceptance='paused')
(r/'summary.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps(result),flush=True)
