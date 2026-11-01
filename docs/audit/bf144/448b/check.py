import copy
import gzip
import json
import math
import sys
from pathlib import Path

r=Path(__file__).resolve().parent
declarations={'root_result':'8191c2ea-49e5-bd21-ee33-86bd87b2f93c','rig_source':'e4f33e44-4019-9bac-8e0a-dcbc0e3ae178','head_base':'316d5caa-4268-b60f-5ce6-2e9c0beb480b','head_additive':'5a7796c6-47c9-baf2-6479-579e079fbb67','base_provider_input':'ccaab6a2-424d-b00d-6794-f3a705d6f8c2'}
source=json.loads(gzip.decompress(Path('docs/audit/bf144/448a/restored.native.json.gz').read_bytes()))['character_animation_probe']['cases'][0]
source_names={v['name']for v in source['frames'][0]['pose_boundaries']['root_result']['pose']['bones']}
assert len(source_names)==79
del source
def load(label):
    p=r/('runtime-'+label)/'parameters.json'
    return json.loads(p.read_bytes()if p.exists()else gzip.decompress(p.with_suffix('.json.gz').read_bytes()))
def validate_packet(packet):
    assert packet['custom_attribute_storage']=='Absent from the current BAG Pose type'
    rows=packet['updated_boundaries'];assert len(rows)==5 and {v['boundary']for v in rows}==set(declarations)
    for row in rows:
        assert row['node_guid']==declarations[row['boundary']]and row['state_key']=='Default'and row['pin']=='Pose'
        assert isinstance(row['curves'],dict)and all(math.isfinite(v)for v in row['curves'].values())
        pose=row['pose'];assert math.isfinite(pose['timestamp'])and pose['root_motion']is None
        names=[b['name']for b in pose['bones']];assert len(names)==len(set(names))and source_names<=set(names)<=source_names|{'SKM_Als'}
        for bone in pose['bones']:
            assert bone['path']and bone['path'][-1]==bone['name']
            for key,count in [('translation',3),('rotation',4),('scale',3)]:assert bone[key]is None or(len(bone[key])==count and all(math.isfinite(v)for v in bone[key]))
            assert bone['weights']is None
    root=next(v['pose']for v in rows if v['boundary']=='root_result')
    assert packet['default_output']==root and len(root['bones'])==80
    return len(root['bones'])
def validate(data):
    assert len(data['animation_before'])==len(data['animation_after'])==319
    before=data['animation_before'][0]['pose_boundaries'];assert before['default_output']is None and before['updated_boundaries']==[]
    for row in data['animation_after']:validate_packet(row['pose_boundaries'])
    old=json.loads(Path('docs/audit/bf144/447b/runtime-restored/parameters.json').read_bytes())
    for key in ['rows','after_motors','camera_after_post_update']:assert data[key]==old[key],key
    for key in ['animation_before','animation_after']:assert [{k:v for k,v in row.items()if k!='pose_boundaries'}for row in data[key]]==old[key],key
    return 319
if __name__=='__main__':
    label=sys.argv[1];data=load(label);count=validate(data)
    rejected=[]
    for kind in ['missing-boundary','wrong-node','missing-bone','nonfinite-curve']:
        packet=copy.deepcopy(data['animation_after'][0]['pose_boundaries'])
        if kind=='missing-boundary':packet['updated_boundaries'].pop()
        elif kind=='wrong-node':packet['updated_boundaries'][0]['node_guid']='wrong-node'
        elif kind=='missing-bone':packet['updated_boundaries'][0]['pose']['bones'].pop()
        else:packet['updated_boundaries'][0]['curves']['invalid']=float('nan')
        try:validate_packet(packet)
        except(AssertionError,KeyError):rejected.append(kind)
        else:raise AssertionError('Invalid actual Pose packet passed: '+kind)
    extra=[]
    for index,row in enumerate(data['animation_after']):
        bone=next(b for b in row['pose_boundaries']['default_output']['bones']if b['name']=='SKM_Als')
        if bone['translation']!=[0,0,0]or bone['rotation']!=[0,0,0,1]or bone['scale']!=[1,1,1]:extra.append(dict(target_query=index,ordinal=row['ordinal'],bone=bone))
    result=dict(gate='PASS_ACTUAL_TARGET_FIVE_POSE_BOUNDARIES_AND_DEFAULT_ROOT',actual_queries_per_phase=count,unique_boundaries_per_after_query=5,default_root_equals_root_cache=True,source_bones_present=79,additional_scene_root='SKM_Als',additional_scene_root_nonidentity_rows=len(extra),first_additional_scene_root_nonidentity=None if not extra else extra[0],absent_pose_channels_preserved=True,
        custom_attribute_type_fact_only=True,current_root_motion_none=True,previous_complete_history_arrays_exact=True,negative_fixtures_rejected=rejected,full_root_or_als_equivalence_claimed=False,screenshots=0,performance_acceptance='paused')
    (r/('check-'+label+'.json')).write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result),flush=True)
