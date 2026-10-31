import hashlib
import json
import os
from pathlib import Path
r=Path(__file__).resolve().parent
config=json.loads(Path('tools/env.local.json').read_text());plugin=Path(config['ue_project']).parent/'Plugins/ALS-Refactored';engine=Path(os.environ['ALS_UE_SOURCE_ROOT'])
items=[
(plugin/'Source/ALSExtras/Private/AlsCharacterExample.cpp','source:Source/ALSExtras/Private/AlsCharacterExample.cpp',[(141,158)]),
(plugin/'Source/ALS/Public/Utility/AlsVector.h','source:Source/ALS/Public/Utility/AlsVector.h',[(114,158)]),
(engine/'Engine/Source/Runtime/Engine/Private/PlayerController.cpp','Engine/Source/Runtime/Engine/Private/PlayerController.cpp',[(1001,1026)]),
(engine/'Engine/Source/Runtime/Engine/Private/PlayerCameraManager.cpp','Engine/Source/Runtime/Engine/Private/PlayerCameraManager.cpp',[(767,772)]),
(Path('src/als/character/movement/motion.rs'),'src/als/character/movement/motion.rs',[(204,219),(2330,2343),(2485,2494),(2540,2552),(3069,3090)]),
(Path('src/als_camera/camera_component.rs'),'src/als_camera/camera_component.rs',[(61,80),(1001,1017)]),
(Path('src/als/state/view.rs'),'src/als/state/view.rs',[(38,65)]),
]
result=[]
for path,key,ranges in items:
    raw=path.read_bytes();lines=raw.decode().splitlines();result.append({'key':key,'sha256':hashlib.sha256(raw).hexdigest(),'ranges':[{'start':a,'end':b,'text':'\n'.join(lines[a-1:b])}for a,b in ranges]})
for name,row in json.loads(Path('docs/audit/bf144/441c/library-identities.json').read_text()).items():assert hashlib.sha256(Path(name).read_bytes()).hexdigest()==row['sha256'],name
(r/'source-facts.json').write_text(json.dumps({'source_first_functions':result,'same_six_rebuilt_library_identities':'docs/audit/bf144/441c/library-identities.json','all_six_current_libraries_unchanged':True,'view_selection_only_no_full_angle_math_claim':True},indent=2)+'\n');print('Bound seven source/target selection and phase units plus unchanged six actual libraries',flush=True)
