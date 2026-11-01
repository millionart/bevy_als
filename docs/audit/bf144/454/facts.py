import hashlib
import json
import os
from pathlib import Path

r=Path(__file__).resolve().parent;e=Path(os.environ['ALS_UE_SOURCE_ROOT']);cargo=Path(os.environ.get('CARGO_HOME',str(Path.home()/'.cargo')))
glam=next((cargo/'registry/src').glob('*/glam-0.32.1'))
units=[('Engine/Source/Runtime/Core/Public/Math/UnrealMathVectorConstants.h.inl',e/'Engine/Source/Runtime/Core/Public/Math/UnrealMathVectorConstants.h.inl',[(78,93)]),('Engine/Source/Runtime/Core/Public/Math/UnrealMathNeon.h',e/'Engine/Source/Runtime/Core/Public/Math/UnrealMathNeon.h',[(930,945),(1022,1030),(1690,1732),(2318,2337)]),('glam-0.32.1/src/f32/neon/quat.rs',glam/'src/f32/neon/quat.rs',[(580,588),(794,836)]),('glam-0.32.1/src/f32/neon/vec4.rs',glam/'src/f32/neon/vec4.rs',[(548,571),(620,630)]),('src/animation_graph_ext/mesh_space_additive.rs',Path('src/animation_graph_ext/mesh_space_additive.rs'),[(30,43),(218,259)])]
facts=[]
for key,p,ranges in units:
    raw=p.read_bytes();lines=raw.decode().splitlines();facts.append(dict(key=key,sha256=hashlib.sha256(raw).hexdigest(),excerpts=[dict(start=a,end=b,text='\n'.join(lines[a-1:b]))for a,b in ranges]))
(r/'source-facts.json').write_text(json.dumps(facts,indent=2)+'\n')
print('Captured fresh UE NEON source order and reciprocal method, frozen glam pair sums and current Head call sites',flush=True)
