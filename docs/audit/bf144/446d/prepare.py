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
request=json.loads(Path('docs/audit/bf144/446b/request.json').read_text())
(r/'legacy.request.json').write_text(json.dumps(request,indent=2)+'\n')
request['capture_proxy_curves']=True
(r/'request.json').write_text(json.dumps(request,indent=2)+'\n')
bad=copy.deepcopy(request)
bad['capture_proxy_curves']='true'
(r/'invalid.request.json').write_text(json.dumps(bad,indent=2)+'\n')
data=json.loads(gzip.decompress(Path('docs/audit/bf144/446b/restored.native.json.gz').read_bytes()))
missing=0
for case in data['character_animation_probe']['cases']:
    snapshots=[case['initialization']['instances']]+[f[k]for f in case['frames']for k in ['instances_before_tick','linked_instances']]
    for snapshot in snapshots:
        for instance in snapshot:
            assert 'proxy_attribute_curves'not in instance
            missing+=1
(r/'old-contract-failure.json').write_text(json.dumps(dict(gate='REJECT_OLD_MISSING_INTERNAL_PROXY_CURVE_HISTORY',missing_snapshot_instances=missing,source='docs/audit/bf144/446b/restored.native.json.gz'),indent=2)+'\n')
print('Prepared optional internal proxy curve/frame observation and rejected old missing-field data',flush=True)
