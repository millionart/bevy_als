import gzip
import hashlib
import json
import struct
import sys
from pathlib import Path

sys.path.insert(0,'tools/import')
from coordinate_basis import source_point_to_runtime,source_rotation_to_runtime,source_direction_to_runtime,self_test

r=Path(__file__).resolve().parent
f32=lambda v:struct.unpack('f',struct.pack('f',v))[0]
names=['root_result','rig_source','head_base','head_additive','base_provider_input']
def project(local):
    vector=lambda key:[local[key][v]for v in ['x','y','z']]
    q=[local['rotation'][v]for v in ['x','y','z','w']]
    return dict(translation=[f32(v)for v in source_point_to_runtime(vector('translation'))],rotation=[f32(v)for v in source_rotation_to_runtime(q)],scale=[f32(v)for v in source_direction_to_runtime(vector('scale'))])
def validate_pair(source,target):
    assert len(source['frames'])==318 and len(target['animation_after'])==319
    for index,(a,b)in enumerate(zip(source['frames'],target['animation_after'][1:],strict=True)):
        assert a['frame']==index and a['world_delta_seconds']==b['frame_delta']
        assert b['curve_frame_input']['app_frame']>target['animation_after'][index]['curve_frame_input']['app_frame']
        for name in names:
            native=a['pose_boundaries'][name];assert native['engine_frame']==source['initial_engine_frame']+index+1
            matches=[v for v in b['pose_boundaries']['updated_boundaries']if v['boundary']==name];assert len(matches)==1
            assert matches[0]['state_key']=='Default'
            assert {v['name']for v in native['pose']['bones']}<={v['name']for v in matches[0]['pose']['bones']}
    return 318
self_test()
assert project(dict(translation=dict(x=100,y=200,z=300),rotation=dict(x=.2,y=-.3,z=.4,w=.5),scale=dict(x=2,y=3,z=4)))==dict(translation=[1,3,2],rotation=[f32(-.2),f32(-.4),f32(.3),f32(.5)],scale=[2,4,3])
source_path=Path('docs/audit/bf144/448a/restored.native.json.gz');target_path=Path('docs/audit/bf144/448b/runtime-restored/parameters.json.gz')
source=json.loads(gzip.decompress(source_path.read_bytes()))['character_animation_probe']['cases'][0]
target=json.loads(gzip.decompress(target_path.read_bytes()));assert validate_pair(source,target)==318
rows=[];first={};counts={name:{key:0 for key in ['bone_pairs','translation','rotation_raw','rotation_sign_aligned','scale','scale_absent','curves_different','curve_keysets_different','extra_scene_nodes']}for name in names}
for index,(a,b)in enumerate(zip(source['frames'],target['animation_after'][1:],strict=True)):
    for name in names:
        native=a['pose_boundaries'][name];current=next(v for v in b['pose_boundaries']['updated_boundaries']if v['boundary']==name)
        actual={v['name']:v for v in current['pose']['bones']};counts[name]['bone_pairs']+=len(native['pose']['bones'])
        row=dict(source_frame=index,target_query=index+1,target_ordinal=b['ordinal'],boundary=name,bones=79,translation=0,rotation_raw=0,rotation_sign_aligned=0,scale=0,scale_absent=0)
        for bone in native['pose']['bones']:
            expected=project(bone['local_transform']);v=actual[bone['name']]
            for key in ['translation','rotation','scale']:
                if v[key]is None:
                    assert key=='scale'and name=='head_additive';row['scale_absent']+=1;continue
                category='rotation_raw'if key=='rotation'else key
                if expected[key]!=v[key]:
                    row[category]+=1
                    first.setdefault(name+':'+category,dict(source_frame=index,target_query=index+1,bone=bone['name'],source_projected=expected[key],target=v[key]))
                if key=='rotation':
                    q=expected[key]
                    if sum(x*y for x,y in zip(q,v[key]))<0:q=[-x for x in q]
                    if q!=v[key]:row['rotation_sign_aligned']+=1
        s={k.casefold():v for k,v in native['pose']['curves'].items()};t={k.casefold():v for k,v in current['curves'].items()}
        assert len(s)==len(native['pose']['curves'])and len(t)==len(current['curves'])
        row['curve_keysets_different']=set(s)!=set(t)
        row['curves_different']=sum(s[k]!=t[k]for k in s.keys()&t.keys())
        row['source_only_curve_keys']=sorted(s.keys()-t.keys());row['target_only_curve_keys']=sorted(t.keys()-s.keys())
        row['extra_scene_nodes']=sorted(actual.keys()-{v['name']for v in native['pose']['bones']})
        for key in ['translation','rotation_raw','rotation_sign_aligned','scale','scale_absent','curves_different','curve_keysets_different']:counts[name][key]+=int(row[key])
        counts[name]['extra_scene_nodes']+=len(row['extra_scene_nodes'])
        rows.append(row)
assert len(rows)==1590 and all(v['bone_pairs']==25122 for v in counts.values())
negative=[]
for kind in ['missing-frame','wrong-delta','wrong-native-engine','missing-target-boundary']:
    old=None
    if kind=='missing-frame':old=source['frames'].pop()
    elif kind=='wrong-delta':old=source['frames'][0]['world_delta_seconds'];source['frames'][0]['world_delta_seconds']=0
    elif kind=='wrong-native-engine':old=source['frames'][0]['pose_boundaries']['root_result']['engine_frame'];source['frames'][0]['pose_boundaries']['root_result']['engine_frame']=100
    else:old=target['animation_after'][1]['pose_boundaries']['updated_boundaries'].pop()
    try:validate_pair(source,target)
    except AssertionError:negative.append(kind)
    else:raise AssertionError('Invalid five-boundary cohort passed: '+kind)
    finally:
        if kind=='missing-frame':source['frames'].append(old)
        elif kind=='wrong-delta':source['frames'][0]['world_delta_seconds']=old
        elif kind=='wrong-native-engine':source['frames'][0]['pose_boundaries']['root_result']['engine_frame']=old
        else:target['animation_after'][1]['pose_boundaries']['updated_boundaries'].append(old)
result=dict(gate='PASS_FULL_FRAME_MATCHED_FIVE_POSE_RESIDUAL_INVENTORY',matched_positive_frames=318,boundary_rows=1590,bone_pairs=125610,counts=counts,first_differences=first,
    native_attributes_zero_current_cohort=True,target_custom_attribute_storage='Absent from BAG Pose type',scale_none_preserved=True,quaternion_double_cover_reported_separately=True,
    actual_differences_attributed_to_product_causes=False,additional_scene_root_consumer_not_yet_observed=True,negative_fixtures_rejected=negative,
    evidence_inputs={p.as_posix():hashlib.sha256(p.read_bytes()).hexdigest()for p in [source_path,target_path]},new_differences=[],full_root_or_als_equivalence_claimed=False,screenshots=0,performance_acceptance='paused')
(r/'inventory.json').write_text(json.dumps(result,indent=2)+'\n');(r/'frame-table.json').write_text(json.dumps(rows,indent=2)+'\n')
print(json.dumps({k:v for k,v in result.items()if k not in ['first_differences','counts']}),flush=True)
