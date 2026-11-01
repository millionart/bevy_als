import copy
import gzip
import json
import math
import struct
import sys
from pathlib import Path

r=Path(__file__).resolve().parent
f32=lambda x:struct.unpack('f',struct.pack('f',x))[0]
def load(label):
    p=r/('runtime-'+label)/'parameters.json';return json.loads(p.read_bytes()if p.exists()else gzip.decompress(p.with_suffix('.json.gz').read_bytes()))
def at(value,path):
    for i in path:
        if 'List'in value:value=value['List'][i]
        elif 'Vector'in value:value={'F64':value['Vector'][i]}
        elif 'EulerDegrees'in value:value={'F64':value['EulerDegrees'][i]}
        else:raise AssertionError('Unsupported actual Rig reference path')
    return value
def input_value(value):
    assert value is not None and len(value)==1
    key,content=next(iter(value.items()));return {dict(Vec3='Vector',Quat='Quaternion').get(key,key):content}
def finite(value,nullable=True):
    if value is None:assert nullable,'Nonfinite numeric Rig value serialized as null'
    elif isinstance(value,float):assert math.isfinite(value)
    elif isinstance(value,list):
        for x in value:finite(x,nullable)
    elif isinstance(value,dict):
        for x in value.values():finite(x,nullable)
def pose_bones(p):return {b['name']:b for b in p['bones']}
def validate_record(s,definition):
    assert s is not None and s['node_guid']=='5514524f-4eea-c42c-19cd-349ea6f99e36'and s['state_key']=='Default'
    assert s['phase']=='after_evaluation'and s['initialization_count']==1
    assert len(s['registers'])==len(definition['program']['initial_values'])==405
    finite(s['registers'],False);finite(s['to_world'],False)
    assert len(s['externals'])==len(definition['externals'])==12 and len(s['bindings'])==11
    assert len(s['hierarchy_local'])==len(definition['bones'])==79
    assert s['executed_steps']>=len(s['calls'])>0,'Actual executed Rig call capture is required'
    assert (s['capture_property']if s['capture_property']is not None else s['capture_global'])is True
    for key in ['rig_source','rig_output','ragdolling_output','root_result']:
        p=s['poses'][key];assert p['updated']and p['pose']is not None and len(p['pose']['bones'])==80
    output=pose_bones(s['poses']['rig_output']['pose'])
    source_pose=s['poses']['rig_source']['pose'];output_pose=s['poses']['rig_output']['pose'];source_bones=pose_bones(source_pose)
    assert output.keys()==source_bones.keys()=={b['name']for b in definition['bones']}|{'SKM_Als'}
    assert output['SKM_Als']==source_bones['SKM_Als']
    for key in ['timestamp','root_motion','skeleton_asset']:assert output_pose[key]==source_pose[key]
    for name,b in output.items():assert b['weights']==source_bones[name]['weights']
    for index,(b,source)in enumerate(zip(s['hierarchy_local'],definition['bones'],strict=True)):
        assert b['index']==index and b['name']==source['name']and b['parent']==source['parent']
        for key in ['translation','rotation','scale']:assert [f32(x)for x in b['local'][key]]==output[b['name']][key]
    for name,x in s['externals'].items():
        assert x['reference']==definition['externals'][name]
        assert at(s['registers'][x['reference']['register']],x['reference']['path'])==x['value']
    for b in s['bindings']:
        resolved=b['resolved_player_property']if b['resolved_player_property']is not None else b['global']
        assert at(s['externals'][b['external']]['value'],b['path'])==input_value(resolved),b['property']
    for c in s['calls']:
        instruction=definition['program']['instructions'][c['instruction']]['Call']
        assert len(c['arguments_after_call'])==len(instruction['arguments'])
        finite(c['arguments_after_call'],False);finite(c['bones_before_call'],False)
        for index,bone in c['bones_before_call']:assert 0<=index<79;finite(bone)
    for trace in s['traces']:assert 'Call'in definition['program']['instructions'][trace['instruction']]
    finite(s)
def validate(data,definition):
    assert len(data['animation_before'])==len(data['animation_after'])==319
    for row in data['animation_after']:validate_record(row['rig_snapshot'],definition)
    old=json.loads(gzip.decompress(Path('docs/audit/bf144/451a/runtime-restored/parameters.json.gz').read_bytes()))
    for key in ['rows','after_motors','camera_after_post_update']:assert data[key]==old[key],key
    for key in ['animation_before','animation_after']:assert [{k:v for k,v in row.items()if k!='rig_snapshot'}for row in data[key]]==old[key],key
    return 319
if __name__=='__main__':
    label=sys.argv[1];data=load(label);assert all('rig_snapshot'in row for row in data['animation_after']),'Actual Rig snapshot field is required';definition=json.loads((r/('runtime-'+label)/'rig-definition.json').read_text());validate(data,definition)
    negative=[]
    for kind in ['missing-calls','wrong-register','wrong-external-reference','wrong-output','wrong-binding','nonfinite-register']:
        s=copy.deepcopy(data['animation_after'][0]['rig_snapshot'])
        if kind=='missing-calls':s['calls']=[]
        elif kind=='wrong-register':s['registers'].pop()
        elif kind=='wrong-external-reference':s['externals']['RigInput']['reference']['register']=0
        elif kind=='wrong-output':s['poses']['rig_output']['pose']['bones'][0]['scale'][0]+=1
        elif kind=='wrong-binding':s['bindings'][0]['resolved_player_property']={'Bool':False}
        else:s['registers'][0]={'F64':None}
        try:validate_record(s,definition)
        except (AssertionError,KeyError):negative.append(kind)
        else:raise AssertionError('Invalid actual Rig observation passed: '+kind)
    before_count=sum(x['rig_snapshot']is not None for x in data['animation_before'])
    result=dict(gate='PASS_ACTUAL319_CONTROL_RIG_VM_AND_INDEPENDENT_OUTPUT_READONLY_ENTRY',after_queries=319,before_queries=319,before_initialized_snapshot_queries=before_count,registers_per_query=405,externals_per_query=12,bindings_per_query=11,hierarchy_bones_per_query=79,hierarchy_to_actual_rig_output_bone_rows_exact=319*79,initialization_count_all1=True,all_old_five_complete_arrays_exact=True,all_actual_input_binding_values_exact=True,negative_fixtures_rejected=negative,source_post_rig_and_locomotion_action_still_unobserved=True,new_differences=[],full_rig_or_root_or_als_equivalence_claimed=False,screenshots=0,performance_acceptance='paused')
    (r/('check-'+label+'.json')).write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result),flush=True)
