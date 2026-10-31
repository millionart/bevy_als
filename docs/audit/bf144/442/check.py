import json
from pathlib import Path
r=Path(__file__).resolve().parent
read=lambda n:json.loads((r/n).read_text())
def validate(v):
    a=v['rows'];c=v['camera_after_post_update'];assert len(a)==len(c)==240
    for i,x in enumerate(a):
        assert x['ordinal']==c[i]['ordinal']==i
        assert x['desired_motion']==x['expected_direction']
        assert x['player_view_yaw']==x['camera_at_update']['yaw']==x['consumed_yaw']
        if i:assert x['player_view_yaw']==c[i-1]['camera']['yaw']
    wrong=[x['ordinal']for x in a if x['desired_motion']!=x['raw_control_direction']];assert len(wrong)==80
    moving=[i for i in range(240)if 20<=i<60 or 120<=i<160];assert wrong==moving
    return a,c,wrong
v=read('runtime-baseline/parameters.json');a,c,wrong=validate(v)
report={'gate':'PASS_ACTUAL_CAMERA_VIEW_SELECTION_AND_TARGET_FRAME_CONSUMPTION240','actual_direction_matches_published_view_formula':240,'published_yaw_matches_update_camera':240,'published_yaw_matches_previous_last_camera':239,'raw_control_yaw_substitution_fails_moving_rows':len(wrong),'example20':{k:a[20][k]for k in ['view_yaw','player_view_yaw','camera_at_update','desired_motion','input_move']},'source_contract':'Source Input_OnMove prefers completed player camera view when available; target prefers published player view before control target','not_claimed':['Full source/target angle precision and SinCos normalization parity','All cold/no-camera fallback lifecycles','Camera lag algorithm equivalence','Complete physical feedback and Root'],'new_differences':[],'screenshots':0,'performance_acceptance':'paused'}
import copy
rejected=0
for index in range(5):
    bad=copy.deepcopy(v)
    if index==0:del bad['rows'][20]['player_view_yaw']
    elif index==1:
        for row in bad['rows']:row['player_view_yaw']=row['view_yaw']
    elif index==2:
        for i in range(239):bad['camera_after_post_update'][i]['camera']=copy.deepcopy(v['camera_after_post_update'][i+1]['camera'])
    elif index==3:bad['camera_after_post_update']=[]
    else:bad['rows'][20]['desired_motion']=[.5,0,0]
    try:validate(bad)
    except(AssertionError,KeyError):rejected+=1
assert rejected==5
report['focused_invalid_field_phase_and_direction_cases_rejected']=rejected
(r/'selection-check.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps({k:v for k,v in report.items()if k!='example20'}),flush=True)
