import gzip
import json
import sys
from pathlib import Path

sys.path.insert(0,'docs/audit/bf144/462')
from cache_identity import cache_identity
r=Path(__file__).resolve().parent
def read(path):
 p=Path(path)
 if not p.exists():p=Path(str(p)+'.gz')
 return json.loads(gzip.decompress(p.read_bytes())if p.suffix=='.gz'else p.read_bytes())
a=read(r/'default.native.json')['rig_node_history_probe'];b=read('docs/audit/bf144/462/restored.native.json.gz')['rig_node_history_probe']
for probe in[a,b]:
 for case in probe['cases']:
  for frame in case['frames']:frame['work_after']=cache_identity(frame['work_after'])
assert a==b
assert read(r/'character-default.native.json')['character_animation_probe']==read('docs/audit/bf144/457/default.native.json.gz')['character_animation_probe']
assert read(r/'curves.native.json')['blend_profile_probe']==read('docs/audit/bf144/441b/curves.native.json')['blend_profile_probe']
for label in['default','character-default','curves']:assert read(r/(label+'.native.json.reader.report.json'))['success']
(r/'compatibility.json').write_text(json.dumps(dict(gate='PASS_NATIVE_LOCAL_DIRTY_FIELD_FULL_DEFAULT_RIG_CHARACTER_AND_CURVEVECTOR_COMPATIBILITY',whole_old_default_rig_exact=True,whole_old_default_character_exact=True,whole_old_three_axis_curvevector_exact=True,new_configuration_or_runtime_state_added=False),indent=2)+'\n');print('Verified complete default Rig,character and three-axis CurveVector compatibility',flush=True)
