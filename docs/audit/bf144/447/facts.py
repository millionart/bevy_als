import hashlib
import json
import os
from pathlib import Path

r=Path(__file__).resolve().parent
plugin=Path(json.loads(Path('tools/env.local.json').read_text())['ue_project']).parent/'Plugins/ALS-Refactored/Source/ALS/Private'
engine=Path(os.environ['ALS_UE_SOURCE_ROOT'])/'Engine/Source/Runtime/Core/Public/Math'
units=[(plugin/'AlsAnimationInstance.cpp','source:ALS/Private/AlsAnimationInstance.cpp',[(523,553),(632,680)]),(engine/'SpringMath.h','source:UE/Core/Public/Math/SpringMath.h',[(137,140),(173,194)]),(engine/'UnrealMathUtility.h','source:UE/Core/Public/Math/UnrealMathUtility.h',[(1551,1559)]),(Path('src/als/state/head.rs'),'src/als/state/head.rs',[(95,115)]),(Path('src/als/character/animation_instances/source_native_refresh.rs'),'src/als/character/animation_instances/source_native_refresh.rs',[(35,108)]),(Path('src/als/character/animation_instances/head_layer.rs'),'src/als/character/animation_instances/head_layer.rs',[(75,165)])]
facts=[]
for p,key,ranges in units:
    lines=p.read_text().splitlines()
    facts.append(dict(key=key,sha256=hashlib.sha256(p.read_bytes()).hexdigest(),excerpts=[dict(start=a,end=b,text='\n'.join(lines[a-1:b]))for a,b in ranges]))
(r/'source-facts.json').write_text(json.dumps(facts,indent=2)+'\n')
print('Captured Head update, spring and actual graph-input writer contracts',flush=True)
