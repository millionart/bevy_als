import hashlib
import json
import os
from pathlib import Path

r=Path(__file__).resolve().parent;config=json.loads(Path('tools/env.local.json').read_bytes());plugin=Path(config['ue_project']).parent/'Plugins/ALS-Refactored/Source/ALS';engine=Path(os.environ['ALS_UE_SOURCE_ROOT']);specs=[('source:Private/Nodes/AlsRigUnit_ApplyFootOffsetRotation.cpp',[(24,56)]),('Engine/Plugins/Animation/ControlRig/Source/ControlRig/Private/Units/Hierarchy/RigUnit_SetTransform.cpp',[(78,96),(123,162),(311,319)]),('Engine/Plugins/Animation/ControlRig/Source/ControlRig/Private/Rigs/RigHierarchy.cpp',[(5454,5471)]),('Engine/Plugins/Animation/ControlRig/Source/ControlRig/Public/Rigs/RigHierarchyElements.h',[(476,481)]),('Engine/Plugins/Runtime/RigVM/Source/RigVM/Private/RigVMFunctions/Math/RigVMMathLibrary.cpp',[(253,261)]),('Engine/Source/Runtime/Core/Private/Math/UnrealMath.cpp',[(1226,1257)]),('Engine/Source/Runtime/Core/Public/Math/Quat.h',[(651,662)]),('src/control_rig/control_rig_hierarchy.rs',[(27,64),(204,223),(391,420),(452,505)])];facts=[]
for fact in json.loads(Path('docs/audit/bf144/471/source-facts.json').read_bytes()):
 if any(token in fact['key']for token in['UnrealMathNeon','UnrealMathVector']):specs.append((fact['key'],[(excerpt['start'],excerpt['end'])for excerpt in fact['excerpts']]))
for key,ranges in specs:
 p=plugin/key.split(':',1)[1]if key.startswith('source:')else engine/key if key.startswith('Engine/')else Path(key);raw=p.read_bytes();lines=raw.decode('utf-8-sig').splitlines();facts.append(dict(key=key,sha256=hashlib.sha256(raw).hexdigest(),excerpts=[dict(start=a,end=b,text='\n'.join(lines[a-1:b]))for a,b in ranges]))
(r/'source-facts.json').write_text(json.dumps(facts,indent=2)+'\n');print('Captured fresh source global SetRotation, weighted Slerp, quaternion tolerance and current public writer source closure',flush=True)
