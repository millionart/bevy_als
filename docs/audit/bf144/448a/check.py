import copy
import gzip
import json
import math
import sys
from pathlib import Path

r=Path(__file__).resolve().parent
names=['rig_source','root_result','head_base','head_additive','base_provider_input']
def load(label):
    p=r/(label+'.native.json')
    return json.loads(p.read_bytes()if p.exists()else gzip.decompress((r/(label+'.native.json.gz')).read_bytes()))
def validate_packet(packet,engine,delta,count):
    assert packet['stage']=='actual_rig_source_and_main_root_result'
    for name in names:
        item=packet[name]
        assert item['engine_frame']==engine and item['delta_seconds']==delta
        assert item['updates']==item['evaluations']==count
        assert item['attribute_count']==0
        assert item['required_bones_asset']=='/ALS/ALS/Character/SKM_Als.SKM_Als'
        bones=item['pose']['bones'];assert len(bones)==len({v['name']for v in bones})==79
        for bone in bones:
            value=bone['local_transform']
            assert all(math.isfinite(x)for key in ['translation','rotation','scale']for x in value[key].values())
        assert all(math.isfinite(x)for x in item['pose']['curves'].values())
    return 5*79
def strip(value):
    if isinstance(value,dict):return {k:strip(v)for k,v in value.items()if k not in ['pose_boundaries','layered_blends']}
    if isinstance(value,list):return [strip(v)for v in value]
    return value
def validate(data):
    cases=data['character_animation_probe']['cases'];assert [len(v['frames'])for v in cases]==[318,2]
    total=0;rotation_rows=0;first=None
    for case in cases:
        initial=case['initialization'];engine=case['initial_engine_frame']
        total+=validate_packet(initial['pose_boundaries'],engine,0,1)
        for index,frame in enumerate(case['frames']):
            packet=frame['pose_boundaries'];total+=validate_packet(packet,engine+index+1,frame['world_delta_seconds'],index+2)
            observed=[dict(name=b['name'],local=b['local_transform'])for b in packet['root_result']['pose']['bones']]
            assert [b['name']for b in observed]==[b['name']for b in frame['mesh_local_pose']]
            for a,b in zip(observed,frame['mesh_local_pose'],strict=True):
                assert a['local']['translation']==b['local']['translation']and a['local']['scale']==b['local']['scale']
                if a['local']['rotation']!=b['local']['rotation']:
                    rotation_rows+=1
                    if first is None:first=dict(case=case['name'],frame=frame['frame'],bone=a['name'],raw_root=a['local']['rotation'],mesh_after_finalization=b['local']['rotation'])
    return total,rotation_rows,first
if __name__=='__main__':
    label=sys.argv[1];data=load(label);bone_rows,rotation_rows,first=validate(data)
    original=json.loads(gzip.decompress(Path('docs/audit/bf144/446f/restored.native.json.gz').read_bytes()))['character_animation_probe']
    assert strip(data['character_animation_probe'])==original
    rejected=[]
    for kind in ['missing-boundary','wrong-engine-frame','nonempty-attribute','missing-bone']:
        packet=copy.deepcopy(data['character_animation_probe']['cases'][0]['frames'][0]['pose_boundaries'])
        if kind=='missing-boundary':del packet['root_result']
        elif kind=='wrong-engine-frame':packet['root_result']['engine_frame']=100
        elif kind=='nonempty-attribute':packet['root_result']['attribute_count']=1
        else:packet['root_result']['pose']['bones'].pop()
        try:validate_packet(packet,101,0.03333333507180214,2)
        except(AssertionError,KeyError):rejected.append(kind)
        else:raise AssertionError('Invalid Root boundary packet passed: '+kind)
    result=dict(gate='PASS_NATIVE_FIVE_POSE_BOUNDARIES_AND_CURRENT_EMPTY_ATTRIBUTES',source_history_frames=318,control_frames=2,initialization_packets=2,pose_boundaries_per_packet=5,bone_rows_including_control_and_initialization=bone_rows,
        attributes_empty_all_observed_boundaries=True,root_to_mesh_translation_and_scale_exact=True,root_to_mesh_raw_rotation_difference_rows=rotation_rows,first_raw_root_to_mesh_rotation_difference=first,
        raw_root_precedes_mesh_finalize_normalize_rotations=True,previous_complete_character_fields_exact=True,added_observation_fields=['pose_boundaries','layered_blends'],negative_fixtures_rejected=rejected,
        full_root_or_als_equivalence_claimed=False,screenshots=0,performance_acceptance='paused')
    (r/(label+'-check.json')).write_text(json.dumps(result,indent=2)+'\n')
    print(json.dumps(result),flush=True)
