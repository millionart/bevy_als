import hashlib
import json
import os
from pathlib import Path

r=Path(__file__).resolve().parent;engine=Path(os.environ['ALS_UE_SOURCE_ROOT']);specs=[('Engine/Plugins/Animation/ControlRig/Source/ControlRig/Private/Tools/ControlRigPoseAdapter.cpp',[(386,399),(415,452)]),('Engine/Plugins/Animation/ControlRig/Source/ControlRig/Private/Tools/ControlRigHierarchyMappings.cpp',[(326,369)]),('Engine/Plugins/Animation/ControlRig/Source/ControlRig/Private/Rigs/RigHierarchy.cpp',[(5324,5334)]),('Engine/Source/Runtime/Core/Private/Math/Transform.cpp',[(7,11)]),('Engine/Source/Runtime/Core/Private/Math/TransformVectorized.cpp',[(205,261)]),('Engine/Source/Runtime/Core/Public/Math/UnrealMathVectorCommon.h.inl',[(765,779)]),('Engine/Source/Runtime/Core/Public/Math/UnrealMathNeon.h',[(939,968),(1582,1589)]),('Engine/Source/Runtime/Core/Public/Math/TransformVectorized.h',[(1404,1420)]),('src/control_rig/control_rig_hierarchy.rs',[(181,202),(304,313),(325,355)])];facts=[]
for key,ranges in specs:
 p=engine/key if key.startswith('Engine/')else Path(key);raw=p.read_bytes();lines=raw.decode('utf-8-sig').splitlines();facts.append(dict(key=key,sha256=hashlib.sha256(raw).hexdigest(),excerpts=[dict(start=a,end=b,text='\n'.join(lines[a-1:b]))for a,b in ranges]))
(r/'source-facts.json').write_text(json.dumps(facts,indent=2)+'\n');print('Captured actual SIMD relative,dirty local copy,reciprocal scale,NEON cross/FMA and target runtime-unit closure',flush=True)
