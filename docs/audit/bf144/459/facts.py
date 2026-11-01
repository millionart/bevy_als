import hashlib
import json
import os
from pathlib import Path

r=Path(__file__).resolve().parent;config=json.loads(Path('tools/env.local.json').read_text());plugin=Path(config['ue_project']).parent/'Plugins/ALS-Refactored'
units=[('src/animation_graph_ext/post_physics_animation.rs',Path('src/animation_graph_ext/post_physics_animation.rs'),[(181,206)]),('src/animation_graph_ext/root_graph_boundaries.rs',Path('src/animation_graph_ext/root_graph_boundaries.rs'),[(208,300),(316,378),(419,438),(500,520)]),('src/control_rig/control_rig_program.rs',Path('src/control_rig/control_rig_program.rs'),[(160,257)]),('source:Private/AlsAnimationInstance.cpp',plugin/'Source/ALS/Private/AlsAnimationInstance.cpp',[(235,260)])]
facts=[]
for key,p,ranges in units:
 raw=p.read_bytes();lines=raw.decode().splitlines();facts.append(dict(key=key,sha256=hashlib.sha256(raw).hexdigest(),excerpts=[dict(start=a,end=b,text='\n'.join(lines[a-1:b]))for a,b in ranges]))
(r/'source-facts.json').write_text(json.dumps(facts,indent=2)+'\n')
print('Captured actual initialization delta override, complete public VM stages, clone-only call capture and source RigInput producer facts',flush=True)
