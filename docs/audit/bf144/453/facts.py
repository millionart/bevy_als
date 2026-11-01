import hashlib
import json
import os
from pathlib import Path

r=Path(__file__).resolve().parent
e=Path(os.environ['ALS_UE_SOURCE_ROOT'])
units=[('Engine/Source/Runtime/Core/Public/Math/TransformVectorized.h',[(1091,1110)]),('src/animation_graph_ext/mesh_space_additive.rs',[(43,61),(185,218)]),('tools/import/coordinate_basis.py',[(1,31)]),('Engine/Source/Runtime/Engine/Private/Animation/AnimationRuntime.cpp',[(1263,1364)])]
facts=[]
for key,ranges in units:
    p=e/key if key.startswith('Engine/')else Path(key)
    raw=p.read_bytes();lines=raw.decode().splitlines()
    facts.append(dict(key=key,sha256=hashlib.sha256(raw).hexdigest(),excerpts=[dict(start=a,end=b,text='\n'.join(lines[a-1:b]))for a,b in ranges]))
(r/'source-facts.json').write_text(json.dumps(facts,indent=2)+'\n')
print('Captured fresh native Head T/S full-alpha accumulation, target merge and physical basis contracts',flush=True)
