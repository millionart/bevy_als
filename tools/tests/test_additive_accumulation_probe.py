import json
import math
import struct
import sys
from pathlib import Path

def f32(value):
    return struct.unpack('f',struct.pack('f',value))[0]

def product(a,b):
    x,y,z,w=a;u,v,k,t=b
    return [w*u+x*t+y*k-z*v,w*v-x*k+y*t+z*u,w*k+x*v-y*u+z*t,w*t-x*u-y*v-z*k]

def normalized(value):
    length=math.sqrt(sum(v*v for v in value))
    return [v/length for v in value]

def reference(sample):
    data=sample['additive_accumulation'];position=list(data['translation_cm']);scale=list(data['scale']);rotation=list(sample['rotation_xyzw'])
    for layer in data['layers']:
        weight=f32(layer['weight'])
        if weight>f32(1e-5):
            additive=list(layer['rotation_xyzw'])
            if weight<f32(1-f32(1e-5)):
                sign=1 if additive[3]>=0 else -1
                additive=normalized([v*weight+(sign*(1-weight) if i==3 else 0) for i,v in enumerate(additive)])
            rotation=product(additive,rotation)
            position=[p+v*weight for p,v in zip(position,layer['translation_cm'])]
            scale=[s*(1+v*weight) for s,v in zip(scale,layer['scale_delta'])]
        rotation=normalized(rotation)
    return position,rotation,scale

def check(path):
    data=json.loads(Path(path).read_text(encoding='utf-8-sig'));samples=data['geometry_query_probe']['samples'];count=0;maximum=0
    for sample in samples:
        if 'additive_accumulation' not in sample['input']:
            continue
        expected=reference(sample['input']);actual=sample['additive_accumulation']
        for field,keys,wanted in zip(['translation','rotation','scale'],['xyz','xyzw','xyz'],expected):
            value=[actual[field][key] for key in keys];error=max(abs(a-b) for a,b in zip(value,wanted))
            if field=='rotation':error=min(error,max(abs(a+b) for a,b in zip(value,wanted)))
            maximum=max(maximum,error);assert error<1e-10,(count,field,value,wanted,error)
        count+=1
    assert count>0
    print(json.dumps({'pass':True,'samples':count,'maximum_error':maximum}),flush=True)

if __name__=='__main__':
    check(sys.argv[1])
