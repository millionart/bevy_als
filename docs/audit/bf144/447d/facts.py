import hashlib
import json
import os
from pathlib import Path

r=Path(__file__).resolve().parent
config=json.loads(Path('tools/env.local.json').read_text())
plugin=Path(config['ue_project']).parent/'Plugins/ALS-Refactored'
engine=Path(os.environ['ALS_UE_SOURCE_ROOT'])
units=[('source:Private/AlsAnimationInstance.cpp',plugin/'Source/ALS/Private/AlsAnimationInstance.cpp',[(372,402),(528,600)]),('source:Private/AlsCharacter.cpp',plugin/'Source/ALS/Private/AlsCharacter.cpp',[(1222,1266)]),('source:Public/State/AlsViewAnimationState.h',plugin/'Source/ALS/Public/State/AlsViewAnimationState.h',[(6,22)]),('source:Public/State/AlsLocomotionAnimationState.h',plugin/'Source/ALS/Public/State/AlsLocomotionAnimationState.h',[(43,54)]),('src/als/character/animation_instances/ab_als_provider.rs',Path('src/als/character/animation_instances/ab_als_provider.rs'),[(272,305)]),('src/als/character/animation_instances/direction_layer.rs',Path('src/als/character/animation_instances/direction_layer.rs'),[(34,43)]),('src/als/utility/orientation.rs',Path('src/als/utility/orientation.rs'),[(1,15)]),('src/als/character/animation_instances/turn_layer.rs',Path('src/als/character/animation_instances/turn_layer.rs'),[(360,362)]),('tools/ue_graph_source_reader/Source/AlsGraphSourceReader/Private/AlsGraphSourceReaderCommandlet.cpp',Path('tools/ue_graph_source_reader/Source/AlsGraphSourceReader/Private/AlsGraphSourceReaderCommandlet.cpp'),[(1130,1150)])]
facts=[]
for key,p,ranges in units:
    raw=p.read_bytes();lines=raw.decode().splitlines()
    facts.append(dict(key=key,sha256=hashlib.sha256(raw).hexdigest(),excerpts=[dict(start=a,end=b,text='\n'.join(lines[a-1:b]))for a,b in ranges]))
(r/'source-facts.json').write_text(json.dumps(facts,indent=2)+'\n')
print('Captured fresh View types, difference/cast order, actual producer and reused native scalar contracts',flush=True)
