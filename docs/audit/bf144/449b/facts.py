import hashlib
import json
import os
from pathlib import Path

r=Path(__file__).resolve().parent
engine=Path(os.environ['ALS_UE_SOURCE_ROOT'])
units=[('src/animation_graph_ext/source_sequence_evaluator.rs',Path('src/animation_graph_ext/source_sequence_evaluator.rs'),[(88,117)]),('src/animation_graph_ext/source_sequence_player.rs',Path('src/animation_graph_ext/source_sequence_player.rs'),[(90,140)]),('assets/ALS/Animations/Base/A_Als_Stand_Pose.anim.ron',Path('assets/ALS/Animations/Base/A_Als_Stand_Pose.anim.ron'),[(1,8)]),('Engine/Plugins/Enterprise/GLTFExporter/Source/GLTFExporter/Private/Utilities/GLTFCoreUtilities.cpp',engine/'Engine/Plugins/Enterprise/GLTFExporter/Source/GLTFExporter/Private/Utilities/GLTFCoreUtilities.cpp',[(6,21)]),('Engine/Plugins/Enterprise/GLTFExporter/Source/GLTFExporter/Private/Tasks/GLTFDelayedAnimationTasks.cpp',engine/'Engine/Plugins/Enterprise/GLTFExporter/Source/GLTFExporter/Private/Tasks/GLTFDelayedAnimationTasks.cpp',[(47,70)]),('Engine/Plugins/Enterprise/GLTFExporter/Source/GLTFExporter/Private/Options/GLTFExportOptions.cpp',engine/'Engine/Plugins/Enterprise/GLTFExporter/Source/GLTFExporter/Private/Options/GLTFExportOptions.cpp',[(9,18)]),('tools/ue_graph_source_reader/Source/AlsGraphSourceReader/Private/AlsGraphSourceReaderCommandlet.cpp',Path('tools/ue_graph_source_reader/Source/AlsGraphSourceReader/Private/AlsGraphSourceReaderCommandlet.cpp'),[(2516,2534)])]
facts=[]
for key,p,ranges in units:
    raw=p.read_bytes();lines=raw.decode().splitlines()
    facts.append(dict(key=key,sha256=hashlib.sha256(raw).hexdigest(),excerpts=[dict(start=a,end=b,text='\n'.join(lines[a-1:b]))for a,b in ranges]))
(r/'source-facts.json').write_text(json.dumps(facts,indent=2)+'\n')
print('Captured actual Idle evaluator/public sampler and native float export-unit contracts',flush=True)
