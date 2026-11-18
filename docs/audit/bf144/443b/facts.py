import hashlib
import json
import os
from pathlib import Path

r=Path(__file__).resolve().parent
config=json.loads(Path('tools/env.local.json').read_text());plugin=Path(config['ue_project']).parent/'Plugins/ALS-Refactored';engine=Path(os.environ['ALS_UE_SOURCE_ROOT'])
units=[(engine/'Engine/Source/Runtime/Engine/Private/Components/CharacterMovementComponent.cpp','Engine/Source/Runtime/Engine/Private/Components/CharacterMovementComponent.cpp',[(98,99),(6295,6354)]),(plugin/'Source/ALS/Private/AlsCharacterMovementComponent.cpp','source:ALS/Private/AlsCharacterMovementComponent.cpp',[(392,398),(500,535)])]
for name,ranges in [('src/als/character/character.rs',[(52,53)]),('src/als/character/movement/motion.rs',[(2538,2583),(3506,3596),(3815,3828),(3846,3864),(3986,4073)]),('src/als/character/movement/source_physics_timing.rs',[(80,98)])]:units.append((Path(name),name,ranges))
tokens=next(Path('target/play/deps').glob('avian3d-*.d')).read_text().split()
for suffix,ranges in [('dynamics/integrator/mod.rs',[(216,249),(258,313),(348,368)]),('dynamics/solver/solver_body/plugin.rs',[(263,291)])]:
 p=Path(next(v for v in tokens if v.endswith('/src/'+suffix)));units.append((p,'dependency:avian3d-0.7.0/src/'+suffix,ranges))
tokens=next(Path('target/play/deps').glob('bevy_tnua_avian3d-*.d')).read_text().split();p=Path(next(v for v in tokens if v.endswith('/src/lib.rs')));units.append((p,'dependency:bevy_tnua_avian3d/src/lib.rs',[(419,451)]))
facts=[]
for p,key,ranges in units:
 lines=p.read_text().splitlines();facts.append(dict(key=key,sha256=hashlib.sha256(p.read_bytes()).hexdigest(),excerpts=[dict(start=a,end=b,text='\n'.join(lines[a-1:b]))for a,b in ranges]))
(r/'source-facts.json').write_text(json.dumps(facts,indent=2)+'\n');print('Captured source floor-band and target transaction/integration/final-floor facts',flush=True)
