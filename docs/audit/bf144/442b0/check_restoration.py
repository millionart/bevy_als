import json
from pathlib import Path
r=Path(__file__).resolve().parent
read=lambda n:json.loads((r/n).read_text())
old=read('before.native.json')['geometry_query_probe']['samples'];new=read('restored.native.json')['geometry_query_probe']['samples'];assert len(old)==len(new)==7
for a,b in zip(old,new):
    stripped={k:v for k,v in b.items()if k!='move_input'};assert a==stripped
assert read('restored.native.json')['geometry_query_probe']==read('initial.native.json')['geometry_query_probe']
result={'gate':'PASS_OPTIONAL_FIELD_ONLY_AND_EXACT_NATIVE_RESTORATION','old_fields_entirely_unchanged':True,'seven_new_stage_packets_restored_exact':True}
(r/'restoration-check.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result),flush=True)
