import copy
import json
import struct
from pathlib import Path

r=Path(__file__).resolve().parent
data=json.loads(Path('docs/audit/bf144/476/inputs.json').read_bytes())
lengths=json.loads(Path('docs/audit/bf144/477/inputs.json').read_bytes())
source_lengths={entry['row']:entry['lengths_cm']for entry in lengths['source']}
current_lengths={entry['row']:entry['lengths_m']for entry in lengths['current']}
vector=lambda v:[v[k]for k in 'xyz']
pin_vector=lambda v:[v[k]for k in 'XYZ']
project=lambda v:[v[0],v[2],v[1]]
project_q=lambda v:[-v[0],-v[2],-v[1],v[3]]
def transform(v):
    return dict(translation=vector(v['translation']),rotation=[v['rotation'][k]for k in 'xyzw'],scale=vector(v['scale']))
def current_transform(v):
    return dict(translation=project(v['translation']),rotation=project_q(v['rotation']),scale=project(v['scale']))

source=[]
current=[]
requests=[]
for index,row in enumerate(data['source']):
    args=row['arguments_after_native_call']
    before=list(map(transform,row['before']))
    effector=dict(translation=pin_vector(args[3]['Translation']),rotation=[args[3]['Rotation'][k]for k in 'XYZW'],scale=pin_vector(args[3]['Scale3D']))
    entry=dict(domain='source_cm',kind='source',row=index,query=row['query'],pc=row['pc'],weight=row['weight'],before=before,effector_transform=effector,effector=effector['translation'],root=before[0]['translation'],rotation=before[0]['rotation'],primary=pin_vector(args[4]),secondary=pin_vector(args[5]),secondary_weight=int(args[6]),pole=pin_vector(args[7]),lengths=source_lengths.get(index,[0,0]),expected_after=list(map(transform,row['after'])))
    assert args[6]==1 and args[10]is False and args[16]is True
    if row['weight']>1e-8:
        entry['core_sample']=len(requests)
        requests.append(dict(rotation_xyzw=entry['rotation'],two_bone_ik=dict(root_cm=entry['root'],effector_cm=effector['translation'],pole_cm=entry['pole'],lengths_cm=entry['lengths'],stretch=False,stretch_ratios=args[11:13],orientation=dict(primary_axis=entry['primary'],secondary_axis=entry['secondary'],secondary_weight=entry['secondary_weight']))))
    source.append(entry)
for index,row in enumerate(data['current']):
    args=row['arguments_before']
    before=[current_transform(bone[1])for bone in row['bones_before']]
    effector=current_transform(dict(rotation=args[3]['List'][0]['Quaternion'],translation=args[3]['List'][1]['Vector'],scale=args[3]['List'][2]['Vector']))
    entry=dict(domain='current_m',kind='actual',row=index,query=row['query'],pc=row['pc'],mode=row['mode'],weight=args[13]['F32'],before=before,effector_transform=effector,effector=effector['translation'],root=before[0]['translation'],rotation=before[0]['rotation'],primary=project(args[4]['Vector']),secondary=project(args[5]['Vector']),secondary_weight=int(args[6]['F32']),pole=project(args[7]['Vector']),lengths=current_lengths[index],expected_after=[current_transform(bone[1])for bone in row['bones_after']])
    assert args[6]==dict(F32=1.0)and entry['weight']==1 and args[10]==dict(Bool=False)
    current.append(entry)
    if row['query']==6:
        for weight in [0,0.25,0.5,0.75,1]:
            controlled=copy.deepcopy(entry)
            controlled.update(kind='current_control',weight=weight)
            del controlled['expected_after']
            current.append(controlled)
for row in source:
    original=data['source'][row['row']]
    if original['dataset']=='weights'and original['query']==6:
        common=copy.deepcopy(row)
        common.update(kind='source_common',current_template=next(i for i,value in enumerate(data['current'])if value['mode']==0 and value['query']==6 and value['pc']==row['pc']))
        current.append(common)
assert len(source)==1332 and len(requests)==1330 and len(current)==1302
assert sum(row['kind']=='actual'for row in current)==1272
assert sum(row['kind']=='current_control'for row in current)==20
assert sum(row['kind']=='source_common'for row in current)==10
small=struct.unpack('f',struct.pack('f',1e-8))[0]
assert struct.unpack('f',struct.pack('f',1.0-small))[0]==1.0
(r/'inputs.json').write_text(json.dumps(dict(source=source,current=current),separators=(',',':'))+'\n')
(r/'core.request.json').write_text(json.dumps(dict(samples=requests),separators=(',',':'))+'\n')
(r/'input-check.json').write_text(json.dumps(dict(gate='PASS_COMPLETE_IK_WRAPPER_INPUT_SCOPE',source_calls=1332,source_zero_weight=2,source_partial=6,source_full=1324,current_actual=1272,current_control=20,common_source_numeric=10,source_and_current_float_partial_cutoff_identical=True),indent=2)+'\n')
print('Prepared1332 source zero/partial/full and1302 public actual/controlled/common wrapper inputs',flush=True)
