import hashlib
import json
from pathlib import Path
from check import load,validate,bones

r=Path(__file__).resolve().parent;data=load('baseline');validate(data);definition=json.loads((r/'runtime-baseline/rig-definition.json').read_text());names=json.loads((r/'source-bone-order.json').read_text())['names'];assert len(names)==79 and set(names)=={b['name']for b in definition['bones']}
samples=[]
for i,row in enumerate(data['animation_after']):
 v=row['ragdoll_blend'][0];p=bones(v['base']);curves=row['rig_snapshot']['upstream_context_curves'];transforms=[]
 for n in names:
  b=p[n];t=b['translation'];q=b['rotation'];s=b['scale'];assert t is not None and q is not None and s is not None
  transforms.append(dict(translation_cm=[t[0]*100,t[2]*100,t[1]*100],rotation=[-q[0],-q[2],-q[1],q[3]],scale=[s[0],s[2],s[1]]))
 leaf=dict(curves=curves,bone_transforms=transforms)
 samples.append(dict(name=f'actual-query-{i:03}',poses=[leaf,leaf],steps=[dict(active=0,delta_seconds=0.0)]))
assert len(samples)==319
(r/'request.json').write_text(json.dumps(dict(mode='Linear',blend_times=[.4,.5],samples=samples),separators=(',',':'))+'\n')
(r/'input-identities.json').write_text(json.dumps({n:hashlib.sha256(Path(n).read_bytes()).hexdigest()for n in ['docs/audit/bf144/458/runtime-baseline/parameters.json.gz','docs/audit/bf144/458/runtime-baseline/rig-definition.json','docs/audit/bf144/458/source-bone-order.json']},indent=2)+'\n')
print('Prepared319 actual full79-bone same-input source BlendList evaluations; inactive second pose explicitly held equal',flush=True)
