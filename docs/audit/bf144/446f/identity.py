import hashlib
import json
import os
from pathlib import Path

r=Path(__file__).resolve().parent
read=lambda p:json.loads(Path(p).read_text())
sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
base=read('docs/audit/bf144/441b/identities.start.json')
reader='tools/ue_graph_source_reader/Source/AlsGraphSourceReader/Private/AlsGraphSourceReaderCommandlet.cpp'
migration=read(r/'reader-migration.json')[reader]
assert migration['before_sha256']==read('docs/audit/bf144/446d/reader-migration.json')[reader]['after_sha256']
base[reader]=migration['after_sha256']
config=read('tools/env.local.json')
plugin=Path(config['ue_project']).parent/'Plugins/ALS-Refactored'
engine=Path(os.environ['ALS_UE_SOURCE_ROOT'])
for index,(key,digest)in enumerate(base.items()):
    if index%64==0:print('Verified source identity batch '+str(index),flush=True)
    if key.startswith('source:'):p=plugin/'Source/ALS'/key.split(':',1)[1]
    elif key.startswith('target:'):p=Path('src')/key.split(':',1)[1]
    elif key.startswith('converter:'):p=Path('tools/import')/key.split(':',1)[1]
    elif key.startswith('source_asset:'):p=plugin/'Content'/key.split(':',1)[1]
    elif key.startswith('Engine/'):p=engine/key
    else:p=Path(key)
    assert sha(p)==digest,key
for name,row in read('docs/audit/bf144/441c/library-identities.json').items():
    assert sha(name)==row['sha256'],name
assert sha('src/als_extras/levels/manual_playground.rs')==read('docs/audit/bf144/444/provenance.json')['original_manual_playground_sha256']
(r/'identity-check.json').write_text(json.dumps(dict(gate='PASS_EXPLICIT_READER_MIGRATION_OTHER395_AND_SIX_LIBRARIES_UNCHANGED',current_base_files_unchanged=len(base),reader_migration_explicit=True,other_base_files_unchanged=395,same_six_runtime_libraries_unchanged=True),indent=2)+'\n')
print('Verified explicit reader migration and all other current identities',flush=True)
