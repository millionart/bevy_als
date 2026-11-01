import hashlib
import json
import os
from pathlib import Path

r=Path(__file__).resolve().parent
engine=Path(os.environ['ALS_UE_SOURCE_ROOT'])/'Engine/Source/Runtime/Engine/Private'
config=json.loads(Path('tools/env.local.json').read_text())
plugin=Path(config['ue_project']).parent/'Plugins/ALS-Refactored/Source/ALS/Private'
units=[(engine/'Controller.cpp','source:UE/Engine/Private/Controller.cpp',[(132,153)]),(plugin/'AlsAnimationInstance.cpp','source:ALS/Private/AlsAnimationInstance.cpp',[(681,719)]),(plugin/'AlsCharacter.cpp','source:ALS/Private/AlsCharacter.cpp',[(1255,1267),(1372,1389)]),(Path('src/animation_graph_ext/post_physics_animation.rs'),'src/animation_graph_ext/post_physics_animation.rs',[(182,211)]),(Path('src/als/character/animation_instances/ab_als_provider.rs'),'src/als/character/animation_instances/ab_als_provider.rs',[(302,339)]),(Path('src/als/character/movement/motion.rs'),'src/als/character/movement/motion.rs',[(147,168)])]
records=[]
for p,key,ranges in units:
    lines=p.read_text().splitlines()
    records.append(dict(key=key,sha256=hashlib.sha256(p.read_bytes()).hexdigest(),excerpts=[dict(start=a,end=b,text='\n'.join(lines[a-1:b]))for a,b in ranges]))
(r/'source-facts.json').write_text(json.dumps(records,indent=2)+'\n')
extra=[(plugin/'AlsAnimationInstance.cpp','source:ALS/Private/AlsAnimationInstance.cpp',[(337,375)]),(engine/'Animation/AnimInstanceProxy.cpp','source:UE/Engine/Private/Animation/AnimInstanceProxy.cpp',[(3565,3594)]),(Path('src/als/character/animation_instances/animation_context_projection.rs'),'src/als/character/animation_instances/animation_context_projection.rs',[(77,111)]),(Path('src/als/character/animation_instances/ab_als_provider.rs'),'src/als/character/animation_instances/ab_als_provider.rs',[(139,155)])]
for p,key,ranges in extra:
    lines=p.read_text().splitlines()
    records.append(dict(key=key,sha256=hashlib.sha256(p.read_bytes()).hexdigest(),excerpts=[dict(start=a,end=b,text='\n'.join(lines[a-1:b]))for a,b in ranges]))
(r/'source-facts.json').write_text(json.dumps(records,indent=2)+'\n')
print('Captured actual controller setter, pending lifecycle and input producer contracts',flush=True)
