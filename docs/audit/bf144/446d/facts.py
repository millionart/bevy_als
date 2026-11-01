import hashlib
import json
import os
from pathlib import Path

r=Path(__file__).resolve().parent
engine=Path(os.environ['ALS_UE_SOURCE_ROOT'])/'Engine/Source/Runtime/Engine'
plugin=Path(json.loads(Path('tools/env.local.json').read_text())['ue_project']).parent/'Plugins/ALS-Refactored/Source/ALS/Private'
units=[(engine/'Public/Animation/AnimInstanceProxy.h','source:UE/Engine/Public/Animation/AnimInstanceProxy.h',[(956,962),(1097,1113)]),(engine/'Private/Animation/AnimInstanceProxy.cpp','source:UE/Engine/Private/Animation/AnimInstanceProxy.cpp',[(1331,1377),(3565,3594)]),(plugin/'AlsAnimationInstance.cpp','source:ALS/Private/AlsAnimationInstance.cpp',[(24,28),(337,360)])]
result=[]
for p,key,ranges in units:
    lines=p.read_text().splitlines()
    result.append(dict(key=key,sha256=hashlib.sha256(p.read_bytes()).hexdigest(),excerpts=[dict(start=a,end=b,text='\n'.join(lines[a-1:b]))for a,b in ranges]))
(r/'source-facts.json').write_text(json.dumps(result,indent=2)+'\n')
print('Captured exact RefreshPose map and per-engine-frame callback gating contracts',flush=True)
