import hashlib
import json
from pathlib import Path

r=Path(__file__).resolve().parent
units=[('src/als_extras/core/game_mode.rs',[(110,140),(150,218)]),('src/als/character/animation_instances/ab_als_root.rs',[(166,188)]),('src/als/character/animation_instances/ab_als_provider.rs',[(270,307),(311,339)]),('src/animation_graph_ext/post_physics_animation.rs',[(182,211)]),('src/als/character/movement/motion.rs',[(4060,4069)])]
records=[]
for name,ranges in units:
    p=Path(name)
    lines=p.read_text().splitlines()
    records.append(dict(key=name,sha256=hashlib.sha256(p.read_bytes()).hexdigest(),excerpts=[dict(start=a,end=b,text='\n'.join(lines[a-1:b]))for a,b in ranges]))
p=Path('tools/ue_graph_source_reader/Source/AlsGraphSourceReader/Private/AlsGraphSourceReaderCommandlet.cpp')
lines=p.read_text().splitlines()
records.append(dict(key=str(p),sha256=hashlib.sha256(p.read_bytes()).hexdigest(),excerpts=[dict(start=a,end=b,text='\n'.join(lines[a-1:b]))for a,b in [(4480,4494),(4601,4633),(4789,4804),(4848,4863)]]))
(r/'source-facts.json').write_text(json.dumps(records,indent=2)+'\n')
print('Captured actual admission, root input, initialization and native replay boundaries',flush=True)
