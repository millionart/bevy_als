import hashlib
import json
import os
from pathlib import Path

r=Path(__file__).resolve().parent
reader='tools/ue_graph_source_reader/Source/AlsGraphSourceReader/Private/AlsGraphSourceReaderCommandlet.cpp'
m=json.loads(Path('docs/audit/bf144/491/reader-migration.json').read_text())[reader]
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
for name, digest in json.loads((r/'final-source.json').read_text()).items():
    assert hashlib.sha256((r/name).read_bytes()).hexdigest()==digest,name
assert hashlib.sha256(Path('src/als_extras/levels/manual_playground.rs').read_bytes()).hexdigest()==json.loads(Path('docs/audit/bf144/451a/provenance.json').read_text())['original_manual_playground_sha256']
for fact in json.loads((r/'source-facts.json').read_text()):
    key=fact['key'];p=plugin/'Source/ALS'/key.split(':',1)[1]if key.startswith('source:')else engine/key if key.startswith('Engine/')else Path(os.environ['ALS_BEVY_SOURCE_ROOT'])/key.split('/',1)[1] if key.startswith('Bevy/') else next((Path.home()/'.cargo/registry/src').glob('*/'+key.split(':',1)[1]))if key.startswith('dependency:')else Path(key)
    if key.startswith('Tnua/'):
        p=next((Path.home()/'.cargo/git/checkouts').glob('bevy-tnua-*/9f83188'))/key.split('/',1)[1]
    assert hashlib.sha256(p.read_bytes()).hexdigest()==fact['sha256'],key
published=json.loads((r.parent/'572'/'evidence-manifest.json').read_text())
for name in ('pelvis.native.json.gz','pelvis.request.json'):
    assert hashlib.sha256((r.parent/'572'/name).read_bytes()).hexdigest()==published['files'][name]['sha256']
previous=r.parent/'575'
native=json.loads((previous/'evidence-manifest.json').read_text())
for name in ('lateral.native.json.gz','lateral.request.json','lateral.package.json'):
    assert hashlib.sha256((previous/name).read_bytes()).hexdigest()==native['files'][name]['sha256']
import tomllib

lock=tomllib.loads(Path('Cargo.lock').read_text())
assert [row['version'] for row in lock['package'] if row['name']=='parry3d']==['0.27.0']
kernel=json.loads((r/'kernel-source.json').read_text())
source_path=next((Path.home()/'.cargo/registry/src').glob('*/'+kernel['path']))
assert source_path.stat().st_size==kernel['bytes']
assert hashlib.sha256(source_path.read_bytes()).hexdigest()==kernel['sha256']
for filename, recorded in json.loads((r/'observer-build-identity.json').read_text()).items():
    path=Path(filename)
    assert path.stat().st_size==recorded['bytes']
    with path.open('rb') as stream:
        assert hashlib.file_digest(stream,'sha256').hexdigest()==recorded['sha256']
(r/'identity-check.json').write_text(json.dumps(dict(
    gate='PASS_ARCHIVED_SOURCE_SAME_SIX_LIBRARIES_AND_CURRENT396_IDENTITIES',
    six_libraries_unchanged=True,original_manual_playground_unchanged=True,
    archived_source_verified=True,current_source_identity_check='docs/audit/bf144/445a/identity-check.json'),
    indent=2)+'\n')
print('Verified archived native source evidence and six production libraries',flush=True)
