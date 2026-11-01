import gzip
import hashlib
import json
import struct
import sys
from pathlib import Path

r=Path(__file__).resolve().parent
label=sys.argv[1]if len(sys.argv)>1 else 'input-verified'
f32=lambda x:struct.unpack('f',struct.pack('f',x))[0]
modes=['cc','sc','cs','ss']
def load_json(p):return json.loads(p.read_bytes()if p.exists()else gzip.decompress(p.with_suffix(p.suffix+'.gz').read_bytes()))
def projected(native):
    out={}
    for b in native['bones']:
        t=b['local_transform'];p=t['translation'];q=t['rotation'];s=t['scale']
        out[b['name']]=dict(translation=[f32(p['x']*.01),f32(p['z']*.01),f32(p['y']*.01)],rotation=[f32(-q['x']),f32(-q['z']),f32(-q['y']),f32(q['w'])],scale=[f32(s['x']),f32(s['z']),f32(s['y'])])
    return out
def runtime(pose):return {b['name']:{k:b[k]for k in ['translation','rotation','scale']}for b in pose['bones']if b['name']!='SKM_Als'}
def difference(a,b):
    assert a.keys()==b.keys();out=dict(translation=0,rotation_raw=0,rotation_aligned=0,scale=0)
    for name in a:
        x=a[name];y=b[name]
        for key in ['translation','scale']:out[key]+=x[key]!=y[key]
        q=x['rotation'];p=y['rotation'];out['rotation_raw']+=q!=p
        if sum(v*w for v,w in zip(q,p))<0:q=[-v for v in q]
        out['rotation_aligned']+=q!=p
    return out
def validate_keys(rows):
    assert len(rows)==1272
    assert len({row['name']for row in rows})==1272
    assert {row['name']for row in rows}=={f'{i:03}:{mode}'for i in range(318)for mode in modes}
    for row in rows:
        assert row['alpha']==1 and not row['root_space_additive']and len(row['pose']['bones'])==79
        assert {b['name']for b in row['pose']['bones']}==set(source_names)
source=load_json(Path('docs/audit/bf144/448a/restored.native.json'))['character_animation_probe']['cases'][0]
source_names=[b['name']for b in source['frames'][0]['pose_boundaries']['rig_source']['pose']['bones']]
rows=[]
for part in json.loads((r/'requests.json').read_text()):
    report=load_json(r/(part['label']+'.native.json.reader.report.json'));assert report['success']and report['process_exit_code']==0
    batch=load_json(r/(part['label']+'.native.json'))['mesh_additive_probe'];assert batch['node_guid']=='d17ea3bf-4d5e-ebd5-fd47-0c867f5cc0e7'
    assert len(batch['samples'])==part['samples'];rows+=batch['samples'];print('Read native Head matrix batch '+part['label'],flush=True)
validate_keys(rows);native={row['name']:row for row in rows}
target=load_json(r/('target-replay-'+label+'.json'));assert target['actual_cc_frames_exact']==318
outputs=target['domain_outputs'];assert len(outputs)==1272;current={f"{row['source_frame']:03}:{row['domain']}":row for row in outputs};assert native.keys()==current.keys()
summaries={mode:{category:{key:0 for key in ['translation','rotation_raw','rotation_aligned','scale']}for category in ['native_to_actual_source','current_to_actual_source','same_domain_native_to_current']}for mode in modes}
effects={system:{edge:{key:0 for key in ['translation','rotation_raw','rotation_aligned','scale']}for edge in ['cc-sc','cc-cs','sc-ss','cs-ss']}for system in ['native','current']}
table=[]
for i in range(318):
    actual=source['frames'][i]['pose_boundaries']['rig_source']['pose'];assert native[f'{i:03}:ss']['pose']==actual
    expected=projected(actual);np={};cp={}
    for mode in modes:
        key=f'{i:03}:{mode}';np[mode]=projected(native[key]['pose']);cp[mode]=runtime(current[key]['pose']);assert len(np[mode])==len(cp[mode])==79
        values=dict(native_to_actual_source=difference(np[mode],expected),current_to_actual_source=difference(cp[mode],expected),same_domain_native_to_current=difference(np[mode],cp[mode]))
        for category,v in values.items():
            for k,n in v.items():summaries[mode][category][k]+=n
        table.append(dict(source_frame=i,target_query=i+1,domain=mode,comparisons=values))
    for system,poses in [('native',np),('current',cp)]:
        for edge in effects[system]:
            a,b=edge.split('-');v=difference(poses[a],poses[b])
            for k,n in v.items():effects[system][edge][k]+=n
assert summaries['ss']['native_to_actual_source']==dict(translation=0,rotation_raw=0,rotation_aligned=0,scale=0)
negative=[]
for kind in ['missing-domain','duplicated-domain','wrong-alpha','wrong-bone-map']:
    if kind=='missing-domain':bad=rows[:-1]
    elif kind=='duplicated-domain':bad=rows[:-1]+[rows[0]]
    elif kind=='wrong-alpha':bad=[dict(rows[0],alpha=0),*rows[1:]]
    else:
        first=dict(rows[0]);first['pose']=dict(first['pose']);first['pose']['bones']=[dict(first['pose']['bones'][0],name='wrong-bone'),*first['pose']['bones'][1:]];bad=[first,*rows[1:]]
    try:validate_keys(bad)
    except AssertionError:negative.append(kind)
    else:raise AssertionError('Invalid four-domain native matrix passed: '+kind)
result=dict(gate='PASS_ALL318_HEAD_FOUR_DOMAIN_NATIVE_AND_CURRENT_MATRIX',matched_frames=318,domains=4,domain_outputs_per_system=1272,bone_outputs_per_system=100488,
    native_ss_full_actual_pose_and_curve_frames_exact=318,current_cc_actual_frames_exact=318,summaries=summaries,factor_effects=effects,
    source_curves_held_common=True,current_extra_scene_node_not_counted_as_source_bone=True,additive_none_scale_native_boundary_semantic_identity_zero=True,
    numerical_operator_residuals_remaining=True,new_differences=[],negative_fixtures_rejected=negative,full_root_or_als_equivalence_claimed=False,screenshots=0,performance_acceptance='paused')
(r/('analysis-'+label+'.json')).write_text(json.dumps(result,indent=2)+'\n');(r/('frame-table-'+label+'.json')).write_text(json.dumps(table,separators=(',',':'))+'\n')
print(json.dumps({k:v for k,v in result.items()if k not in ['summaries','factor_effects']}),flush=True)
