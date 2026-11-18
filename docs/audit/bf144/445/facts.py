import hashlib
import json
import os
from pathlib import Path

r=Path(__file__).resolve().parent
units=[('src/animation_graph_ext/animation_context.rs',[(40,64),(281,312)]),('src/animation_graph_ext/source_sequence_evaluator.rs',[(96,112)]),('src/animation_graph_ext/source_sequence_player.rs',[(517,551)]),('src/animation_graph_ext/normalized_blendspace.rs',[(850,879),(906,916)])]
dep=next(Path('target/play/deps').glob('bevy_animation_graph_core-*.d')).read_text().split()
records=[]
for name,ranges in units:
    p=Path(name)
    lines=p.read_text().splitlines()
    records.append(dict(key=name,sha256=hashlib.sha256(p.read_bytes()).hexdigest(),excerpts=[dict(start=a,end=b,text='\n'.join(lines[a-1:b]))for a,b in ranges]))
for name,ranges in [('context/node_caches.rs',[(18,41),(123,139)]),('context/node_states.rs',[(101,119),(164,183)])]:
    p=Path(next(v for v in dep if v.endswith('/'+name)))
    lines=p.read_text().splitlines()
    records.append(dict(key='dependency:bevy_animation_graph/'+name,sha256=hashlib.sha256(p.read_bytes()).hexdigest(),excerpts=[dict(start=a,end=b,text='\n'.join(lines[a-1:b]))for a,b in ranges]))
(r/'source-facts.json').write_text(json.dumps(records,indent=2)+'\n')
engine=Path(os.environ['ALS_UE_SOURCE_ROOT'])/'Engine/Source/Runtime/Engine/Private/Animation'
for name,ranges in [('AnimSequenceBase.cpp',[(501,541)]),('AnimationRuntime.cpp',[(1562,1590)])]:
    p=engine/name
    lines=p.read_text().splitlines()
    records.append(dict(key='source:UE/Engine/Private/Animation/'+name,sha256=hashlib.sha256(p.read_bytes()).hexdigest(),excerpts=[dict(start=a,end=b,text='\n'.join(lines[a-1:b]))for a,b in ranges]))
p=Path('src/animation_graph_ext/sync_group_phase.rs')
lines=p.read_text().splitlines()
records.append(dict(key=str(p),sha256=hashlib.sha256(p.read_bytes()).hexdigest(),excerpts=[dict(start=530,end=549,text='\n'.join(lines[529:549]))]))
(r/'source-facts.json').write_text(json.dumps(records,indent=2)+'\n')
print('Captured current public cache/key and leaf clock source contracts',flush=True)
