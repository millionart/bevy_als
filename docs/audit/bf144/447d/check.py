import copy
import gzip
import hashlib
import json
import struct
import sys
from pathlib import Path

r=Path(__file__).resolve().parent
f32=lambda x:struct.unpack('f',struct.pack('f',x))[0]
def validate(replay,request,native):
    rows=replay['rows'];asked=request['samples'];samples=native['geometry_query_probe']['samples']
    assert len(rows)==len(asked)==len(samples)==319
    for index,(row,query,sample)in enumerate(zip(rows,asked,samples,strict=True)):
        assert row['target_query']==index and query==sample['input']
        assert query['yaw_difference']==dict(velocity_yaw_degrees=row['body_runtime_degrees'],view_yaw_radians=row['raw_view_radians'])
        assert row['late_view_cast_control']==sample['yaw_difference']
    assert rows[79]['current']==44.999996185302734 and rows[79]['late_view_cast_control']==45
    return len([v for v in rows if v['current']!=v['late_view_cast_control']])
label=sys.argv[1]if len(sys.argv)>1 else 'baseline'
replay=json.loads((r/('replay-'+label+'.json')).read_text());request=json.loads((r/'request.json').read_text());native=json.loads((r/'baseline.native.json').read_text())
assert validate(replay,request,native)==319
negative=[]
for kind in ['missing-row','wrong-input','wrong-native']:
    bad=copy.deepcopy(native)
    if kind=='missing-row':bad['geometry_query_probe']['samples'].pop()
    elif kind=='wrong-input':bad['geometry_query_probe']['samples'][79]['input']['yaw_difference']['view_yaw_radians']=0
    else:bad['geometry_query_probe']['samples'][79]['yaw_difference']=44.999996185302734
    try:validate(replay,request,bad)
    except AssertionError:negative.append(kind)
    else:raise AssertionError('Invalid same-input native evidence passed: '+kind)
source_path=Path('docs/audit/bf144/446f/restored.native.json.gz')
source=json.loads(gzip.decompress(source_path.read_bytes()))['character_animation_probe']['cases'][0]
def fields(items,name):return next(v['fields']for v in items if v['name']==name)
def number(items,name):return next(v['numeric_value']for v in items if v['name']==name)
source_rows=[]
for index,frame in enumerate(source['frames']):
    state=frame['source_state_after_tick'];view=fields(state,'ViewState');loco=fields(state,'LocomotionState')
    view_yaw=number(fields(view,'RotationWorldSpace'),'Yaw');body_yaw=number(fields(loco,'RotationWorldSpace'),'Yaw')
    angle=f32(view_yaw-body_yaw)
    while angle>180:angle=f32(angle-360)
    while angle<-180:angle=f32(angle+360)
    assert angle==number(view,'YawAngle')
    source_rows.append(dict(source_frame=index,actual_view_degrees=view_yaw,actual_body_degrees=body_yaw,cast_after_difference=angle,native=number(view,'YawAngle')))
assert len(source_rows)==318
result=dict(gate='PASS_ACTUAL_VIEW_PRODUCERS_AND_NATIVE_COMMON_INPUT_CAST_FACTOR',actual_target_producer_rows_exact=319,native_actual_source_formula_rows_exact=318,
    native_same_input_late_cast_rows_exact=319,view_cast_factor_rows_changed=319,first_look=dict(target_query=79,source_frame=78,current=replay['rows'][79]['current'],native_common_input=native['geometry_query_probe']['samples'][79]['yaw_difference']),
    source_report=dict(path=source_path.as_posix(),sha256=hashlib.sha256(source_path.read_bytes()).hexdigest()),negative_fixtures_rejected=negative,recorded_difference='BF144-D237',new_differences=[],
    actual_body_producer_equivalence_claimed=False,source_actual_formula_rows=source_rows)
(r/('check-'+label+'.json')).write_text(json.dumps(result,indent=2)+'\n')
print('Verified319 actual target producers,318 native actual endpoints and319 same-input late-cast controls; rejected3 negative fixtures',flush=True)
