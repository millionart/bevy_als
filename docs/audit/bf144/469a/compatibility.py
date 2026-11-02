import gzip
import json
import sys
from pathlib import Path
sys.path.insert(0,'docs/audit/bf144/462')
from cache_identity import cache_identity

r=Path(__file__).resolve().parent
def read(p):
 p=Path(p)
 if p.suffix=='.gz':return json.loads(gzip.decompress(p.read_bytes()))
 return json.loads(p.read_bytes()if p.exists()else gzip.decompress(p.with_suffix(p.suffix+'.gz').read_bytes()))
a=read(r/'default.native.json')['rig_node_history_probe'];b=read('docs/audit/bf144/462/restored.native.json.gz')['rig_node_history_probe']
for probe in [a,b]:
 for case in probe['cases']:
  for frame in case['frames']:frame['work_after']=cache_identity(frame['work_after'])
assert a==b
assert read(r/'character-default.native.json')['character_animation_probe']==read('docs/audit/bf144/457/default.native.json.gz')['character_animation_probe']
assert read(r/'curves.native.json')['blend_profile_probe']==read('docs/audit/bf144/441b/curves.native.json')['blend_profile_probe']
for label in ['default','character-default','curves']:
 result=read(r/(label+'.native.json.reader.report.json'));assert result['success']and result['process_exit_code']==0
for label in ['bad-type','bad-item']:assert read(r/(label+'.native.json.reader.report.json'))['process_exit_code']==7
assert read(r/'no-config-guard.native.json.reader.report.json')['process_exit_code']==0
(r/'compatibility.json').write_text(json.dumps(dict(gate='PASS_UNIT_BOUNDARY_CAPTURE_FULL_LEGACY_COMPATIBILITY',whole_old_rig_probe_exact_except_source_defined_instance_cache_hash=True,whole_old_default_character_probe_exact=True,whole_old_three_axis_curve_vector_probe_exact=True,strict_typed_request_rejected=True,type_guard_ablation_accepts_invalid_request=True,screenshots=0,performance_acceptance='paused'),indent=2)+'\n');print('Verified full default Rig and character probes and complete three-axis CurveVector compatibility',flush=True)
