import hashlib
import json
import os
from pathlib import Path

r=Path(__file__).resolve().parent
engine=Path(os.environ['ALS_UE_SOURCE_ROOT'])
specs=[('Engine/Plugins/Animation/ControlRig/Source/ControlRig/Public/Rigs/RigHierarchyElements.h',[(364,422),(1050,1085)]),('Engine/Plugins/Animation/ControlRig/Source/ControlRig/Private/Rigs/RigHierarchyElements.cpp',[(683,692),(1051,1070)]),('Engine/Plugins/Animation/ControlRig/Source/ControlRig/Public/Rigs/RigHierarchy.h',[(2363,2378),(5497,5519)]),('Engine/Plugins/Animation/ControlRig/Source/ControlRig/Private/Rigs/RigHierarchy.cpp',[(3049,3060),(5455,5471),(7419,7465)]),('Engine/Plugins/Animation/ControlRig/Source/ControlRig/Private/Units/Highlevel/Hierarchy/RigUnit_TwoBoneIKSimple.cpp',[(111,221)])]
facts=[]
for key,ranges in specs:
    raw=(engine/key).read_bytes()
    lines=raw.decode('utf-8-sig').splitlines()
    facts.append(dict(key=key,sha256=hashlib.sha256(raw).hexdigest(),excerpts=[dict(start=a,end=b,text='\n'.join(lines[a-1:b]))for a,b in ranges]))
key='tools/ue_graph_source_reader/Source/AlsGraphSourceReader/Private/AlsGraphSourceReaderCommandlet.cpp'
raw=(r/'reader.final.cpp').read_bytes()
lines=raw.decode().splitlines()
i=next(i for i,line in enumerate(lines)if 'TArray<TSharedPtr<FJsonValue>> HierarchyCache;'in line)
facts.append(dict(key=key,sha256=hashlib.sha256(raw).hexdigest(),excerpts=[dict(start=i+1,end=i+22,text='\n'.join(lines[i:i+22]))]))
(r/'source-facts.json').write_text(json.dumps(facts,indent=2)+'\n')
print('Captured const raw storage/dirty reads and excluded allocating pose-version tracking; no new pose getters',flush=True)
