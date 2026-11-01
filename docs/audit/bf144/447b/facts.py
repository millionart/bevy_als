import hashlib
import json
from pathlib import Path

r=Path(__file__).resolve().parent
units=[('src/als/character/animation_instances/source_native_refresh.rs',[(34,108)]),('src/als/character/animation_instances/head_layer.rs',[(75,165),(240,252)]),('src/animation_graph_ext/source_instance.rs',[(141,170),(180,211)]),('src/als/state/head.rs',[(55,76),(116,212)]),('assets/ALS/Character/AnimationInstances/AB_Als_Head/AnimGraph.animgraph.ron',[(8,11),(34,39)])]
facts=[]
for name,ranges in units:
    p=Path(name);lines=p.read_text().splitlines()
    facts.append(dict(key=name,sha256=hashlib.sha256(p.read_bytes()).hexdigest(),excerpts=[dict(start=a,end=b,text='\n'.join(lines[a-1:b]))for a,b in ranges]))
(r/'source-facts.json').write_text(json.dumps(facts,indent=2)+'\n')
print('Captured actual Head inputs/state resolver and Look consumer pin contracts',flush=True)
