import hashlib
import json
import os
from pathlib import Path

r=Path(__file__).resolve().parent;engine=Path(os.environ['ALS_UE_SOURCE_ROOT']);config=json.loads(Path('tools/env.local.json').read_bytes());plugin=Path(config['ue_project']).parent/'Plugins/ALS-Refactored/Source/ALS';specs=[('Engine/Plugins/Runtime/RigVM/Source/RigVM/Public/RigVMCore/RigVM.h',[(899,920)]),('Engine/Plugins/Runtime/RigVM/Source/RigVM/Public/RigVMCore/RigVMFunction.h',[(26,31),(80,113),(143,173)]),('Engine/Plugins/Runtime/RigVM/Source/RigVM/Public/RigVMCore/RigVMMemoryStorage.h',[(266,325),(399,417)]),('Engine/Plugins/Runtime/RigVM/Source/RigVM/Private/RigVMCore/RigVM.cpp',[(2088,2114)]),('Engine/Plugins/Animation/ControlRig/Source/ControlRig/Private/Units/Highlevel/Hierarchy/RigUnit_TwoBoneIKSimple.cpp',[(143,156),(186,220)]),('Engine/Plugins/Animation/ControlRig/Source/ControlRig/Public/Rigs/RigHierarchyElements.h',[(476,481)]),('source:Private/Nodes/AlsRigUnit_ApplyFootOffsetRotation.cpp',[(13,60),(99,113)])];facts=[]
for key,ranges in specs:
 p=plugin/key.split(':',1)[1]if key.startswith('source:')else engine/key;raw=p.read_bytes();lines=raw.decode('utf-8-sig').splitlines();facts.append(dict(key=key,sha256=hashlib.sha256(raw).hexdigest(),excerpts=[dict(start=a,end=b,text='\n'.join(lines[a-1:b]))for a,b in ranges]))
key='tools/ue_graph_source_reader/Source/AlsGraphSourceReader/Private/AlsGraphSourceReaderCommandlet.cpp';raw=(r/'reader.final.cpp').read_bytes();lines=raw.decode().splitlines();markers=[('using FNativeRigUnitBoundary',10),('    FString BoundaryFunctionName;',17),('        URigVM* BoundaryVM = Rig->GetVM();',79),('            if (!BoundaryFunctionName.IsEmpty()) { Row->SetArrayField',6),('        CaseRow->SetStringField(TEXT("external_controls_stage")',9)];excerpts=[]
for marker,count in markers:
 i=next(i for i,line in enumerate(lines)if line.startswith(marker));excerpts.append(dict(start=i+1,end=i+count,text='\n'.join(lines[i:i+count])))
facts.append(dict(key=key,sha256=hashlib.sha256(raw).hexdigest(),excerpts=excerpts));(r/'source-facts.json').write_text(json.dumps(facts,indent=2)+'\n');print('Captured public VM descriptor dispatch, exact key memory handles, readonly native call stage and scoped restoration',flush=True)
