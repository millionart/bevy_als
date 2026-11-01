import hashlib
import json
import os
from pathlib import Path

r=Path(__file__).resolve().parent;config=json.loads(Path('tools/env.local.json').read_text());plugin=Path(config['ue_project']).parent/'Plugins/ALS-Refactored';engine=Path(os.environ['ALS_UE_SOURCE_ROOT'])
glam=list((Path.home()/'.cargo/registry/src').glob('*/glam-0.32.1/src/f64/dquat.rs'));assert glam and len({hashlib.sha256(p.read_bytes()).hexdigest()for p in glam})==1
units=[('source:Private/Nodes/AlsRigUnit_FootOffsetTrace.cpp',[(18,59)]),('Engine/Source/Runtime/Core/Public/Math/TransformVectorized.h',[(1698,1715),(1746,1770),(1791,1811)]),('Engine/Source/Runtime/Core/Public/Math/UnrealMathVectorCommon.h.inl',[(765,800)]),('dependency:glam/src/f64/dquat.rs',[(750,765)]),('src/control_rig/control_rig_query.rs',[(24,59)]),('src/control_rig/control_rig_units.rs',[(80,134)]),('src/control_rig/control_rig_hierarchy.rs',[(58,80)])]
facts=[]
for key,ranges in units:
 p=plugin/'Source/ALS'/key.split(':',1)[1]if key.startswith('source:')else engine/key if key.startswith('Engine/')else glam[0]if key.startswith('dependency:')else Path(key)
 raw=p.read_bytes();lines=raw.decode('utf-8-sig').splitlines();facts.append(dict(key=key,sha256=hashlib.sha256(raw).hexdigest(),excerpts=[dict(start=a,end=b,text='\n'.join(lines[a-1:b]))for a,b in ranges]))
(r/'source-facts.json').write_text(json.dumps(facts,indent=2)+'\n');print('Captured exact source forward and inverse vector formulas, centimeter scalar stage and current public query reconstruction',flush=True)
