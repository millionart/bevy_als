import hashlib
import json
import os
from pathlib import Path
r=Path(__file__).resolve().parent
config=json.loads(Path('tools/env.local.json').read_text());plugin=Path(config['ue_project']).parent/'Plugins/ALS-Refactored';engine=Path(os.environ['ALS_UE_SOURCE_ROOT'])
items=[(plugin/'Source/ALS/Public/Utility/AlsVector.h','source:Source/ALS/Public/Utility/AlsVector.h',[(114,158)]),(plugin/'Source/ALSExtras/Private/AlsCharacterExample.cpp','source:Source/ALSExtras/Private/AlsCharacterExample.cpp',[(141,158)]),(engine/'Engine/Source/Runtime/Engine/Private/Components/CharacterMovementComponent.cpp','Engine/Source/Runtime/Engine/Private/Components/CharacterMovementComponent.cpp',[(8134,8157)])]
result=[]
for path,key,ranges in items:
    raw=path.read_bytes();lines=raw.decode().splitlines();result.append({'key':key,'sha256':hashlib.sha256(raw).hexdigest(),'ranges':[{'start':a,'end':b,'text':'\n'.join(lines[a-1:b])}for a,b in ranges]})
(r/'source-facts.json').write_text(json.dumps({'source_units':result,'execution':'Verified source inline algorithm mirrored using native Core APIs; not a character event invocation','source_files_modified':False},indent=2)+'\n');print('Bound source clamp SinCos input construction and CMC clamp facts',flush=True)
