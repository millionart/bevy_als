import hashlib
import json
from pathlib import Path

r=Path(__file__).resolve().parent
units=[('src/als/character/control_rig_apply.rs',[(388,431)]),('tools/animation_visual_validation.rs',[(775,785)]),('src/animation_graph_ext/root_graph_boundaries.rs',[(30,46),(75,115),(208,300),(316,378),(419,450),(500,520)]),('src/control_rig/control_rig_program.rs',[(12,18),(131,142)]),('src/control_rig/control_rig_hierarchy.rs',[(304,355)]),('src/animation_graph_ext/source_instance.rs',[(141,199)])]
facts=[]
for key,ranges in units:
    p=Path(key);raw=p.read_bytes();lines=raw.decode().splitlines();facts.append(dict(key=key,sha256=hashlib.sha256(raw).hexdigest(),excerpts=[dict(start=a,end=b,text='\n'.join(lines[a-1:b]))for a,b in ranges]))
(r/'source-facts.json').write_text(json.dumps(facts,indent=2)+'\n')
print('Captured actual Rig public state access, clone-only pre-call bone capture, post-call arguments, input priorities and local-pose writeback',flush=True)
