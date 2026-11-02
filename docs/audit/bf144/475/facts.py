import hashlib
import json
import os
from pathlib import Path

r=Path(__file__).resolve().parent;engine=Path(os.environ['ALS_UE_SOURCE_ROOT']);specs=[('Engine/Source/Runtime/Core/Private/Math/TransformVectorized.cpp',[(205,254)]),('Engine/Source/Runtime/Core/Public/Math/UnrealMathVectorCommon.h.inl',[(765,778)]),('src/control_rig/control_rig_hierarchy.rs',[(181,203)]),('docs/audit/bf144/453/replay.rs',[(18,30)])];facts=[]
for key,ranges in specs:
 p=engine/key if key.startswith('Engine/')else Path(key);raw=p.read_bytes();lines=raw.decode('utf-8-sig').splitlines();facts.append(dict(key=key,sha256=hashlib.sha256(raw).hexdigest(),excerpts=[dict(start=a,end=b,text='\n'.join(lines[a-1:b]))for a,b in ranges]))
(r/'source-facts.json').write_text(json.dumps(facts,indent=2)+'\n');print('Captured fresh relative double/unit semantics and existing D232 independent early-unit implementation',flush=True)
