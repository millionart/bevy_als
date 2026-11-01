import hashlib
import json
import os
from pathlib import Path

r=Path(__file__).resolve().parent
e=Path(os.environ['ALS_UE_SOURCE_ROOT'])
units=[('Engine/Source/Runtime/Core/Public/Math/UnrealMathNeon.h',[(1022,1043),(1690,1747),(2310,2337)]),('Engine/Source/Runtime/Core/Public/Math/UnrealMathVectorCommon.h.inl',[(375,383),(420,446),(599,609)]),('Engine/Source/Runtime/Core/Public/Math/UnrealMathVectorConstants.h.inl',[(85,93)]),('Engine/Source/Runtime/Core/Public/Math/Quat.h',[(1105,1118),(1261,1274)]),('Engine/Source/Runtime/Core/Public/Math/TransformVectorized.h',[(1091,1110),(1134,1138)]),('Engine/Source/Runtime/Engine/Public/BonePose.h',[(314,322)]),('Engine/Source/Runtime/Engine/Private/Animation/AnimationRuntime.cpp',[(1150,1210),(1263,1364)]),('Engine/Source/Runtime/Engine/Private/Animation/AnimNode_ApplyMeshSpaceAdditive.cpp',[(86,110)])]
facts=[]
for key,ranges in units:
    p=e/key if key.startswith('Engine/')else Path(key)
    raw=p.read_bytes();lines=raw.decode().splitlines()
    facts.append(dict(key=key,sha256=hashlib.sha256(raw).hexdigest(),excerpts=[dict(start=a,end=b,text='\n'.join(lines[a-1:b]))for a,b in ranges]))
(r/'source-facts.json').write_text(json.dumps(facts,indent=2)+'\n')
print('Captured fresh full-alpha source F64 quaternion arithmetic, two local normalizations and ARM64 primitive closure',flush=True)
