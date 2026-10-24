import argparse
import copy
import json
import math
from pathlib import Path


def check(document, request):
    samples=document['geometry_query_probe']['samples']
    assert len(samples)==len(request['samples'])>0
    for sample,wanted in zip(samples,request['samples']):
        assert sample['input']==wanted
        if 'foot_lock_rotation'not in wanted:
            continue
        value=sample['foot_lock_rotation']
        assert type(value['limited'])is bool
        for key in ['twist_radians','twist_degrees','clamped_degrees']:
            assert math.isfinite(value[key])
        for key in ['relative_rotation','relative_rotation_float','offset_rotation_float','final_rotation']:
            assert set(value[key])==set('xyzw')
            assert all(math.isfinite(v)for v in value[key].values())
        assert abs(value['clamped_degrees'])<=wanted['foot_lock_rotation']['limit_degrees']


def self_test():
    request={'samples':[{'rotation_xyzw':[0,0,0,1],'foot_lock_rotation':{'target_rotation_xyzw':[0,0,0,1],'limit_degrees':40}}]}
    q={'x':0,'y':0,'z':0,'w':1}
    value={'limited':False,'twist_radians':0,'twist_degrees':0,'clamped_degrees':0,'relative_rotation':q,'relative_rotation_float':q,'offset_rotation_float':q,'final_rotation':q}
    doc={'geometry_query_probe':{'samples':[{'input':request['samples'][0],'foot_lock_rotation':value}]}}
    check(doc,request)
    rejected=0
    for kind in range(5):
        changed=copy.deepcopy(doc)
        item=changed['geometry_query_probe']['samples'][0]
        if kind==0:item.pop('foot_lock_rotation')
        elif kind==1:item['foot_lock_rotation']['twist_degrees']=math.inf
        elif kind==2:item['foot_lock_rotation']['limited']=0
        elif kind==3:item['foot_lock_rotation']['final_rotation'].pop('w')
        else:changed['geometry_query_probe']['samples']=[]
        try:check(changed,request)
        except (AssertionError,KeyError):rejected+=1
    assert rejected==5
    return {'self_test':'PASS','invalid_output_cases_rejected':rejected}


def main():
    parser=argparse.ArgumentParser()
    parser.add_argument('--self-test',action='store_true')
    parser.add_argument('--input',type=Path)
    parser.add_argument('--request',type=Path)
    args=parser.parse_args()
    if args.self_test:result=self_test()
    else:
        assert args.input and args.request
        check(json.loads(args.input.read_text()),json.loads(args.request.read_text()))
        result={'foot_lock_rotation_probe':'PASS'}
    print(json.dumps(result))


if __name__=='__main__':
    main()
