import copy
import json
import struct
import sys
from pathlib import Path

r=Path(__file__).resolve().parent
f32=lambda v:struct.unpack('<f',struct.pack('<f',v))[0]


def validate(data):
    rows=data['rows'];motors=data['after_motors'];raw=data['before_floor_sync'];late=data['camera_after_post_update'];coeff=data['integration_coefficients']
    assert len(rows)==len(motors)==len(raw)==len(late)==len(coeff)==240
    changed=[];band=[];qualified=[]
    for i,(row,motor,physics,post) in enumerate(zip(rows,motors,raw,late)):
        assert row['ordinal']==motor['ordinal']==physics['ordinal']==post['ordinal']==i
        probe=row['floor_probe'];center=row['position'][1]
        if motor['position'][1]!=center:changed.append(i)
        if probe['eligible']:
            assert probe['forward'] is None
            hit=probe['floor'];assert hit is not None
            assert probe['edge_valid'] and hit['normal1'][1] > 0.9 and hit['distance'] > 0
            assert len(probe['scale'])==3 and all(v>0 for v in probe['scale'])
            expected=max(center,probe['predicted_y'])
            assert motor['position'][1]==expected,(i,expected,motor['position'][1])
            qualified.append(i)
            cm=f32(hit['distance']*100)
            if f32(1.9)<=cm<=f32(2.4) and expected!=center:band.append(i)
        else:
            assert motor['position'][1]==center
        floor=physics['floor'];assert floor is not None
        assert floor['adjustment_hit'] is None
        assert type(floor['target_y'])in(int,float)
    assert changed and band
    return dict(rows=240,query_qualified=len(qualified),motor_y_changed=len(changed),held_source_band_but_target_moved=len(band),changed_ordinals=changed,band_ordinals=band,raw_to_final_y_changed=sum(a['position'][1]!=b['position'][1]for a,b in zip(raw,late)))


def self_test():
    data=dict(rows=[],after_motors=[],before_floor_sync=[],camera_after_post_update=[],integration_coefficients=[])
    for i in range(240):
        data['rows'].append(dict(ordinal=i,position=[0,.92,0],floor_probe=dict(eligible=True,forward=None,floor=dict(distance=.02,normal1=[0,1,0]),edge_valid=True,scale=[1,1,1],predicted_y=.924)))
        data['after_motors'].append(dict(ordinal=i,position=[0,.924,0]))
        data['before_floor_sync'].append(dict(ordinal=i,position=[0,.924,0],floor=dict(target_y=.9215,adjustment_hit=None)))
        data['camera_after_post_update'].append(dict(ordinal=i,position=[0,.9215,0]))
        data['integration_coefficients'].append(dict(ordinal=i))
    validate(data)
    for kind in range(4):
        bad=copy.deepcopy(data)
        if kind==0:del bad['rows'][0]['floor_probe']
        elif kind==1:bad['rows'][0]['floor_probe']['forward']={}
        elif kind==2:bad['after_motors'][0]['position'][1]=.92
        else:bad['integration_coefficients']=[]
        try:validate(bad)
        except(AssertionError,KeyError):continue
        raise AssertionError(kind)
    return dict(rejected=4)


if __name__=='__main__':
    label=sys.argv[1]
    result=self_test()if label=='self-test'else validate(json.loads((r/f'runtime-{label}/parameters.json').read_text()))
    (r/f'{label}-check.json').write_text(json.dumps(result,indent=2)+'\n')
    print(json.dumps(result),flush=True)
