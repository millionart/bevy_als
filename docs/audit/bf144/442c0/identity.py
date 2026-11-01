import hashlib
import json
import os
from pathlib import Path
r=Path(__file__).resolve().parent
read=lambda p:json.loads(Path(p).read_text())
sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
base=read('docs/audit/bf144/441b/identities.start.json');config=read('tools/env.local.json');plugin=Path(config['ue_project']).parent/'Plugins/ALS-Refactored';engine=Path(os.environ['ALS_UE_SOURCE_ROOT']);before=read(r/'before-identities.json');migration={}
reader='tools/ue_graph_source_reader/Source/AlsGraphSourceReader/Private/AlsGraphSourceReaderCommandlet.cpp'
assert before[reader]==read('docs/audit/bf144/442b0/reader-migration.json')[reader]['after_sha256']
base[reader]=before[reader]
for i,(key,digest)in enumerate(base.items()):
    if i%64==0:print('Verified current identity batch '+str(i),flush=True)
    if key.startswith('source:'):p=plugin/'Source/ALS'/key.split(':',1)[1]
    elif key.startswith('target:'):p=Path('src')/key.split(':',1)[1]
    elif key.startswith('converter:'):p=Path('tools/import')/key.split(':',1)[1]
    elif key.startswith('source_asset:'):p=plugin/'Content'/key.split(':',1)[1]
    elif key.startswith('Engine/'):p=engine/key
    else:p=Path(key)
    current=sha(p)
    if key==reader:assert digest==before[reader]and current!=digest
    else:assert current==digest,key
for key,digest in before.items():
    current=sha(key);assert current!=digest,key;migration[key]={'before_sha256':digest,'after_sha256':current,'reason':'Native source player-view read-only diagnostic capability or its documentation'}
assert len(migration)==4
assert sha(reader)==read(r/'reader-final-sha256.json')['sha256']
source=Path(reader);package=Path('target/ue_graph_source_reader_package/Source/AlsGraphSourceReader/Private/AlsGraphSourceReaderCommandlet.cpp');assert source.read_bytes()==package.read_bytes()
for name,row in read('docs/audit/bf144/441c/library-identities.json').items():assert sha(name)==row['sha256'],name
(r/'reader-migration.json').write_text(json.dumps(migration,indent=2)+'\n')
result={'gate':'PASS_EXPLICIT_DIAGNOSTIC_READER_IDENTITY_MIGRATION','base_current_identity_files':len(base),'unchanged_base_keys':len(base)-1,'changed_base_key':reader,'explicit_reader_package_and_documentation_migrations':4,'production_als_engine_converter_assets_and_six_libraries_unchanged':True,'source_bytes_equal_rebuilt_package':True,'new_test_sha256':sha('tools/tests/test_character_player_view_probe.py'),'source_player_controller_reference_sha256':sha(engine/'Engine/Source/Runtime/Engine/Private/PlayerController.cpp'),'source_input_handler_sha256':sha(plugin/'Source/ALSExtras/Private/AlsCharacterExample.cpp')}
(r/'identity-check.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result),flush=True)
