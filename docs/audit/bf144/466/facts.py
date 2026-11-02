import hashlib
import json
import os
from pathlib import Path

r=Path(__file__).resolve().parent;config=json.loads(Path('tools/env.local.json').read_text());plugin=Path(config['ue_project']).parent/'Plugins/ALS-Refactored/Source/ALS';engine=Path(os.environ['ALS_UE_SOURCE_ROOT']);specs=[('source:Private/Nodes/AlsRigUnit_ApplyFootOffsetLocation.cpp',[(5,60)]),('source:Public/Nodes/AlsRigUnit_ApplyFootOffsetLocation.h',[(12,101)]),('source:Public/Utility/AlsMath.h',[(7,25),(61,68),(98,126)]),('source:Private/Utility/AlsMath.cpp',[(5,9)]),('Engine/Source/Runtime/Core/Public/Math/UnrealMathUtility.h',[(1550,1561),(1670,1730)]),('Engine/Plugins/Animation/ControlRig/Source/ControlRig/Private/Units/Hierarchy/RigUnit_SetTransform.cpp',[(78,96),(123,161),(237,244)]),('Engine/Plugins/Animation/ControlRig/Source/ControlRig/Private/Rigs/RigHierarchy.cpp',[(5454,5470)]),('Engine/Plugins/Animation/ControlRig/Source/ControlRig/Public/Rigs/RigHierarchyElements.h',[(476,481)]),('Engine/Plugins/Animation/ControlRig/Source/ControlRig/Private/Units/Highlevel/Hierarchy/RigUnit_TwoBoneIKSimple.cpp',[(111,133),(215,225)]),('src/control_rig/control_rig_units.rs',[(133,220)]),('src/control_rig/control_rig_math.rs',[(86,147)]),('tools/ue_graph_source_reader/Source/AlsGraphSourceReader/Private/AlsGraphSourceReaderCommandlet.cpp',[(192,203),(5280,5287),(5305,5322)])]
specs += [('source:Private/Nodes/AlsRigUnit_ApplyFootOffsetRotation.cpp',[(13,24)]),('source:Private/Nodes/AlsRigUnits.cpp',[(49,76)]),('source:Private/Nodes/AlsRigUnit_ChainLength.cpp',[(6,15),(73,96)])]
facts=[]
for key,ranges in specs:
 p=plugin/key.split(':',1)[1]if key.startswith('source:')else engine/key if key.startswith('Engine/')else Path(key);raw=p.read_bytes();lines=raw.decode('utf-8-sig').splitlines();facts.append(dict(key=key,sha256=hashlib.sha256(raw).hexdigest(),excerpts=[dict(start=a,end=b,text='\n'.join(lines[a-1:b]))for a,b in ranges]))
(r/'source-facts.json').write_text(json.dumps(facts,indent=2)+'\n');print('Captured actual foot scalar Target, initialization, public kernel, hierarchy skip bounds and writer semantics',flush=True)
