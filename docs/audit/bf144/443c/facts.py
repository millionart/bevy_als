import hashlib
import json
import os
from pathlib import Path

r=Path(__file__).resolve().parent;engine=Path(os.environ['ALS_UE_SOURCE_ROOT']);config=json.loads(Path('tools/env.local.json').read_text());plugin=Path(config['ue_project']).parent/'Plugins/ALS-Refactored'
units=[(engine/'Engine/Source/Runtime/Engine/Classes/Components/CapsuleComponent.h','Engine/Source/Runtime/Engine/Classes/Components/CapsuleComponent.h',[(202,221)]),(engine/'Engine/Source/Runtime/Core/Public/Math/TransformVectorized.h','Engine/Source/Runtime/Core/Public/Math/TransformVectorized.h',[(1235,1244)]),(engine/'Engine/Source/Runtime/Core/Public/Math/TransformNonVectorized.h','Engine/Source/Runtime/Core/Public/Math/TransformNonVectorized.h',[(1126,1135)]),(engine/'Engine/Source/Runtime/Engine/Private/Character.cpp','Engine/Source/Runtime/Engine/Private/Character.cpp',[(74,87)]),(plugin/'Source/ALS/Private/AlsCharacterMovementComponent.cpp','source:ALS/Private/AlsCharacterMovementComponent.cpp',[(636,646)]),(Path('src/als/character/movement/motion.rs'),'src/als/character/movement/motion.rs',[(3996,4019)])]
tokens=next(Path('target_game/play/deps').glob('bevy_transform-*.d')).read_text().split();p=Path(next(v for v in tokens if v.endswith('/src/components/global_transform.rs')));units.append((p,'dependency:bevy_transform/src/components/global_transform.rs',[(200,202),(308,312)]))
tokens=next(Path('target_game/play/deps').glob('glam-*.d')).read_text().split();p=Path(next(v for v in tokens if v.endswith('/src/f32/affine3a.rs')));lines=p.read_text().splitlines();start=next(i for i,l in enumerate(lines,1)if 'pub fn to_scale_rotation_translation('in l);units.append((p,'dependency:glam/src/f32/affine3a.rs',[(start,start+23)]))
facts=[]
for p,key,ranges in units:
    lines=p.read_text().splitlines();facts.append(dict(key=key,sha256=hashlib.sha256(p.read_bytes()).hexdigest(),excerpts=[dict(start=a,end=b,text='\n'.join(lines[a-1:b]))for a,b in ranges]))
(r/'source-facts.json').write_text(json.dumps(facts,indent=2)+'\n');print('Captured stored source scale, mapped capsule sizes and actual affine getters',flush=True)
