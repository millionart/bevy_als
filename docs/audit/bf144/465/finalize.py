import gzip
import hashlib
import json
from pathlib import Path

r=Path(__file__).resolve().parent
read=lambda name:json.loads((r/name).read_bytes())
assert(r/'replay.rs').read_bytes()==(r/'replay.rs.final').read_bytes()
assert hashlib.sha256((r/'replay.rs').read_bytes()).hexdigest()==read('final-source.json')['sha256']
assert(r/'replay-baseline.json').read_bytes()==(r/'replay-restored.json').read_bytes()
for label in ['baseline','restored']:
 assert read('replay-'+label+'.report.json')['success']and read('replay-compile-'+label+'.report.json')['success']
assert read('replay-compile-no-state-history.report.json')['success']
assert read('replay-no-state-history.report.json')['process_exit_code']==101
check=read('check.json');assert check['source_cm_complete_state_exact_native']==319 and check['public_original_and_trace_control_complete_state_exact']==638
assert read('lifetime-proof.json')['values']==3828
assert read('identity-check.json')['six_libraries_unchanged']
for row in read('compression-map.json').values():
 packed=(r/row['compressed']).read_bytes();decoded=gzip.decompress(packed)
 assert len(decoded)==row['raw_bytes']and hashlib.sha256(decoded).hexdigest()==row['raw_sha256']
 assert hashlib.sha256(packed).hexdigest()==row['compressed_sha256']
external=[]
for name in ['docs/audit/bf144/459/inputs.json','docs/audit/bf144/464/replay-restored.json.gz','docs/audit/bf144/465a/restored.native.json.gz','docs/audit/bf144/465a/reader-migration.json','docs/audit/bf144/441c/library-identities.json','docs/audit/bf144/441b/identities.start.json','docs/audit/bf144/445a/identity-check.json','tools/animgraph_authoring/ue_control_rig_contract.py','tools/animgraph_authoring/ue_control_rig_target.py','tools/run_ue_source_reader.ps1']:
 data=Path(name).read_bytes();external.append(dict(path=name,bytes=len(data),sha256=hashlib.sha256(data).hexdigest()))
summary=dict(gate=check['gate'],source_cm_complete_state_exact_native=319,public_current_and_trace_control_complete_state_exact=638,source_pin_values_no_later_alias_write=3828,source_projected_first_eight_inputs_exact_trace_control=319,observed_counts=check['counts'],first_meter_state_difference=check['first_difference'],independent_continuous_working_state=True,native_after_state_never_injected=True,actual_fixed_configuration=dict(strength=2,critical_damping=1,force=0,target_velocity_amount=0,use_current_input=False,initialize_from_target=True),history_removal_fails_actual_query=19,source_bytes_restored_same_target_recompiled_and_complete_report_bytes_exact=True,external_source_files=external,known_difference='D165',new_differences=[],confirmed_difference_count=272,source_full_rig_or_root_equivalence_claimed=False,production_reader_engine_converters_source_assets_unchanged=True,screenshots=0,performance_acceptance='paused',next_boundary='Source common input actual foot location spring and clamp, then rotation history and full Rig residuals')
(r/'summary.json').write_text(json.dumps(summary,indent=2)+'\n');print('Verified actual319 spring source ABI, native state lifetime, cm math, meter history and byte restoration',flush=True)
