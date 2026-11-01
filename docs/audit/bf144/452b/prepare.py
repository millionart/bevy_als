import gzip
import hashlib
import json
from pathlib import Path

r=Path(__file__).resolve().parent
source=Path('docs/audit/bf144/451b')
rows=[];names=None;parents=None;identities={}
for batch in json.loads((source/'requests.json').read_text()):
    request_path=source/batch['request'];native_path=source/(batch['label']+'.native.json.gz');report_path=source/(batch['label']+'.native.json.reader.report.json')
    request=json.loads(request_path.read_text());native=json.loads(gzip.decompress(native_path.read_bytes()))['mesh_additive_probe'];report=json.loads(report_path.read_text());assert report['success']and report['process_exit_code']==0
    if names is None:names=request['bone_names']
    assert request['bone_names']==names and len(names)==79 and len(request['samples'])==len(native['samples'])
    for original,actual in zip(request['samples'],native['samples'],strict=True):
        assert original['name']==actual['name']and original['alpha']==actual['alpha']==1 and not actual['root_space_additive']
        bones=actual['pose']['bones'];assert [b['name']for b in bones]==names
        p=[b['parent_index']for b in bones]
        if parents is None:parents=p
        assert p==parents and p[0]==-1 and all(0<=v<i for i,v in enumerate(p)if i)
        q=lambda t:[t[k]for k in ['x','y','z','w']]
        rows.append(dict(name=original['name'],base=[b['rotation']for b in original['base']['bone_transforms']],additive=[b['rotation']for b in original['additive']['bone_transforms']],expected=[q(b['local_transform']['rotation'])for b in bones]))
    for p in [request_path,native_path,report_path]:
        data=p.read_bytes();identities[str(p)]=dict(bytes=len(data),sha256=hashlib.sha256(data).hexdigest())
    print('Prepared native rotation arithmetic '+batch['label'],flush=True)
assert len(rows)==1272 and len({x['name']for x in rows})==1272
(r/'inputs.json').write_text(json.dumps(dict(names=names,parents=parents,cases=rows),separators=(',',':'))+'\n')
(r/'input-identities.json').write_text(json.dumps(identities,indent=2)+'\n')
print('Prepared all1272 four-domain rotation inputs with100488 exact native expectations',flush=True)
