import hashlib
import json
import os
from pathlib import Path

r=Path(__file__).resolve().parent
units=[(Path('src/als/character/movement/motion.rs'),'src/als/character/movement/motion.rs',[(147,169),(2570,2584),(4060,4069)]),(Path('src/als/character/movement/source_physics_timing.rs'),'src/als/character/movement/source_physics_timing.rs',[(80,100),(164,194)])]
dep=next(Path('target/play/deps').glob('bevy_tnua-*.d')).read_text().split()
p=Path(next(v for v in dep if v.endswith('/src/builtins/walk.rs')))
units.append((p,'dependency:bevy_tnua/src/builtins/walk.rs',[(157,170),(404,427),(624,647)]))
p=Path('tools/env.local.json');config=json.loads(p.read_text())
plugin=Path(config['ue_project']).parent/'Plugins/ALS-Refactored/Source/ALS/Private'
units.append((plugin/'AlsCharacterMovementComponent.cpp','source:ALS/Private/AlsCharacterMovementComponent.cpp',[(392,399),(523,528)]))
p=Path(os.environ['ALS_UE_SOURCE_ROOT'])/'Engine/Source/Runtime/Engine/Private/Components/CharacterMovementComponent.cpp'
units.append((p,'source:UE/Engine/Private/Components/CharacterMovementComponent.cpp',[(6295,6336)]))
records=[]
for p,key,ranges in units:
    lines=p.read_text().splitlines()
    records.append(dict(key=key,sha256=hashlib.sha256(p.read_bytes()).hexdigest(),excerpts=[dict(start=a,end=b,text='\n'.join(lines[a-1:b]))for a,b in ranges]))
(r/'source-facts.json').write_text(json.dumps(records,indent=2)+'\n')
extra=[(Path('src/als_extras/levels/manual_playground.rs'),'src/als_extras/levels/manual_playground.rs',[(580,590),(678,690)])]
tnua=next(Path('target/play/deps').glob('bevy_tnua_avian3d-*.d')).read_text().split()
p=Path(next(v for v in tnua if v.endswith('/avian3d/src/lib.rs')))
extra.append((p,'dependency:bevy_tnua/avian3d/src/lib.rs',[(419,452)]))
avian=next(Path('target/play/deps').glob('avian3d-*.d')).read_text().split()
for suffix,ranges in [('dynamics/solver/solver_body/plugin.rs',[(38,75)]),('dynamics/integrator/mod.rs',[(260,310)])]:
    p=Path(next(v for v in avian if v.endswith('/src/'+suffix)))
    extra.append((p,'dependency:avian3d/src/'+suffix,ranges))
for p,key,ranges in extra:
    lines=p.read_text().splitlines()
    records.append(dict(key=key,sha256=hashlib.sha256(p.read_bytes()).hexdigest(),excerpts=[dict(start=a,end=b,text='\n'.join(lines[a-1:b]))for a,b in ranges]))
(r/'source-facts.json').write_text(json.dumps(records,indent=2)+'\n')
print('Captured spring force, exact physics phase and source in-band floor contracts',flush=True)
