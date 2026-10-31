import hashlib
import json
import os
from pathlib import Path
r=Path(__file__).resolve().parent
old=json.loads((r/'identities.start.json').read_text());config=json.loads(Path('tools/env.local.json').read_text());plugin=Path(config['ue_project']).parent/'Plugins/ALS-Refactored';engine=Path(os.environ['ALS_UE_SOURCE_ROOT'])
for i,(key,digest)in enumerate(old.items()):
    if i%64==0:print('Verified current source identity batch '+str(i),flush=True)
    if key.startswith('target:'):p=Path('src')/key.split(':',1)[1]
    elif key.startswith('source:'):p=plugin/'Source/ALS'/key.split(':',1)[1]
    elif key.startswith('converter:'):p=Path('tools/import')/key.split(':',1)[1]
    elif key.startswith('source_asset:'):p=plugin/'Content'/key.split(':',1)[1]
    elif key.startswith('Engine/'):p=engine/key
    else:p=Path(key)
    assert hashlib.sha256(p.read_bytes()).hexdigest()==digest,key
source=Path('tools/ue_graph_source_reader/Source/AlsGraphSourceReader/Private/AlsGraphSourceReaderCommandlet.cpp');package=Path('target/ue_graph_source_reader_package/Source/AlsGraphSourceReader/Private/AlsGraphSourceReaderCommandlet.cpp');assert source.read_bytes()==package.read_bytes()
dll=Path('target/ue_graph_source_reader_package/Binaries/Mac/libUnrealEditor-AlsGraphSourceReader.dylib')
result={'gate':'PASS_CURRENT_SOURCE_IDENTITIES_UNCHANGED_AND_REBUILT_READER_BOUND','current_files':len(old),'als_source_files':sum(k.startswith('source:')for k in old),'target_rust_files':sum(k.startswith('target:')for k in old),'converter_files':sum(k.startswith('converter:')for k in old),'all_current_source_inputs_unchanged':True,'reader_source_equals_package':True,'reader_dylib_sha256':hashlib.sha256(dll.read_bytes()).hexdigest(),'previous12018_identity_chain':'missing; not revalidated','full_target_game_or_frozen_rlib':'missing; not claimed'}
(r/'identity-check.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result),flush=True)
