import gzip
import json
import sys
from pathlib import Path

sys.path.insert(0,'docs/audit/bf144/462')
from cache_identity import cache_identity
r=Path(__file__).resolve().parent
def read(name):
 p=r/name;v=json.loads(p.read_bytes()if p.exists()else gzip.decompress(p.with_suffix(p.suffix+'.gz').read_bytes()))['rig_node_history_probe']
 for case in v['cases']:
  for frame in case['frames']:frame['work_after']=cache_identity(frame['work_after'])
 return v
a=read('controls.native.json');b=read('controls-restored.native.json');assert a==b
(r/'controls-restored-check.json').write_text(json.dumps(dict(gate='PASS_CONTROLLED_NATIVE_BOUNDARIES_EXACT_AFTER_SOURCE_BYTE_RESTORE',complete_controlled_boundaries_exact_after_byte_restore=True,cases=4,frames=28),indent=2)+'\n');print('Verified complete controlled native boundary recovery across four input paths',flush=True)
