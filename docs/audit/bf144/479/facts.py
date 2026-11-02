import hashlib
import json
import os
from pathlib import Path

r=Path(__file__).resolve().parent
engine=Path(os.environ['ALS_UE_SOURCE_ROOT'])
specs=[('Engine/Plugins/Animation/ControlRig/Source/ControlRig/Private/Math/ControlRigMathLibrary.cpp',[(6,98)]),('Engine/Source/Runtime/Core/Private/Math/UnrealMath.cpp',[(1402,1451)]),('Engine/Source/Runtime/Core/Public/Math/TransformVectorized.h',[(1730,1745)]),('Engine/Source/Runtime/Core/Public/Math/UnrealMathVectorCommon.h.inl',[(375,384),(765,778)]),('Engine/Source/Runtime/Core/Public/Math/UnrealMathNeon.h',[(1030,1042),(1582,1589),(1717,1746),(2328,2338)]),('Engine/Source/Runtime/Core/Public/Math/Quat.h',[(1105,1143)]),('src/control_rig/control_rig_ik.rs',[(18,25),(46,70),(140,226)]),('src/control_rig/control_rig_hierarchy.rs',[(26,32)])]
facts=[]
for key,ranges in specs:
    p=engine/key if key.startswith('Engine/')else Path(key)
    raw=p.read_bytes()
    lines=raw.decode('utf-8-sig').splitlines()
    facts.append(dict(key=key,sha256=hashlib.sha256(raw).hexdigest(),excerpts=[dict(start=a,end=b,text='\n'.join(lines[a-1:b]))for a,b in ranges]))
(r/'source-facts.json').write_text(json.dumps(facts,indent=2)+'\n')
print('Captured fresh complete IK orientation and scalar/vector/NEON quaternion primitive closures',flush=True)
