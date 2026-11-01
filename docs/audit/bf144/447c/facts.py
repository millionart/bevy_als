import hashlib
import json
import os
from pathlib import Path

r=Path(__file__).resolve().parent
config=json.loads(Path('tools/env.local.json').read_text())
plugin=Path(config['ue_project']).parent/'Plugins/ALS-Refactored'
engine=Path(os.environ['ALS_UE_SOURCE_ROOT'])
units=[('source:Private/AlsAnimationInstance.cpp',plugin/'Source/ALS/Private/AlsAnimationInstance.cpp',[(136,146),(387,402),(528,678)]),('Engine/Source/Runtime/Core/Public/Math/SpringMath.h',engine/'Engine/Source/Runtime/Core/Public/Math/SpringMath.h',[(15,25),(130,145),(173,195)]),('src/als/state/head.rs',Path('src/als/state/head.rs'),[(33,50),(82,205)]),('src/als/character/animation_instances/ab_als_provider.rs',Path('src/als/character/animation_instances/ab_als_provider.rs'),[(280,305)]),('src/als/character/animation_instances/turn_layer.rs',Path('src/als/character/animation_instances/turn_layer.rs'),[(360,362)])]
facts=[]
for key,p,ranges in units:
    raw=p.read_bytes();lines=raw.decode().splitlines()
    facts.append(dict(key=key,sha256=hashlib.sha256(raw).hexdigest(),excerpts=[dict(start=a,end=b,text='\n'.join(lines[a-1:b]))for a,b in ranges]))
(r/'source-facts.json').write_text(json.dumps(facts,indent=2)+'\n')
print('Captured current Head spring, settings use and first View producer contracts',flush=True)
