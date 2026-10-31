import argparse
import copy
import json
import math
import struct
from pathlib import Path

def validate(document):
    count=0
    for row in document['geometry_query_probe']['samples']:
        wanted=row['input'].get('move_input')
        if wanted is None:continue
        actual=row['move_input']
        assert type(wanted['yaw_degrees'])in[int,float]and math.isfinite(wanted['yaw_degrees'])
        assert len(wanted['axes'])==2 and all(type(x)in[int,float]and math.isfinite(x)for x in wanted['axes'])
        for name in ['yaw_degrees_float','yaw_radians_float','clamped_x','clamped_y']:
            assert type(actual[name])in[int,float]and math.isfinite(actual[name])
        assert actual['yaw_degrees_float']==struct.unpack('f',struct.pack('f',wanted['yaw_degrees']))[0]
        for name in ['forward','right','world_input','cmc_input']:
            assert set(actual[name])=={'x','y','z'}and all(type(x)in[int,float]and math.isfinite(x)for x in actual[name].values())
        assert actual['world_input']['z']==actual['cmc_input']['z']==0
        assert abs(actual['clamped_x']**2+actual['clamped_y']**2)<=1.000000000001
        assert sum(x*x for x in actual['cmc_input'].values())<=1.000000000001
        count+=1
    assert count>0
    return {'native_source_move_input_samples':count,'precise_source_stage_contract':True}

def self_test():
    value={'yaw_degrees_float':0,'yaw_radians_float':0,'clamped_x':0,'clamped_y':1,'forward':{'x':1,'y':0,'z':0},'right':{'x':0,'y':1,'z':0},'world_input':{'x':1,'y':0,'z':0},'cmc_input':{'x':1,'y':0,'z':0}}
    document={'geometry_query_probe':{'samples':[{'input':{'move_input':{'yaw_degrees':0,'axes':[0,1]}},'move_input':value}]}}
    assert validate(document)['native_source_move_input_samples']==1
    rejected=0
    for i in range(5):
        bad=copy.deepcopy(document);row=bad['geometry_query_probe']['samples'][0]
        if i==0:del row['move_input']
        elif i==1:row['move_input']['cmc_input']['x']=float('nan')
        elif i==2:row['move_input']['clamped_y']=2
        elif i==3:row['input']['move_input']['axes'][0]='0'
        else:row['move_input']['world_input']['z']=1
        try:validate(bad)
        except(KeyError,AssertionError):rejected+=1
    assert rejected==5
    return {'self_test':'PASS','invalid_contract_cases_rejected':rejected}

if __name__=='__main__':
    parser=argparse.ArgumentParser();parser.add_argument('--input',type=Path);parser.add_argument('--self-test',action='store_true');args=parser.parse_args()
    print(json.dumps(self_test()if args.self_test else validate(json.loads(args.input.read_text()))))
