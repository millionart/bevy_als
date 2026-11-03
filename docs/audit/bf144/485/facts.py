import hashlib
import json
import os
from pathlib import Path

root = Path(__file__).resolve().parent
engine = Path(os.environ['ALS_UE_SOURCE_ROOT'])
config = json.loads(Path('tools/env.local.json').read_text())
als = Path(config['ue_project']).parent/'Plugins/ALS-Refactored/Source/ALS'
specs = {
    'source:Private/AlsAnimationInstance.cpp':[(1245,1327)],
    'Engine/Source/Runtime/Engine/Private/SkinnedAsset.cpp':[(484,495),(527,545)],
    'Engine/Source/Runtime/Engine/Private/Components/SkinnedMeshComponent.cpp':[(2401,2415),(2465,2476),(3525,3565)],
    'Engine/Source/Runtime/Core/Private/Math/TransformVectorized.cpp':[(295,343)],
    'Engine/Source/Runtime/Core/Public/Math/UnrealMathNeon.h':[(2328,2336)],
    'src/als/character/foot_ik_apply.rs':[(145,172),(477,539),(565,679)],
    'src/als/state/foot_ik.rs':[(150,158),(297,367),(875,930)],
    'src/control_rig/ik_space.rs':[(46,64),(91,104)],
    'src/als/character/control_rig_apply.rs':[(27,50)],
    'src/animation_graph_ext/post_physics_animation.rs':[(44,54),(213,228)],
    'assets/ALS/Character/CR_Als.runtime.json':[(101,120)],
}
facts = []
for key, ranges in specs.items():
    if key.startswith('source:'):
        path = als/key.split(':',1)[1]
    elif key.startswith('Engine/'):
        path = engine/key
    else:
        path = Path(key)
    raw = path.read_bytes()
    lines = raw.decode('utf-8-sig').splitlines()
    facts.append(dict(key=key,sha256=hashlib.sha256(raw).hexdigest(),
                      excerpts=[dict(start=a,end=b,text='\n'.join(lines[a-1:b])) for a,b in ranges]))
(root/'source-facts.json').write_text(json.dumps(facts,indent=2)+'\n')
print('Captured native socket branch, UE FTransform composition and actual FootIk update ordering',flush=True)
