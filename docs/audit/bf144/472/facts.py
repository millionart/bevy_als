import hashlib
import json
import os
from pathlib import Path

r=Path(__file__).resolve().parent;config=json.loads(Path('tools/env.local.json').read_bytes());plugin=Path(config['ue_project']).parent/'Plugins/ALS-Refactored/Source/ALS';engine=Path(os.environ['ALS_UE_SOURCE_ROOT']);specs=[('source:Private/Nodes/AlsRigUnit_ApplyFootOffsetRotation.cpp',[(24,113)]),('Engine/Plugins/Animation/ControlRig/Source/ControlRig/Private/Units/Hierarchy/RigUnit_SetTransform.cpp',[(78,96),(123,161),(311,319)]),('src/control_rig/control_rig_units.rs',[(220,283)]),('src/control_rig/control_rig_hierarchy.rs',[(452,505)])];facts=[]
for key,ranges in specs:
 p=plugin/key.split(':',1)[1]if key.startswith('source:')else engine/key if key.startswith('Engine/')else Path(key);raw=p.read_bytes();lines=raw.decode('utf-8-sig').splitlines();facts.append(dict(key=key,sha256=hashlib.sha256(raw).hexdigest(),excerpts=[dict(start=a,end=b,text='\n'.join(lines[a-1:b]))for a,b in ranges]))
(r/'source-facts.json').write_text(json.dumps(facts,indent=2)+'\n');print('Captured actual rotation data groups and direct source/global SetRotation consumption signature',flush=True)
