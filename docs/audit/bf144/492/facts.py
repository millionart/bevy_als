import hashlib
import json
import os
from pathlib import Path

root = Path(__file__).resolve().parent
engine = Path(os.environ['ALS_UE_SOURCE_ROOT'])
config = json.loads(Path('tools/env.local.json').read_text())
als = Path(config['ue_project']).parent/'Plugins/ALS-Refactored/Source/ALS'
facts = []
for base,key,ranges in [
    (als,'Private/AlsAnimationInstance.cpp',[(235,257),(1277,1295)]),
    (engine,'Engine/Plugins/Animation/ControlRig/Source/ControlRig/Private/AnimNode_ControlRig.cpp',[(142,155),(249,264)]),
    (engine,'Engine/Plugins/Animation/ControlRig/Source/ControlRig/Private/AnimNode_ControlRigBase.cpp',[(174,194),(333,348),(420,475)]),
    (Path('.'),'src/animation_graph_ext/root_graph_boundaries.rs',[(218,241),(295,301),(443,485)]),
    (Path('.'),'src/animation_graph_ext/post_physics_animation.rs',[(182,210)]),
]:
    raw = (base/key).read_bytes()
    lines = raw.decode('utf-8-sig').splitlines()
    facts.append(dict(key=f'source:{key}' if base==als else key,sha256=hashlib.sha256(raw).hexdigest(),
                      excerpts=[dict(start=a,end=b,text='\n'.join(lines[a-1:b])) for a,b in ranges]))
(root/'source-facts.json').write_text(json.dumps(facts,indent=2)+'\n')
print('Captured native and target control rig construction boundaries',flush=True)
