import gzip
import hashlib
import json
from pathlib import Path

r=Path(__file__).resolve().parent;source=Path('docs/audit/bf144/451b')
def load(p):
    if p.suffix=='.gz':return json.loads(gzip.decompress(p.read_bytes()))
    return json.loads(p.read_bytes()if p.exists()else gzip.decompress(p.with_suffix(p.suffix+'.gz').read_bytes()))
current={f"{x['source_frame']:03}:{x['domain']}":{b['name']:b for b in x['pose']['bones']if b['name']!='SKM_Als'}for x in load(source/'target-replay-restored.json')['domain_outputs']}
rows=[];identities={};names=None
for batch in load(source/'requests.json'):
    request_path=source/batch['request'];native_path=source/(batch['label']+'.native.json.gz');report_path=source/(batch['label']+'.native.json.reader.report.json')
    request=load(request_path);native=load(native_path)['mesh_additive_probe'];report=load(report_path);assert report['success']and report['process_exit_code']==0
    if names is None:names=request['bone_names']
    assert request['bone_names']==names and len(names)==79
    for original,actual in zip(request['samples'],native['samples'],strict=True):
        assert original['name']==actual['name']and original['alpha']==actual['alpha']==1 and not actual['root_space_additive']
        bones=actual['pose']['bones'];assert [b['name']for b in bones]==names
        native_ts=[dict(translation_cm=[b['local_transform']['translation'][k]for k in ['x','y','z']],scale=[b['local_transform']['scale'][k]for k in ['x','y','z']])for b in bones]
        rows.append(dict(name=original['name'],base=original['base']['bone_transforms'],additive=original['additive']['bone_transforms'],expected_native=native_ts,expected_current=[dict(translation=current[original['name']][name]['translation'],scale=current[original['name']][name]['scale'])for name in names]))
    for p in [request_path,native_path,report_path]:data=p.read_bytes();identities[str(p)]=dict(bytes=len(data),sha256=hashlib.sha256(data).hexdigest())
    print('Prepared complete position/scale data '+batch['label'],flush=True)
assert len(rows)==1272 and len({x['name']for x in rows})==1272
p=source/'target-replay-restored.json.gz';data=p.read_bytes();identities[str(p)]=dict(bytes=len(data),sha256=hashlib.sha256(data).hexdigest())
(r/'inputs.json').write_text(json.dumps(dict(names=names,cases=rows),separators=(',',':'))+'\n');(r/'input-identities.json').write_text(json.dumps(identities,indent=2)+'\n')
print('Prepared1272 source/current T/S cases from actual four-domain endpoints',flush=True)
