import hashlib
import json
import os
from pathlib import Path
r=Path(__file__).resolve().parent
read=lambda p:json.loads(Path(p).read_text())
sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
base=read('docs/audit/bf144/441b/identities.start.json');reader='tools/ue_graph_source_reader/Source/AlsGraphSourceReader/Private/AlsGraphSourceReaderCommandlet.cpp';base[reader]=read('docs/audit/bf144/442c0/reader-migration.json')[reader]['after_sha256']
config=read('tools/env.local.json');plugin=Path(config['ue_project']).parent/'Plugins/ALS-Refactored';engine=Path(os.environ['ALS_UE_SOURCE_ROOT'])
for i,(key,digest)in enumerate(base.items()):
    if i%64==0:print('Checked current source identity batch '+str(i),flush=True)
    if key.startswith('source:'):p=plugin/'Source/ALS'/key.split(':',1)[1]
    elif key.startswith('target:'):p=Path('src')/key.split(':',1)[1]
    elif key.startswith('converter:'):p=Path('tools/import')/key.split(':',1)[1]
    elif key.startswith('source_asset:'):p=plugin/'Content'/key.split(':',1)[1]
    elif key.startswith('Engine/'):p=engine/key
    else:p=Path(key)
    assert sha(p)==digest,key
for name,row in read('docs/audit/bf144/441c/library-identities.json').items():assert sha(name)==row['sha256'],name

for name,digest in read(r/'final-sources.json').items():assert sha(r/name)==digest
assert sha('src/als_extras/levels/manual_playground.rs')==read(r/'provenance.json')['original_manual_playground_sha256']
report={'gate':'PASS_CURRENT396_AND_SIX_LIBRARIES_WITH_OBSERVER_SOURCE_RESTORED','current_base_files_unchanged':len(base),'reader_migration_from_442c0_respected':True,'same_six_actual_libraries_unchanged':True,'original_manual_playground_unchanged':True,'observer_sources_restored':True}
(r/'identity-check.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report),flush=True)
