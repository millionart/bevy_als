import hashlib
import json
import os
from pathlib import Path

r=Path(__file__).resolve().parent;engine=Path(os.environ['ALS_UE_SOURCE_ROOT']);specs=[('Engine/Plugins/Animation/ControlRig/Source/ControlRig/Public/Rigs/RigHierarchyElements.h',[(364,422),(1070,1080)]),('Engine/Plugins/Animation/ControlRig/Source/ControlRig/Private/Rigs/RigHierarchyElements.cpp',[(1048,1072)]),('Engine/Plugins/Animation/ControlRig/Source/ControlRig/Private/Tools/ControlRigPoseAdapter.cpp',[(386,399),(415,452)]),('Engine/Plugins/Animation/ControlRig/Source/ControlRig/Private/Rigs/RigHierarchy.cpp',[(5454,5471)])];facts=[]
for key,ranges in specs:
 p=engine/key;raw=p.read_bytes();lines=raw.decode('utf-8-sig').splitlines();facts.append(dict(key=key,sha256=hashlib.sha256(raw).hexdigest(),excerpts=[dict(start=a,end=b,text='\n'.join(lines[a-1:b]))for a,b in ranges]))
key='tools/ue_graph_source_reader/Source/AlsGraphSourceReader/Private/AlsGraphSourceReaderCommandlet.cpp';raw=(r/'reader.final.cpp').read_bytes();lines=raw.decode().splitlines();i=next(i for i,line in enumerate(lines)if'const FRigTransformElement* TransformElement = Hierarchy->Find'in line);facts.append(dict(key=key,sha256=hashlib.sha256(raw).hexdigest(),excerpts=[dict(start=i+1,end=i+13,text='\n'.join(lines[i:i+13]))]));(r/'source-facts.json').write_text(json.dumps(facts,indent=2)+'\n');print('Captured readonly native dirty-state reference and direct cache-branch source closure',flush=True)
