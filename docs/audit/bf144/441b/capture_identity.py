import hashlib
import json
import os
from pathlib import Path
r=Path(__file__).resolve().parent
config=json.loads(Path('tools/env.local.json').read_text());plugin=Path(config['ue_project']).parent/'Plugins/ALS-Refactored';engine=Path(os.environ['ALS_UE_SOURCE_ROOT'])
paths={}
for folder,label,suffixes in [(Path('src'),'target',{'.rs'}),(plugin/'Source/ALS','source',{'.cpp','.h'}),(Path('tools/import'),'converter',{'.py'})]:
    for p in sorted(folder.rglob('*')):
        if p.is_file() and p.suffix in suffixes:paths[label+':'+str(p.relative_to(folder))]=p
for relative in ['Engine/Source/Runtime/Engine/Private/Curves/CurveEvaluation.cpp','Engine/Source/Runtime/Engine/Private/Curves/RichCurve.cpp','Engine/Source/Runtime/Engine/Public/Curves/CurveEvaluation.h','Engine/Source/Runtime/Core/Public/Math/UnrealMathUtility.h','Engine/Source/Runtime/Engine/Private/Components/CharacterMovementComponent.cpp','Engine/Source/Runtime/Engine/Public/Animation/AnimCurveTypes.h']:
    paths[relative]=engine/relative
for relative in ['ALS/Data/Character/Movement/MS_Als_Normal.uasset','ALS/Data/Character/Movement/CV_Als_AccelerationAndDecelerationAndGroundFriction_Normal.uasset','ALS/Character/B_Als_Character.uasset','ALS/Character/SK_Als.uasset']:paths['source_asset:'+relative]=plugin/'Content'/relative
for relative in ['tools/run_ue_source_reader.ps1','tools/build_ue_source_reader.ps1','tools/ue_graph_source_reader/Source/AlsGraphSourceReader/Private/AlsGraphSourceReaderCommandlet.cpp','tools/export/fit_ground_dynamics_curves.py']:
    paths[relative]=Path(relative)
result={}
for i,(key,path)in enumerate(paths.items()):
    result[key]=hashlib.sha256(path.read_bytes()).hexdigest()
    if i%64==0:print('Recorded current source identity batch '+str(i),flush=True)
(r/'identities.start.json').write_text(json.dumps(result,indent=2)+'\n')
print('Bound '+str(len(result))+' current source production converter reader asset and engine identities',flush=True)
