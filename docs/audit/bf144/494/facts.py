import hashlib
import json
import os
import re
from pathlib import Path

root = Path(__file__).resolve().parent
lock = Path('Cargo.lock').read_text()
version = re.search(r'\[\[package\]\]\nname = "bevy_transform"\nversion = "([^"]+)"',lock)[1]
cargo_sources = Path.home()/'.cargo/registry/src'
registry = next(cargo_sources.glob(f'*/bevy_transform-{version}'))
bevy_math = lock.split('name = "bevy_math"\n',1)[1].split('[[package]]',1)[0]
glam_version = re.search(r'"glam ([^"]+)"',bevy_math)[1]
glam = next(cargo_sources.glob(f'*/glam-{glam_version}'))
engine = Path(os.environ['ALS_UE_SOURCE_ROOT'])
config = json.loads(Path('tools/env.local.json').read_text())
als = Path(config['ue_project']).parent/'Plugins/ALS-Refactored/Source/ALS'
facts = []
for base, key, ranges in [
    (als,'Private/AlsAnimationInstance.cpp',[(1277,1295)]),
    (engine,'Engine/Source/Runtime/Engine/Private/SkinnedAsset.cpp',[(484,495),(520,547)]),
    (engine,'Engine/Source/Runtime/Engine/Private/Components/SkinnedMeshComponent.cpp',[(2465,2476),(3525,3565)]),
    (engine,'Engine/Source/Runtime/Core/Private/Math/TransformVectorized.cpp',[(295,343)]),
    (registry,'src/helper.rs',[(26,46)]),
    (registry,'src/components/transform.rs',[(270,281),(641,654)]),
    (registry,'src/components/global_transform.rs',[(190,208),(224,232),(307,326)]),
    (glam,'src/f32/affine3a.rs',[(295,353)]),
    (Path('.'),'src/als/character/foot_ik_apply.rs',[(632,677)]),
    (Path('.'),'src/als/state/foot_ik.rs',[(298,318)]),
]:
    raw = (base/key).read_bytes()
    lines = raw.decode('utf-8-sig').splitlines()
    identifier = f'dependency:bevy_transform-{version}/{key}' if base==registry else key
    if base==glam:
        identifier = f'dependency:glam-{glam_version}/{key}'
    if base==als:
        identifier = f'source:{key}'
    facts.append(dict(key=identifier, sha256=hashlib.sha256(raw).hexdigest(),
                      excerpts=[dict(start=a,end=b,text='\n'.join(lines[a-1:b])) for a,b in ranges]))
(root/'source-facts.json').write_text(json.dumps(facts,indent=2)+'\n')
print('Captured current UE TQS, locked Bevy affine and virtual foot sampling boundaries',flush=True)
