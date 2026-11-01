import hashlib
import json
import os
from pathlib import Path

r=Path(__file__).resolve().parent
dep=Path(os.environ['ALS_BAG_CORE_SOURCE_ROOT'])
units=[('src/animation_graph_ext/post_physics_animation.rs',Path('src/animation_graph_ext/post_physics_animation.rs'),[(78,88),(213,231)]),('dependency:skeleton/loader.rs',dep/'skeleton/loader.rs',[(57,88)]),('dependency:animation_graph_player.rs',dep/'animation_graph_player.rs',[(340,362)])]
facts=[]
for key,p,ranges in units:
    raw=p.read_bytes();lines=raw.decode().splitlines()
    facts.append(dict(key=key,sha256=hashlib.sha256(raw).hexdigest(),excerpts=[dict(start=a,end=b,text='\n'.join(lines[a-1:b]))for a,b in ranges]))
(r/'source-facts.json').write_text(json.dumps(facts,indent=2)+'\n')
print('Captured actual project pose application gate and scene-target binding lookup contracts',flush=True)
