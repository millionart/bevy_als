import gzip
import hashlib
import json
import os
from pathlib import Path

root = Path(__file__).resolve().parent
temporary = Path('tools/tmp/bf144/490')
temporary.mkdir(parents=True,exist_ok=True)
def read(path):
    return json.loads(gzip.decompress((root.parent/path).read_bytes()))

source = read('485/native-input.json.gz')
target = read('484/inputs.json.gz')
markers = ['ik_foot_l','ik_foot_r','VB foot_l','VB foot_r','ik_foot_root','VB foot_root']
for data in [source,target]:
    assert all(data['names'].count(name)==1 for name in markers)
target['rows'] = [row for row in target['rows'] if row['query'] in [1,105,203]]
assert len(target['rows'])==3
native = {name:source['names'].index(name) for name in markers}
(temporary/'input.json').write_text(json.dumps(target,separators=(',',':')))
(root/'asset-marker-check.json').write_text(json.dumps(dict(source_indices=native,target_indices={name:target['names'].index(name) for name in markers},target_queries=[row['query'] for row in target['rows']]),indent=2)+'\n')
print('Qualified both native and target asset markers and three actual target skeleton poses',flush=True)

engine = Path(os.environ['ALS_UE_SOURCE_ROOT'])
config = json.loads(Path('tools/env.local.json').read_text())
als = Path(config['ue_project']).parent/'Plugins/ALS-Refactored/Source/ALS'
facts = []
for base,key,ranges in [
    (als,'Public/Settings/AlsGeneralAnimationSettings.h',[(8,20)]),
    (als,'Public/Utility/AlsConstants.h',[(20,26),(341,359)]),
    (als,'Private/AlsAnimationInstance.cpp',[(235,257),(1277,1295)]),
    (engine,'Engine/Source/Runtime/Engine/Classes/Components/SkinnedMeshComponent.h',[(1358,1361)]),
    (engine,'Engine/Source/Runtime/Engine/Private/Components/SkinnedMeshComponent.cpp',[(3525,3591)]),
    (Path('.'),'src/als/state/foot_ik.rs',[(150,157),(904,930)]),
    (Path('.'),'src/als/character/foot_ik_apply.rs',[(151,168),(188,208),(584,589),(632,681)]),
    (Path('.'),'src/als/character/control_rig_apply.rs',[(59,90),(272,290),(370,384),(410,427)]),
    (Path('.'),'src/als/character/animation_instances/head_layer.rs',[(31,40)]),
]:
    raw = (base/key).read_bytes()
    lines = raw.decode('utf-8-sig').splitlines()
    facts.append(dict(key=f'source:{key}' if base==als else key,sha256=hashlib.sha256(raw).hexdigest(),
                      excerpts=[dict(start=a,end=b,text='\n'.join(lines[a-1:b])) for a,b in ranges]))
(root/'source-facts.json').write_text(json.dumps(facts,indent=2)+'\n')
