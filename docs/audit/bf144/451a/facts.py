import hashlib
import json
import os
from pathlib import Path

r=Path(__file__).resolve().parent
units=[('src/animation_graph_ext/source_pose_nodes.rs',Path('src/animation_graph_ext/source_pose_nodes.rs'),[(161,239),(621,697)]),('src/animation_graph_ext/mesh_space_additive.rs',Path('src/animation_graph_ext/mesh_space_additive.rs'),[(185,261)]),('src/animation_graph_ext/source_instance.rs',Path('src/animation_graph_ext/source_instance.rs'),[(141,199)])]
facts=[]
for key,p,ranges in units:
    raw=p.read_bytes();lines=raw.decode().splitlines()
    facts.append(dict(key=key,sha256=hashlib.sha256(raw).hexdigest(),excerpts=[dict(start=a,end=b,text='\n'.join(lines[a-1:b]))for a,b in ranges]))
(r/'source-facts.json').write_text(json.dumps(facts,indent=2)+'\n')
print('Captured alpha resolution, noninterpolated state semantics and actual full Head composition contracts',flush=True)
