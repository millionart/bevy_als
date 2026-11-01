import gzip
import hashlib
import json
from pathlib import Path

r=Path(__file__).resolve().parent;path=Path('docs/audit/bf144/456/runtime-restored/parameters.json.gz');data=json.loads(gzip.decompress(path.read_bytes()));definition_path=Path('docs/audit/bf144/456/runtime-restored/rig-definition.json');definition=json.loads(definition_path.read_text());frames=[]
for i,row in enumerate(data['animation_after']):
 s=row['rig_snapshot'];assert len(s['registers'])==405 and len(s['hierarchy_local'])==79
 before=data['animation_before'][i];initialize=before['gate']is not None and before['arena_frame']in [None,0];assert initialize==(i==0)
 first=next(c for c in s['calls']if definition['program']['instructions'][c['instruction']]['Call']['function']=='FAlsRigVMFunction_IsGameWorld::Execute')
 assert first['arguments_after_call']==[{'Name':'True'}]
 frames.append(dict(query=i,delta_seconds=0.0 if initialize else row['head_values']['floats']['animation.delta_seconds']['resolved'],game_world=True,to_world=s['to_world'],pose=s['poses']['rig_source']['pose'],curves=s['upstream_context_curves'],bindings=s['bindings'],expected_registers=s['registers'],expected_local=s['hierarchy_local'],expected_calls=s['calls'],expected_traces=s['traces'],expected_steps=s['executed_steps']))
assert len(frames)==319
(r/'inputs.json').write_text(json.dumps(dict(definition=definition,frames=frames),separators=(',',':'))+'\n')
(r/'input-identities.json').write_text(json.dumps({str(p):dict(bytes=len(p.read_bytes()),sha256=hashlib.sha256(p.read_bytes()).hexdigest())for p in [path,definition_path]},indent=2)+'\n')
print('Prepared all319 actual public VM inputs, working-state endpoints, call order and trace callbacks',flush=True)
