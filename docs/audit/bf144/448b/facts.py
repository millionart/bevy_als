import hashlib
import json
import os
from pathlib import Path

r=Path(__file__).resolve().parent
dep=Path(os.environ['ALS_BAG_CORE_SOURCE_ROOT'])
units=[('dependency:pose.rs',dep/'pose.rs',[(95,106),(230,249)]),('dependency:animation_graph_player.rs',dep/'animation_graph_player.rs',[(340,361)]),('src/animation_graph_ext/animation_context.rs',Path('src/animation_graph_ext/animation_context.rs'),[(281,307)]),('src/als/character/animation_instances/animation_context_projection.rs',Path('src/als/character/animation_instances/animation_context_projection.rs'),[(20,32),(77,113)]),('src/animation_graph_ext/source_linked_input_pose.rs',Path('src/animation_graph_ext/source_linked_input_pose.rs'),[(35,67)]),('src/animation_graph_ext/source_instance.rs',Path('src/animation_graph_ext/source_instance.rs'),[(234,253)])]
facts=[]
for key,p,ranges in units:
    raw=p.read_bytes();lines=raw.decode().splitlines()
    facts.append(dict(key=key,sha256=hashlib.sha256(raw).hexdigest(),excerpts=[dict(start=a,end=b,text='\n'.join(lines[a-1:b]))for a,b in ranges]))
(r/'source-facts.json').write_text(json.dumps(facts,indent=2)+'\n')
print('Captured actual Pose outputs, optional channel presence, no custom attributes and per-node curve observation contracts',flush=True)
