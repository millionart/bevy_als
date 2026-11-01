import copy
import gzip
import hashlib
import json
import subprocess
from pathlib import Path

r=Path(__file__).resolve().parent
reader='tools/ue_graph_source_reader/Source/AlsGraphSourceReader/Private/AlsGraphSourceReaderCommandlet.cpp'
old=subprocess.check_output(['git','show','HEAD:'+reader])
(r/'reader-migration.json').write_text(json.dumps({reader:dict(before_sha256=hashlib.sha256(old).hexdigest(),after_sha256=hashlib.sha256(Path(reader).read_bytes()).hexdigest())},indent=2)+'\n')
request=json.loads(Path('docs/audit/bf144/446d/request.json').read_text())
(r/'legacy.request.json').write_text(json.dumps(request,indent=2)+'\n')
request['advance_before_first_world_tick']=True
(r/'request.json').write_text(json.dumps(request,indent=2)+'\n')
bad=copy.deepcopy(request);bad['advance_before_first_world_tick']='true'
(r/'invalid.request.json').write_text(json.dumps(bad,indent=2)+'\n')
data=json.loads(gzip.decompress(Path('docs/audit/bf144/446d/restored.native.json.gz').read_bytes()))['character_animation_probe']['cases'][0]
a=next(v for v in data['initialization']['instances']if v['main']);b=next(v for v in data['frames'][0]['instances_before_tick']if v['main'])
assert a['proxy_observed_engine_frame']==b['proxy_observed_engine_frame']==b['proxy_native_update_engine_frame']
(r/'before-frame-control-boundary.json').write_text(json.dumps(dict(source='docs/audit/bf144/446d/restored.native.json.gz',initialization_engine_frame=a['proxy_observed_engine_frame'],first_world_engine_frame=b['proxy_observed_engine_frame'],last_native_update_engine_frame=b['proxy_native_update_engine_frame'],matching_target_next_frame_control_missing=True),indent=2)+'\n')
print('Prepared next-engine-frame-only native control and legacy same-frame request',flush=True)
