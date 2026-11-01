import gzip
import hashlib
import json
import struct
from pathlib import Path

r=Path(__file__).resolve().parent
source_path=Path('docs/audit/bf144/448a/restored.native.json.gz')
target_path=Path('docs/audit/bf144/451a/runtime-restored/parameters.json.gz')
source=json.loads(gzip.decompress(source_path.read_bytes()))['character_animation_probe']['cases'][0]
target=json.loads(gzip.decompress(target_path.read_bytes()))
names=[b['name']for b in source['frames'][0]['pose_boundaries']['head_base']['pose']['bones']]
assert len(names)==79 and len(source['frames'])==318 and len(target['animation_after'])==319
f32=lambda v:struct.unpack('f',struct.pack('f',v))[0]
def native_leaf(pose,curves):
    return dict(curves=curves,bone_transforms=[dict(translation_cm=[b['local_transform']['translation'][k]for k in ['x','y','z']],rotation=[b['local_transform']['rotation'][k]for k in ['x','y','z','w']],scale=[b['local_transform']['scale'][k]for k in ['x','y','z']])for b in pose['bones']])
def lift_current(pose,curves,additive):
    bones={b['name']:b for b in pose['bones']};out=[]
    for name in names:
        b=bones[name];p=b['translation'];q=b['rotation'];s=b['scale']
        assert p is not None and q is not None
        if s is None:assert additive;s=[0,0,0]
        out.append(dict(translation_cm=[p[0]*100,p[2]*100,p[1]*100],rotation=[-q[0],-q[2],-q[1],q[3]],scale=[s[0],s[2],s[1]]))
    return dict(curves=curves,bone_transforms=out)
def project_source(pose):
    bones=[]
    for b in pose['bones']:
        t=b['local_transform'];p=t['translation'];q=t['rotation'];s=t['scale']
        bones.append(dict(name=b['name'],translation=[f32(p['x']*.01),f32(p['z']*.01),f32(p['y']*.01)],rotation=[f32(-q['x']),f32(-q['z']),f32(-q['y']),f32(q['w'])],scale=[f32(s['x']),f32(s['z']),f32(s['y'])],weights=None))
    return bones
samples=[];replay=[]
for i,(native,current)in enumerate(zip(source['frames'],target['animation_after'][1:],strict=True)):
    packet=native['pose_boundaries'];head=current['head_apply'][0];assert head['effective_alpha']==packet['head_alpha']==1 and head['pose_exact_public']
    base=head['inputs'][0]['pose'];delta=head['inputs'][1]['pose'];assert native['world_delta_seconds']==current['frame_delta']
    sc=packet['head_base']['pose']['curves'];ac=packet['head_additive']['pose']['curves']
    source_base=native_leaf(packet['head_base']['pose'],sc);source_delta=native_leaf(packet['head_additive']['pose'],ac)
    current_base=lift_current(base,sc,False);current_delta=lift_current(delta,ac,True)
    for mode,b,d in [('cc',current_base,current_delta),('sc',source_base,current_delta),('cs',current_base,source_delta),('ss',source_base,source_delta)]:samples.append(dict(name=f'{i:03}:{mode}',alpha=1,base=b,additive=d))
    replay.append(dict(source_frame=i,target_query=i+1,current_base=base,current_additive=delta,current_output=head['actual'],source_base=project_source(packet['head_base']['pose']),source_additive=project_source(packet['head_additive']['pose']),source_output=packet['rig_source']['pose']))
    if i%64==0:print('Prepared complete four-domain Head frame '+str(i),flush=True)
assert len(samples)==1272 and len(replay)==318
requests=[]
for part,start in enumerate(range(0,len(samples),240)):
    name=f'batch-{part:02}.request.json';data=dict(node_guid='d17ea3bf-4d5e-ebd5-fd47-0c867f5cc0e7',bone_names=names,samples=samples[start:start+240]);assert len(data['samples'])<=256
    (r/name).write_text(json.dumps(data,separators=(',',':'))+'\n');requests.append(dict(label=f'batch-{part:02}',request=name,samples=len(data['samples'])))
(r/'requests.json').write_text(json.dumps(requests,indent=2)+'\n')
(r/'replay-inputs.json').write_text(json.dumps(dict(frames=replay,source_names=names,curves_held_common_from_source=True,extra_scene_node_held_current=True,additive_none_scale_lifted_as_identity_zero=True),separators=(',',':'))+'\n')
(r/'input-identities.json').write_text(json.dumps({p.as_posix():hashlib.sha256(p.read_bytes()).hexdigest()for p in [source_path,target_path,Path('docs/audit/bf144/451a/runtime-restored/skeleton.json')]},indent=2)+'\n')
print('Prepared all318 matched Head frames,1272 native sample combinations and complete target replay inputs',flush=True)
