import hashlib
import json
import os
from pathlib import Path

r=Path(__file__).resolve().parent;config=json.loads(Path('tools/env.local.json').read_text());plugin=Path(config['ue_project']).parent/'Plugins/ALS-Refactored';engine=Path(os.environ['ALS_UE_SOURCE_ROOT'])
units=[('source:Private/Nodes/AlsRigUnit_FootOffsetTrace.cpp',[(18,59)]),('source:Private/Nodes/AlsRigUnit_ApplyFootOffsetLocation.cpp',[(12,67)]),('source:Private/Nodes/AlsRigUnit_ApplyFootOffsetRotation.cpp',[(12,62),(115,135)]),('Engine/Plugins/Animation/ControlRig/Source/ControlRig/Private/Units/Simulation/RigUnit_SpringInterp.cpp',[(62,86)]),('src/control_rig/control_rig_program.rs',[(201,249)]),('src/control_rig/control_rig_units.rs',[(135,228),(229,320)]),('src/animation_graph_ext/root_graph_boundaries.rs',[(208,300)]),('src/animation_graph_ext/post_physics_animation.rs',[(181,206)])]
facts=[]
for key,ranges in units:
 p=plugin/'Source/ALS'/key.split(':',1)[1]if key.startswith('source:')else engine/key if key.startswith('Engine/')else Path(key)
 raw=p.read_bytes();lines=raw.decode('utf-8-sig').splitlines();facts.append(dict(key=key,sha256=hashlib.sha256(raw).hexdigest(),excerpts=[dict(start=a,end=b,text='\n'.join(lines[a-1:b]))for a,b in ranges]))
(r/'source-facts.json').write_text(json.dumps(facts,indent=2)+'\n');print('Captured source trace consumer springs and foot rotation plus complete VM output writeback and actual initialization lifetime',flush=True)
