import hashlib
import json
import os
from pathlib import Path

r=Path(__file__).resolve().parent
config=json.loads(Path('tools/env.local.json').read_text());plugin=Path(config['ue_project']).parent/'Plugins/ALS-Refactored';engine=Path(os.environ['ALS_UE_SOURCE_ROOT'])
units=[('source:Private/Nodes/AlsRigUnit_FootOffsetTrace.cpp',[(8,61)]),('Engine/Source/Runtime/Core/Public/Math/TransformVectorized.h',[(1793,1811)]),('Engine/Source/Runtime/Core/Public/Math/UnrealMathVectorCommon.h.inl',[(765,800)]),('Engine/Plugins/Runtime/RigVM/Source/RigVM/Public/RigVMCore/RigVMExecuteContext.h',[(445,464)]),('Engine/Plugins/Runtime/RigVM/Source/RigVM/Public/RigVMCore/RigVMProfilingInfo.h',[(18,45)]),('Engine/Plugins/Runtime/RigVM/Source/RigVM/Private/RigVMCore/RigVM.cpp',[(3833,3847)]),('Engine/Plugins/Animation/ControlRig/Source/ControlRig/Private/ControlRig.cpp',[(1382,1393)]),('tools/ue_graph_source_reader/Source/AlsGraphSourceReader/Private/AlsGraphSourceReaderCommandlet.cpp',[(5195,5208),(5264,5295)]),('src/control_rig/control_rig_units.rs',[(80,134)]),('src/control_rig/control_rig_hierarchy.rs',[(58,79)]),('src/control_rig/control_rig_query.rs',[(24,59)]),('tools/animgraph_authoring/ue_control_rig_target.py',[(191,207)])]
facts=[]
for key,ranges in units:
 p=plugin/'Source/ALS'/key.split(':',1)[1]if key.startswith('source:')else engine/key if key.startswith('Engine/')else Path(key)
 raw=p.read_bytes();lines=raw.decode('utf-8-sig').splitlines();facts.append(dict(key=key,sha256=hashlib.sha256(raw).hexdigest(),excerpts=[dict(start=a,end=b,text='\n'.join(lines[a-1:b]))for a,b in ranges]))
(r/'source-facts.json').write_text(json.dumps(facts,indent=2)+'\n')
print('Captured actual source trace unit and quaternion inverse formula, memory capture and initial event queue visit lifetime',flush=True)
