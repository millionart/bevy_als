import hashlib
import json
import os
from pathlib import Path

r=Path(__file__).resolve().parent
dep=Path(os.environ['ALS_BAG_CORE_SOURCE_ROOT'])
gltf=Path(os.environ['ALS_GLTF_LOADER_SOURCE'])
units=[('tools/import/coordinate_basis.py',Path('tools/import/coordinate_basis.py'),[(5,42)]),('dependency:systems.rs',dep/'systems.rs',[(104,133),(204,241)]),('dependency:skeleton/loader.rs',dep/'skeleton/loader.rs',[(57,88)]),('dependency:gltf/loader/mod.rs',gltf,[(1543,1562)]),('src/animation_graph_ext/post_physics_animation.rs',Path('src/animation_graph_ext/post_physics_animation.rs'),[(213,231)])]
facts=[]
for key,p,ranges in units:
    raw=p.read_bytes();lines=raw.decode().splitlines()
    facts.append(dict(key=key,sha256=hashlib.sha256(raw).hexdigest(),excerpts=[dict(start=a,end=b,text='\n'.join(lines[a-1:b]))for a,b in ranges]))
(r/'source-facts.json').write_text(json.dumps(facts,indent=2)+'\n')
print('Captured neutral coordinate projection and conditional scene-target writeback qualification',flush=True)
