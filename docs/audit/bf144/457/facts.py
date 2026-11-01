import hashlib
import json
import os
from pathlib import Path

r=Path(__file__).resolve().parent;e=Path(os.environ['ALS_UE_SOURCE_ROOT']);config=json.loads(Path('tools/env.local.json').read_text());plugin=Path(config['ue_project']).parent/'Plugins/ALS-Refactored'
units=[('source:Public/AlsCharacter.h',plugin/'Source/ALS/Public/AlsCharacter.h',[(68,74),(674,677)]),('Engine/Source/Runtime/Engine/Private/Animation/AnimNodeBase.cpp',e/'Engine/Source/Runtime/Engine/Private/Animation/AnimNodeBase.cpp',[(184,203),(230,254)]),('Engine/Source/Runtime/Engine/Private/Animation/AnimNode_LinkedInputPose.cpp',e/'Engine/Source/Runtime/Engine/Private/Animation/AnimNode_LinkedInputPose.cpp',[(29,65),(79,96)]),('Engine/Source/Runtime/Engine/Private/Animation/AnimNode_LinkedAnimGraph.cpp',e/'Engine/Source/Runtime/Engine/Private/Animation/AnimNode_LinkedAnimGraph.cpp',[(368,407)])]
facts=[]
for key,p,ranges in units:
    raw=p.read_bytes();lines=raw.decode().splitlines();facts.append(dict(key=key,sha256=hashlib.sha256(raw).hexdigest(),excerpts=[dict(start=a,end=b,text='\n'.join(lines[a-1:b]))for a,b in ranges]))
(r/'source-facts.json').write_text(json.dumps(facts,indent=2)+'\n')
print('Captured raw action property/getter, live copied linked-input semantics and exact owner link restoration facts',flush=True)
