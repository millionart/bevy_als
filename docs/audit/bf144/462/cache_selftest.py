import copy
import json
import re
import gzip
from cache_identity import cache_identity
from pathlib import Path

r=Path(__file__).resolve().parent
canonical=cache_identity;memory=json.loads(gzip.decompress(Path('docs/audit/bf144/460/common-rig.native.json.gz').read_bytes()))['rig_node_history_probe']['cases'][0]['frames'][0]['work_after'];expected=canonical(memory)
changed=copy.deepcopy(memory)
def replace_versions(value):
 if isinstance(value,list):return [replace_versions(v)for v in value]
 if not isinstance(value,dict):return value
 if value.get('name')=='ContainerVersion'and int(value['exported_text'])!=-1:value['exported_text']='1234567'
 for key,v in value.items():
  if key=='exported_text'and isinstance(v,str):value[key]=re.sub(r'ContainerVersion=(-?\d+)',lambda m:'ContainerVersion='+('1234567'if int(m[1])!=-1 else '-1'),v)
  elif isinstance(v,(list,dict)):value[key]=replace_versions(v)
 return value
for row in changed['properties']:
 if 'FCachedRigElement'in row['cpp_type']:replace_versions(row)
assert canonical(changed)==expected
for name in ['Index','ContainerVersion']:
 altered=copy.deepcopy(memory);row=next(x for x in altered['properties']if x['cpp_type']=='TArray<FCachedRigElement>'and x.get('elements')and x['elements'][0].get('fields'));field=next(x for x in row['elements'][0]['fields']if x['name']==name)
 field['exported_text']=str(int(field['exported_text'])+1)if name=='Index'else'-1'
 assert canonical(altered)!=expected,name
altered=copy.deepcopy(memory);field=next(x for x in altered['properties']if x.get('numeric_value')is not None);field['numeric_value']+=1
assert canonical(altered)!=expected
(r/'cache-selftest.json').write_text(json.dumps(dict(gate='PASS_INSTANCE_HASH_ONLY_CANONICALIZATION_WITH_CACHE_INDEX_INVALIDATION_AND_NUMERIC_COUNTEREXAMPLES',valid_instance_hash_change_only_accepted=True,changed_index_rejected=True,invalid_cache_minus_one_change_rejected=True,changed_numeric_register_rejected=True),indent=2)+'\n');print('Verified only runtime cache identity is canonicalized; index validity and numeric changes rejected',flush=True)
