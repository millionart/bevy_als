import hashlib
import json
import os
from pathlib import Path

r=Path(__file__).resolve().parent
reader='tools/ue_graph_source_reader/Source/AlsGraphSourceReader/Private/AlsGraphSourceReaderCommandlet.cpp'
m=json.loads(Path('docs/audit/bf144/446f/reader-migration.json').read_text())[reader]
assert hashlib.sha256(Path(reader).read_bytes()).hexdigest()==m['after_sha256']
base=json.loads(Path('docs/audit/bf144/445a/identity-check.json').read_text())
assert base['current_base_files_unchanged']==396
expected=json.loads(Path('docs/audit/bf144/441b/identities.start.json').read_text())
expected[reader]=m['after_sha256']
config=json.loads(Path('tools/env.local.json').read_text())
plugin=Path(config['ue_project']).parent/'Plugins/ALS-Refactored'
engine=Path(os.environ['ALS_UE_SOURCE_ROOT'])
for index,(key,digest)in enumerate(expected.items()):
    if index%64==0:print('Checked current identity batch '+str(index),flush=True)
    if key.startswith('source:'):p=plugin/'Source/ALS'/key.split(':',1)[1]
    elif key.startswith('target:'):p=Path('src')/key.split(':',1)[1]
    elif key.startswith('converter:'):p=Path('tools/import')/key.split(':',1)[1]
    elif key.startswith('source_asset:'):p=plugin/'Content'/key.split(':',1)[1]
    elif key.startswith('Engine/'):p=engine/key
    else:p=Path(key)
    assert hashlib.sha256(p.read_bytes()).hexdigest()==digest,key
for name,row in json.loads(Path('docs/audit/bf144/441c/library-identities.json').read_text()).items():
    assert hashlib.sha256(Path(name).read_bytes()).hexdigest()==row['sha256']
assert hashlib.sha256((r/'replay.rs').read_bytes()).hexdigest()==json.loads((r/'final-source.json').read_text())['sha256']
assert hashlib.sha256(Path('src/als_extras/levels/manual_playground.rs').read_bytes()).hexdigest()==json.loads(Path('docs/audit/bf144/451a/provenance.json').read_text())['original_manual_playground_sha256']
(r/'identity-check.json').write_text(json.dumps(dict(gate='PASS_CURRENT_HEAD_ROTATION_STAGE_FIXTURE_RESTORED_SAME_SIX_LIBRARIES_AND_CURRENT396_IDENTITIES',reader_migration_from_446f_preserved=True,six_libraries_unchanged=True,original_manual_playground_unchanged=True,observer_sources_restored=True,current_source_identity_check='docs/audit/bf144/445a/identity-check.json'),indent=2)+'\n')
print('Verified restored observer bytes, six libraries and explicit reader identity',flush=True)
