import hashlib
import json
import os
from pathlib import Path

r=Path(__file__).resolve().parent
engine=Path(os.environ['ALS_UE_SOURCE_ROOT'])
specs=[('Engine/Plugins/Animation/ControlRig/Source/ControlRig/Private/Rigs/RigHierarchy.cpp',[(598,615),(5254,5263),(5320,5334),(5356,5373),(5453,5472),(7419,7465)]),('Engine/Plugins/Animation/ControlRig/Source/ControlRig/Public/Rigs/RigHierarchyElements.h',[(476,482)]),('Engine/Source/Runtime/Core/Private/Math/TransformVectorized.cpp',[(205,254),(295,345)]),('src/control_rig/control_rig_hierarchy.rs',[(26,32),(170,226),(250,273),(325,420)])]
facts=[]
for key,ranges in specs:
    p=engine/key if key.startswith('Engine/')else Path(key)
    raw=p.read_bytes()
    lines=raw.decode('utf-8-sig').splitlines()
    facts.append(dict(key=key,sha256=hashlib.sha256(raw).hexdigest(),excerpts=[dict(start=a,end=b,text='\n'.join(lines[a-1:b]))for a,b in ranges]))
(r/'source-facts.json').write_text(json.dumps(facts,indent=2)+'\n')
print('Captured fresh single-parent dirty edges,lazy getter arithmetic,ordered global writer and pure Debug observer source',flush=True)
