import hashlib
import json
import os
from pathlib import Path

r=Path(__file__).resolve().parent
engine=Path(os.environ['ALS_UE_SOURCE_ROOT'])/'Engine/Source/Runtime'
units=[('Engine/Private/Animation/AnimNode_AssetPlayerBase.cpp',[(28,60),(73,87)]),('Engine/Private/Animation/AnimNode_StateMachine.cpp',[(1225,1260)]),('Engine/Private/Animation/AnimNode_SequencePlayer.cpp',[(24,27),(85,108)]),('AnimGraphRuntime/Private/AnimNodes/AnimNode_SequenceEvaluator.cpp',[(40,83)]),('AnimGraphRuntime/Private/AnimNodes/AnimNode_BlendSpacePlayer.cpp',[(22,31),(177,196)]),('Engine/Private/Animation/BlendSpace.cpp',[(527,544)]),('Engine/Public/Animation/AnimInstanceProxy.h',[(231,243)]),('Engine/Public/Animation/AnimSync.h',[(64,92)]),('Engine/Private/Animation/AnimSync.cpp',[(18,29),(396,411)]),('Engine/Private/Animation/AnimInstanceProxy.cpp',[(523,533),(916,924)]),('Engine/Classes/Animation/AnimationAsset.h',[(100,120),(384,398),(420,429),(612,624)])]
result=[]
for name,ranges in units:
    p=engine/name
    lines=p.read_text().splitlines()
    result.append(dict(key='source:UE/Runtime/'+name,sha256=hashlib.sha256(p.read_bytes()).hexdigest(),excerpts=[dict(start=a,end=b,text='\n'.join(lines[a-1:b]))for a,b in ranges]))
(r/'source-facts.json').write_text(json.dumps(result,indent=2)+'\n')
print('Captured eleven actual tick, read-buffer and clock unit source contracts',flush=True)
