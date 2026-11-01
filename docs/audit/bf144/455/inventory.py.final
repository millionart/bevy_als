import copy
import gzip
import hashlib
import json
import struct
import sys
from pathlib import Path

sys.path.insert(0,'tools/import')
from coordinate_basis import source_point_to_runtime,source_rotation_to_runtime,source_direction_to_runtime
r=Path(__file__).resolve().parent
label=sys.argv[1]if len(sys.argv)>1 else 'baseline'
f32=lambda x:struct.unpack('f',struct.pack('f',x))[0]
source_path=Path('docs/audit/bf144/448a/restored.native.json.gz');target_path=Path('docs/audit/bf144/448b/runtime-restored/parameters.json.gz')
source=json.loads(gzip.decompress(source_path.read_bytes()))['character_animation_probe']['cases'][0];target=json.loads(gzip.decompress(target_path.read_bytes()))
def source_pose(packet):
    result={}
    for b in packet['pose']['bones']:
        t=b['local_transform'];result[b['name']]={k:[t[k][a]for a in ['x','y','z','w']if a in t[k]]for k in ['translation','rotation','scale']}
    assert len(result)==79
    return result
def project(pose):
    return {n:dict(translation=[f32(x)for x in source_point_to_runtime(b['translation'])],rotation=[f32(x)for x in source_rotation_to_runtime(b['rotation'])],scale=[f32(x)for x in source_direction_to_runtime(b['scale'])])for n,b in pose.items()}
def current_pose(packet):
    bones=packet['pose']['bones'];result={b['name']:{k:b[k]for k in ['translation','rotation','scale']}for b in bones if b['name']!='SKM_Als'};assert len(result)==79
    return result
names=set(source_pose(source['frames'][0]['pose_boundaries']['rig_source']))
def validate(s,t):
    assert len(s['frames'])==318 and len(t['animation_after'])==319
    for i,(a,b)in enumerate(zip(s['frames'],t['animation_after'][1:],strict=True)):
        assert a['frame']==i and a['world_delta_seconds']==b['frame_delta']and a['world_delta_seconds']>0
        assert b['curve_frame_input']['app_frame']>t['animation_after'][i]['curve_frame_input']['app_frame']
        for n in ['rig_source','root_result']:
            native=a['pose_boundaries'][n];assert native['engine_frame']==s['initial_engine_frame']+i+1 and native['attribute_count']==0
            assert source_pose(native).keys()==names
            rows=[x for x in b['pose_boundaries']['updated_boundaries']if x['boundary']==n];assert len(rows)==1 and rows[0]['state_key']=='Default'
            assert current_pose(rows[0]).keys()==names
validate(source,target)
def difference(a,b):
    assert a.keys()==b.keys();out={k:0 for k in ['translation','rotation','scale']};changes=[]
    for bone in sorted(a):
        for k in out:
            if a[bone][k]!=b[bone][k]:out[k]+=1;changes.append(dict(bone=bone,channel=k,before=a[bone][k],after=b[bone][k]))
    return out,changes
counts={domain:{k:0 for k in ['translation','rotation','scale']}for domain in ['source_raw','source_projected','current']}
by_bone={domain:{}for domain in counts};curves={domain:dict(value_changed_frames=0,keyset_changed_frames=0)for domain in ['source','current']};table=[];first={};actions={}
for i,(a,b)in enumerate(zip(source['frames'],target['animation_after'][1:],strict=True)):
    sp=a['pose_boundaries'];tp={n:next(x for x in b['pose_boundaries']['updated_boundaries']if x['boundary']==n)for n in ['rig_source','root_result']}
    raw_before=source_pose(sp['rig_source']);raw_after=source_pose(sp['root_result'])
    domains=dict(source_raw=(raw_before,raw_after),source_projected=(project(raw_before),project(raw_after)),current=(current_pose(tp['rig_source']),current_pose(tp['root_result'])))
    for domain,(before,after)in domains.items():
        values,changes=difference(before,after)
        for k,v in values.items():counts[domain][k]+=v
        for x in changes:
            key=x['bone']+':'+x['channel'];by_bone[domain][key]=by_bone[domain].get(key,0)+1
            if domain not in first:first[domain]=dict(source_frame=i,target_query=i+1,**x)
        table.append(dict(source_frame=i,target_query=i+1,domain=domain,changed=values,changes=changes))
    for domain,old,new in [('source',sp['rig_source']['pose']['curves'],sp['root_result']['pose']['curves']),('current',tp['rig_source']['curves'],tp['root_result']['curves'])]:
        assert isinstance(old,dict)and isinstance(new,dict)
        curves[domain]['value_changed_frames']+=old!=new;curves[domain]['keyset_changed_frames']+=old.keys()!=new.keys()
    getter={x['name']:x for x in a['getter_after_tick']['fields']}
    fields=a['rig_variables_after_tick']['RigInput']['fields'];actual={x['name']:x for x in fields}
    for name in ['bUseHandIkBones','bUseFootIkBones','bFootTransformsValid']:
        key=name+':'+actual[name]['exported_text'];actions[key]=actions.get(key,0)+1
        assert actual[name]['exported_text']==getter[name]['exported_text']
    if i%64==0:print('Inventoried actual composite Rig/Ragdolling increment frame '+str(i),flush=True)
assert all(counts['source_raw'][k]>counts['source_projected'][k]for k in ['translation','rotation','scale']),'Missing F32 projection boundary signal'
basis={'bone':dict(translation=[100,200,300],rotation=[.2,-.3,.4,.5],scale=[2,3,4])}
expected_basis={'bone':dict(translation=[1,3,2],rotation=[f32(-.2),f32(-.4),f32(.3),f32(.5)],scale=[2,4,3])}
assert project(basis)==expected_basis
wrong_basis=copy.deepcopy(expected_basis);wrong_basis['bone']['translation']=[1,2,3]
try:assert wrong_basis==expected_basis
except AssertionError:pass
else:raise AssertionError('Invalid coordinate basis passed')
negative=['wrong-coordinate-basis']
for kind in ['wrong-frame','wrong-delta','missing-root','wrong-bone']:
    s=dict(source);s['frames']=[copy.deepcopy(source['frames'][0]),*source['frames'][1:]];t=target
    if kind=='wrong-frame':s['frames'][0]['frame']=99
    elif kind=='wrong-delta':s['frames'][0]['world_delta_seconds']=1
    elif kind=='missing-root':del s['frames'][0]['pose_boundaries']['root_result']
    else:s['frames'][0]['pose_boundaries']['rig_source']['pose']['bones'][0]['name']='invalid'
    try:validate(s,t)
    except (AssertionError,KeyError):negative.append(kind)
    else:raise AssertionError('Invalid composite increment pair passed: '+kind)
result=dict(gate='PASS_ACTUAL318_COMPOSITE_RIG_RAGDOLLING_INCREMENT_INVENTORY',matched_frames=318,bone_rows_each_domain=25122,domains=3,counts=counts,changed_bone_channels=by_bone,first_changes=first,curve_changes=curves,source_rig_actual_bool_observations=actions,source_locomotion_action_not_captured=True,negative_fixtures_rejected=negative,zero_observed_change_does_not_prove_no_writer=True,composite_changes_not_attributed_to_single_node=True,source_raw_separate_from_f32_projection=True,new_differences=[],full_root_or_als_equivalence_claimed=False,screenshots=0,performance_acceptance='paused')
(r/('analysis-'+label+'.json')).write_text(json.dumps(result,indent=2)+'\n');(r/('frame-table-'+label+'.json')).write_text(json.dumps(table,separators=(',',':'))+'\n')
print(json.dumps(dict(counts=counts,curves=curves,actions=actions)),flush=True)
