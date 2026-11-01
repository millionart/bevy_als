import copy
import gzip
import hashlib
import json
import struct
import sys
from pathlib import Path

sys.path.insert(0,'tools/animgraph_authoring')
from ue_control_rig_contract import compile_contract
from ue_control_rig_target import compile_target
r=Path(__file__).resolve().parent;path=r/'rig.native.json';native_raw=path.read_bytes()if path.exists()else gzip.decompress(path.with_suffix('.json.gz').read_bytes());native=json.loads(native_raw);target=compile_target(compile_contract(native));actual=json.loads(Path('assets/ALS/Character/CR_Als.program.json').read_text());captured=json.loads(Path('docs/audit/bf144/456/runtime-restored/rig-definition.json').read_text())
a=copy.deepcopy(target);b=copy.deepcopy(actual)
for value in [a,b]:
 for bone in value['bones']:bone.pop('initial_global')
assert a==b
changes=json.loads((r/'fresh-definition-diff.json').read_text());assert len(changes)==365 and len({x[0].split('/')[2]for x in changes})==71
assert all(x[0].startswith('/bones/')and '/initial_global/'in x[0]for x in changes)
def f32_bits(value):return struct.pack('<f',value).hex()
def canonical(value,path=''):
 if isinstance(value,dict):
  return {k:f32_bits(v)if k=='F32'or path=='/geometry'and k=='minimum_length'else canonical(v,path+'/'+k)for k,v in value.items()}
 if isinstance(value,list):return [canonical(v,path+'/'+str(i))for i,v in enumerate(value)]
 return value
expected={k:v for k,v in actual.items()if k!='format'};assert canonical(expected)==canonical(captured)
result=dict(gate='PASS_FRESH_NATIVE_CR_VM_CURRENT405_LAYOUT_WITH_EXPLICIT_KNOWN_INITIAL_GLOBAL_DIFFERENCE',program_local_topology_curves_geometry_exact_source=True,actual_runtime_definition_exact_asset_by_typed_f32_bits=True,f32_shortest_json_text_not_treated_as_f64_data_difference=True,known_initial_global_difference='D242',initial_global_double_components_different=365,initial_global_bones_different=71,initial_globals_not_replaced_for_target_replay=True,instructions=len(target['program']['instructions']),registers=len(target['program']['initial_values']),literals=target['program']['literal_count'],entries=target['program']['entries'],existing_compilers_readonly=True,source_native_sha256=hashlib.sha256(native_raw).hexdigest(),new_differences=[])
(r/'source-contract.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result),flush=True)
