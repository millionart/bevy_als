import hashlib
import json
import os
from pathlib import Path

r=Path(__file__).resolve().parent
config=json.loads(Path('tools/env.local.json').read_text())
plugin=Path(config['ue_project']).parent/'Plugins/ALS-Refactored'
engine=Path(os.environ['ALS_UE_SOURCE_ROOT'])
units=[('tools/ue_graph_source_reader/Source/AlsGraphSourceReader/Private/AlsGraphSourceReaderCommandlet.cpp',Path('tools/ue_graph_source_reader/Source/AlsGraphSourceReader/Private/AlsGraphSourceReaderCommandlet.cpp'),[(4091,4118),(4318,4342),(4791,4808),(4984,4995)]),('Engine/Source/Runtime/Engine/Private/Components/SkeletalMeshComponent.cpp',engine/'Engine/Source/Runtime/Engine/Private/Components/SkeletalMeshComponent.cpp',[(900,928)])]
facts=[]
for key,p,ranges in units:
    raw=p.read_bytes();lines=raw.decode().splitlines()
    facts.append(dict(key=key,sha256=hashlib.sha256(raw).hexdigest(),excerpts=[dict(start=a,end=b,text='\n'.join(lines[a-1:b]))for a,b in ranges]))
(r/'source-facts.json').write_text(json.dumps(facts,indent=2)+'\n')
print('Captured existing five Pose observation boundaries and raw Root versus mesh finalization contract',flush=True)
