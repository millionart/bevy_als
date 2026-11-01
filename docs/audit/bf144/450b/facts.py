import hashlib
import json
import os
from pathlib import Path

r=Path(__file__).resolve().parent
engine=Path(os.environ['ALS_UE_SOURCE_ROOT'])
units=[('src/animation_graph_ext/reference_mesh_difference.rs',Path('src/animation_graph_ext/reference_mesh_difference.rs'),[(10,61)]),('src/animation_graph_ext/mesh_space_difference.rs',Path('src/animation_graph_ext/mesh_space_difference.rs'),[(80,103),(129,173)]),('Engine/Source/Runtime/Engine/Private/Animation/AnimSequence.cpp',engine/'Engine/Source/Runtime/Engine/Private/Animation/AnimSequence.cpp',[(1679,1700),(1938,1971)]),('tools/ue_graph_source_reader/Source/AlsGraphSourceReader/Private/AlsGraphSourceReaderCommandlet.cpp',Path('tools/ue_graph_source_reader/Source/AlsGraphSourceReader/Private/AlsGraphSourceReaderCommandlet.cpp'),[(2394,2400),(2462,2472),(2516,2528)])]
facts=[]
for key,p,ranges in units:
    raw=p.read_bytes();lines=raw.decode().splitlines()
    facts.append(dict(key=key,sha256=hashlib.sha256(raw).hexdigest(),excerpts=[dict(start=a,end=b,text='\n'.join(lines[a-1:b]))for a,b in ranges]))
(r/'source-facts.json').write_text(json.dumps(facts,indent=2)+'\n')
print('Captured actual reference/target difference and raw versus compressed additive contracts',flush=True)
