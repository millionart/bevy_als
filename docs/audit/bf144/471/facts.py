import hashlib
import json
import os
from pathlib import Path

r=Path(__file__).resolve().parent;config=json.loads(Path('tools/env.local.json').read_bytes());plugin=Path(config['ue_project']).parent/'Plugins/ALS-Refactored/Source/ALS';engine=Path(os.environ['ALS_UE_SOURCE_ROOT']);specs=[('source:Private/Nodes/AlsRigUnit_ApplyFootOffsetRotation.cpp',[(24,113)]),('Engine/Source/Runtime/Core/Private/Math/UnrealMath.cpp',[(483,515),(691,730),(1402,1451)]),('Engine/Source/Runtime/Core/Public/Math/Quat.h',[(1024,1035),(1260,1273)]),('Engine/Source/Runtime/Core/Public/Math/UnrealMathNeon.h',[(1030,1041),(1727,1745),(2330,2341),(2720,2756)]),('Engine/Source/Runtime/Core/Public/Math/UnrealMathVectorConstants.h.inl',[(87,100)]),('Engine/Source/Runtime/Core/Public/Math/UnrealMathVectorCommon.h.inl',[(375,384),(605,609)]),('src/control_rig/control_rig_units.rs',[(38,48),(220,283)]),('src/control_rig/control_rig_math.rs',[(14,64)]),('src/control_rig/control_rig_ik.rs',[(50,71)])];facts=[]
for key,ranges in specs:
 p=plugin/key.split(':',1)[1]if key.startswith('source:')else engine/key if key.startswith('Engine/')else Path(key);raw=p.read_bytes();lines=raw.decode('utf-8-sig').splitlines();facts.append(dict(key=key,sha256=hashlib.sha256(raw).hexdigest(),excerpts=[dict(start=a,end=b,text='\n'.join(lines[a-1:b]))for a,b in ranges]))
(r/'source-facts.json').write_text(json.dumps(facts,indent=2)+'\n');print('Captured complete source foot rotation, current clamp, UE Euler and constructor, ARM64 dot/reciprocal and between-vector paths',flush=True)
