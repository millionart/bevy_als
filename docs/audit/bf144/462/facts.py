import hashlib
import json
import os
from pathlib import Path

r=Path(__file__).resolve().parent
config=json.loads(Path('tools/env.local.json').read_text());plugin=Path(config['ue_project']).parent/'Plugins/ALS-Refactored';engine=Path(os.environ['ALS_UE_SOURCE_ROOT'])
units=[('source:Private/Nodes/AlsRigUnit_FootOffsetTrace.cpp',[(18,59)]),('Engine/Plugins/Animation/ControlRig/Source/ControlRig/Public/Rigs/RigHierarchy.h',[(3990,4010)]),('Engine/Plugins/Animation/ControlRig/Source/ControlRig/Private/Rigs/RigHierarchyCache.cpp',[(23,80)]),('Engine/Plugins/Runtime/RigVM/Source/RigVM/Public/RigVMCore/RigVMExecuteContext.h',[(445,464)]),('Engine/Plugins/Animation/ControlRig/Source/ControlRig/Private/ControlRig.cpp',[(1135,1158)]),('tools/ue_graph_source_reader/Source/AlsGraphSourceReader/Private/AlsGraphSourceReaderCommandlet.cpp',[(5154,5182),(5230,5255),(5264,5306)])]
facts=[]
for key,ranges in units:
 p=plugin/'Source/ALS'/key.split(':',1)[1]if key.startswith('source:')else engine/key if key.startswith('Engine/')else Path(key)
 raw=p.read_bytes();lines=raw.decode('utf-8-sig').splitlines();facts.append(dict(key=key,sha256=hashlib.sha256(raw).hexdigest(),excerpts=[dict(start=a,end=b,text='\n'.join(lines[a-1:b]))for a,b in ranges]))
(r/'source-facts.json').write_text(json.dumps(facts,indent=2)+'\n')
print('Captured source trace context APIs, static world and owner setup, strict opt-in protocol and exact native fields',flush=True)
