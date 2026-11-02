import hashlib
import json
import os
from pathlib import Path

r=Path(__file__).resolve().parent
engine=Path(os.environ['ALS_UE_SOURCE_ROOT'])
specs=[('Engine/Plugins/Animation/ControlRig/Source/ControlRig/Private/Units/Highlevel/Hierarchy/RigUnit_TwoBoneIKSimple.cpp',[(111,221)]),('Engine/Source/Runtime/Core/Private/Math/UnrealMath.cpp',[(1226,1261)]),('Engine/Source/Runtime/Core/Public/Math/TransformVectorized.h',[(1675,1695),(1748,1769)]),('Engine/Plugins/Animation/ControlRig/Source/ControlRig/Public/Rigs/RigHierarchyElements.h',[(472,486)]),('Engine/Plugins/Animation/ControlRig/Source/ControlRig/Private/Rigs/RigHierarchy.cpp',[(5458,5483)]),('src/control_rig/control_rig_units.rs',[(322,441)]),('src/control_rig/control_rig_hierarchy.rs',[(26,46),(58,85),(391,420)])]
facts=[]
for key,ranges in specs:
    p=engine/key if key.startswith('Engine/')else Path(key)
    raw=p.read_bytes()
    lines=raw.decode('utf-8-sig').splitlines()
    facts.append(dict(key=key,sha256=hashlib.sha256(raw).hexdigest(),excerpts=[dict(start=a,end=b,text='\n'.join(lines[a-1:b]))for a,b in ranges]))
(r/'source-facts.json').write_text(json.dumps(facts,indent=2)+'\n')
print('Captured fresh complete IK weight wrapper, scalar Slerp, source transforms and ordered global writer boundaries',flush=True)
